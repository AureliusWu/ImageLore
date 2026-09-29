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

## v0.18.1 — Editor & Semantic Security Hardening ✅
- 图库右键打开文件所在位置
- Prompt Revision 原子保存，消除 autosave 竞态
- 固定 CLIP revision + ONNX SHA-256 / 大小可信校验
- 模型 staging / trusted cache 原子安装与配套 JSON 边界校验

## v0.19.0 — Search Scale ✅
- Migration v7 + CJK FTS5 trigram 候选索引
- 3 字及以上 CJK 查询先走 trigram 收窄，再以既有 LIKE 子串规则复核
- 1–2 字 CJK 查询保持原有兼容路径
- Prompt / Model / Tag / 文件名变化与删除保持搜索索引一致
- 50k 图库 CJK 索引搜索性能 smoke test

## v0.20.0 — Windows Upgrade Reliability ✅
- NSIS 安装身份与 currentUser 安装模式保持稳定，升级覆盖同一安装记录
- 禁止安装器降级覆盖
- 用户数据库 / 备份 / 模型 / 缓存迁出程序安装目录
- v0.19.x 旧数据首次启动自动迁移
- Windows Release 实机式 v0.19.0 → 当前版本覆盖升级 smoke test
- 验证唯一安装项、原安装目录、EXE 版本、快捷方式与数据库保留

## v0.21.0 — Preview Workflow ✅
- 图片完整右键菜单：打开、定位、复制图像 / 路径、另存为、收藏、相似图
- 中央预览 25%–400% 滚轮缩放、拖拽平移、双击 Fit / 100%
- 缩略图与主预览统一 AssetContextMenu
- ← / →、J / K 切图与 F 收藏快捷键
- Inspector 基于容器宽度自适应窄栏
- 主预览底部高频快捷操作条

## v1.0.0 — Stable Local Library
- Windows 代码签名与自动更新
- 启动故障恢复与 rotating logs
- 大图库 50k+ 性能验收
- 正式 Backup / Restore 灾难恢复验收
- 稳定迁移兼容性测试矩阵
