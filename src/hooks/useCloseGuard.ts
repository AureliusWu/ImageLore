import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "../api";

export function useCloseGuard(
  beforeClose: () => Promise<void>,
  cancelTask?: () => Promise<void>,
  onError?: (error: unknown) => void,
) {
  const beforeRef = useRef(beforeClose);
  const cancelRef = useRef(cancelTask);
  const errorRef = useRef(onError);

  useEffect(() => {
    beforeRef.current = beforeClose;
  }, [beforeClose]);
  useEffect(() => {
    cancelRef.current = cancelTask;
  }, [cancelTask]);
  useEffect(() => {
    errorRef.current = onError;
  }, [onError]);

  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let closing = false;
    const win = getCurrentWindow();

    win
      .onCloseRequested(async (event) => {
        event.preventDefault();
        if (closing) return;
        closing = true;
        try {
          await beforeRef.current();
          await cancelRef.current?.();
          await win.destroy();
        } catch (error) {
          // Saving reports its error to the editor. Keep the window available for retry.
          closing = false;
          errorRef.current?.(error);
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
