import type { RemixSource,VisualDna } from "./types";

export const REMIX_DNA_FIELDS=[
  ["subject","主体"],["character","角色"],["outfit","服装"],["pose","姿势"],["expression","表情"],
  ["composition","构图"],["camera","镜头"],["lighting","光线"],["environment","环境"],["palette","色彩"],
  ["material","材质"],["style","风格"]
] as const;

export type RemixDnaField=typeof REMIX_DNA_FIELDS[number][0];

export const remixFieldLabel=(key:string)=>REMIX_DNA_FIELDS.find(([field])=>field===key)?.[1]||key;

export const nonEmptyDnaFields=(dna:VisualDna|null|undefined):string[]=>{
  if(!dna)return[];
  return REMIX_DNA_FIELDS.filter(([key])=>String(dna[key]||"").trim()).map(([key])=>key);
};

export const buildRemixPrompt=(basePrompt:string,sources:RemixSource[],baseAssetId:number|null|undefined)=>{
  const base=basePrompt.trim();
  const seen=new Set<string>();
  const lines:string[]=[];
  for(const source of sources){
    for(const key of source.fields){
      if(!REMIX_DNA_FIELDS.some(([field])=>field===key))continue;
      const value=String(source.visual_dna[key as RemixDnaField]||"").trim();
      if(!value)continue;
      const dedupe=`${key}\u0000${value.toLowerCase()}`;
      if(seen.has(dedupe))continue;
      seen.add(dedupe);
      const label=remixFieldLabel(key);
      lines.push(source.asset_id===baseAssetId?`${label}：${value}`:`${label}（参考《${source.asset_name}》）：${value}`);
    }
  }
  if(!lines.length)return base;
  return [base,"Remix 视觉约束：\n"+lines.join("\n")].filter(Boolean).join("\n\n");
};
