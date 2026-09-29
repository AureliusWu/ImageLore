import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AssetRecord,
  AssetSession,
  AssetSummary,
  BackupRecord,
  CollectionRecord,
  DiagnosticStatus,
  DuplicateGroup,
  GenerationSession,
  ImportSummary,
  LibraryFacets,
  LibraryFilter,
  LibraryHealth,
  LibraryPage,
  Lineage,
  ModelAlias,
  PromptPatch,
  Revision,
  SavedFilter,
  SemanticHit,
  SemanticStatus,
  SourceFolder,
  VisionSettings,
  ImagePromptAnalysis,
  VisualDna,
  VisualDnaPatch,
  RemixDraft,
  RemixSourceInput,
  ReferenceSource,
} from "./types";
import { mockCall } from "./api/mock";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return isTauri ? invoke<T>(command, args) : mockCall<T>(command, args);
}

export const api = {
  page: (filter: LibraryFilter, offset = 0, limit = 240) =>
    call<LibraryPage>("library_page", { filter, offset, limit }),
  facets: () => call<LibraryFacets>("library_facets"),
  get: (id: number) => call<AssetRecord>("get_asset", { id }),
  preview: async (id: number, maxEdge: number, thumbnail = false) => {
    const source = await call<string>("preview_cache_path", { id, maxEdge, thumbnail });
    return isTauri ? convertFileSrc(source) : source;
  },
  importPaths: (paths: string[]) => call<ImportSummary>("import_paths", { paths }),
  importFolder: (path: string) => call<ImportSummary>("import_folder", { path }),
  importDroppedPaths: (paths: string[]) => call<ImportSummary>("import_dropped_paths", { paths }),
  startImportPaths: (paths: string[]) => call<number>("start_import_paths", { paths }),
  startImportFolder: (path: string) => call<number>("start_import_folder", { path }),
  startImportDroppedPaths: (paths: string[]) =>
    call<number>("start_import_dropped_paths", { paths }),
  cancelImport: (jobId: number) => call<boolean>("cancel_import", { jobId }),
  updatePrompt: (id: number, patch: PromptPatch) =>
    call<AssetRecord>("update_prompt", { id, patch }),
  replaceTags: (id: number, tags: string[]) => call<AssetRecord>("replace_tags", { id, tags }),
  batchAddTags: (ids: number[], tags: string[]) => call<boolean>("batch_add_tags", { ids, tags }),
  toggleFavorite: (id: number) => call<AssetRecord>("toggle_favorite", { id }),
  deleteAsset: (id: number) => call<boolean>("delete_asset", { id }),
  addRevision: (id: number, note = "") => call<boolean>("add_revision", { id, note }),
  savePromptRevision: (id: number, patch: PromptPatch, tags: string[], note = "") =>
    call<AssetRecord>("save_prompt_revision", { id, patch, tags, note }),
  revisions: (id: number) => call<Revision[]>("list_revisions", { id }),
  restoreRevision: (revisionId: number) => call<AssetRecord>("restore_revision", { revisionId }),
  lineage: (id: number) => call<Lineage>("lineage", { id }),
  addRelation: (parentId: number, childId: number, relationType: string, note = "") =>
    call<boolean>("add_relation", { parentId, childId, relationType, note }),
  sessions: () => call<GenerationSession[]>("generation_sessions"),
  createSession: (name: string, note = "") =>
    call<GenerationSession>("create_generation_session", { name, note }),
  assetSession: (assetId: number) => call<AssetSession | null>("asset_session", { assetId }),
  setAssetSession: (assetId: number, sessionId: number | null, note = "") =>
    call<boolean>("set_asset_session", { assetId, sessionId, note }),
  updateRelationNote: (relationId: number, note: string) =>
    call<boolean>("update_relation_note", { relationId, note }),
  modelAliases: () => call<ModelAlias[]>("model_aliases"),
  upsertModelAlias: (alias: string, canonical: string) =>
    call<boolean>("upsert_model_alias", { alias, canonical }),
  deleteModelAlias: (alias: string) => call<boolean>("delete_model_alias", { alias }),
  savedFilters: () => call<SavedFilter[]>("saved_filters"),
  saveFilter: (name: string, filter: LibraryFilter) =>
    call<SavedFilter>("save_filter", { name, filter }),
  deleteSavedFilter: (id: number) => call<boolean>("delete_saved_filter", { id }),
  libraryHealth: () => call<LibraryHealth>("library_health"),
  sourceFolders: () => call<SourceFolder[]>("source_folders"),
  addSourceFolder: (path: string) => call<SourceFolder>("add_source_folder", { path }),
  removeSourceFolder: (id: number) => call<boolean>("remove_source_folder", { id }),
  setSourceAutoSync: (id: number, enabled: boolean) =>
    call<SourceFolder>("set_source_auto_sync", { id, enabled }),
  startSyncSources: (ids: number[]) => call<number>("start_sync_sources", { ids }),
  ensureReferenceInbox: () => call<SourceFolder>("ensure_reference_inbox"),
  referenceSources: (assetId: number) => call<ReferenceSource[]>("reference_sources", { assetId }),
  openReferenceUrl: (url: string) => call<boolean>("open_reference_url", { url }),
  collections: () => call<CollectionRecord[]>("collections"),
  duplicateGroups: () => call<DuplicateGroup[]>("duplicate_groups"),
  createBackup: () => call<BackupRecord>("create_backup"),
  ensureAutoBackup: () => call<BackupRecord | null>("ensure_auto_backup"),
  backups: () => call<BackupRecord[]>("list_backups"),
  stageRestore: (name: string) => call<boolean>("stage_restore", { name }),
  renameTag: (oldName: string, newName: string) =>
    call<boolean>("rename_tag", { oldName, newName }),
  mergeTags: (sourceName: string, targetName: string) =>
    call<boolean>("merge_tags", { sourceName, targetName }),
  deleteTag: (name: string) => call<boolean>("delete_tag", { name }),
  renameCollection: (id: number, name: string) => call<boolean>("rename_collection", { id, name }),
  deleteCollection: (id: number) => call<boolean>("delete_collection", { id }),
  createCollection: (name: string, description = "") =>
    call<CollectionRecord>("create_collection", { name, description }),
  addToCollection: (collectionId: number, assetIds: number[]) =>
    call<boolean>("add_to_collection", { collectionId, assetIds }),
  rescan: (id: number) => call<AssetRecord>("rescan_metadata", { id }),
  exportSidecar: (id: number) => call<string>("export_sidecar", { id }),
  refreshMissing: () => call<number>("refresh_missing"),
  relocateMissing: (root: string) => call<number>("relocate_missing", { root }),
  openExternal: (id: number) => call<boolean>("open_external", { id }),
  openFolder: (id: number) => call<boolean>("open_containing_folder", { id }),
  copyAssetTo: (id: number, destination: string) =>
    call<boolean>("copy_asset_to", { id, destination }),
  visualDna: (id: number) => call<VisualDna>("get_visual_dna", { id }),
  updateVisualDna: (id: number, value: VisualDnaPatch) =>
    call<VisualDna>("update_visual_dna", { id, value }),
  visionSettings: () => call<VisionSettings>("vision_settings"),
  saveVisionSettings: (baseUrl: string, model: string) =>
    call<VisionSettings>("save_vision_settings", { baseUrl, model }),
  setVisionApiKey: (apiKey: string) => call<boolean>("set_vision_api_key", { apiKey }),
  latestImagePromptAnalysis: (id: number) =>
    call<ImagePromptAnalysis | null>("latest_image_prompt_analysis", { id }),
  analyzeImageToPrompt: (id: number) =>
    call<ImagePromptAnalysis>("analyze_image_to_prompt", { id }),
  applyImagePromptDna: (id: number, analysisId: number, overwrite = false) =>
    call<VisualDna>("apply_image_prompt_dna", { id, analysisId, overwrite }),
  saveImagePromptRevision: (id: number, analysisId: number) =>
    call<boolean>("save_image_prompt_revision", { id, analysisId }),
  latestRemixDraft: (baseAssetId: number) =>
    call<RemixDraft | null>("latest_remix_draft", { baseAssetId }),
  saveRemixDraft: (
    id: number | null,
    baseAssetId: number,
    prompt: string,
    sources: RemixSourceInput[],
  ) => call<RemixDraft>("save_remix_draft", { id, baseAssetId, prompt, sources }),
  deleteRemixDraft: (id: number) => call<boolean>("delete_remix_draft", { id }),
  applyRemixLineage: (draftId: number, childId: number) =>
    call<boolean>("apply_remix_lineage", { draftId, childId }),
  diagnosticsStatus: () => call<DiagnosticStatus>("diagnostics_status"),
  openDataFolder: () => call<boolean>("open_data_folder"),
  openLogsFolder: () => call<boolean>("open_logs_folder"),
  semanticStatus: () => call<SemanticStatus>("semantic_status"),
  startSemanticIndex: () => call<number>("start_semantic_index"),
  cancelSemanticIndex: (jobId: number) => call<boolean>("cancel_semantic_index", { jobId }),
  semanticSearchText: (query: string, filter: LibraryFilter, limit = 240) =>
    call<SemanticHit[]>("semantic_search_text", { query, filter, limit }),
  semanticSearchSimilar: (assetId: number, filter: LibraryFilter, limit = 240) =>
    call<SemanticHit[]>("semantic_search_similar", { assetId, filter, limit }),
  clearSemanticIndex: () => call<boolean>("clear_semantic_index"),
  deleteSemanticModels: () => call<boolean>("delete_semantic_models"),
};
