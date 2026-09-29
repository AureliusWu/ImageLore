import {inboxPath,referenceSidecar,sidecarDataUrl,sidecarPath,suggestedImageName} from "./shared.js";

const MENU_SAVE="imagelore-save";
const MENU_REMIX="imagelore-save-remix";

function createMenus(){
  chrome.contextMenus.removeAll(()=>{
    chrome.contextMenus.create({id:MENU_SAVE,title:"保存到 ImageLore",contexts:["image"]});
    chrome.contextMenus.create({id:MENU_REMIX,title:"保存到 ImageLore · Remix 参考",contexts:["image"]});
  });
}
chrome.runtime.onInstalled.addListener(createMenus);
chrome.runtime.onStartup.addListener(createMenus);

function waitForDownload(id){
  return new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{chrome.downloads.onChanged.removeListener(listener);reject(new Error("图片下载超时"));},120000);
    const listener=delta=>{
      if(delta.id!==id)return;
      if(delta.error?.current){
        clearTimeout(timer);chrome.downloads.onChanged.removeListener(listener);reject(new Error(delta.error.current));return;
      }
      if(delta.state?.current==="complete"){
        clearTimeout(timer);chrome.downloads.onChanged.removeListener(listener);
        chrome.downloads.search({id},items=>items[0]?resolve(items[0]):reject(new Error("无法读取下载结果")));
      }
    };
    chrome.downloads.onChanged.addListener(listener);
  });
}

function finalBaseName(filename){
  return String(filename||"").split(/[\\/]/).pop()||"imagelore-reference.png";
}

async function capture(info,tab,intent){
  const src=info.srcUrl||"";
  if(!src)throw new Error("没有可保存的图片地址");
  const pageUrl=info.pageUrl||tab?.url||"";
  const pageTitle=tab?.title||"";
  const requested=suggestedImageName(src,pageTitle);
  const id=await chrome.downloads.download({
    url:src,
    filename:inboxPath(requested),
    conflictAction:"uniquify",
    saveAs:false
  });
  const item=await waitForDownload(id);
  const imageName=finalBaseName(item.filename);
  const sidecar=referenceSidecar({sourceUrl:src,pageUrl,pageTitle,intent});
  await chrome.downloads.download({
    url:sidecarDataUrl(sidecar),
    filename:sidecarPath(imageName),
    conflictAction:"overwrite",
    saveAs:false
  });
  await chrome.storage.local.set({lastCapture:{imageName,pageUrl,pageTitle,intent,capturedAt:Date.now()}});
  await chrome.action.setBadgeBackgroundColor({color:"#3A8FA3"});
  await chrome.action.setBadgeText({text:"✓"});
  setTimeout(()=>chrome.action.setBadgeText({text:""}),3000);
}

chrome.contextMenus.onClicked.addListener((info,tab)=>{
  if(info.menuItemId!==MENU_SAVE&&info.menuItemId!==MENU_REMIX)return;
  capture(info,tab,info.menuItemId===MENU_REMIX?"remix":"reference").catch(async error=>{
    await chrome.storage.local.set({lastError:String(error?.message||error)});
    await chrome.action.setBadgeBackgroundColor({color:"#B55B5B"});
    await chrome.action.setBadgeText({text:"!"});
  });
});
