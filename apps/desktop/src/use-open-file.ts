import {useEffect, useLayoutEffect, useRef} from 'react';

// Subscribe once; the menu always reads the committed active workspace state.
export function useOpenFile(enabled:boolean, choose:()=>Promise<void>) {
  const current=useRef({enabled,choose});
  useLayoutEffect(()=>{current.current={enabled,choose};});
  useEffect(()=>window.fileform.onOpenFile(()=>{
    if(current.current.enabled)void current.current.choose();
  }),[]);
}
