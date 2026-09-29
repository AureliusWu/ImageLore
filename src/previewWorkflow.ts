export type PreviewModeValue="fit"|"actual";

export const MIN_ZOOM=25;
export const MAX_ZOOM=400;

export const clampZoom=(value:number)=>Math.max(MIN_ZOOM,Math.min(MAX_ZOOM,Math.round(value)));

export const zoomStepFor=(zoom:number)=>zoom>=200?25:zoom>=100?10:5;

export const zoomFromWheel=(zoom:number,deltaY:number,mode:PreviewModeValue)=>{
  const base=mode==="fit"?100:clampZoom(zoom);
  if(deltaY===0)return base;
  return clampZoom(base+(deltaY<0?1:-1)*zoomStepFor(base));
};

export const nextAssetIndex=(length:number,currentIndex:number,direction:-1|1)=>{
  if(length<=0)return -1;
  const start=currentIndex>=0&&currentIndex<length?currentIndex:(direction>0?-1:length);
  return Math.max(0,Math.min(length-1,start+direction));
};

export const contextMenuPosition=(x:number,y:number,viewportWidth:number,viewportHeight:number,menuWidth=236,menuHeight=310,padding=8)=>{
  const maxX=Math.max(padding,viewportWidth-menuWidth-padding);
  const maxY=Math.max(padding,viewportHeight-menuHeight-padding);
  return {left:Math.max(padding,Math.min(x,maxX)),top:Math.max(padding,Math.min(y,maxY))};
};

const IMAGE_EXTENSIONS=new Set(["png","jpg","jpeg","webp","bmp","gif"]);

export const saveExtension=(name:string,format:string)=>{
  const fromName=name.match(/\.([^.\\/]+)$/)?.[1]?.toLowerCase();
  if(fromName&&IMAGE_EXTENSIONS.has(fromName))return fromName;
  const normalized=(format||"").trim().toLowerCase().replace(/^image\//,"");
  if(normalized==="jpeg")return "jpg";
  return IMAGE_EXTENSIONS.has(normalized)?normalized:"png";
};
