import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { startBackupSchedule } from "../src/backupWorkflow.ts";

function deferred() {
  let resolve, reject;
  const promise = new Promise((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function hookHarness(name, api = {}, getCurrentWindow = () => {}) {
  const slots = [];
  const pending = [];
  let cursor = 0;
  let timerId = 0;
  const timers = new Map();
  let eventListener;
  const changed = (previous, deps) =>
    !previous || !deps || deps.some((value, i) => !Object.is(value, previous.deps[i]));
  const react = {
    useState(initial) {
      const index = cursor++;
      if (!(index in slots)) slots[index] = { value: initial };
      return [
        slots[index].value,
        (value) => {
          slots[index].value = typeof value === "function" ? value(slots[index].value) : value;
        },
      ];
    },
    useRef(initial) {
      const index = cursor++;
      if (!(index in slots)) slots[index] = { current: initial };
      return slots[index];
    },
    useCallback(callback, deps) {
      const index = cursor++;
      if (changed(slots[index], deps)) slots[index] = { deps, callback };
      return slots[index].callback;
    },
    useEffect(callback, deps) {
      const index = cursor++;
      if (changed(slots[index], deps)) {
        pending.push(() => {
          slots[index]?.cleanup?.();
          slots[index] = { deps, cleanup: callback() };
        });
      }
    },
  };
  const module = { exports: {} };
  const source = readFileSync(new URL(`../src/hooks/${name}.ts`, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  vm.runInNewContext(outputText, {
    module,
    exports: module.exports,
    window: {
      clearTimeout: (id) => timers.delete(id),
      setTimeout: (callback) => {
        timers.set(++timerId, callback);
        return timerId;
      },
      confirm: () => true,
    },
    require(dependency) {
      if (dependency === "react") return react;
      if (dependency === "../api") return { api, isTauri: true };
      if (dependency === "@tauri-apps/api/window") return { getCurrentWindow };
      if (dependency === "@tauri-apps/api/event") {
        return {
          listen: async (_event, listener) => {
            eventListener = listener;
            return () => {};
          },
        };
      }
      throw new Error(`Unexpected Hook dependency: ${dependency}`);
    },
  });
  const renderOnce = (args) => {
    cursor = 0;
    return module.exports[name](...args);
  };
  return {
    render(...args) {
      renderOnce(args);
      for (const effect of pending.splice(0)) effect();
      return renderOnce(args);
    },
    emitProgress(payload) {
      eventListener({ payload });
    },
  };
}

const asset = (id, prompt = `prompt ${id}`) => ({
  id,
  name: `${id}.png`,
  prompt,
  negative_prompt: "",
  model: "",
  tags: [],
  updated_at: 1,
});

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const appSyntax = ts.createSourceFile(
  "App.tsx",
  appSource,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX,
);
function appCallbackSource(name) {
  let callback;
  function visit(node) {
    if (
      ts.isVariableDeclaration(node) &&
      node.name.getText(appSyntax) === name &&
      node.initializer &&
      ts.isCallExpression(node.initializer) &&
      node.initializer.expression.getText(appSyntax) === "useCallback"
    ) {
      callback = node.initializer.arguments[0].getText(appSyntax);
    }
    ts.forEachChild(node, visit);
  }
  visit(appSyntax);
  assert.ok(callback, `Actual App callback ${name} was not found`);
  return ts.transpileModule(`module.exports = ${callback};`, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
}
const appCallbackSources = Object.fromEntries(
  ["selectRecord", "refresh", "loadMore"].map((name) => [name, appCallbackSource(name)]),
);

function manualBackupHarness(
  api,
  flushEditor = async () => {},
  refreshManager = async () => {},
  schedule = null,
) {
  const manualBackupOperation = { current: null };
  const closePending = { current: false };
  const backupSchedule = { current: schedule };
  const values = { status: "", confirmations: 0, confirm: true };
  const context = vm.createContext({
    api,
    manualBackupOperation,
    closePending,
    backupSchedule,
    flushEditor,
    refreshManager,
    setStatus: (value) => (values.status = value),
    window: {
      confirm: () => {
        values.confirmations++;
        return values.confirm;
      },
    },
    module: { exports: {} },
  });
  for (const name of ["runManualBackupOperation", "beforeClose", "createBackup", "restoreBackup"]) {
    vm.runInContext(appCallbackSource(name), context);
    context[name] = context.module.exports;
  }
  let closeError;
  function visit(node) {
    if (ts.isCallExpression(node) && node.expression.getText(appSyntax) === "useCloseGuard")
      closeError = node.arguments[2].getText(appSyntax);
    ts.forEachChild(node, visit);
  }
  visit(appSyntax);
  assert.ok(closeError, "Actual App close error callback was not found");
  vm.runInContext(
    ts.transpileModule(`module.exports = ${closeError};`, {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  return {
    values,
    manualBackupOperation,
    closePending,
    createBackup: context.createBackup,
    restoreBackup: context.restoreBackup,
    beforeClose: context.beforeClose,
    onCloseError: context.module.exports,
  };
}

for (const kind of ["backup", "restore"]) {
  test(`actual App ${kind} prevents duplicate or competing requests and close waits before flushing latest edits`, async () => {
    const native = deferred();
    const events = [];
    let latest = "before task";
    let backupCalls = 0;
    let restoreCalls = 0;
    const app = manualBackupHarness(
      {
        createBackup: async () => {
          backupCalls++;
          await native.promise;
          events.push("native terminal");
        },
        stageRestore: async () => {
          restoreCalls++;
          await native.promise;
          events.push("native terminal");
        },
      },
      async () => events.push(`flush ${latest}`),
    );
    const request = kind === "backup" ? app.createBackup : () => app.restoreBackup("safe.sqlite3");
    const operation = request();
    const duplicate = request();
    const competing = kind === "backup" ? app.restoreBackup("other.sqlite3") : app.createBackup();
    await new Promise(setImmediate);
    assert.equal(backupCalls + restoreCalls, 1);
    assert.equal(app.values.confirmations, kind === "restore" ? 1 : 0);
    let onClose;
    let destroyed = 0;
    let prevented = 0;
    const close = hookHarness("useCloseGuard", {}, () => ({
      onCloseRequested: async (callback) => {
        onClose = callback;
        return () => {};
      },
      destroy: async () => {
        destroyed++;
        events.push("destroy");
      },
    }));
    close.render(app.beforeClose, async () => events.push("cancel jobs"), app.onCloseError);
    await new Promise(setImmediate);
    const event = { preventDefault: () => prevented++ };
    const closing = onClose(event);
    await onClose(event);
    await app.createBackup();
    await app.restoreBackup("during-close.sqlite3");
    assert.equal(backupCalls + restoreCalls, 1, "Closing rejects new manual work");
    assert.equal(prevented, 2);
    assert.equal(destroyed, 0);
    latest = "typed while task and close were pending";
    native.resolve();
    await Promise.all([operation, duplicate, competing, closing]);
    assert.equal(destroyed, 1);
    assert.deepEqual(events.slice(-4), [
      "native terminal",
      `flush ${latest}`,
      "cancel jobs",
      "destroy",
    ]);
    assert.equal(app.manualBackupOperation.current, null);
  });

  test(`actual App ${kind} failure keeps a pending close open, reports failure and permits retry`, async () => {
    const native = deferred();
    let attempts = 0;
    let flushes = 0;
    const execute = async () => {
      if (++attempts === 1) await native.promise;
    };
    const app = manualBackupHarness(
      { createBackup: execute, stageRestore: execute },
      async () => flushes++,
    );
    const request = kind === "backup" ? app.createBackup : () => app.restoreBackup("safe.sqlite3");
    const operation = request();
    await new Promise(setImmediate);
    let onClose;
    let destroyed = 0;
    let cancelled = 0;
    const close = hookHarness("useCloseGuard", {}, () => ({
      onCloseRequested: async (callback) => {
        onClose = callback;
        return () => {};
      },
      destroy: async () => destroyed++,
    }));
    close.render(app.beforeClose, async () => cancelled++, app.onCloseError);
    await new Promise(setImmediate);
    const event = { preventDefault() {} };
    const closing = onClose(event);
    native.reject(new Error("synthetic native failure"));
    await Promise.all([operation, closing]);
    assert.equal(destroyed, 0);
    assert.equal(cancelled, 0);
    assert.equal(flushes, kind === "backup" ? 1 : 0, "Failure prevents the closing flush");
    assert.match(app.values.status, /关闭前.*synthetic native failure/);
    assert.equal(app.closePending.current, false);
    assert.equal(app.manualBackupOperation.current, null);
    await request();
    assert.equal(attempts, 2);
    assert.match(app.values.status, kind === "backup" ? /备份完成/ : /恢复已准备完成/);
    await onClose(event);
    assert.equal(cancelled, 1);
    assert.equal(destroyed, 1);
  });
}

test("actual App backup save failure never starts native work and releases the request for retry", async () => {
  let fail = true;
  let nativeCalls = 0;
  const app = manualBackupHarness({ createBackup: async () => nativeCalls++ }, async () => {
    if (fail) throw new Error("synthetic save failure");
  });
  await app.createBackup();
  assert.equal(nativeCalls, 0);
  assert.match(app.values.status, /资料库备份失败.*synthetic save failure/);
  assert.equal(app.manualBackupOperation.current, null);
  fail = false;
  await app.createBackup();
  assert.equal(nativeCalls, 1);
});

test("actual App cancelled restore confirmation releases the operation without invoking native staging", async () => {
  let staged = 0;
  const app = manualBackupHarness({ stageRestore: async () => staged++ });
  app.values.confirm = false;
  await app.restoreBackup("safe.sqlite3");
  assert.equal(staged, 0);
  assert.equal(app.manualBackupOperation.current, null);
  app.values.confirm = true;
  await app.restoreBackup("safe.sqlite3");
  assert.equal(staged, 1);
});

function backupHostHarness() {
  let timer;
  let focus;
  return {
    host: {
      setInterval(callback) {
        timer = callback;
        return 1;
      },
      clearInterval() {
        timer = undefined;
      },
      addEventListener(_event, callback) {
        focus = callback;
      },
      removeEventListener() {
        focus = undefined;
      },
    },
    focus: () => focus?.(),
    tick: () => timer?.(),
  };
}

test("actual App close pauses automatic triggers and waits for both manual and automatic terminal results", async () => {
  const automatic = deferred();
  const manual = deferred();
  const host = backupHostHarness();
  let automaticCalls = 0;
  let manualCalls = 0;
  const schedule = startBackupSchedule(
    host.host,
    async () => {
      automaticCalls++;
      await automatic.promise;
    },
    () => {},
  );
  const edits = [];
  let latest = "initial";
  const app = manualBackupHarness(
    {
      createBackup: async () => {
        manualCalls++;
        await manual.promise;
      },
    },
    async () => edits.push(latest),
    async () => {},
    schedule,
  );
  const operation = app.createBackup();
  await new Promise(setImmediate);
  let onClose;
  let destroyed = 0;
  const close = hookHarness("useCloseGuard", {}, () => ({
    onCloseRequested: async (callback) => {
      onClose = callback;
      return () => {};
    },
    destroy: async () => destroyed++,
  }));
  close.render(app.beforeClose, undefined, app.onCloseError);
  await new Promise(setImmediate);
  const closing = onClose({ preventDefault() {} });
  manual.resolve();
  await operation;
  assert.equal(destroyed, 0, "Automatic backup is still running");
  assert.deepEqual(edits, ["initial"]);
  host.focus();
  host.tick();
  await app.createBackup();
  assert.equal(automaticCalls, 1);
  assert.equal(manualCalls, 1);
  latest = "typed while waiting for automatic backup";
  automatic.resolve();
  await closing;
  assert.deepEqual(edits, ["initial", latest]);
  assert.equal(destroyed, 1);
  schedule();
});

test("automatic failure during actual App close keeps the window open and resumes checks for retry", async () => {
  const failure = deferred();
  const host = backupHostHarness();
  let attempts = 0;
  let automaticError;
  const schedule = startBackupSchedule(
    host.host,
    async () => {
      if (++attempts === 1) await failure.promise;
    },
    (error) => (automaticError = error),
  );
  let flushes = 0;
  const app = manualBackupHarness(
    {},
    async () => flushes++,
    async () => {},
    schedule,
  );
  let onClose;
  let destroyed = 0;
  const close = hookHarness("useCloseGuard", {}, () => ({
    onCloseRequested: async (callback) => {
      onClose = callback;
      return () => {};
    },
    destroy: async () => destroyed++,
  }));
  close.render(app.beforeClose, undefined, app.onCloseError);
  await new Promise(setImmediate);
  const closing = onClose({ preventDefault() {} });
  failure.reject(new Error("synthetic automatic failure"));
  await closing;
  assert.equal(destroyed, 0);
  assert.equal(flushes, 0);
  assert.equal(app.closePending.current, false);
  assert.match(automaticError.message, /synthetic automatic failure/);
  assert.match(app.values.status, /关闭前.*synthetic automatic failure/);
  await new Promise(setImmediate);
  host.focus();
  await new Promise(setImmediate);
  assert.equal(attempts, 2, "Failed close resumed the automatic scheduler");
  await onClose({ preventDefault() {} });
  assert.equal(flushes, 1);
  assert.equal(destroyed, 1);
  schedule();
});

test("a later cancellation failure also resumes actual App backup scheduling and permits a new close", async () => {
  const host = backupHostHarness();
  let checks = 0;
  const schedule = startBackupSchedule(
    host.host,
    async () => checks++,
    () => {},
  );
  const app = manualBackupHarness(
    {},
    async () => {},
    async () => {},
    schedule,
  );
  let onClose;
  let destroyed = 0;
  let cancellations = 0;
  const close = hookHarness("useCloseGuard", {}, () => ({
    onCloseRequested: async (callback) => {
      onClose = callback;
      return () => {};
    },
    destroy: async () => destroyed++,
  }));
  close.render(
    app.beforeClose,
    async () => {
      if (++cancellations === 1) throw new Error("synthetic cancellation failure");
    },
    app.onCloseError,
  );
  await new Promise(setImmediate);
  await onClose({ preventDefault() {} });
  assert.equal(destroyed, 0);
  assert.equal(app.closePending.current, false);
  host.focus();
  await new Promise(setImmediate);
  assert.equal(checks, 2);
  await onClose({ preventDefault() {} });
  assert.equal(destroyed, 1);
  schedule();
});

function appWorkflowHarness(api, initial = {}) {
  const values = {
    current: asset(1),
    assets: [asset(1), asset(2)],
    total: 4,
    loading: false,
    effectiveFilter: { query: "" },
    searchMode: "normal",
    similarSource: null,
    status: "",
    ...initial,
  };
  const refreshSeq = { current: 0 };
  const selectSeq = { current: 0 };
  const setter = (key) => (value) => {
    values[key] = typeof value === "function" ? value(values[key]) : value;
  };
  const draft = hookHarness("useEditorDraft", api);
  const render = () => {
    const editor = draft.render(
      values.current,
      (record) => {
        if (values.current?.id === record.id) values.current = record;
      },
      async () => {},
      setter("status"),
    );
    const scope = {
      ...values,
      api,
      PAGE_SIZE: 240,
      refreshSeq,
      selectSeq,
      flushEditor: editor.flush,
      getEditEpoch: editor.getEditEpoch,
      loadIfUnchanged: editor.loadIfUnchanged,
      setLoading: setter("loading"),
      setAssets: setter("assets"),
      setTotal: setter("total"),
      setSemanticScores: setter("scores"),
      setCurrent: setter("current"),
      setSelected: setter("selected"),
      setStatus: setter("status"),
    };
    const callbacks = Object.fromEntries(
      Object.entries(appCallbackSources).map(([name, source]) => {
        const module = { exports: {} };
        vm.runInNewContext(source, { ...scope, module });
        return [name, module.exports];
      }),
    );
    return { ...callbacks, editor };
  };
  return { values, render };
}

test("actual App selection does not discard an in-flight next page", async () => {
  const page = deferred();
  const record = deferred();
  const app = appWorkflowHarness({ page: () => page.promise, get: () => record.promise });
  const loading = app.render().loadMore();
  assert.equal(app.values.loading, true);
  const selection = app.render().selectRecord(2);
  await Promise.resolve();
  assert.equal(app.values.loading, true, "Selecting does not clear gallery loading");
  record.resolve(asset(2));
  await selection;
  page.resolve({ items: [asset(3), asset(4)], total: 4 });
  await loading;
  assert.deepEqual(
    Array.from(app.values.assets, (item) => item.id),
    [1, 2, 3, 4],
  );
  assert.equal(app.values.current.id, 2);
  assert.equal(app.values.loading, false);
});

test("actual App filter refresh rejects the previous page and pending selection", async () => {
  const oldPage = deferred();
  const newPage = deferred();
  const oldRecord = deferred();
  let pageRequests = 0;
  const app = appWorkflowHarness({
    page: () => (++pageRequests === 1 ? oldPage.promise : newPage.promise),
    get: (id) => (id === 2 ? oldRecord.promise : Promise.resolve(asset(id))),
  });
  const paging = app.render().loadMore();
  const selection = app.render().selectRecord(2);
  app.values.effectiveFilter = { query: "new filter" };
  const refreshing = app.render().refresh();
  await Promise.resolve();
  oldRecord.resolve(asset(2));
  oldPage.resolve({ items: [asset(3), asset(4)], total: 4 });
  await Promise.all([paging, selection]);
  assert.equal(app.values.current.id, 1);
  assert.equal(app.values.loading, true, "Old paging cannot clear the new refresh loading");
  newPage.resolve({ items: [asset(10)], total: 1 });
  await refreshing;
  assert.deepEqual(
    Array.from(app.values.assets, (item) => item.id),
    [10],
  );
  assert.equal(app.values.current.id, 10);
  assert.equal(app.values.loading, false);
});

test("actual App refresh applies its gallery without replacing a newer selection", async () => {
  const page = deferred();
  const app = appWorkflowHarness({
    page: () => page.promise,
    get: async (id) => asset(id),
  });
  const refresh = app.render().refresh();
  await Promise.resolve();
  await app.render().selectRecord(2);
  page.resolve({ items: [asset(1), asset(2), asset(3)], total: 3 });
  await refresh;
  assert.deepEqual(
    Array.from(app.values.assets, (item) => item.id),
    [1, 2, 3],
  );
  assert.equal(app.values.current.id, 2);
  assert.equal(app.values.loading, false);
});

test("actual App refresh record completion preserves input typed during the request", async () => {
  const record = deferred();
  const app = appWorkflowHarness({
    page: async () => ({ items: [asset(1), asset(2)], total: 2 }),
    get: () => record.promise,
  });
  const refresh = app.render().refresh();
  await Promise.resolve();
  await Promise.resolve();
  app.render().editor.setPrompt("new input during refresh");
  record.resolve(asset(1, "server prompt"));
  await refresh;
  assert.equal(app.render().editor.prompt, "new input during refresh");
  assert.equal(app.values.current.prompt, "prompt 1");
  assert.match(app.values.status, /编辑已保留/);
  assert.equal(app.values.loading, false);
});

function visionParams(current) {
  const values = { current, analysis: null, dna: null, loading: false, prompt: "", status: "" };
  const setter = (key) => (value) => {
    values[key] = typeof value === "function" ? value(values[key]) : value;
  };
  return {
    values,
    params: {
      current,
      imagePromptAnalysis: null,
      imagePromptLoading: false,
      setImagePromptAnalysis: setter("analysis"),
      setImagePromptLoading: setter("loading"),
      setVisualDna: setter("dna"),
      setCurrent: setter("current"),
      setAssets: () => {},
      setTab: () => {},
      setPrompt: setter("prompt"),
      setStatus: setter("status"),
    },
  };
}

test("late Vision analysis and completion cannot change the next asset", async () => {
  const firstRequest = deferred();
  const secondRequest = deferred();
  const hook = hookHarness("useVisionWorkflow", {
    visionSettings: async () => null,
    analyzeImageToPrompt: (id) => (id === 1 ? firstRequest.promise : secondRequest.promise),
  });
  const { params, values } = visionParams(asset(1));
  const first = hook.render(params).analyzeCurrentImage();
  params.current = values.current = asset(2);
  params.imagePromptLoading = false;
  const second = hook.render(params).analyzeCurrentImage();
  firstRequest.resolve({ id: 10, asset_id: 1, model: "old model", prompt: "old" });
  await first;
  assert.equal(values.analysis, null);
  assert.equal(values.loading, true);
  secondRequest.resolve({ id: 20, asset_id: 2, model: "new model", prompt: "new" });
  await second;
  assert.equal(values.analysis.asset_id, 2);
  assert.equal(values.loading, false);
});

test("switching away and back also rejects an analysis from the earlier visit", async () => {
  const request = deferred();
  const hook = hookHarness("useVisionWorkflow", {
    visionSettings: async () => null,
    analyzeImageToPrompt: () => request.promise,
  });
  const { params, values } = visionParams(asset(1));
  const pending = hook.render(params).analyzeCurrentImage();
  params.current = asset(2);
  hook.render(params);
  params.current = values.current = asset(1);
  hook.render(params);
  request.resolve({ id: 10, asset_id: 1, model: "old model" });
  await pending;
  assert.equal(values.analysis, null);
});

test("late Visual DNA save and late record refresh cannot select the old asset", async () => {
  const write = deferred();
  const refresh = deferred();
  const hook = hookHarness("useVisionWorkflow", {
    visionSettings: async () => null,
    updateVisualDna: () => write.promise,
    get: () => refresh.promise,
  });
  const { params, values } = visionParams(asset(1));
  const saved = hook.render(params).saveVisualDna({ subject: "first" });
  write.resolve({ subject: "first" });
  await new Promise(setImmediate);
  params.current = values.current = asset(2);
  values.dna = { subject: "second" };
  hook.render(params);
  refresh.resolve(asset(1));
  await saved;
  assert.equal(values.current.id, 2);
  assert.equal(values.dna.subject, "second");
});

test("an analysis belonging to another asset cannot fill DNA, prompt, or revisions", async () => {
  let writes = 0;
  const hook = hookHarness("useVisionWorkflow", {
    visionSettings: async () => null,
    applyImagePromptDna: async () => ++writes,
    saveImagePromptRevision: async () => ++writes,
  });
  const { params, values } = visionParams(asset(2));
  params.imagePromptAnalysis = { id: 10, asset_id: 1, prompt: "wrong prompt" };
  const workflow = hook.render(params);
  await workflow.applyAnalysisDna();
  workflow.useAnalysisPrompt();
  await workflow.saveAnalysisRevision();
  assert.equal(writes, 0);
  assert.equal(values.prompt, "");
});

test("editor flush rejects a failed write, keeps the draft, and allows retry", async () => {
  let attempts = 0;
  let saved;
  let status = "";
  const hook = hookHarness("useEditorDraft", {
    updatePrompt: async (id, patch) => {
      attempts++;
      if (attempts === 1) throw new Error("synthetic disk failure");
      return { ...asset(id), ...patch };
    },
  });
  const current = asset(1);
  const onSaved = (value) => (saved = value);
  const onTagsSaved = () => {};
  const setStatus = (value) => (status = value);
  const draft = hook.render(current, onSaved, onTagsSaved, setStatus);
  draft.setPrompt("keep this draft");
  await assert.rejects(draft.flush(), /synthetic disk failure/);
  assert.match(status, /保存失败/);
  assert.equal(hook.render(current, onSaved, onTagsSaved, setStatus).prompt, "keep this draft");
  await draft.flush();
  assert.equal(attempts, 2);
  assert.equal(saved.prompt, "keep this draft");
});

test("revision save reports failure and retries the same draft", async () => {
  let attempts = 0;
  const hook = hookHarness("useEditorDraft", {
    savePromptRevision: async (id, patch) => {
      if (++attempts === 1) throw new Error("synthetic revision failure");
      return { ...asset(id), ...patch };
    },
  });
  const draft = hook.render(
    asset(1),
    () => {},
    () => {},
    () => {},
  );
  draft.setPrompt("revision draft");
  await assert.rejects(draft.saveRevision("note"), /synthetic revision failure/);
  const saved = await draft.saveRevision("note");
  assert.equal(saved.prompt, "revision draft");
});

test("a late editor save cannot corrupt the baseline of the next asset", async () => {
  const firstWrite = deferred();
  const writes = [];
  const hook = hookHarness("useEditorDraft", {
    updatePrompt: (id, patch) => {
      writes.push({ id, patch });
      return id === 1 ? firstWrite.promise : Promise.resolve({ ...asset(id), ...patch });
    },
  });
  const callbacks = [() => {}, () => {}, () => {}];
  const first = hook.render(asset(1), ...callbacks);
  first.setPrompt("old draft");
  const pending = first.flush();
  await new Promise(setImmediate);
  const second = hook.render(asset(2), ...callbacks);
  second.setPrompt("new draft");
  firstWrite.resolve(asset(1, "old draft"));
  await pending;
  await second.flush();
  assert.deepEqual(
    writes.map((write) => write.id),
    [1, 2],
  );
  assert.equal(writes[1].patch.prompt, "new draft");
});

test("edits made during an in-flight save are preserved and saved in order", async () => {
  const firstWrite = deferred();
  const writes = [];
  const hook = hookHarness("useEditorDraft", {
    updatePrompt: (id, patch) => {
      writes.push(patch.prompt);
      return writes.length === 1 ? firstWrite.promise : Promise.resolve({ ...asset(id), ...patch });
    },
  });
  const callbacks = [() => {}, () => {}, () => {}];
  const current = asset(1);
  const draft = hook.render(current, ...callbacks);
  draft.setPrompt("first change");
  const firstSave = draft.flush();
  await new Promise(setImmediate);
  draft.setPrompt("newer change");
  const secondSave = draft.flush();
  firstWrite.resolve(asset(1, "first change"));
  await Promise.all([firstSave, secondSave]);
  assert.deepEqual(writes, ["first change", "newer change"]);
  assert.equal(hook.render(current, ...callbacks).prompt, "newer change");
  await draft.flush();
  assert.equal(writes.length, 2);
});

test("a delayed reload cannot erase new input for the same asset or switch away from it", async () => {
  const writes = [];
  const hook = hookHarness("useEditorDraft", {
    updatePrompt: async (id, patch) => {
      writes.push(patch.prompt);
      return { ...asset(id), ...patch };
    },
  });
  const callbacks = [() => {}, () => {}, () => {}];
  const current = asset(1);
  const draft = hook.render(current, ...callbacks);
  await draft.flush();
  const epoch = draft.getEditEpoch();
  const response = deferred();
  const reload = response.promise.then((record) => draft.loadIfUnchanged(record, epoch));
  draft.setPrompt("typed while reading");
  assert.equal(draft.hasUnsavedChanges(), true);
  response.resolve(asset(1, "old response"));
  assert.equal(await reload, false);
  assert.equal(draft.loadIfUnchanged(asset(2), epoch), false);
  assert.equal(hook.render(current, ...callbacks).prompt, "typed while reading");
  await draft.flush();
  assert.equal(draft.hasUnsavedChanges(), false);
  assert.deepEqual(writes, ["typed while reading"]);
  assert.equal(draft.loadIfUnchanged(asset(1, "stale saved response"), epoch), false);
  const cleanEpoch = draft.getEditEpoch();
  assert.equal(draft.loadIfUnchanged(asset(2), cleanEpoch), true);
});

test("a late server mutation rebases the baseline and persists newer input again", async () => {
  const writes = [];
  const hook = hookHarness("useEditorDraft", {
    updatePrompt: async (id, patch) => {
      writes.push(patch.prompt);
      return { ...asset(id), ...patch };
    },
  });
  const callbacks = [() => {}, () => {}, () => {}];
  const current = asset(1);
  const draft = hook.render(current, ...callbacks);
  const epoch = draft.getEditEpoch();
  draft.setPrompt("newer durable edit");
  await draft.flush();
  assert.equal(draft.hasUnsavedChanges(), false);
  const restored = asset(1, "late restored revision");
  assert.equal(draft.loadIfUnchanged(restored, epoch), false);
  assert.equal(draft.rebaseIfCurrent(restored), true);
  assert.equal(draft.hasUnsavedChanges(), true);
  await draft.flush();
  assert.deepEqual(writes, ["newer durable edit", "newer durable edit"]);
  assert.equal(draft.hasUnsavedChanges(), false);
  assert.equal(hook.render(current, ...callbacks).prompt, "newer durable edit");
  assert.equal(draft.rebaseIfCurrent(asset(2)), false);
});

test("close waits for saving, blocks repeated requests, and remains open after failure", async () => {
  let callback;
  let destroyed = 0;
  let cancelled = 0;
  let closeError;
  const request = deferred();
  let fail = true;
  const win = {
    onCloseRequested: async (handler) => {
      callback = handler;
      return () => {};
    },
    destroy: async () => destroyed++,
  };
  const hook = hookHarness("useCloseGuard", {}, () => win);
  hook.render(
    async () => {
      if (fail) await request.promise;
    },
    async () => cancelled++,
    (error) => (closeError = error),
  );
  await new Promise(setImmediate);
  let prevented = 0;
  const event = { preventDefault: () => prevented++ };
  const closing = callback(event);
  await callback(event);
  assert.equal(prevented, 2);
  assert.equal(destroyed, 0);
  request.reject(new Error("synthetic save failure"));
  await closing;
  assert.equal(destroyed, 0);
  assert.equal(cancelled, 0);
  assert.match(closeError.message, /synthetic save failure/);
  fail = false;
  await callback(event);
  assert.equal(cancelled, 1);
  assert.equal(destroyed, 1);
});

for (const kind of ["Import", "Semantic"]) {
  test(`real ${kind} job cancellation rejects failure and remains available for retry`, async () => {
    let attempts = 0;
    let status = "";
    const api = {
      [`cancel${kind === "Import" ? "Import" : "SemanticIndex"}`]: async () => {
        if (++attempts === 1) throw new Error("synthetic cancellation failure");
        return true;
      },
      startSemanticIndex: async () => 7,
    };
    const hook = hookHarness(`use${kind}Job`, api);
    const callbacks = [() => {}, (value) => (status = value)];
    const job = hook.render(...callbacks);
    if (kind === "Import") await job.start("starting", async () => 7);
    else await job.start();
    await assert.rejects(job.cancel(), /synthetic cancellation failure/);
    assert.match(status, /取消.*失败/);
    assert.equal(hook.render(...callbacks).active, true);
    const retry = job.cancel();
    await new Promise(setImmediate);
    hook.emitProgress({
      job_id: 7,
      processed: 1,
      total: 1,
      added: 0,
      skipped: 0,
      duplicates: 0,
      indexed: 0,
      failed: 0,
      last_id: null,
      current_name: "",
      done: true,
      cancelled: true,
    });
    await retry;
    assert.equal(hook.render(...callbacks).active, false);
    assert.equal(attempts, 2);
  });
}

test("closing with a real import cancellation failure keeps the window open", async () => {
  const jobHook = hookHarness("useImportJob", {
    cancelImport: async () => {
      throw new Error("synthetic cancellation failure");
    },
  });
  const job = jobHook.render(
    () => {},
    () => {},
  );
  await job.start("starting", async () => 7);
  let onClose;
  let destroyed = 0;
  let closeError;
  const closeHook = hookHarness("useCloseGuard", {}, () => ({
    onCloseRequested: async (callback) => {
      onClose = callback;
      return () => {};
    },
    destroy: async () => destroyed++,
  }));
  closeHook.render(
    async () => {},
    job.cancel,
    (error) => (closeError = error),
  );
  await new Promise(setImmediate);
  await onClose({ preventDefault() {} });
  assert.equal(destroyed, 0);
  assert.match(closeError.message, /synthetic cancellation failure/);
});
