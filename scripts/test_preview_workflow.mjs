import test from "node:test";
import assert from "node:assert/strict";
import {
  MIN_ZOOM,MAX_ZOOM,clampZoom,zoomFromWheel,nextAssetIndex,contextMenuPosition,saveExtension
} from "../src/previewWorkflow.ts";

test("zoom is clamped to the supported range",()=>{
  assert.equal(clampZoom(0),MIN_ZOOM);
  assert.equal(clampZoom(99.6),100);
  assert.equal(clampZoom(999),MAX_ZOOM);
});

test("wheel zoom uses stable steps and ignores zero-delta events",()=>{
  assert.equal(zoomFromWheel(100,-1,"fit"),110);
  assert.equal(zoomFromWheel(100,1,"actual"),90);
  assert.equal(zoomFromWheel(210,-1,"actual"),235);
  assert.equal(zoomFromWheel(30,1,"actual"),25);
  assert.equal(zoomFromWheel(175,0,"actual"),175);
});

test("gallery navigation clamps at ends and recovers when current item is missing",()=>{
  assert.equal(nextAssetIndex(5,2,1),3);
  assert.equal(nextAssetIndex(5,2,-1),1);
  assert.equal(nextAssetIndex(5,4,1),4);
  assert.equal(nextAssetIndex(5,0,-1),0);
  assert.equal(nextAssetIndex(5,-1,1),0);
  assert.equal(nextAssetIndex(5,-1,-1),4);
  assert.equal(nextAssetIndex(0,-1,1),-1);
});

test("context menu stays inside the viewport",()=>{
  assert.deepEqual(contextMenuPosition(-20,-30,1000,800),{left:8,top:8});
  assert.deepEqual(contextMenuPosition(999,799,1000,800),{left:756,top:482});
  assert.deepEqual(contextMenuPosition(20,20,200,160),{left:8,top:8});
});

test("save-as extension only accepts supported image formats",()=>{
  assert.equal(saveExtension("image.PNG",""),"png");
  assert.equal(saveExtension("photo.jpeg","PNG"),"jpeg");
  assert.equal(saveExtension("strange.output","WEBP"),"webp");
  assert.equal(saveExtension("no-extension","image/jpeg"),"jpg");
  assert.equal(saveExtension("no-extension","unknown"),"png");
});
