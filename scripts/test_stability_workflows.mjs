import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

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
