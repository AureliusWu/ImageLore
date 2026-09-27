import { useMemo } from "react";
import type { AssetRecord } from "../types";

type Part={kind:"same"|"add"|"del";text:string};
const tokenize=(text:string)=>text.split(/(\s+|[,，。；;：:、])/).filter(Boolean);
function diff(before:string,after:string):Part[]{const a=tokenize(before),b=tokenize(after);if(a.length>500||b.length>500)return before===after?[{kind:"same",text:after}]:[{kind:"del",text:before},{kind:"add",text:after}];const dp=Array.from({length:a.length+1},()=>new Uint16Array(b.length+1));for(let i=a.length-1;i>=0;i--)for(let j=b.length-1;j>=0;j--)dp[i][j]=a[i]===b[j]?dp[i+1][j+1]+1:Math.max(dp[i+1][j],dp[i][j+1]);const out:Part[]=[];let i=0,j=0;const push=(kind:Part["kind"],text:string)=>{const last=out.at(-1);if(last?.kind===kind)last.text+=text;else out.push({kind,text})};while(i<a.length&&j<b.length){if(a[i]===b[j]){push("same",a[i]);i++;j++}else if(dp[i+1][j]>=dp[i][j+1])push("del",a[i++]);else push("add",b[j++])}while(i<a.length)push("del",a[i++]);while(j<b.length)push("add",b[j++]);return out}
function DiffText({before,after}:{before:string;after:string}){const parts=useMemo(()=>diff(before||"",after||""),[before,after]);if(before===after)return <div className="diff-unchanged">无变化</div>;return <div className="diff-text">{parts.map((p,i)=><span key={i} className={`diff-${p.kind}`}>{p.text}</span>)}</div>}
function Scalar({label,before,after}:{label:string;before:string;after:string}){if(before===after)return null;return <div className="scalar-diff"><strong>{label}</strong><div><del>{before||"—"}</del><span>→</span><ins>{after||"—"}</ins></div></div>}

export function ComparePanel({parent,current,parentSrc,currentSrc}:{parent:AssetRecord;current:AssetRecord;parentSrc?:string;currentSrc?:string}){
  return <section className="compare-panel">
    <div className="compare-head"><div><span>对比</span><strong>{parent.name} → {current.name}</strong></div><span className="compare-badge">图片 + 提示词</span></div>
    <div className="compare-images">
      <figure><div>{parentSrc?<img src={parentSrc} alt={parent.name}/>:<span>加载中…</span>}</div><figcaption>父图 · {parent.name}</figcaption></figure>
      <figure><div>{currentSrc?<img src={currentSrc} alt={current.name}/>:<span>加载中…</span>}</div><figcaption>当前 · {current.name}</figcaption></figure>
    </div>
    <DiffText before={parent.prompt} after={current.prompt}/>
    <details><summary>反向提示词</summary><DiffText before={parent.negative_prompt} after={current.negative_prompt}/></details>
    <Scalar label="模型" before={parent.model} after={current.model}/>
    <Scalar label="标签" before={parent.tags.join(", ")} after={current.tags.join(", ")}/>
  </section>
}
