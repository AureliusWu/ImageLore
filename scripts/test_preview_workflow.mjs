import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {
  MIN_ZOOM,
  MAX_ZOOM,
  clampZoom,
  zoomFromWheel,
  nextAssetIndex,
  contextMenuPosition,
  saveExtension,
} from "../src/previewWorkflow.ts";

test("zoom is clamped to the supported range", () => {
  assert.equal(clampZoom(0), MIN_ZOOM);
  assert.equal(clampZoom(99.6), 100);
  assert.equal(clampZoom(999), MAX_ZOOM);
});

test("wheel zoom uses stable steps and ignores zero-delta events", () => {
  assert.equal(zoomFromWheel(100, -1, "fit"), 110);
  assert.equal(zoomFromWheel(100, 1, "actual"), 90);
  assert.equal(zoomFromWheel(210, -1, "actual"), 235);
  assert.equal(zoomFromWheel(30, 1, "actual"), 25);
  assert.equal(zoomFromWheel(175, 0, "actual"), 175);
});

test("gallery navigation clamps at ends and recovers when current item is missing", () => {
  assert.equal(nextAssetIndex(5, 2, 1), 3);
  assert.equal(nextAssetIndex(5, 2, -1), 1);
  assert.equal(nextAssetIndex(5, 4, 1), 4);
  assert.equal(nextAssetIndex(5, 0, -1), 0);
  assert.equal(nextAssetIndex(5, -1, 1), 0);
  assert.equal(nextAssetIndex(5, -1, -1), 4);
  assert.equal(nextAssetIndex(0, -1, 1), -1);
});

test("context menu stays inside the viewport", () => {
  assert.deepEqual(contextMenuPosition(-20, -30, 1000, 800), { left: 8, top: 8 });
  assert.deepEqual(contextMenuPosition(999, 799, 1000, 800), { left: 756, top: 482 });
  assert.deepEqual(contextMenuPosition(20, 20, 200, 160), { left: 8, top: 8 });
});

test("save-as extension only accepts supported image formats", () => {
  assert.equal(saveExtension("image.PNG", ""), "png");
  assert.equal(saveExtension("photo.jpeg", "PNG"), "jpeg");
  assert.equal(saveExtension("strange.output", "WEBP"), "webp");
  assert.equal(saveExtension("no-extension", "image/jpeg"), "jpg");
  assert.equal(saveExtension("no-extension", "unknown"), "png");
});

function assetContextHarness() {
  const states = [];
  const effects = [];
  const pendingEffects = [];
  const requests = new Map();
  let stateIndex = 0;
  let effectIndex = 0;
  const react = {
    useState(initial) {
      const index = stateIndex++;
      if (!(index in states)) states[index] = initial;
      return [
        states[index],
        (value) => {
          states[index] = typeof value === "function" ? value(states[index]) : value;
        },
      ];
    },
    useEffect(callback, dependencies) {
      const index = effectIndex++;
      const previous = effects[index];
      if (
        !previous ||
        dependencies.some((value, i) => !Object.is(value, previous.dependencies[i]))
      ) {
        pendingEffects.push(() => {
          previous?.cleanup?.();
          effects[index] = { dependencies, cleanup: callback() };
        });
      }
    },
  };
  const api = {
    visualDna: async (id) => ({ subject: `subject ${id}` }),
    latestImagePromptAnalysis: async () => null,
    latestRemixDraft: async () => null,
    referenceSources(id) {
      let resolve;
      const promise = new Promise((done) => {
        resolve = done;
      });
      requests.set(id, { promise, resolve });
      return promise;
    },
  };
  const module = { exports: {} };
  const source = readFileSync(new URL("../src/hooks/useAssetContext.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  vm.runInNewContext(
    outputText,
    {
      module,
      exports: module.exports,
      require(name) {
        if (name === "react") return react;
        if (name === "../api") return { api };
        throw new Error(`Unexpected Hook dependency: ${name}`);
      },
    },
    { filename: "useAssetContext.ts" },
  );
  const renderOnce = (current) => {
    stateIndex = 0;
    effectIndex = 0;
    return module.exports.useAssetContext(current);
  };
  return {
    render(current) {
      renderOnce(current);
      for (const effect of pendingEffects.splice(0)) effect();
      return renderOnce(current);
    },
    async resolveReferences(id, references) {
      requests.get(id).resolve(references);
      await new Promise(setImmediate);
    },
  };
}

test("switching assets clears the previous references while the next asset loads", async () => {
  const hook = assetContextHarness();
  const first = { id: 1, name: "first.png", prompt: "first prompt" };
  const second = { id: 2, name: "second.png", prompt: "second prompt" };
  const firstReferences = [{ id: 11, asset_id: 1, page_title: "First source" }];
  const secondReferences = [{ id: 22, asset_id: 2, page_title: "Second source" }];

  hook.render(first);
  await hook.resolveReferences(first.id, firstReferences);
  assert.equal(hook.render(first).referenceSources, firstReferences);

  assert.equal(hook.render(second).referenceSources.length, 0);
  await hook.resolveReferences(second.id, secondReferences);
  assert.equal(hook.render(second).referenceSources, secondReferences);
  assert.equal(hook.render(null).referenceSources.length, 0);
});

test("a late response from the previous asset cannot replace the current context", async () => {
  const hook = assetContextHarness();
  const first = { id: 1, name: "first.png", prompt: "first prompt" };
  const second = { id: 2, name: "second.png", prompt: "second prompt" };
  const firstReferences = [{ id: 11, asset_id: 1, page_title: "First source" }];
  const secondReferences = [{ id: 22, asset_id: 2, page_title: "Second source" }];

  hook.render(first);
  hook.render(second);
  await hook.resolveReferences(second.id, secondReferences);
  assert.equal(hook.render(second).referenceSources, secondReferences);

  await hook.resolveReferences(first.id, firstReferences);
  const current = hook.render(second);
  assert.equal(current.referenceSources, secondReferences);
  assert.equal(current.visualDna.subject, "subject 2");
  assert.equal(current.remixSources[0].asset_id, second.id);
  assert.equal(current.remixPrompt, second.prompt);
});
