# ImageLore Roadmap

## v0.10.0 — Clean Core ✅
全新 SQLite 数据基线、规范化标签、FTS、Collection、Lineage。

## v0.11.0 — Chinese Desktop ✅
中文界面、Windows GUI subsystem、正式 NSIS 安装包。

## v0.12.0 — Native Drag & Drop ✅
原生图片/文件夹拖拽导入。

## v0.13.0 — Reliability & Flow ✅
无损自动保存、轻量图库记录、事务写入、导入解锁、CJK 搜索、模块拆分、CI 验证。

## v0.13.1 — Build Hygiene ✅
依赖锁定、快速 CI / Release 分离、并发取消、单次 NSIS 构建与可复现发布。

## v0.14.0 — Library Continuity ✅
- 自动 / 手动 Backup 与安全 Restore
- SHA-256 重复内容检测
- 后台导入进度与取消
- Tag / Collection 管理
- 全库可搜索 Parent Picker
- 更完整的 ComfyUI Metadata Adapter
- 本地 asset protocol 预览与缓存治理
- Migration v2

## v0.14.1 — Stability Fixes ✅
- 导入 / 拖拽 / 切图异步竞态修复
- 关闭窗口前自动 flush
- WAL-safe Backup / Restore
- SQLite 长耗时锁释放与事务补全
- CJK 多关键词搜索修复
- Preview 缓存失效修复
- Fast CI / Native CI 完全分离
- Rust regression tests

## v0.15.0 — Generation Intelligence ✅
- A1111 / ComfyUI / NovelAI / InvokeAI / 通用 JSON Metadata Adapter
- Sidecar v3 与 Portable ID + fingerprint 跨库谱系恢复
- 图片并排 Compare + Prompt Diff
- Generation Session / Branch Notes
- Model alias / normalization
- 保存筛选与 Library Health
- Migration v3

## v0.15.2 — Showcase ✅
- README / OG visual preview
- Current Chinese UI preview
- Reproducible 1280×640 OG renderer
- Metadata source label polish

## v0.16.0 — Source Sync ✅
- 持久化资料源目录
- 启动时自动同步 / 一键同步
- 单目录同步、移除与自动同步开关
- 复用后台导入进度、取消与重复检测
- Migration v4

## v0.17.0 — Generation Explorer ✅
- Migration v5 + 可重建的结构化 Generation Index
- Seed / Steps / Sampler / Scheduler / CFG / Denoise 高级筛选
- 元数据来源与横图 / 竖图 / 方图筛选
- 图库排序与 Active Filter chips
- Saved View 2.0 保存完整筛选与排序
- 批量收藏、元数据重读与 Generation Session 分配
- 50k 图库筛选性能 smoke test
- 布局缓存、缩略图异步、关闭导入任务与重复组查询稳定性修复

## v0.18.0 — Semantic Recall ✅
- Migration v6 + 可重建 Semantic Embedding Index
- FastEmbed / CLIP ViT-B/32 本地图文共同向量空间
- 自然语言描述找图与视觉相似图片搜索
- Semantic Recall 与 Generation Explorer 结构化筛选组合
- 关键词 / 语义双搜索模式与相似度结果提示
- AI 索引管理、增量更新、进度、取消与模型缓存治理
- Import / Source Sync 与 Semantic Index 共用后台任务注册/取消底座
- 50k 图库线性语义扫描性能 smoke test

## v1.0.0 — Stable Local Library
- Windows 代码签名与自动更新
- 启动故障恢复与 rotating logs
- 大图库 50k+ 性能验收
- CJK n-gram / trigram 搜索索引
- 正式 Backup / Restore 灾难恢复验收
- 稳定迁移兼容性测试矩阵
