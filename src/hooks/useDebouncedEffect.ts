import { useEffect, useRef } from "react";
export function useDebouncedEffect(fn:()=>void,deps:unknown[],delay=550){const first=useRef(true);useEffect(()=>{if(first.current){first.current=false;return}const t=window.setTimeout(fn,delay);return()=>window.clearTimeout(t)},deps);}
