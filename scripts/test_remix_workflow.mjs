import test from "node:test";
import assert from "node:assert/strict";
import {buildRemixPrompt,nonEmptyDnaFields,remixFieldLabel} from "../src/remixWorkflow.ts";

const dna=(values={})=>({subject:"",character:"",outfit:"",pose:"",expression:"",composition:"",camera:"",lighting:"",environment:"",palette:"",material:"",style:"",source:"manual",updated_at:0,...values});

test("detects only non-empty DNA fields",()=>{
  assert.deepEqual(nonEmptyDnaFields(dna({subject:"角色",lighting:"柔光"})),["subject","lighting"]);
});

test("builds provenance-aware remix prompt and deduplicates identical field values",()=>{
  const sources=[
    {asset_id:1,asset_name:"Base.png",fields:["subject","style"],source_url:"",visual_dna:dna({subject:"蓝发角色",style:"写实摄影"})},
    {asset_id:2,asset_name:"Light.png",fields:["lighting","style"],source_url:"",visual_dna:dna({lighting:"霓虹侧光",style:"写实摄影"})}
  ];
  const prompt=buildRemixPrompt("基础提示词",sources,1);
  assert.match(prompt,/基础提示词/);
  assert.match(prompt,/主体：蓝发角色/);
  assert.match(prompt,/光线（参考《Light.png》）：霓虹侧光/);
  assert.equal((prompt.match(/写实摄影/g)||[]).length,1);
});

test("returns the base prompt when no reusable fields are selected",()=>{
  assert.equal(buildRemixPrompt("  original  ",[],1),"original");
  assert.equal(remixFieldLabel("composition"),"构图");
});
