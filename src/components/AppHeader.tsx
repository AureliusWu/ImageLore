import type { SearchMode } from "../types";

export function AppHeader({version,query,mode,similarSourceName,onMode,onQuery,onClearSimilar,onImport,onFolder}:{
  version:string;query:string;mode:SearchMode;similarSourceName?:string|null;
  onMode:(mode:SearchMode)=>void;onQuery:(v:string)=>void;onClearSimilar:()=>void;onImport:()=>void;onFolder:()=>void;
}){
  return <header className="app-header glass-surface">
    <div className="brand"><span className="brand-orb"><i/></span><div><strong>ImageLore</strong><small>AI 视觉生成记忆库</small></div><span className="version-pill">v{version}</span></div>
    <div className="search-cluster">
      <div className="search-mode-switch"><button className={mode==="keyword"?"active":""} onClick={()=>onMode("keyword")}>⌕ 关键词</button><button className={mode==="semantic"?"active":""} onClick={()=>onMode("semantic")}>✦ 语义</button></div>
      <label className="search-box"><span>{mode==="semantic"?"✦":"⌕"}</span><input id="search" value={query} onChange={e=>onQuery(e.target.value)} placeholder={mode==="semantic"?"描述你记得的画面…":"搜索提示词、文件名、标签或模型"}/><kbd>Ctrl F</kbd></label>
      {similarSourceName?<button className="similar-source-chip" onClick={onClearSimilar} title="退出以图找图">◎ {similarSourceName}<span>×</span></button>:null}
    </div>
    <div className="header-actions"><button className="button secondary" onClick={onFolder}>▣ 导入文件夹</button><button className="button primary" onClick={onImport}>＋ 导入图片</button></div>
  </header>
}
