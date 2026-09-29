import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, isTauri } from "./api";
import { APP_VERSION } from "./version";
import type {
  AssetRecord,
  AssetSession,
  AssetSummary,
  BackupRecord,
  DiagnosticStatus,
  DuplicateGroup,
  GenerationSession,
  ImportSummary,
  LibraryFacets,
  LibraryFilter,
  LibraryHealth,
  Lineage,
  ModelAlias,
  RemixDraft,
  SavedFilter,
  SearchMode,
  SemanticStatus,
  SourceFolder,
} from "./types";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { useEditorDraft } from "./hooks/useEditorDraft";
import { useNativeDrop } from "./hooks/useNativeDrop";
import { useWorkspaceLayout } from "./hooks/useWorkspaceLayout";
import { AppHeader } from "./components/AppHeader";
import { LibraryPane } from "./components/LibraryPane";
import { PreviewPane } from "./components/PreviewPane";
import { InspectorPane, type InspectorTab } from "./components/InspectorPane";
import { AppDialogs, type AppModalState } from "./components/AppDialogs";
import { LibraryManager } from "./components/LibraryManager";
import { ParentPicker } from "./components/ParentPicker";
import { useImportJob } from "./hooks/useImportJob";
import { useSemanticJob } from "./hooks/useSemanticJob";
import { useCloseGuard } from "./hooks/useCloseGuard";
import { EMPTY_VISUAL_DNA, useAssetContext } from "./hooks/useAssetContext";
import { useVisionWorkflow } from "./hooks/useVisionWorkflow";
import { nextAssetIndex, saveExtension } from "./previewWorkflow";
import { buildRemixPrompt, nonEmptyDnaFields } from "./remixWorkflow";

const PAGE_SIZE = 240;
const emptyFacets: LibraryFacets = {
  tags: [],
  models: [],
  collections: [],
  metadata_types: [],
  samplers: [],
  schedulers: [],
};
const emptyLineage: Lineage = { parents: [], children: [] };
const parseTags = (text: string) => [
  ...new Set(
    text
      .split(/[,，]/)
      .map((x) => x.trim())
      .filter(Boolean),
  ),
];
const isTextEntry = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.matches("input,textarea,select") || target.isContentEditable);
export default function App() {
  const [version, setVersion] = useState(APP_VERSION);
  const [status, setStatus] = useState("就绪");
  const [filter, setFilter] = useState<LibraryFilter>({
    query: "",
    view: "all",
    tag: null,
    model: null,
    collection_id: null,
    sort: "smart",
  });
  const [searchMode, setSearchMode] = useState<SearchMode>("keyword");
  const [similarSource, setSimilarSource] = useState<{ id: number; name: string } | null>(null);
  const [semanticScores, setSemanticScores] = useState<Map<number, number>>(new Map());
  const [semanticStatus, setSemanticStatus] = useState<SemanticStatus | null>(null);
  const keywordQuery = useDebouncedValue(filter.query, 180);
  const semanticQuery = useDebouncedValue(filter.query, 550);
  const debouncedQuery = searchMode === "semantic" ? semanticQuery : keywordQuery;
  const effectiveFilter = useMemo(
    () => ({ ...filter, query: debouncedQuery }),
    [
      debouncedQuery,
      filter.view,
      filter.tag,
      filter.model,
      filter.collection_id,
      filter.metadata_type,
      filter.sampler,
      filter.scheduler,
      filter.seed,
      filter.steps_min,
      filter.steps_max,
      filter.cfg_min,
      filter.cfg_max,
      filter.denoise_min,
      filter.denoise_max,
      filter.orientation,
      filter.sort,
    ],
  );

  const [assets, setAssets] = useState<AssetSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [facets, setFacets] = useState<LibraryFacets>(emptyFacets);
  const [current, setCurrent] = useState<AssetRecord | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [preview, setPreview] = useState("");
  const [tab, setTab] = useState<InspectorTab>("prompt");
  const [lineage, setLineage] = useState<Lineage>(emptyLineage);
  const [compareRecord, setCompareRecord] = useState<AssetRecord | null>(null);
  const [compareParentSrc, setCompareParentSrc] = useState("");
  const [compareCurrentSrc, setCompareCurrentSrc] = useState("");
  const [sessions, setSessions] = useState<GenerationSession[]>([]);
  const [assetSession, setAssetSession] = useState<AssetSession | null>(null);
  const {
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
  } = useAssetContext(current);
  const [modelAliases, setModelAliases] = useState<ModelAlias[]>([]);
  const [savedFilters, setSavedFilters] = useState<SavedFilter[]>([]);
  const [sourceFolders, setSourceFolders] = useState<SourceFolder[]>([]);
  const [health, setHealth] = useState<LibraryHealth | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticStatus | null>(null);
  const [managerOpen, setManagerOpen] = useState(false);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
  const [parentOpen, setParentOpen] = useState(false);
  const [parentMode, setParentMode] = useState<"lineage" | "remix">("lineage");
  const [parentQuery, setParentQuery] = useState("");
  const [parentChoice, setParentChoice] = useState<number | null>(null);
  const [parentResults, setParentResults] = useState<AssetSummary[]>([]);
  const [parentLoading, setParentLoading] = useState(false);
  const debouncedParentQuery = useDebouncedValue(parentQuery, 180);

  const [modal, setModal] = useState<AppModalState>(null);
  const [dialogText, setDialogText] = useState("");
  const [dialogChoice, setDialogChoice] = useState("");
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const refreshSeq = useRef(0);
  const selectSeq = useRef(0);
  const autoSyncStarted = useRef(false);
  const inboxFocusSyncAt = useRef(0);
  const previewAssetId = useRef<number | null>(null);
  const { previewMode, setPreviewMode, leftWidth, rightWidth, drag } = useWorkspaceLayout();

  const refreshFacets = useCallback(
    () =>
      api
        .facets()
        .then(setFacets)
        .catch(() => setFacets(emptyFacets)),
    [],
  );
  const refreshSessions = useCallback(
    () =>
      api
        .sessions()
        .then(setSessions)
        .catch(() => setSessions([])),
    [],
  );
  const refreshSavedFilters = useCallback(
    () =>
      api
        .savedFilters()
        .then(setSavedFilters)
        .catch(() => setSavedFilters([])),
    [],
  );
  const refreshSources = useCallback(
    () =>
      api
        .sourceFolders()
        .then(setSourceFolders)
        .catch(() => setSourceFolders([])),
    [],
  );
  const refreshSemanticStatus = useCallback(
    () =>
      api
        .semanticStatus()
        .then(setSemanticStatus)
        .catch(() => setSemanticStatus(null)),
    [],
  );
  const onEditorSaved = useCallback(
    (a: AssetRecord) => setCurrent((prev) => (prev?.id === a.id ? a : prev)),
    [],
  );
  const editor = useEditorDraft(current, onEditorSaved, refreshFacets, setStatus);
  const {
    prompt,
    negative,
    model,
    tagsText,
    setPrompt,
    setNegative,
    setModel,
    setTagsText,
    flush: flushEditor,
    saveRevision: saveEditorRevision,
    load: loadEditor,
  } = editor;

  const {
    visionSettings,
    setVisionSettings,
    saveVisualDna,
    analyzeCurrentImage,
    applyAnalysisDna,
    useAnalysisPrompt,
    saveAnalysisRevision,
    saveVisionProvider,
    setVisionKey,
  } = useVisionWorkflow({
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
  });

  const selectRecord = useCallback(
    async (id: number, selection?: Set<number>) => {
      const seq = ++selectSeq.current;
      ++refreshSeq.current;
      setLoading(false);
      await flushEditor();
      try {
        const record = await api.get(id);
        if (seq !== selectSeq.current) return;
        setCurrent(record);
        setSelected(selection ?? new Set([id]));
      } catch (e) {
        if (seq === selectSeq.current) setStatus("记录加载失败：" + String(e));
      }
    },
    [flushEditor],
  );

  const refresh = useCallback(
    async (preferId?: number) => {
      const seq = ++refreshSeq.current;
      await flushEditor();
      if (seq !== refreshSeq.current) return;
      setLoading(true);
      try {
        let items: AssetSummary[] = [];
        let nextTotal = 0;
        if (searchMode === "semantic" && (similarSource || effectiveFilter.query.trim())) {
          const semanticFilter = { ...effectiveFilter, query: "" };
          const hits = similarSource
            ? await api.semanticSearchSimilar(similarSource.id, semanticFilter, PAGE_SIZE)
            : await api.semanticSearchText(effectiveFilter.query, semanticFilter, PAGE_SIZE);
          if (seq !== refreshSeq.current) return;
          items = hits.map((x) => x.asset);
          nextTotal = hits.length;
          setSemanticScores(new Map(hits.map((x) => [x.asset.id, x.score])));
        } else {
          const page = await api.page(effectiveFilter, 0, PAGE_SIZE);
          if (seq !== refreshSeq.current) return;
          items = page.items;
          nextTotal = page.total;
          setSemanticScores(new Map());
        }
        const preferredVisible =
          preferId && items.some((x) => x.id === preferId) ? preferId : undefined;
        const nextId =
          preferredVisible ??
          (current?.id && items.some((x) => x.id === current.id) ? current.id : items[0]?.id);
        const next = nextId ? await api.get(nextId).catch(() => null) : null;
        if (seq !== refreshSeq.current) return;
        setAssets(items);
        setTotal(nextTotal);
        setCurrent(next);
        loadEditor(next);
        setSelected(next ? new Set([next.id]) : new Set());
        if (searchMode === "semantic" && (similarSource || effectiveFilter.query.trim()))
          setStatus("语义召回完成");
      } catch (e) {
        if (seq === refreshSeq.current) {
          setSemanticScores(new Map());
          setStatus(
            searchMode === "semantic" ? "语义搜索失败：" + String(e) : "图库加载失败：" + String(e),
          );
        }
      } finally {
        if (seq === refreshSeq.current) setLoading(false);
      }
    },
    [effectiveFilter, searchMode, similarSource, current?.id, flushEditor, loadEditor],
  );

  const loadMore = useCallback(async () => {
    if (searchMode === "semantic" && (similarSource || effectiveFilter.query.trim())) return;
    if (loading || assets.length >= total) return;
    const seq = refreshSeq.current;
    setLoading(true);
    try {
      const page = await api.page(effectiveFilter, assets.length, PAGE_SIZE);
      if (seq !== refreshSeq.current) return;
      setAssets((prev) => {
        const ids = new Set(prev.map((x) => x.id));
        return [...prev, ...page.items.filter((x) => !ids.has(x.id))];
      });
      setTotal(page.total);
    } catch (e) {
      if (seq === refreshSeq.current) setStatus(String(e));
    } finally {
      if (seq === refreshSeq.current) setLoading(false);
    }
  }, [loading, assets, effectiveFilter, total, searchMode, similarSource]);

  useEffect(() => {
    if (isTauri)
      getVersion()
        .then(setVersion)
        .catch(() => setVersion(APP_VERSION));
  }, []);
  useEffect(() => {
    void refresh();
  }, [effectiveFilter, searchMode, similarSource?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    void refreshFacets();
    void refreshSemanticStatus();
  }, [refreshFacets, refreshSemanticStatus]);
  useEffect(() => {
    void refreshSessions();
    void refreshSavedFilters();
  }, [refreshSessions, refreshSavedFilters]);
  useEffect(() => {
    if (isTauri) void api.ensureAutoBackup().catch((e) => setStatus("自动备份失败：" + String(e)));
  }, []);
  useEffect(() => {
    if (!parentOpen) return;
    let cancelled = false;
    setParentLoading(true);
    api
      .page(
        { query: debouncedParentQuery, view: "all", tag: null, model: null, collection_id: null },
        0,
        100,
      )
      .then((page) => {
        if (!cancelled)
          setParentResults(
            page.items.filter(
              (x) =>
                x.id !== current?.id &&
                !(parentMode === "remix" && remixSources.some((s) => s.asset_id === x.id)),
            ),
          );
      })
      .catch(() => {
        if (!cancelled) setParentResults([]);
      })
      .finally(() => {
        if (!cancelled) setParentLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [parentOpen, debouncedParentQuery, current?.id, parentMode, remixSources]);
  useEffect(() => {
    if (!current) {
      previewAssetId.current = null;
      setPreview("");
      return;
    }
    let cancelled = false;
    const assetId = current.id;
    const changedAsset = previewAssetId.current !== assetId;
    previewAssetId.current = assetId;
    if (changedAsset) setPreview("");
    setStatus("正在加载预览…");
    api
      .preview(assetId, previewMode === "fit" ? 2200 : 0, false)
      .then((src) => {
        if (!cancelled) {
          setPreview(src);
          setStatus("就绪");
        }
      })
      .catch((e) => {
        if (!cancelled) {
          if (changedAsset) setPreview("");
          setStatus(`预览失败：${String(e)}`);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [current?.id, previewMode]);

  useEffect(() => {
    if (!current) {
      setLineage(emptyLineage);
      setCompareRecord(null);
      setAssetSession(null);
      return;
    }
    let cancelled = false;
    const assetId = current.id;
    setLineage(emptyLineage);
    setCompareRecord(null);
    void Promise.all([api.lineage(assetId), api.assetSession(assetId).catch(() => null)])
      .then(async ([x, session]) => {
        const p = x.parents[0];
        const parent = p ? await api.get(p.other_id).catch(() => null) : null;
        if (!cancelled) {
          setLineage(x);
          setCompareRecord(parent);
          setAssetSession(session);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLineage(emptyLineage);
          setAssetSession(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [current?.id]);

  useEffect(() => {
    if (!current || !compareRecord) {
      setCompareParentSrc("");
      setCompareCurrentSrc("");
      return;
    }
    let cancelled = false;
    setCompareParentSrc("");
    setCompareCurrentSrc("");
    Promise.all([
      api.preview(compareRecord.id, 1200, false).catch(() => ""),
      api.preview(current.id, 1200, false).catch(() => ""),
    ]).then(([parentSrc, currentSrc]) => {
      if (!cancelled) {
        setCompareParentSrc(parentSrc);
        setCompareCurrentSrc(currentSrc);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [current?.id, compareRecord?.id]);

  const semanticJob = useSemanticJob(async () => {
    await refreshSemanticStatus();
    if (searchMode === "semantic") await refresh(current?.id);
  }, setStatus);
  const {
    start: startSemanticIndex,
    cancel: cancelSemanticIndex,
    active: semanticActive,
    progress: semanticProgress,
  } = semanticJob;

  const importDone = useCallback(
    async (result: ImportSummary) => {
      await Promise.all([refreshFacets(), refreshSources()]);
      await refresh(result.last_id ?? undefined);
      const semantic = await api.semanticStatus().catch(() => null);
      if (semantic) {
        setSemanticStatus(semantic);
        if (semantic.enabled && semantic.stale > 0 && !semanticActive) void startSemanticIndex();
      }
    },
    [refreshFacets, refreshSources, refresh, semanticActive, startSemanticIndex],
  );
  const importJob = useImportJob(importDone, setStatus);
  const {
    start: startImportJob,
    cancel: cancelImportJob,
    active: importActive,
    progress: importProgress,
  } = importJob;
  useEffect(() => {
    if (!isTauri || autoSyncStarted.current) return;
    autoSyncStarted.current = true;
    void api
      .sourceFolders()
      .then((list) => {
        setSourceFolders(list);
        const ids = list.filter((x) => x.auto_sync).map((x) => x.id);
        if (ids.length) void startImportJob("正在同步资料源目录…", () => api.startSyncSources(ids));
      })
      .catch((e) => setStatus("来源目录读取失败：" + String(e)));
  }, [startImportJob]);
  useEffect(() => {
    if (!isTauri) return;
    const focus = () => {
      const inbox = sourceFolders.find((x) => x.name === "ImageLore Inbox");
      if (!inbox || importActive) return;
      const now = Date.now();
      if (now - inboxFocusSyncAt.current < 3000) return;
      inboxFocusSyncAt.current = now;
      void startImportJob("正在同步浏览器 Inbox…", () => api.startSyncSources([inbox.id]));
    };
    window.addEventListener("focus", focus);
    return () => window.removeEventListener("focus", focus);
  }, [sourceFolders, importActive, startImportJob]);
  const cancelBackground = useCallback(async () => {
    if (importActive) await cancelImportJob();
    if (semanticActive) await cancelSemanticIndex();
  }, [importActive, semanticActive, cancelImportJob, cancelSemanticIndex]);
  useCloseGuard(flushEditor, importActive || semanticActive ? cancelBackground : undefined);
  const importImmediate = useCallback(
    async (label: string, task: () => Promise<ImportSummary>) => {
      await flushEditor();
      setStatus(label);
      try {
        const result = await task();
        setStatus(
          "导入完成：新增 " +
            result.added +
            "，重复 " +
            result.duplicates +
            "，跳过 " +
            result.skipped +
            "，失败 " +
            result.failed,
        );
        await importDone(result);
      } catch (e) {
        setStatus("导入失败：" + String(e));
      }
    },
    [flushEditor, importDone],
  );
  const startBackgroundImport = useCallback(
    async (
      label: string,
      starter: () => Promise<number>,
      fallback: () => Promise<ImportSummary>,
    ) => {
      await flushEditor();
      if (isTauri) {
        await startImportJob(label, starter);
      } else {
        await importImmediate(label, fallback);
      }
    },
    [flushEditor, startImportJob, importImmediate],
  );

  const chooseImages = async () => {
    if (!isTauri) {
      setStatus("文件选择器仅在桌面版中可用");
      return;
    }
    const picked = await open({
      multiple: true,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    await startBackgroundImport(
      "正在准备导入…",
      () => api.startImportPaths(paths),
      () => api.importPaths(paths),
    );
  };
  const chooseFolder = async () => {
    if (!isTauri) {
      setStatus("文件夹选择器仅在桌面版中可用");
      return;
    }
    const picked = await open({ directory: true, multiple: false });
    if (!picked || Array.isArray(picked)) return;
    await startBackgroundImport(
      "正在扫描文件夹…",
      () => api.startImportFolder(picked),
      () => api.importFolder(picked),
    );
  };
  const handleDrop = useCallback(
    (paths: string[]) =>
      startBackgroundImport(
        "正在扫描拖入内容…",
        () => api.startImportDroppedPaths(paths),
        () => api.importDroppedPaths(paths),
      ),
    [startBackgroundImport],
  );
  const drop = useNativeDrop(handleDrop, setStatus);

  const importDerivative = async () => {
    if (!current) return;
    if (!isTauri) {
      setStatus("此功能仅在桌面版中可用");
      return;
    }
    const picked = await open({
      multiple: false,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    await flushEditor();
    const result = await api.importPaths([picked]);
    if (result.duplicates) {
      setStatus("该派生图与资料库中的现有图片内容完全相同，未建立重复谱系");
      return;
    }
    if (result.last_id === current.id) {
      setStatus("所选图片与当前记录内容完全相同，未建立自引用关系");
      return;
    }
    if (result.last_id) {
      await api.addRelation(current.id, result.last_id, "derived_from", "");
      if (assetSession) await api.setAssetSession(result.last_id, assetSession.session_id, "");
      await refreshSessions();
      await refresh(result.last_id);
      setTab("lineage");
      setStatus("派生图已关联");
      return;
    }
  };

  const onAsset = async (asset: AssetSummary, e: React.MouseEvent) => {
    let next = new Set([asset.id]);
    if (e.ctrlKey || e.metaKey) {
      next = new Set(selected);
      next.has(asset.id) ? next.delete(asset.id) : next.add(asset.id);
    }
    await selectRecord(asset.id, next);
  };
  const openAsset = async (asset: AssetSummary) => {
    if (asset.missing) {
      setStatus("文件缺失，无法打开图片");
      return;
    }
    try {
      await api.openExternal(asset.id);
      setStatus(`已打开：${asset.name}`);
    } catch (e) {
      setStatus("打开图片失败：" + String(e));
    }
  };
  const openAssetFolder = async (asset: AssetSummary) => {
    if (asset.missing) {
      setStatus("文件缺失，无法打开所在位置");
      return;
    }
    try {
      await api.openFolder(asset.id);
      setStatus(`已定位：${asset.name}`);
    } catch (e) {
      setStatus("打开文件所在位置失败：" + String(e));
    }
  };
  const copyAssetPath = async (asset: AssetSummary) => {
    try {
      await navigator.clipboard.writeText(asset.path);
      setStatus("文件路径已复制");
    } catch (e) {
      setStatus("复制文件路径失败：" + String(e));
    }
  };
  const copyAssetImage = async (asset: AssetSummary) => {
    if (asset.missing) {
      setStatus("文件缺失，无法复制图像");
      return;
    }
    try {
      const source = await api.preview(asset.id, 0, false);
      const response = await fetch(source);
      if (!response.ok && !source.startsWith("data:"))
        throw new Error(`读取图像失败：${response.status}`);
      const blob = await response.blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("无法创建图像画布");
      context.drawImage(bitmap, 0, 0);
      bitmap.close();
      const png = await new Promise<Blob>((resolve, reject) =>
        canvas.toBlob(
          (value) => (value ? resolve(value) : reject(new Error("图像转换失败"))),
          "image/png",
        ),
      );
      await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
      setStatus("图像已复制到剪贴板");
    } catch (e) {
      setStatus("复制图像失败：" + String(e));
    }
  };
  const saveAssetAs = async (asset: AssetSummary) => {
    if (asset.missing) {
      setStatus("文件缺失，无法另存为");
      return;
    }
    if (!isTauri) {
      setStatus("另存为仅在桌面版中可用");
      return;
    }
    const extension = saveExtension(asset.name, asset.format || "");
    const destination = await save({
      defaultPath: asset.name,
      filters: [{ name: "图片", extensions: [extension] }],
    });
    if (!destination) return;
    try {
      await api.copyAssetTo(asset.id, destination);
      setStatus("图片副本已保存：" + destination);
    } catch (e) {
      setStatus("另存为失败：" + String(e));
    }
  };
  const toggleAssetFavorite = async (asset: AssetSummary) => {
    try {
      const a = await api.toggleFavorite(asset.id);
      setCurrent((prev) => (prev?.id === a.id ? a : prev));
      setAssets((xs) =>
        xs.map((x) =>
          x.id === a.id ? { ...x, favorite: a.favorite, updated_at: a.updated_at } : x,
        ),
      );
      setStatus(a.favorite ? "已收藏" : "已取消收藏");
    } catch (e) {
      setStatus("更新收藏失败：" + String(e));
    }
  };
  const toggleFavorite = async () => {
    if (current) await toggleAssetFavorite(current);
  };
  const copyPrompt = () => {
    if (current) void navigator.clipboard.writeText(prompt).then(() => setStatus("提示词已复制"));
  };
  const saveRevision = async () => {
    if (!current) return;
    await saveEditorRevision("");
  };
  const openHistory = async () => {
    if (current) {
      setDialogChoice("");
      setModal({ kind: "history", revisions: await api.revisions(current.id) });
    }
  };
  const openCollection = async () => {
    setDialogChoice("");
    setDialogText("");
    setModal({ kind: "collection", collections: await api.collections() });
  };
  const rescan = async () => {
    if (!current) return;
    await flushEditor();
    const a = await api.rescan(current.id);
    setCurrent(a);
    loadEditor(a);
    setStatus("元数据已刷新");
  };
  const exportSidecar = async () => {
    if (current) setStatus(`Sidecar 已导出：${await api.exportSidecar(current.id)}`);
  };
  const refreshMissing = async () => {
    setStatus("正在检查文件位置…");
    const n = await api.refreshMissing();
    setStatus(`发现 ${n} 个缺失文件`);
    await refresh();
  };
  const repairMissing = async () => {
    if (!isTauri) {
      setStatus("此功能仅在桌面版中可用");
      return;
    }
    const root = await open({ directory: true, multiple: false });
    if (!root || Array.isArray(root)) return;
    const n = await api.relocateMissing(root);
    setStatus(`已重新定位 ${n} 条记录`);
    await refresh();
  };
  const compare = async (id: number) => setCompareRecord(await api.get(id).catch(() => null));
  const refreshManager = useCallback(async () => {
    const [b, d, a, s, h, sources, semantic, diag, vision] = await Promise.all([
      api.backups(),
      api.duplicateGroups(),
      api.modelAliases(),
      api.savedFilters(),
      api.libraryHealth(),
      api.sourceFolders(),
      api.semanticStatus(),
      api.diagnosticsStatus(),
      api.visionSettings(),
    ]);
    setBackups(b);
    setDuplicates(d);
    setModelAliases(a);
    setSavedFilters(s);
    setHealth(h);
    setSourceFolders(sources);
    setSemanticStatus(semantic);
    setDiagnostics(diag);
    setVisionSettings(vision);
    await refreshFacets();
  }, [refreshFacets]);
  const openManager = async () => {
    try {
      await flushEditor();
      await refreshManager();
      setManagerOpen(true);
    } catch (e) {
      setStatus("打开资料库管理失败：" + String(e));
    }
  };
  const createBackup = async () => {
    try {
      setStatus("正在备份资料库…");
      await api.createBackup();
      await refreshManager();
      setStatus("资料库备份完成");
    } catch (e) {
      setStatus("资料库备份失败：" + String(e));
    }
  };
  const openDataFolder = async () => {
    try {
      await api.openDataFolder();
      setStatus("已打开 ImageLore 数据目录");
    } catch (e) {
      setStatus("打开数据目录失败：" + String(e));
    }
  };
  const openLogsFolder = async () => {
    try {
      await api.openLogsFolder();
      setStatus("已打开 ImageLore 日志目录");
    } catch (e) {
      setStatus("打开日志目录失败：" + String(e));
    }
  };
  const restoreBackup = async (name: string) => {
    if (!window.confirm("确定恢复到这份备份吗？当前资料库会在重启时先自动保留一份安全副本。"))
      return;
    try {
      await api.stageRestore(name);
      setStatus("恢复已准备完成，请重启 ImageLore 后生效");
    } catch (e) {
      setStatus("准备恢复失败：" + String(e));
    }
  };
  const renameTag = async (oldName: string, newName: string) => {
    try {
      await api.renameTag(oldName, newName);
      setFilter((f) =>
        f.tag?.toLocaleLowerCase() === oldName.toLocaleLowerCase() ? { ...f, tag: newName } : f,
      );
      await refreshManager();
      await refresh(current?.id);
    } catch (e) {
      setStatus("标签修改失败：" + String(e));
    }
  };
  const deleteTag = async (name: string) => {
    if (!window.confirm("删除标签“" + name + "”？图片记录本身不会被删除。")) return;
    try {
      await api.deleteTag(name);
      setFilter((f) =>
        f.tag?.toLocaleLowerCase() === name.toLocaleLowerCase() ? { ...f, tag: null } : f,
      );
      await refreshManager();
      await refresh(current?.id);
    } catch (e) {
      setStatus("删除标签失败：" + String(e));
    }
  };
  const renameCollection = async (id: number, name: string) => {
    try {
      await api.renameCollection(id, name);
      await refreshManager();
    } catch (e) {
      setStatus("集合重命名失败：" + String(e));
    }
  };
  const deleteCollection = async (id: number) => {
    if (!window.confirm("删除这个集合？集合内的图片记录不会被删除。")) return;
    try {
      await api.deleteCollection(id);
      await refreshManager();
      setFilter((f) => (f.collection_id === id ? { ...f, collection_id: null } : f));
    } catch (e) {
      setStatus("删除集合失败：" + String(e));
    }
  };
  const upsertModelAlias = async (alias: string, canonical: string) => {
    try {
      await api.upsertModelAlias(alias, canonical);
      setFilter((f) =>
        f.model?.toLocaleLowerCase() === alias.toLocaleLowerCase() ? { ...f, model: canonical } : f,
      );
      await refreshManager();
      await refreshFacets();
      await refresh(current?.id);
    } catch (e) {
      setStatus("模型别名保存失败：" + String(e));
    }
  };
  const deleteModelAlias = async (alias: string) => {
    try {
      await api.deleteModelAlias(alias);
      await refreshManager();
      await refreshFacets();
      await refresh(current?.id);
    } catch (e) {
      setStatus("模型别名删除失败：" + String(e));
    }
  };
  const saveCurrentView = async () => {
    const name = window.prompt("保存当前筛选为", "");
    if (!name?.trim()) return;
    try {
      await api.saveFilter(name.trim(), filter);
      await refreshSavedFilters();
      setStatus("筛选视图已保存");
    } catch (e) {
      setStatus("保存筛选失败：" + String(e));
    }
  };
  const applySavedView = (view: SavedFilter) => {
    setFilter({ ...view.filter });
    setStatus("已应用保存视图：" + view.name);
  };
  const deleteSavedView = async (id: number) => {
    try {
      await api.deleteSavedFilter(id);
      await refreshSavedFilters();
      await refreshManager();
    } catch (e) {
      setStatus("删除保存视图失败：" + String(e));
    }
  };
  const ensureReferenceInbox = async () => {
    if (!isTauri) {
      setStatus("浏览器 Inbox 仅在桌面版中可用");
      return;
    }
    try {
      const inbox = await api.ensureReferenceInbox();
      await refreshSources();
      setStatus("浏览器 Inbox 已准备：" + inbox.path);
      if (!importActive)
        await startImportJob("正在同步浏览器 Inbox…", () => api.startSyncSources([inbox.id]));
    } catch (e) {
      setStatus("准备浏览器 Inbox 失败：" + String(e));
    }
  };
  const openReferenceUrl = async (url: string) => {
    try {
      await api.openReferenceUrl(url);
    } catch (e) {
      setStatus("打开来源链接失败：" + String(e));
    }
  };
  const addSourceFolder = async () => {
    if (!isTauri) {
      setStatus("来源目录仅在桌面版中可用");
      return;
    }
    const picked = await open({ directory: true, multiple: false });
    if (!picked || Array.isArray(picked)) return;
    try {
      const item = await api.addSourceFolder(picked);
      await refreshSources();
      setStatus("已添加来源目录：" + item.name);
    } catch (e) {
      setStatus("添加来源目录失败：" + String(e));
    }
  };
  const removeSourceFolder = async (id: number) => {
    const item = sourceFolders.find((x) => x.id === id);
    if (!window.confirm("移除来源目录“" + (item?.name || "") + "”？不会删除其中任何图片。")) return;
    try {
      await api.removeSourceFolder(id);
      await refreshSources();
      setStatus("来源目录已移除");
    } catch (e) {
      setStatus("移除来源目录失败：" + String(e));
    }
  };
  const toggleSourceAutoSync = async (id: number, enabled: boolean) => {
    try {
      await api.setSourceAutoSync(id, enabled);
      await refreshSources();
      setStatus(enabled ? "已开启启动同步" : "已关闭启动同步");
    } catch (e) {
      setStatus("更新来源目录失败：" + String(e));
    }
  };
  const syncSourceFolders = async (ids: number[]) => {
    if (!isTauri) {
      setStatus("来源目录同步仅在桌面版中可用");
      return;
    }
    if (!ids.length) return;
    await flushEditor();
    await startImportJob(ids.length === 1 ? "正在同步来源目录…" : "正在同步全部来源目录…", () =>
      api.startSyncSources(ids),
    );
  };
  const changeSearchMode = (mode: SearchMode) => {
    setSearchMode(mode);
    if (mode === "keyword") {
      setSimilarSource(null);
      setSemanticScores(new Map());
    }
  };
  const changeSearchQuery = (query: string) => {
    if (similarSource) setSimilarSource(null);
    setFilter((f) => ({ ...f, query }));
  };
  const findSimilarAsset = (asset: AssetSummary) => {
    setFilter((f) => ({ ...f, query: "" }));
    setSearchMode("semantic");
    setSimilarSource({ id: asset.id, name: asset.name });
    setStatus("正在查找视觉相似图片…");
  };
  const findSimilar = () => {
    if (current) findSimilarAsset(current);
  };
  const clearSimilar = () => {
    setSimilarSource(null);
    setSemanticScores(new Map());
  };
  const rebuildSemantic = async () => {
    await startSemanticIndex();
  };
  const clearSemantic = async () => {
    if (!window.confirm("清空语义索引？原图和资料库记录不会受到影响。")) return;
    if (semanticActive) await cancelSemanticIndex();
    await api.clearSemanticIndex();
    setSimilarSource(null);
    setSemanticScores(new Map());
    await refreshSemanticStatus();
    await refresh();
    setStatus("语义索引已清空");
  };
  const deleteSemanticModels = async () => {
    if (!window.confirm("删除本地语义模型缓存？下次使用语义功能时会重新下载。")) return;
    if (semanticActive) await cancelSemanticIndex();
    await api.deleteSemanticModels();
    await refreshSemanticStatus();
    setStatus("本地语义模型缓存已删除");
  };

  const setSession = async (sessionId: number | null) => {
    if (!current) return;
    try {
      await api.setAssetSession(current.id, sessionId, assetSession?.asset_note || "");
      setAssetSession(await api.assetSession(current.id));
      await refreshSessions();
      setStatus(sessionId ? "已加入生成会话" : "已移出生成会话");
    } catch (e) {
      setStatus("生成会话更新失败：" + String(e));
    }
  };
  const selectedIds = () => (selected.size ? [...selected] : current ? [current.id] : []);
  const batchFavorite = async (favorite: boolean) => {
    const ids = selectedIds();
    if (!ids.length) return;
    try {
      for (const id of ids) {
        const item = assets.find((x) => x.id === id);
        const value = item?.favorite ?? (current?.id === id ? current.favorite : 0);
        if (Boolean(value) !== favorite) await api.toggleFavorite(id);
      }
      await refresh(current?.id);
      setStatus(favorite ? `已收藏 ${ids.length} 项` : `已取消收藏 ${ids.length} 项`);
    } catch (e) {
      setStatus("批量收藏失败：" + String(e));
    }
  };
  const batchRescan = async () => {
    const ids = selectedIds();
    if (!ids.length) return;
    try {
      setStatus(`正在重读 ${ids.length} 项元数据…`);
      for (const id of ids) await api.rescan(id);
      await refreshFacets();
      await refresh(current?.id);
      setStatus(`已重读 ${ids.length} 项元数据`);
    } catch (e) {
      setStatus("批量重读失败：" + String(e));
    }
  };
  const batchSession = async (sessionId: number | null) => {
    const ids = selectedIds();
    if (!ids.length) return;
    try {
      for (const id of ids) await api.setAssetSession(id, sessionId, "");
      await refreshSessions();
      if (current && ids.includes(current.id)) setAssetSession(await api.assetSession(current.id));
      setStatus(
        sessionId
          ? `已将 ${ids.length} 项加入 Generation Session`
          : `已将 ${ids.length} 项移出 Generation Session`,
      );
    } catch (e) {
      setStatus("批量分配 Session 失败：" + String(e));
    }
  };
  const createSession = async () => {
    if (!current) return;
    const name = window.prompt("新建 Generation Session", "");
    if (!name?.trim()) return;
    const note = window.prompt("会话说明（可留空）", "") || "";
    try {
      const session = await api.createSession(name.trim(), note);
      await api.setAssetSession(current.id, session.id, "");
      await refreshSessions();
      setAssetSession(await api.assetSession(current.id));
      setStatus("Generation Session 已创建");
    } catch (e) {
      setStatus("新建会话失败：" + String(e));
    }
  };
  const editSessionNote = async () => {
    if (!current || !assetSession) return;
    const note = window.prompt("当前图片在此会话中的备注", assetSession.asset_note || "");
    if (note === null) return;
    try {
      await api.setAssetSession(current.id, assetSession.session_id, note);
      setAssetSession(await api.assetSession(current.id));
      setStatus("会话备注已保存");
    } catch (e) {
      setStatus("会话备注保存失败：" + String(e));
    }
  };
  const editRelationNote = async (relationId: number, currentNote: string) => {
    const note = window.prompt("Branch Note", currentNote);
    if (note === null || !current) return;
    try {
      await api.updateRelationNote(relationId, note);
      setLineage(await api.lineage(current.id));
      setStatus("分支备注已保存");
    } catch (e) {
      setStatus("分支备注保存失败：" + String(e));
    }
  };
  const openParentPicker = () => {
    setParentMode("lineage");
    setParentQuery("");
    setParentChoice(null);
    setParentOpen(true);
  };
  const openRemixSourcePicker = () => {
    setParentMode("remix");
    setParentQuery("");
    setParentChoice(null);
    setParentOpen(true);
  };
  const confirmParent = async () => {
    if (!current || !parentChoice) return;
    try {
      if (parentMode === "remix") {
        const [record, dna] = await Promise.all([
          api.get(parentChoice),
          api.visualDna(parentChoice).catch(() => EMPTY_VISUAL_DNA),
        ]);
        setRemixSources((xs) => [
          ...xs,
          {
            asset_id: record.id,
            asset_name: record.name,
            fields: nonEmptyDnaFields(dna),
            source_url: "",
            visual_dna: dna,
          },
        ]);
        setParentOpen(false);
        setStatus("已加入 Remix 参考图：" + record.name);
        return;
      }
      await api.addRelation(parentChoice, current.id, "reference", "");
      setParentOpen(false);
      setLineage(await api.lineage(current.id));
      setCompareRecord(await api.get(parentChoice));
      setStatus("父图已关联");
    } catch (e) {
      setStatus(
        parentMode === "remix"
          ? "添加 Remix 参考图失败：" + String(e)
          : "关联父图失败：" + String(e),
      );
    }
  };
  const toggleRemixField = (assetId: number, field: string) =>
    setRemixSources((xs) =>
      xs.map((source) =>
        source.asset_id !== assetId
          ? source
          : {
              ...source,
              fields: source.fields.includes(field)
                ? source.fields.filter((x) => x !== field)
                : [...source.fields, field],
            },
      ),
    );
  const removeRemixSource = (assetId: number) => {
    if (assetId === current?.id) return;
    setRemixSources((xs) => xs.filter((x) => x.asset_id !== assetId));
  };
  const composeRemix = () => {
    if (!current) return;
    setRemixPrompt(buildRemixPrompt(prompt, remixSources, current.id));
    setStatus("Remix Prompt 已按所选 DNA 重新组合");
  };
  const persistRemix = async () => {
    if (!current) throw new Error("没有当前图片");
    const saved = await api.saveRemixDraft(
      remixDraft?.id ?? null,
      current.id,
      remixPrompt,
      remixSources.map((x) => ({
        asset_id: x.asset_id,
        fields: x.fields,
        source_url: x.source_url,
      })),
    );
    setRemixDraft(saved);
    setRemixSources(saved.sources);
    return saved;
  };
  const saveRemix = async () => {
    try {
      await persistRemix();
      setStatus("Remix 草稿已保存");
    } catch (e) {
      setStatus("保存 Remix 草稿失败：" + String(e));
    }
  };
  const copyRemix = () => {
    if (remixPrompt.trim())
      void navigator.clipboard.writeText(remixPrompt).then(() => setStatus("Remix Prompt 已复制"));
  };
  const resetRemix = async () => {
    if (!current) return;
    try {
      if (remixDraft) await api.deleteRemixDraft(remixDraft.id);
    } catch {}
    const dna = visualDna || EMPTY_VISUAL_DNA;
    setRemixDraft(null);
    setRemixSources([
      {
        asset_id: current.id,
        asset_name: current.name,
        fields: [],
        source_url: "",
        visual_dna: dna,
      },
    ]);
    setRemixPrompt(prompt);
    setStatus("Remix 草稿已重置");
  };
  const importRemixResult = async () => {
    if (!current) {
      return;
    }
    if (!isTauri) {
      setStatus("导入 Remix 结果仅在桌面版中可用");
      return;
    }
    let saved: RemixDraft;
    try {
      saved = await persistRemix();
    } catch (e) {
      setStatus("请先保存有效 Remix 草稿：" + String(e));
      return;
    }
    const picked = await open({
      multiple: false,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    try {
      await flushEditor();
      const result = await api.importPaths([picked]);
      if (result.duplicates) {
        setStatus("所选 Remix 结果与资料库现有图片完全相同，未建立重复谱系");
        return;
      }
      if (!result.last_id) {
        setStatus("Remix 结果没有导入成功");
        return;
      }
      await api.applyRemixLineage(saved.id, result.last_id);
      await refreshSessions();
      await refresh(result.last_id);
      setTab("lineage");
      setStatus("Remix 结果已导入，并记录全部参考来源与 DNA 字段");
    } catch (e) {
      setStatus("导入 Remix 结果失败：" + String(e));
    }
  };

  const confirmModal = async () => {
    if (!modal) return;
    if (modal.kind === "batch-tags") {
      const ids = selected.size ? [...selected] : current ? [current.id] : [];
      await api.batchAddTags(ids, parseTags(dialogText));
      setModal(null);
      await refreshFacets();
      await refresh(current?.id);
      return;
    }
    if (modal.kind === "history") {
      if (!dialogChoice) return;
      const a = await api.restoreRevision(Number(dialogChoice));
      setCurrent(a);
      loadEditor(a);
      setModal(null);
      setStatus("历史版本已恢复");
      return;
    }
    if (modal.kind === "collection") {
      let id = Number(dialogChoice);
      if (dialogChoice === "new") {
        if (!dialogText.trim()) return;
        id = (await api.createCollection(dialogText.trim(), "")).id;
      }
      if (!id) return;
      const ids = selected.size ? [...selected] : current ? [current.id] : [];
      await api.addToCollection(id, ids);
      setModal(null);
      await refreshFacets();
      setStatus("已加入集合");
      return;
    }
    if (modal.kind === "remove") {
      if (!current) return;
      await flushEditor();
      await api.deleteAsset(current.id);
      setModal(null);
      setCurrent(null);
      await refreshFacets();
      await refresh();
      setStatus("记录已移除");
    }
  };

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (modal) {
          setModal(null);
          return;
        }
        if (parentOpen) {
          setParentOpen(false);
          return;
        }
        if (managerOpen) {
          setManagerOpen(false);
          return;
        }
      }
      if (modal || parentOpen || managerOpen) return;
      const typing = isTextEntry(e.target);
      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        document.querySelector<HTMLInputElement>("#search")?.focus();
        return;
      }
      if (e.key === "F6") {
        e.preventDefault();
        promptRef.current?.focus();
        return;
      }
      if (e.key === "F7") {
        e.preventDefault();
        setPreviewMode((x) => (x === "fit" ? "actual" : "fit"));
        return;
      }
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        copyPrompt();
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void saveRevision();
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === "i") {
        e.preventDefault();
        void chooseImages();
        return;
      }
      if (typing) return;
      const key = e.key.toLowerCase();
      if (key === "arrowleft" || key === "arrowright" || key === "j" || key === "k") {
        e.preventDefault();
        if (!assets.length) return;
        const index = current ? assets.findIndex((x) => x.id === current.id) : -1;
        const forward = key === "arrowright" || key === "j";
        const next = nextAssetIndex(assets.length, index, forward ? 1 : -1);
        if (next >= 0 && assets[next] && assets[next].id !== current?.id)
          void selectRecord(assets[next].id);
        return;
      }
      if (key === "f" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        if (current) void toggleAssetFavorite(current);
        return;
      }
      if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        e.preventDefault();
        if (!current) return;
        const i = assets.findIndex((x) => x.id === current.id),
          n = nextAssetIndex(assets.length, i, e.key === "ArrowUp" ? -1 : 1);
        if (n >= 0 && assets[n]) void selectRecord(assets[n].id);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [modal, parentOpen, managerOpen, current, assets, prompt, selectRecord]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="app-shell">
      <div className="aero-background" aria-hidden="true">
        <i className="cloud a" />
        <i className="cloud b" />
        <i className="bubble a" />
        <i className="bubble b" />
      </div>
      {drop.active ? (
        <div className="drop-overlay" aria-live="polite">
          <div className="drop-card">
            <span className="drop-orb">⇩</span>
            <strong>松开鼠标即可导入</strong>
            <p>
              {drop.count ? `检测到 ${drop.count} 个项目` : "正在识别拖入内容"} · 支持图片和文件夹
            </p>
            <small>文件夹会递归扫描；已存在的图片会自动跳过</small>
          </div>
        </div>
      ) : null}
      <AppHeader
        version={version}
        query={filter.query}
        mode={searchMode}
        similarSourceName={similarSource?.name}
        onMode={changeSearchMode}
        onQuery={changeSearchQuery}
        onClearSimilar={clearSimilar}
        onImport={chooseImages}
        onFolder={chooseFolder}
      />
      <main
        className="workspace"
        style={{ gridTemplateColumns: `${leftWidth}px 8px minmax(360px,1fr) 8px ${rightWidth}px` }}
      >
        <LibraryPane
          assets={assets}
          total={total}
          currentId={current?.id}
          selected={selected}
          loading={loading}
          scores={searchMode === "semantic" ? semanticScores : undefined}
          filter={filter}
          facets={facets}
          savedFilters={savedFilters}
          sessions={sessions}
          onFilter={setFilter}
          onAsset={onAsset}
          onOpenAsset={openAsset}
          onOpenAssetFolder={openAssetFolder}
          onCopyAssetImage={copyAssetImage}
          onCopyAssetPath={copyAssetPath}
          onSaveAssetAs={saveAssetAs}
          onToggleAssetFavorite={toggleAssetFavorite}
          onFindSimilarAsset={findSimilarAsset}
          onLoadMore={loadMore}
          onBatchTags={() => {
            setDialogText("");
            setModal({ kind: "batch-tags" });
          }}
          onBatchFavorite={batchFavorite}
          onBatchRescan={batchRescan}
          onBatchSession={batchSession}
          onCollection={openCollection}
          onClearSelection={() => setSelected(new Set())}
          onRefreshMissing={refreshMissing}
          onManage={openManager}
          onSaveView={saveCurrentView}
          onApplySavedView={applySavedView}
        />
        <div className="splitter" onPointerDown={drag("left")} />
        <PreviewPane
          asset={current}
          src={preview}
          mode={previewMode}
          onMode={setPreviewMode}
          onImport={chooseImages}
          onOpen={openAsset}
          onFolder={openAssetFolder}
          onCopyImage={copyAssetImage}
          onCopyPath={copyAssetPath}
          onSaveAs={saveAssetAs}
          onFavorite={toggleAssetFavorite}
          onFindSimilar={findSimilarAsset}
        />
        <div className="splitter" onPointerDown={drag("right")} />
        <InspectorPane
          asset={current}
          tab={tab}
          references={referenceSources}
          onOpenReference={openReferenceUrl}
          onTab={setTab}
          visualDna={visualDna}
          onSaveVisualDna={saveVisualDna}
          imagePromptAnalysis={imagePromptAnalysis}
          imagePromptLoading={imagePromptLoading}
          visionModel={visionSettings?.model || ""}
          onAnalyzeImage={analyzeCurrentImage}
          onApplyAnalysisDna={() => void applyAnalysisDna(false)}
          onOverwriteAnalysisDna={() => void applyAnalysisDna(true)}
          onUseAnalysisPrompt={useAnalysisPrompt}
          onSaveAnalysisRevision={() => void saveAnalysisRevision()}
          onOpenVisionSettings={openManager}
          remixDraft={remixDraft}
          remixSources={remixSources}
          remixPrompt={remixPrompt}
          onRemixPrompt={setRemixPrompt}
          onAddRemixSource={openRemixSourcePicker}
          onRemoveRemixSource={removeRemixSource}
          onToggleRemixField={toggleRemixField}
          onComposeRemix={composeRemix}
          onSaveRemix={() => void saveRemix()}
          onCopyRemix={copyRemix}
          onImportRemixResult={() => void importRemixResult()}
          onResetRemix={() => void resetRemix()}
          prompt={prompt}
          onPrompt={setPrompt}
          negative={negative}
          onNegative={setNegative}
          model={model}
          onModel={setModel}
          tagsText={tagsText}
          onTagsText={setTagsText}
          lineage={lineage}
          compareRecord={compareRecord}
          compareParentSrc={compareParentSrc}
          compareCurrentSrc={compareCurrentSrc}
          sessions={sessions}
          assetSession={assetSession}
          onCompare={compare}
          onCopy={copyPrompt}
          onSaveRevision={saveRevision}
          onHistory={openHistory}
          onFavorite={toggleFavorite}
          onFindSimilar={findSimilar}
          onRescan={rescan}
          onSidecar={exportSidecar}
          onCollection={openCollection}
          onRemove={() => setModal({ kind: "remove" })}
          onImportDerivative={importDerivative}
          onLinkParent={openParentPicker}
          onSetSession={setSession}
          onCreateSession={createSession}
          onEditSessionNote={editSessionNote}
          onEditRelationNote={editRelationNote}
          promptRef={promptRef}
        />
      </main>
      <footer className="statusbar glass-surface">
        <span className={`runtime-dot ${isTauri ? "native" : "preview"}`} />
        <strong>{isTauri ? "桌面版" : "浏览器预览"}</strong>
        <span>v{version}</span>
        <span className="status-message">{status}</span>
        {importActive ? (
          <>
            <progress max={Math.max(1, importProgress.total)} value={importProgress.processed} />
            <button onClick={cancelImportJob}>取消导入</button>
          </>
        ) : null}
        {semanticActive ? (
          <>
            <progress
              max={Math.max(1, semanticProgress.total)}
              value={semanticProgress.processed}
            />
            <button onClick={cancelSemanticIndex}>取消索引</button>
          </>
        ) : null}
        <span>
          已加载 {assets.length}/{total}
        </span>
        <button onClick={repairMissing} title="根据文件指纹查找移动后的文件">
          修复缺失文件
        </button>
        <span className="shortcut">←→ / J K 切图 · F 收藏 · F6 提示词 · F7 预览 · Ctrl+S 保存</span>
      </footer>
      <LibraryManager
        open={managerOpen}
        backups={backups}
        tags={facets.tags}
        collections={facets.collections}
        duplicates={duplicates}
        modelAliases={modelAliases}
        savedFilters={savedFilters}
        sourceFolders={sourceFolders}
        sourceSyncing={importActive}
        health={health}
        diagnostics={diagnostics}
        visionSettings={visionSettings}
        semanticStatus={semanticStatus}
        semanticIndexing={semanticActive}
        semanticProgress={semanticProgress}
        onRebuildSemantic={rebuildSemantic}
        onCancelSemantic={cancelSemanticIndex}
        onClearSemantic={clearSemantic}
        onDeleteSemanticModels={deleteSemanticModels}
        onClose={() => setManagerOpen(false)}
        onBackup={createBackup}
        onRestore={restoreBackup}
        onOpenDataFolder={openDataFolder}
        onOpenLogsFolder={openLogsFolder}
        onSaveVisionSettings={saveVisionProvider}
        onSetVisionApiKey={setVisionKey}
        onRenameTag={renameTag}
        onDeleteTag={deleteTag}
        onRenameCollection={renameCollection}
        onDeleteCollection={deleteCollection}
        onUpsertModelAlias={upsertModelAlias}
        onDeleteModelAlias={deleteModelAlias}
        onDeleteSavedFilter={deleteSavedView}
        onEnsureReferenceInbox={ensureReferenceInbox}
        onAddSourceFolder={addSourceFolder}
        onRemoveSourceFolder={removeSourceFolder}
        onToggleSourceAutoSync={toggleSourceAutoSync}
        onSyncSourceFolders={syncSourceFolders}
      />
      <ParentPicker
        open={parentOpen}
        query={parentQuery}
        results={parentResults}
        choice={parentChoice}
        loading={parentLoading}
        title={parentMode === "remix" ? "添加 Remix 参考图" : "关联父图"}
        description={
          parentMode === "remix"
            ? "选择一张资料库图片，把其中的 Visual DNA 片段加入当前 Remix 草稿。"
            : "搜索整个 ImageLore 资料库，选择来源或参考父图。"
        }
        confirmLabel={parentMode === "remix" ? "加入 Remix" : "建立关联"}
        onQuery={setParentQuery}
        onChoice={setParentChoice}
        onClose={() => setParentOpen(false)}
        onConfirm={confirmParent}
      />
      <AppDialogs
        modal={modal}
        assets={assets}
        currentId={current?.id}
        text={dialogText}
        setText={setDialogText}
        choice={dialogChoice}
        setChoice={setDialogChoice}
        onClose={() => setModal(null)}
        onConfirm={confirmModal}
      />
    </div>
  );
}
