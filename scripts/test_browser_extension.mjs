import test from "node:test";
import assert from "node:assert/strict";
import {
  httpUrl,
  imageExtension,
  safeBaseName,
  suggestedImageName,
  referenceSidecar,
  sidecarPath,
} from "../browser-extension/shared.js";

test("browser capture filenames are Windows-safe and image-import compatible", () => {
  assert.equal(safeBaseName("bad:<name>?*."), "bad name");
  assert.equal(imageExtension("https://cdn.example/x/photo.webp?x=1"), "webp");
  assert.equal(imageExtension("https://cdn.example/render?id=2"), "png");
  assert.match(suggestedImageName("https://cdn.example/a/猫 图.jpg", "title", 1), /猫 图\.jpg$/);
});

test("reference sidecar keeps only http sources and explicit intent", () => {
  const sidecar = referenceSidecar({
    sourceUrl: "https://cdn.example/a.png",
    pageUrl: "https://example.test/post",
    pageTitle: " Inspiration ",
    intent: "remix",
    capturedAt: 42,
  });
  assert.equal(sidecar.schema, "imagelore.sidecar.v3");
  assert.equal(sidecar.reference.source_type, "browser-extension");
  assert.equal(sidecar.reference.metadata.intent, "remix");
  assert.equal(sidecar.reference.captured_at, 42);
  assert.equal(
    referenceSidecar({ sourceUrl: "file:///secret", pageUrl: "javascript:x", pageTitle: "x" })
      .reference.source_url,
    "",
  );
});

test("sidecar filename exactly follows the final downloaded image name", () => {
  assert.equal(sidecarPath("image (1).png"), "ImageLore Inbox/image (1).png.imagelore.json");
  assert.equal(httpUrl("HTTPS://example.com/a"), "HTTPS://example.com/a");
});
