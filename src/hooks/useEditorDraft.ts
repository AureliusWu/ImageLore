import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { AssetRecord } from "../types";

type Draft = {
  assetId: number | null;
  prompt: string;
  negative: string;
  model: string;
  tagsText: string;
};
const empty: Draft = { assetId: null, prompt: "", negative: "", model: "", tagsText: "" };
const parseTags = (text: string) => [
  ...new Set(
    text
      .split(/[,，]/)
      .map((x) => x.trim())
      .filter(Boolean),
  ),
];
const sameTags = (a: string[], b: string[]) => JSON.stringify(a) === JSON.stringify(b);

export function useEditorDraft(
  asset: AssetRecord | null,
  onSaved: (asset: AssetRecord) => void,
  onTagsSaved: () => void,
  setStatus: (text: string) => void,
) {
  const [prompt, setPromptState] = useState("");
  const [negative, setNegativeState] = useState("");
  const [model, setModelState] = useState("");
  const [tagsText, setTagsTextState] = useState("");
  const draft = useRef<Draft>({ ...empty });
  const baseline = useRef<Draft>({ ...empty });
  const timer = useRef<number | undefined>(undefined);
  const saving = useRef<Promise<void>>(Promise.resolve());
  const editEpoch = useRef(0);

  const hydrate = useCallback((next: AssetRecord | null) => {
    editEpoch.current++;
    const value: Draft = next
      ? {
          assetId: next.id,
          prompt: next.prompt,
          negative: next.negative_prompt,
          model: next.model,
          tagsText: next.tags.join(", "),
        }
      : { ...empty };
    draft.current = value;
    baseline.current = { ...value };
    setPromptState(value.prompt);
    setNegativeState(value.negative);
    setModelState(value.model);
    setTagsTextState(value.tagsText);
  }, []);

  useEffect(() => {
    hydrate(asset);
  }, [asset?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const setPrompt = (v: string) => {
    if (draft.current.prompt !== v) editEpoch.current++;
    draft.current.prompt = v;
    setPromptState(v);
  };
  const setNegative = (v: string) => {
    if (draft.current.negative !== v) editEpoch.current++;
    draft.current.negative = v;
    setNegativeState(v);
  };
  const setModel = (v: string) => {
    if (draft.current.model !== v) editEpoch.current++;
    draft.current.model = v;
    setModelState(v);
  };
  const setTagsText = (v: string) => {
    if (draft.current.tagsText !== v) editEpoch.current++;
    draft.current.tagsText = v;
    setTagsTextState(v);
  };

  const hasUnsavedChanges = useCallback(() => {
    const current = draft.current;
    const base = baseline.current;
    return (
      current.assetId !== null &&
      (current.assetId !== base.assetId ||
        current.prompt !== base.prompt ||
        current.negative !== base.negative ||
        current.model !== base.model ||
        !sameTags(parseTags(current.tagsText), parseTags(base.tagsText)))
    );
  }, []);
  const getEditEpoch = useCallback(() => editEpoch.current, []);
  const loadIfUnchanged = useCallback(
    (next: AssetRecord | null, expectedEpoch: number) => {
      if (editEpoch.current !== expectedEpoch || hasUnsavedChanges()) return false;
      hydrate(next);
      return true;
    },
    [hasUnsavedChanges, hydrate],
  );
  const rebaseIfCurrent = useCallback((next: AssetRecord) => {
    if (draft.current.assetId !== next.id) return false;
    editEpoch.current++;
    baseline.current = {
      assetId: next.id,
      prompt: next.prompt,
      negative: next.negative_prompt,
      model: next.model,
      tagsText: next.tags.join(", "),
    };
    return true;
  }, []);

  const flush = useCallback(async () => {
    const snapshot = { ...draft.current };
    const assetId = snapshot.assetId;
    if (assetId === null) return;
    window.clearTimeout(timer.current);
    saving.current = saving.current
      .catch(() => {})
      .then(async () => {
        const base = baseline.current;
        if (base.assetId !== assetId) return;
        let latest: AssetRecord | null = null;
        const textChanged =
          snapshot.prompt !== base.prompt ||
          snapshot.negative !== base.negative ||
          snapshot.model !== base.model;
        const tags = parseTags(snapshot.tagsText),
          baseTags = parseTags(base.tagsText);
        if (textChanged) {
          setStatus("正在保存提示词…");
          latest = await api.updatePrompt(assetId, {
            prompt: snapshot.prompt,
            negative_prompt: snapshot.negative,
            model: snapshot.model,
          });
        }
        if (!sameTags(tags, baseTags)) {
          latest = await api.replaceTags(assetId, tags);
          onTagsSaved();
        }
        if (latest) {
          const savedBaseline: Draft = {
            assetId: latest.id,
            prompt: latest.prompt,
            negative: latest.negative_prompt,
            model: latest.model,
            tagsText: latest.tags.join(", "),
          };
          if (draft.current.assetId === latest.id) {
            baseline.current = savedBaseline;
            onSaved(latest);
            if (
              draft.current.prompt === snapshot.prompt &&
              draft.current.negative === snapshot.negative &&
              draft.current.model === snapshot.model &&
              draft.current.tagsText === snapshot.tagsText
            ) {
              baseline.current = { ...draft.current };
            }
          }
          setStatus("已保存");
        }
      })
      .catch((e) => {
        setStatus(`保存失败：${String(e)}`);
        throw e;
      });
    await saving.current;
  }, [onSaved, onTagsSaved, setStatus]);

  const saveRevision = useCallback(
    async (note = "") => {
      const snapshot = { ...draft.current };
      const assetId = snapshot.assetId;
      if (assetId === null) return null;
      window.clearTimeout(timer.current);
      let saved: AssetRecord | null = null;
      saving.current = saving.current
        .catch(() => {})
        .then(async () => {
          if (draft.current.assetId !== assetId) return;
          setStatus("正在保存提示词版本…");
          const latest = await api.savePromptRevision(
            assetId,
            { prompt: snapshot.prompt, negative_prompt: snapshot.negative, model: snapshot.model },
            parseTags(snapshot.tagsText),
            note,
          );
          saved = latest;
          const savedBaseline: Draft = {
            assetId: latest.id,
            prompt: latest.prompt,
            negative: latest.negative_prompt,
            model: latest.model,
            tagsText: latest.tags.join(", "),
          };
          if (draft.current.assetId === latest.id) {
            baseline.current = savedBaseline;
            onSaved(latest);
            if (
              draft.current.prompt === snapshot.prompt &&
              draft.current.negative === snapshot.negative &&
              draft.current.model === snapshot.model &&
              draft.current.tagsText === snapshot.tagsText
            ) {
              draft.current = { ...baseline.current };
              setPromptState(latest.prompt);
              setNegativeState(latest.negative_prompt);
              setModelState(latest.model);
              setTagsTextState(latest.tags.join(", "));
            }
          }
          onTagsSaved();
          setStatus("提示词版本已保存");
        })
        .catch((e) => {
          setStatus(`保存版本失败：${String(e)}`);
          throw e;
        });
      await saving.current;
      return saved;
    },
    [onSaved, onTagsSaved, setStatus],
  );

  useEffect(() => {
    if (!draft.current.assetId) return;
    const base = baseline.current,
      current = draft.current;
    const dirty =
      current.prompt !== base.prompt ||
      current.negative !== base.negative ||
      current.model !== base.model ||
      !sameTags(parseTags(current.tagsText), parseTags(base.tagsText));
    if (!dirty) return;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void flush().catch(() => {});
    }, 700);
    return () => window.clearTimeout(timer.current);
  }, [prompt, negative, model, tagsText, flush]);

  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
      void flush().catch(() => {});
    },
    [flush],
  );

  return {
    prompt,
    negative,
    model,
    tagsText,
    setPrompt,
    setNegative,
    setModel,
    setTagsText,
    flush,
    saveRevision,
    load: hydrate,
    hasUnsavedChanges,
    getEditEpoch,
    loadIfUnchanged,
    rebaseIfCurrent,
  };
}
