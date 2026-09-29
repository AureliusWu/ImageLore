export const INBOX_DIR="ImageLore Inbox";
export const SUPPORTED_EXTENSIONS=new Set(["png","jpg","jpeg","webp","gif","bmp"]);

export function httpUrl(value){
  const text=String(value||"").trim();
  return /^https?:\/\//i.test(text)?text.slice(0,2048):"";
}

export function safeBaseName(value){
  let name=String(value||"").replace(/[<>:"/\\|?*\x00-\x1f]/g," ").replace(/\s+/g," ").trim();
  name=name.replace(/[. ]+$/g,"").slice(0,120);
  if(!name||/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(name))name="imagelore-reference";
  return name;
}

export function imageExtension(sourceUrl){
  const raw=String(sourceUrl||"");
  const data=raw.match(/^data:image\/([a-zA-Z0-9.+-]+);/);
  if(data){
    const ext=data[1].toLowerCase()==="jpeg"?"jpg":data[1].toLowerCase();
    return SUPPORTED_EXTENSIONS.has(ext)?ext:"png";
  }
  try{
    const path=new URL(raw).pathname;
    const match=path.match(/\.([a-zA-Z0-9]+)$/);
    const ext=(match?.[1]||"").toLowerCase();
    return SUPPORTED_EXTENSIONS.has(ext)?ext:"png";
  }catch{return "png"}
}

export function suggestedImageName(sourceUrl,pageTitle,stamp=Date.now()){
  let fromUrl="";
  try{
    const part=decodeURIComponent(new URL(sourceUrl).pathname.split("/").filter(Boolean).pop()||"");
    fromUrl=part.replace(/\.[^.]+$/,"");
  }catch{}
  const base=safeBaseName(fromUrl||pageTitle||`imagelore-${stamp}`);
  return `${base}.${imageExtension(sourceUrl)}`;
}

export function referenceSidecar({sourceUrl,pageUrl,pageTitle,intent="reference",capturedAt=Math.floor(Date.now()/1000)}){
  const source=httpUrl(sourceUrl),page=httpUrl(pageUrl);
  return {
    schema:"imagelore.sidecar.v3",
    reference:{
      source_url:source,
      page_url:page,
      page_title:String(pageTitle||"").trim().slice(0,512),
      source_type:"browser-extension",
      captured_at:capturedAt,
      metadata:{
        host:(()=>{try{return new URL(page||source).host}catch{return""}})(),
        intent:intent==="remix"?"remix":"reference",
        extension:"save-to-imagelore",
        extension_version:"0.25.0"
      }
    }
  };
}

export function sidecarDataUrl(value){
  return "data:application/json;charset=utf-8,"+encodeURIComponent(JSON.stringify(value,null,2));
}

export function inboxPath(name){return `${INBOX_DIR}/${name}`;}
export function sidecarPath(imageName){return inboxPath(`${imageName}.imagelore.json`);}
