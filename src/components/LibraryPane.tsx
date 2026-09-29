import { AssetGrid } from "./AssetGrid";
import type {
  AssetSummary,
  GenerationSession,
  LibraryFacets,
  LibraryFilter,
  LibraryView,
  SavedFilter,
} from "../types";

const views: Array<[LibraryView, string, string]> = [
  ["all", "全部", "▦"],
  ["favorites", "收藏", "★"],
  ["recent", "最近", "◷"],
  ["missing", "缺失", "!"],
];
const numberValue = (value: string) => (value.trim() === "" ? null : Number(value));

export function LibraryPane({
  assets,
  total,
  currentId,
  selected,
  loading,
  scores,
  filter,
  facets,
  savedFilters,
  sessions,
  onFilter,
  onAsset,
  onOpenAsset,
  onOpenAssetFolder,
  onCopyAssetImage,
  onCopyAssetPath,
  onSaveAssetAs,
  onToggleAssetFavorite,
  onFindSimilarAsset,
  onLoadMore,
  onBatchTags,
  onBatchFavorite,
  onBatchRescan,
  onBatchSession,
  onCollection,
  onClearSelection,
  onRefreshMissing,
  onManage,
  onSaveView,
  onApplySavedView,
}: {
  assets: AssetSummary[];
  total: number;
  currentId?: number;
  selected: Set<number>;
  loading: boolean;
  scores?: Map<number, number>;
  filter: LibraryFilter;
  facets: LibraryFacets;
  savedFilters: SavedFilter[];
  sessions: GenerationSession[];
  onFilter: (next: LibraryFilter) => void;
  onAsset: (asset: AssetSummary, e: React.MouseEvent) => void;
  onOpenAsset: (asset: AssetSummary) => void;
  onOpenAssetFolder: (asset: AssetSummary) => void;
  onCopyAssetImage: (asset: AssetSummary) => void;
  onCopyAssetPath: (asset: AssetSummary) => void;
  onSaveAssetAs: (asset: AssetSummary) => void;
  onToggleAssetFavorite: (asset: AssetSummary) => void;
  onFindSimilarAsset: (asset: AssetSummary) => void;
  onLoadMore: () => void;
  onBatchTags: () => void;
  onBatchFavorite: (favorite: boolean) => void;
  onBatchRescan: () => void;
  onBatchSession: (sessionId: number | null) => void;
  onCollection: () => void;
  onClearSelection: () => void;
  onRefreshMissing: () => void;
  onManage: () => void;
  onSaveView: () => void;
  onApplySavedView: (view: SavedFilter) => void;
}) {
  const chips: Array<[string, () => void]> = [];
  if (filter.collection_id)
    chips.push(["集合", () => onFilter({ ...filter, collection_id: null })]);
  if (filter.tag) chips.push([`标签 · ${filter.tag}`, () => onFilter({ ...filter, tag: null })]);
  if (filter.model)
    chips.push([`模型 · ${filter.model}`, () => onFilter({ ...filter, model: null })]);
  if (filter.metadata_type)
    chips.push([
      `来源 · ${filter.metadata_type}`,
      () => onFilter({ ...filter, metadata_type: null }),
    ]);
  if (filter.sampler)
    chips.push([`采样器 · ${filter.sampler}`, () => onFilter({ ...filter, sampler: null })]);
  if (filter.scheduler)
    chips.push([`调度器 · ${filter.scheduler}`, () => onFilter({ ...filter, scheduler: null })]);
  if (filter.seed) chips.push([`Seed · ${filter.seed}`, () => onFilter({ ...filter, seed: null })]);
  if (filter.steps_min != null || filter.steps_max != null)
    chips.push([
      `Steps · ${filter.steps_min ?? "…"}–${filter.steps_max ?? "…"}`,
      () => onFilter({ ...filter, steps_min: null, steps_max: null }),
    ]);
  if (filter.cfg_min != null || filter.cfg_max != null)
    chips.push([
      `CFG · ${filter.cfg_min ?? "…"}–${filter.cfg_max ?? "…"}`,
      () => onFilter({ ...filter, cfg_min: null, cfg_max: null }),
    ]);
  if (filter.denoise_min != null || filter.denoise_max != null)
    chips.push([
      `Denoise · ${filter.denoise_min ?? "…"}–${filter.denoise_max ?? "…"}`,
      () => onFilter({ ...filter, denoise_min: null, denoise_max: null }),
    ]);
  if (filter.orientation)
    chips.push([
      `构图 · ${filter.orientation === "portrait" ? "竖图" : filter.orientation === "landscape" ? "横图" : "方图"}`,
      () => onFilter({ ...filter, orientation: null }),
    ]);

  const advancedCount = [
    filter.metadata_type,
    filter.sampler,
    filter.scheduler,
    filter.seed,
    filter.steps_min != null || filter.steps_max != null,
    filter.cfg_min != null || filter.cfg_max != null,
    filter.denoise_min != null || filter.denoise_max != null,
    filter.orientation,
  ].filter(Boolean).length;

  const clearAdvanced = () =>
    onFilter({
      ...filter,
      metadata_type: null,
      sampler: null,
      scheduler: null,
      seed: null,
      steps_min: null,
      steps_max: null,
      cfg_min: null,
      cfg_max: null,
      denoise_min: null,
      denoise_max: null,
      orientation: null,
    });

  return (
    <aside className="library-pane panel glass-surface">
      <div className="pane-heading">
        <div>
          <span className="eyebrow">图库</span>
          <div className="heading-line">
            <strong>生成记录</strong>
            <span className="count-pill">{total}</span>
          </div>
        </div>
        <div className="pane-actions">
          <button className="icon-button" title="资料库管理" onClick={onManage}>
            ⚙
          </button>
          <button className="icon-button" title="刷新缺失文件状态" onClick={onRefreshMissing}>
            ↻
          </button>
        </div>
      </div>
      <div className="view-switch" role="tablist">
        {views.map(([value, label, icon]) => (
          <button
            key={value}
            className={filter.view === value ? "active" : ""}
            onClick={() => onFilter({ ...filter, view: value })}
          >
            <span>{icon}</span>
            {label}
          </button>
        ))}
      </div>

      <div className="saved-view-bar">
        <select
          defaultValue=""
          onChange={(e) => {
            const item = savedFilters.find((x) => x.id === Number(e.target.value));
            if (item) onApplySavedView(item);
            e.currentTarget.value = "";
          }}
        >
          <option value="">保存视图…</option>
          {savedFilters.map((x) => (
            <option key={x.id} value={x.id}>
              {x.name}
            </option>
          ))}
        </select>
        <select
          value={filter.sort || "smart"}
          onChange={(e) => onFilter({ ...filter, sort: e.target.value })}
          title="排序"
        >
          <option value="smart">智能排序</option>
          <option value="updated_desc">最近更新</option>
          <option value="updated_asc">最早更新</option>
          <option value="created_desc">最近导入</option>
          <option value="created_asc">最早导入</option>
          <option value="name_asc">文件名 A–Z</option>
          <option value="name_desc">文件名 Z–A</option>
          <option value="resolution_desc">分辨率从高到低</option>
          <option value="size_desc">文件从大到小</option>
        </select>
        <button onClick={onSaveView}>＋ 保存</button>
      </div>

      <div className="filter-strip">
        <label>
          <span>集合</span>
          <select
            value={filter.collection_id ?? ""}
            onChange={(e) =>
              onFilter({ ...filter, collection_id: e.target.value ? Number(e.target.value) : null })
            }
          >
            <option value="">全部集合</option>
            {facets.collections.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} ({c.count})
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>标签</span>
          <select
            value={filter.tag ?? ""}
            onChange={(e) => onFilter({ ...filter, tag: e.target.value || null })}
          >
            <option value="">全部标签</option>
            {facets.tags.map((x) => (
              <option key={x.name} value={x.name}>
                {x.name} ({x.count})
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>模型</span>
          <select
            value={filter.model ?? ""}
            onChange={(e) => onFilter({ ...filter, model: e.target.value || null })}
          >
            <option value="">全部模型</option>
            {facets.models.map((x) => (
              <option key={x.name} value={x.name}>
                {x.name} ({x.count})
              </option>
            ))}
          </select>
        </label>
      </div>

      <details className="advanced-filter" open={advancedCount > 0}>
        <summary>
          <span>生成参数筛选</span>
          <b>{advancedCount || ""}</b>
        </summary>
        <div className="advanced-filter-grid">
          <label>
            <span>元数据来源</span>
            <select
              value={filter.metadata_type ?? ""}
              onChange={(e) => onFilter({ ...filter, metadata_type: e.target.value || null })}
            >
              <option value="">全部来源</option>
              {facets.metadata_types.map((x) => (
                <option key={x.name} value={x.name}>
                  {x.name} ({x.count})
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>采样器</span>
            <select
              value={filter.sampler ?? ""}
              onChange={(e) => onFilter({ ...filter, sampler: e.target.value || null })}
            >
              <option value="">全部采样器</option>
              {facets.samplers.map((x) => (
                <option key={x.name} value={x.name}>
                  {x.name} ({x.count})
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>调度器</span>
            <select
              value={filter.scheduler ?? ""}
              onChange={(e) => onFilter({ ...filter, scheduler: e.target.value || null })}
            >
              <option value="">全部调度器</option>
              {facets.schedulers.map((x) => (
                <option key={x.name} value={x.name}>
                  {x.name} ({x.count})
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Seed</span>
            <input
              value={filter.seed ?? ""}
              onChange={(e) => onFilter({ ...filter, seed: e.target.value || null })}
              placeholder="精确 Seed"
            />
          </label>
          <label>
            <span>Steps 最小</span>
            <input
              type="number"
              value={filter.steps_min ?? ""}
              onChange={(e) => onFilter({ ...filter, steps_min: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>Steps 最大</span>
            <input
              type="number"
              value={filter.steps_max ?? ""}
              onChange={(e) => onFilter({ ...filter, steps_max: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>CFG 最小</span>
            <input
              type="number"
              step="0.1"
              value={filter.cfg_min ?? ""}
              onChange={(e) => onFilter({ ...filter, cfg_min: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>CFG 最大</span>
            <input
              type="number"
              step="0.1"
              value={filter.cfg_max ?? ""}
              onChange={(e) => onFilter({ ...filter, cfg_max: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>Denoise 最小</span>
            <input
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={filter.denoise_min ?? ""}
              onChange={(e) => onFilter({ ...filter, denoise_min: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>Denoise 最大</span>
            <input
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={filter.denoise_max ?? ""}
              onChange={(e) => onFilter({ ...filter, denoise_max: numberValue(e.target.value) })}
            />
          </label>
          <label>
            <span>构图方向</span>
            <select
              value={filter.orientation ?? ""}
              onChange={(e) => onFilter({ ...filter, orientation: e.target.value || null })}
            >
              <option value="">全部构图</option>
              <option value="portrait">竖图</option>
              <option value="landscape">横图</option>
              <option value="square">方图</option>
            </select>
          </label>
          <button className="clear-filter" onClick={clearAdvanced}>
            清空生成参数
          </button>
        </div>
      </details>

      {chips.length ? (
        <div className="filter-chips">
          {chips.map(([label, clear]) => (
            <button key={label} onClick={clear}>
              {label}
              <span>×</span>
            </button>
          ))}
        </div>
      ) : null}

      {selected.size > 1 ? (
        <div className="selection-bar generation-batch">
          <strong>已选择 {selected.size} 项</strong>
          <button onClick={onBatchTags}>标签</button>
          <button onClick={() => onBatchFavorite(true)}>收藏</button>
          <button onClick={() => onBatchFavorite(false)}>取消收藏</button>
          <button onClick={onBatchRescan}>重读元数据</button>
          <select
            defaultValue=""
            onChange={(e) => {
              if (e.target.value !== "")
                onBatchSession(e.target.value === "none" ? null : Number(e.target.value));
              e.currentTarget.value = "";
            }}
          >
            <option value="">分配 Session…</option>
            <option value="none">移出 Session</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <button onClick={onCollection}>集合</button>
          <button onClick={onClearSelection}>取消</button>
        </div>
      ) : null}

      {assets.length ? (
        <AssetGrid
          assets={assets}
          total={total}
          currentId={currentId}
          selected={selected}
          loading={loading}
          scores={scores}
          onAsset={onAsset}
          onOpen={onOpenAsset}
          onOpenFolder={onOpenAssetFolder}
          onCopyImage={onCopyAssetImage}
          onCopyPath={onCopyAssetPath}
          onSaveAs={onSaveAssetAs}
          onFavorite={onToggleAssetFavorite}
          onFindSimilar={onFindSimilarAsset}
          onLoadMore={onLoadMore}
        />
      ) : (
        <div className="library-empty">
          <span className="empty-orb">✦</span>
          <strong>没有符合条件的生成记录</strong>
          <p>可以清除筛选，或导入新的图片与生成元数据。</p>
        </div>
      )}
    </aside>
  );
}
