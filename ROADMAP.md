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

## v0.21.1 — Preview Regression Hardening ✅
- 缩放模式切换保留当前预览，消除空状态闪烁
- 预览加载与谱系 / Session 查询解耦
- 缩放、切图、右键定位、另存为扩展名抽为可测试纯逻辑
- Save As 同路径保护与文件复制回归测试
- Node 行为测试 + Rust 单元测试接入常规 CI

## v0.22.0 — Visual DNA Foundation ✅
- Migration v8 + 12 维结构化 Visual DNA
- Prompt Card：Prompt / 模型 / 尺寸 / 核心视觉特征 / 谱系汇总
- Visual DNA 接入 FTS、CJK 搜索、Sidecar v3 与普通重建索引流程
- 启动数据库损坏时保留原件并尝试最近可验证备份
- 1 MB × 5 本地滚动日志与资料库诊断入口
- Visual DNA / Sidecar / Search / Recovery / Logging 回归测试

## v0.23.0 — Image to Prompt ✅
- OpenAI-compatible Vision Provider 配置
- 当前图片 → 结构化 Visual DNA + 可生成 Prompt
- 本地缩图后仅在显式分析时上传；API Key 仅驻留当前进程内存
- 分析结果记录 Provider / 模型 / 时间 / 摘要 / Prompt / DNA
- 默认只补充 Visual DNA 空字段；显式操作才覆盖已有手工内容
- AI Prompt 可载入当前编辑器或保存为独立 Prompt Revision
- Migration v9 + JSON / Provider / DNA merge / schema / contract 回归测试

## v0.24.0 — Remix Workspace ✅
- 从一张或多张参考图选择可复用 Visual DNA 片段
- 按 12 维 Visual DNA 选择来源并组合 provenance-aware Prompt
- Remix 草稿独立保存，不修改原记录
- 导入结果自动写入 derived_from / reference Generation Lineage
- 谱系备注记录实际使用的 DNA 字段；结果 Prompt 已存在时不覆盖
- Generation Session 继承
- 为浏览器 Save to ImageLore 预留 source URL / reference metadata
- Migration v10 + Node / Rust Remix 回归测试

## v0.25.0 — Save to ImageLore ✅
- Chrome / Edge Manifest V3 图片右键收藏
- Downloads/ImageLore Inbox + 同名 Sidecar 作为无服务桥接
- Migration v11 + 多来源 Reference Source 持久化
- SHA-256 重复图片合并来源，不重复创建资产
- Reference Card：页面 / 原图 / 标题 / 时间 / 采集用途
- Reference 来源进入 FTS / CJK 搜索与 Sidecar v3
- 启动和窗口重新聚焦时同步 Inbox，不常驻 watcher
- 浏览器扩展 + Migration / Schema / Merge / Contract 回归测试

## v0.25.1 — Codebase Hygiene ✅
- Biome 前端格式化门禁 + TypeScript strict 检查
- Rust fmt / check / clippy / test 四重 Native CI 门禁
- 浏览器 Preview Mock 与正式 Tauri API Bridge 解耦
- App 选中资产上下文 / Vision 工作流拆成领域 Hook
- Rust 文件操作与搜索查询从大模块中拆出
- ARCHITECTURE / WINDOWS_INSTALLER 文档同步当前架构
- 项目完整性检查改为格式化无关的结构断言

## v0.26.0 — Stability Acceptance

执行中；全库验证、手动备份关闭等待及正式内存窗口的进度见[性能跟进](docs/V0_26_PERFORMANCE_FOLLOWUP.md)，阶段退出条件通过后再推进下一版本。
- 五组10分钟[内存隔离窗口](docs/V0_26_MEMORY_ISOLATION.md)已完成；连续换图与缩略图滚动仍超过20%预算，保留WARN。坏语义向量的[严格读取与增量重建](docs/V0_26_VECTOR_INTEGRITY.md)已修复，源码CI通过；新候选真实模型及规模验收继续执行。
- 当前开发版本已推进到 `0.26.0`；按本阶段任务实施。验收记录：[存储/恢复](docs/V0_26_STORAGE_ACCEPTANCE.md)、[关键工作流](docs/V0_26_WORKFLOW_ACCEPTANCE.md)、[原生规模](docs/V0_26_NATIVE_SCALE.md)、[桌面程序](docs/V0_26_DESKTOP_ACCEPTANCE.md)、[真实模型](docs/V0_26_MODEL_ACCEPTANCE.md)。实际快捷方式、桌面保存重启和恢复已有验证；CLIP 三图集成已通过，启动/内存、全规模模型和完整退出标准仍需关闭。
- [版本化桌面快捷方式](docs/DESKTOP_SHORTCUT.md)：本地原生运行目录与实际 EXE 版本联动，保护手工链接、拒绝自动降级、失败回退。
- 执行方案、分阶段任务与可量化验收见 [v1.0.0 更新方案](docs/V1_0_0_PLAN.md)
- 50k+ 大图库端到端性能验收：图库、关键词、CJK、Visual DNA、Reference、Remix
- Backup / Restore 灾难恢复验收与故障注入矩阵
- 稳定 Migration 兼容矩阵，覆盖支持的历史数据库版本 → 当前版本
- 对启动恢复、Sidecar、来源同步和 AI 派生数据做长期一致性验收
- 为 v1.0 代码签名 / 自动更新前建立可量化发布门槛
- 50k 原生 SQL/512 维合成扫描已进入建议预算；75k 扩展英文/引号查询及尾延迟波动保留 WARN，不放宽预算。

## v1.0.0 — Stable Local Library
- 以 v0.26 稳定性验收 → 可信分发 beta → RC 原生升级验收 → 正式发布分阶段推进；见 [实施与退出标准](docs/V1_0_0_PLAN.md)
- Windows 代码签名与自动更新
- 大图库 50k+ 性能验收
- 正式 Backup / Restore 灾难恢复验收
- 稳定迁移兼容性测试矩阵
