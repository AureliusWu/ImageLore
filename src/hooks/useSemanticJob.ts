import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauri } from "../api";
import type { SemanticProgress } from "../types";

const empty: SemanticProgress = {
  job_id: 0,
  processed: 0,
  total: 0,
  indexed: 0,
  skipped: 0,
  failed: 0,
  current_name: "",
  done: false,
  cancelled: false,
};

export function useSemanticJob(
  onDone: () => void | Promise<void>,
  setStatus: (value: string) => void,
) {
  const [progress, setProgress] = useState<SemanticProgress>(empty);
  const [starting, setStarting] = useState(false);
  const jobRef = useRef(0);
  const startingRef = useRef(false);
  const bufferedRef = useRef<SemanticProgress[]>([]);
  const startWaitersRef = useRef<Array<() => void>>([]);
  const doneWaitersRef = useRef<Array<() => void>>([]);
  const onDoneRef = useRef(onDone);
  const statusRef = useRef(setStatus);

  useEffect(() => {
    onDoneRef.current = onDone;
  }, [onDone]);
  useEffect(() => {
    statusRef.current = setStatus;
  }, [setStatus]);

  const applyProgress = useCallback((next: SemanticProgress) => {
    setProgress((prev) => ({
      ...prev,
      ...next,
      total: next.total || prev.total,
      processed: next.done ? prev.processed : next.processed,
    }));
    if (next.done) {
      jobRef.current = 0;
      startingRef.current = false;
      setStarting(false);
      statusRef.current(
        next.cancelled
          ? "语义索引已取消"
          : next.failed
            ? "语义索引完成：新增 " + next.indexed + "，失败 " + next.failed
            : "语义索引完成：新增 " + next.indexed,
      );
      void onDoneRef.current();
      for (const resolve of doneWaitersRef.current.splice(0)) resolve();
    } else if (next.current_name.startsWith("正在准备")) {
      statusRef.current(next.current_name);
    } else {
      statusRef.current(
        "正在建立语义索引 " + next.processed + "/" + next.total + " · " + next.current_name,
      );
    }
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<SemanticProgress>("imagelore://semantic-progress", (event) => {
      const next = event.payload;
      if (jobRef.current === 0) {
        if (startingRef.current) bufferedRef.current.push(next);
        return;
      }
      if (next.job_id === jobRef.current) applyProgress(next);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e) => statusRef.current("语义索引监听启动失败：" + String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyProgress]);

  const start = useCallback(async () => {
    if (jobRef.current || startingRef.current) {
      statusRef.current("已有语义索引任务正在进行");
      return false;
    }
    startingRef.current = true;
    setStarting(true);
    bufferedRef.current = [];
    setProgress(empty);
    statusRef.current("正在准备本地语义模型…");
    try {
      const id = await api.startSemanticIndex();
      jobRef.current = id;
      setProgress((p) => ({ ...p, job_id: id }));
      const buffered = bufferedRef.current;
      bufferedRef.current = [];
      for (const event of buffered) {
        if (event.job_id === id) applyProgress(event);
      }
      return true;
    } catch (e) {
      jobRef.current = 0;
      bufferedRef.current = [];
      setProgress(empty);
      statusRef.current("语义索引启动失败：" + String(e));
      return false;
    } finally {
      startingRef.current = false;
      setStarting(false);
      for (const resolve of startWaitersRef.current.splice(0)) resolve();
    }
  }, [applyProgress]);

  const cancel = useCallback(async () => {
    if (startingRef.current)
      await new Promise<void>((resolve) => startWaitersRef.current.push(resolve));
    const id = jobRef.current;
    if (!id) return;
    statusRef.current("正在取消语义索引…");
    let resolveDone: () => void = () => {};
    const done = new Promise<void>((resolve) => {
      resolveDone = resolve;
      doneWaitersRef.current.push(resolve);
    });
    try {
      const accepted = await api.cancelSemanticIndex(id);
      if (!accepted || jobRef.current === 0) resolveDone();
      await done;
    } catch (e) {
      resolveDone();
      statusRef.current("取消语义索引失败：" + String(e));
    } finally {
      doneWaitersRef.current = doneWaitersRef.current.filter((x) => x !== resolveDone);
    }
  }, []);

  return { active: starting || (progress.job_id !== 0 && !progress.done), progress, start, cancel };
}
