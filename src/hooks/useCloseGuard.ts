import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "../api";

export function useCloseGuard(beforeClose: () => Promise<void>, cancelTask?: () => Promise<void>) {
  const beforeRef = useRef(beforeClose);
  const cancelRef = useRef(cancelTask);

  useEffect(() => {
    beforeRef.current = beforeClose;
  }, [beforeClose]);
  useEffect(() => {
    cancelRef.current = cancelTask;
  }, [cancelTask]);

  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let closing = false;
    const win = getCurrentWindow();

    win
      .onCloseRequested(async (event) => {
        if (closing) return;
        event.preventDefault();
        closing = true;
        try {
          await beforeRef.current();
          await cancelRef.current?.();
        } finally {
          await win.destroy();
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
