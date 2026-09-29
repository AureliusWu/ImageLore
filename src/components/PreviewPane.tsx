import { useEffect, useRef, useState } from "react";
import type { AssetRecord, AssetSummary } from "../types";
import { AssetContextMenu, type AssetMenuAction } from "./AssetContextMenu";
import { clampZoom, zoomFromWheel } from "../previewWorkflow";

type PreviewMode = "fit" | "actual";
const bytes = (n: number | null) =>
  !n
    ? ""
    : n < 1024
      ? `${n} B`
      : n < 1024 * 1024
        ? `${(n / 1024).toFixed(1)} KB`
        : `${(n / 1024 / 1024).toFixed(1)} MB`;
const metadataLabel = (value: string) =>
  (
    ({
      manual: "手动",
      a1111: "AUTOMATIC1111",
      comfyui: "ComfyUI",
      novelai: "NovelAI",
      invokeai: "InvokeAI",
      json: "通用 JSON",
      none: "无生成元数据",
    }) as Record<string, string>
  )[value] || value;

export function PreviewPane({
  asset,
  src,
  mode,
  onMode,
  onImport,
  onOpen,
  onFolder,
  onCopyImage,
  onCopyPath,
  onSaveAs,
  onFavorite,
  onFindSimilar,
}: {
  asset: AssetRecord | null;
  src: string;
  mode: PreviewMode;
  onMode: (m: PreviewMode) => void;
  onImport: () => void;
  onOpen: AssetMenuAction;
  onFolder: AssetMenuAction;
  onCopyImage: AssetMenuAction;
  onCopyPath: AssetMenuAction;
  onSaveAs: AssetMenuAction;
  onFavorite: AssetMenuAction;
  onFindSimilar: AssetMenuAction;
}) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [zoom, setZoom] = useState(100);
  const [natural, setNatural] = useState({ width: 0, height: 0 });
  const [panning, setPanning] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const panRef = useRef<{ x: number; y: number; left: number; top: number } | null>(null);

  useEffect(() => {
    setMenu(null);
    setZoom(100);
    setNatural({ width: 0, height: 0 });
    setPanning(false);
    panRef.current = null;
    stageRef.current?.scrollTo({ left: 0, top: 0 });
  }, [asset?.id]);

  useEffect(() => {
    if (mode === "fit") {
      setZoom(100);
      setPanning(false);
      panRef.current = null;
      stageRef.current?.scrollTo({ left: 0, top: 0 });
    }
  }, [mode]);

  const setActual = (next: number) => {
    const value = clampZoom(next);
    setZoom(value);
    if (mode !== "actual") onMode("actual");
  };
  const zoomBy = (delta: number) => setActual(zoom + delta);
  const fit = () => {
    setZoom(100);
    onMode("fit");
  };
  const actual = () => {
    setZoom(100);
    onMode("actual");
  };
  const wheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!src) return;
    e.preventDefault();
    if (e.deltaY === 0) return;
    setActual(zoomFromWheel(zoom, e.deltaY, mode));
  };
  const doubleClick = () => (mode === "fit" ? actual() : fit());
  const pointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (mode !== "actual" || e.button !== 0) return;
    const stage = stageRef.current;
    if (!stage) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    panRef.current = { x: e.clientX, y: e.clientY, left: stage.scrollLeft, top: stage.scrollTop };
    setPanning(true);
  };
  const pointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const start = panRef.current,
      stage = stageRef.current;
    if (!start || !stage) return;
    stage.scrollLeft = start.left - (e.clientX - start.x);
    stage.scrollTop = start.top - (e.clientY - start.y);
  };
  const pointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!panRef.current) return;
    panRef.current = null;
    setPanning(false);
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {}
  };

  const image = src ? (
    <img
      src={src}
      alt={asset?.name || "预览"}
      draggable={false}
      onLoad={(e) =>
        setNatural({ width: e.currentTarget.naturalWidth, height: e.currentTarget.naturalHeight })
      }
      onDoubleClick={doubleClick}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    />
  ) : null;

  return (
    <section className="preview-pane panel glass-surface">
      <div className="preview-head">
        <div className="file-title">
          <span className={`status-dot ${asset?.missing ? "bad" : ""}`} />
          <div>
            <strong>{asset?.name || "未选择图片"}</strong>
            <small>
              {asset
                ? `${asset.format || "图片"}${asset.width && asset.height ? ` · ${asset.width}×${asset.height}` : ""}`
                : "请从左侧图库选择一条记录"}
            </small>
          </div>
        </div>
        <div className="preview-controls">
          <div className="zoom-controls" aria-label="预览缩放">
            <button onClick={() => zoomBy(-10)} disabled={!src}>
              −
            </button>
            <span>{mode === "fit" ? "适应" : `${zoom}%`}</span>
            <button onClick={() => zoomBy(10)} disabled={!src}>
              ＋
            </button>
          </div>
          <div className="segmented">
            <button className={mode === "fit" ? "active" : ""} onClick={fit}>
              适应窗口
            </button>
            <button className={mode === "actual" && zoom === 100 ? "active" : ""} onClick={actual}>
              100%
            </button>
          </div>
        </div>
      </div>
      <div
        ref={stageRef}
        className={`image-stage ${mode} ${panning ? "panning" : ""}`}
        onWheel={wheel}
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={pointerUp}
        onPointerCancel={pointerUp}
      >
        {src ? (
          mode === "actual" ? (
            <div
              className="image-zoom-shell"
              style={{
                width: Math.max(1, (natural.width * zoom) / 100),
                height: Math.max(1, (natural.height * zoom) / 100),
              }}
            >
              {image}
            </div>
          ) : (
            image
          )
        ) : (
          <div className="empty-state">
            <div className="empty-orb">✦</div>
            <strong>从第一张图片开始建立你的生成记忆</strong>
            <span>导入图片、记录提示词，ImageLore 会把与它有关的上下文保存下来。</span>
            <button className="button primary" onClick={onImport}>
              ＋ 导入图片
            </button>
          </div>
        )}
      </div>
      {menu && asset ? (
        <AssetContextMenu
          asset={asset as AssetSummary}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          onOpen={onOpen}
          onFolder={onFolder}
          onCopyImage={onCopyImage}
          onCopyPath={onCopyPath}
          onSaveAs={onSaveAs}
          onFavorite={onFavorite}
          onFindSimilar={onFindSimilar}
        />
      ) : null}
      {asset ? (
        <div className="preview-foot">
          <div className="preview-meta">
            <span className="metadata-pill">{metadataLabel(asset.metadata_type || "manual")}</span>
            <span className="preview-size">{bytes(asset.file_size)}</span>
            <span className="preview-model" title={asset.model || "未记录模型"}>
              {asset.model || "未记录模型"}
            </span>
            <span className="path" title={asset.path}>
              {asset.path}
            </span>
          </div>
          <div className="preview-actionbar" aria-label="图片快捷操作">
            <button
              className={asset.favorite ? "active" : ""}
              onClick={() => onFavorite(asset)}
              title={asset.favorite ? "取消收藏 (F)" : "收藏 (F)"}
            >
              {asset.favorite ? "★" : "☆"}
              <span>收藏</span>
            </button>
            <button onClick={() => onCopyPath(asset)} title="复制文件路径">
              ⧉<span>路径</span>
            </button>
            <button onClick={() => onCopyImage(asset)} disabled={!!asset.missing} title="复制图像">
              ▣<span>图像</span>
            </button>
            <button onClick={() => onFindSimilar(asset)} title="查找视觉相似图片">
              ◎<span>相似</span>
            </button>
            <button
              onClick={() => onFolder(asset)}
              disabled={!!asset.missing}
              title="打开文件所在位置"
            >
              ⌖<span>定位</span>
            </button>
            <button onClick={() => onOpen(asset)} disabled={!!asset.missing} title="打开图片">
              ↗<span>打开</span>
            </button>
          </div>
        </div>
      ) : null}
    </section>
  );
}
