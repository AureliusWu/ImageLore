import { useEffect, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import { api } from "../api";
import type {
  AssetRecord,
  AssetSummary,
  ImagePromptAnalysis,
  VisionSettings,
  VisualDna,
  VisualDnaPatch,
} from "../types";

type Params = {
  current: AssetRecord | null;
  imagePromptAnalysis: ImagePromptAnalysis | null;
  imagePromptLoading: boolean;
  setImagePromptAnalysis: Dispatch<SetStateAction<ImagePromptAnalysis | null>>;
  setImagePromptLoading: Dispatch<SetStateAction<boolean>>;
  setVisualDna: Dispatch<SetStateAction<VisualDna | null>>;
  setCurrent: Dispatch<SetStateAction<AssetRecord | null>>;
  setAssets: Dispatch<SetStateAction<AssetSummary[]>>;
  setTab: (tab: "prompt" | "dna") => void;
  setPrompt: (prompt: string) => void;
  setStatus: Dispatch<SetStateAction<string>>;
};

export function useVisionWorkflow({
  current,
  imagePromptAnalysis,
  imagePromptLoading,
  setImagePromptAnalysis,
  setImagePromptLoading,
  setVisualDna,
  setCurrent,
  setAssets,
  setTab,
  setPrompt,
  setStatus,
}: Params) {
  const [visionSettings, setVisionSettings] = useState<VisionSettings | null>(null);

  useEffect(() => {
    void api
      .visionSettings()
      .then(setVisionSettings)
      .catch(() => setVisionSettings(null));
  }, []);

  const refreshCurrent = async () => {
    if (!current) return null;
    const record = await api.get(current.id);
    setCurrent(record);
    setAssets((items) =>
      items.map((item) =>
        item.id === record.id ? { ...item, updated_at: record.updated_at } : item,
      ),
    );
    return record;
  };

  const saveVisualDna = async (value: VisualDnaPatch) => {
    if (!current) return;
    try {
      const saved = await api.updateVisualDna(current.id, value);
      setVisualDna(saved);
      await refreshCurrent();
      setStatus("Visual DNA 已保存并加入搜索索引");
    } catch (error) {
      setStatus("保存 Visual DNA 失败：" + String(error));
    }
  };

  const analyzeCurrentImage = async () => {
    if (!current || imagePromptLoading) return;
    setImagePromptLoading(true);
    setStatus("正在分析当前图片…");
    try {
      const analysis = await api.analyzeImageToPrompt(current.id);
      setImagePromptAnalysis(analysis);
      setTab("dna");
      setStatus(`Image to Prompt 完成 · ${analysis.model}`);
    } catch (error) {
      setStatus("Image to Prompt 失败：" + String(error));
    } finally {
      setImagePromptLoading(false);
    }
  };

  const applyAnalysisDna = async (overwrite = false) => {
    if (!current || !imagePromptAnalysis) return;
    if (overwrite && !window.confirm("覆盖现有 Visual DNA？已有手工字段会被本次 AI 分析替换。")) {
      return;
    }
    try {
      const saved = await api.applyImagePromptDna(current.id, imagePromptAnalysis.id, overwrite);
      setVisualDna(saved);
      await refreshCurrent();
      setStatus(overwrite ? "AI Visual DNA 已覆盖写入" : "AI Visual DNA 已补充到空字段");
    } catch (error) {
      setStatus("写入 Visual DNA 失败：" + String(error));
    }
  };

  const useAnalysisPrompt = () => {
    if (!imagePromptAnalysis?.prompt) return;
    setPrompt(imagePromptAnalysis.prompt);
    setTab("prompt");
    setStatus("AI Prompt 已载入编辑器，将按现有自动保存规则保存");
  };

  const saveAnalysisRevision = async () => {
    if (!current || !imagePromptAnalysis) return;
    try {
      await api.saveImagePromptRevision(current.id, imagePromptAnalysis.id);
      setStatus("AI Prompt 已保存为独立 Revision");
    } catch (error) {
      setStatus("保存 AI Prompt Revision 失败：" + String(error));
    }
  };

  const saveVisionProvider = async (baseUrl: string, visionModel: string) => {
    try {
      const saved = await api.saveVisionSettings(baseUrl, visionModel);
      setVisionSettings(saved);
      setStatus("图像分析 Provider 配置已保存");
    } catch (error) {
      setStatus("保存图像分析配置失败：" + String(error));
    }
  };

  const setVisionKey = async (key: string) => {
    try {
      await api.setVisionApiKey(key);
      setVisionSettings(await api.visionSettings());
      setStatus(key.trim() ? "API Key 已载入当前会话" : "当前会话 API Key 已清除");
    } catch (error) {
      setStatus("设置 API Key 失败：" + String(error));
    }
  };

  return {
    visionSettings,
    setVisionSettings,
    saveVisualDna,
    analyzeCurrentImage,
    applyAnalysisDna,
    useAnalysisPrompt,
    saveAnalysisRevision,
    saveVisionProvider,
    setVisionKey,
  };
}
