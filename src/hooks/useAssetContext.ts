import { useEffect, useState } from "react";
import { api } from "../api";
import type {
  AssetRecord,
  ImagePromptAnalysis,
  ReferenceSource,
  RemixDraft,
  RemixSource,
  VisualDna,
} from "../types";

export const EMPTY_VISUAL_DNA: VisualDna = {
  subject: "",
  character: "",
  outfit: "",
  pose: "",
  expression: "",
  composition: "",
  camera: "",
  lighting: "",
  environment: "",
  palette: "",
  material: "",
  style: "",
  source: "manual",
  updated_at: 0,
};

export function useAssetContext(current: AssetRecord | null) {
  const [visualDna, setVisualDna] = useState<VisualDna | null>(null);
  const [imagePromptAnalysis, setImagePromptAnalysis] = useState<ImagePromptAnalysis | null>(null);
  const [imagePromptLoading, setImagePromptLoading] = useState(false);
  const [remixDraft, setRemixDraft] = useState<RemixDraft | null>(null);
  const [remixSources, setRemixSources] = useState<RemixSource[]>([]);
  const [remixPrompt, setRemixPrompt] = useState("");
  const [referenceSources, setReferenceSources] = useState<ReferenceSource[]>([]);

  useEffect(() => {
    if (!current) {
      setVisualDna(null);
      setImagePromptAnalysis(null);
      setRemixDraft(null);
      setRemixSources([]);
      setRemixPrompt("");
      setReferenceSources([]);
      return;
    }

    let cancelled = false;
    const assetId = current.id;
    const assetName = current.name;
    const assetPrompt = current.prompt;

    setVisualDna(null);
    setImagePromptAnalysis(null);
    setRemixDraft(null);
    setRemixSources([]);
    setRemixPrompt("");

    Promise.all([
      api.visualDna(assetId).catch(() => null),
      api.latestImagePromptAnalysis(assetId).catch(() => null),
      api.latestRemixDraft(assetId).catch(() => null),
      api.referenceSources(assetId).catch(() => []),
    ]).then(([dna, analysis, draft, references]) => {
      if (cancelled) return;
      const base: RemixSource = {
        asset_id: assetId,
        asset_name: assetName,
        fields: [],
        source_url: "",
        visual_dna: dna || EMPTY_VISUAL_DNA,
      };
      setVisualDna(dna);
      setImagePromptAnalysis(analysis);
      setRemixDraft(draft);
      setReferenceSources(references);
      setRemixSources(draft?.sources?.length ? draft.sources : [base]);
      setRemixPrompt(draft?.prompt || assetPrompt);
    });

    return () => {
      cancelled = true;
    };
  }, [current?.id]);

  return {
    visualDna,
    setVisualDna,
    imagePromptAnalysis,
    setImagePromptAnalysis,
    imagePromptLoading,
    setImagePromptLoading,
    remixDraft,
    setRemixDraft,
    remixSources,
    setRemixSources,
    remixPrompt,
    setRemixPrompt,
    referenceSources,
  };
}
