# ImageLore Roadmap

## 当前进度（2026-10-23）

安全依赖升级候选 `4d8451f` 的优化完整库回归实际 **236/0/3（157.19 秒）**，严格 check/Clippy（含测试）、前端检查与构建通过；SQLite bundled 头文件为 3.53.2，实际 Windows 构建选中新依赖，动态版本与桌面性能另验。改动及剩余公告范围见[依赖安全回归](docs/V0_26_DEPENDENCY_SECURITY.md)。开发版本仍为 `0.26.0`，阶段 A 未关闭，运行时尚未推广。

固定 ABBA×10 的预处理备份对照已完成 **40 正式样本、每方案 20 次**，另排除两个预热；168 份终态通过，十三原始源文件保全。重操作耗时较低，但启动后端 B p95 **2.934 秒**、A **2.234 秒**，十组中有八组 B 启动中位数更高，两次控制器停止及跨夜续跑全部保留。详见[全部样本与边界](docs/V0_26_PRECONDITIONED_BACKUP_SERIES.md)。两路候选不据此推广，GUI、完整应用冷暖和长期内存仍按原阶段验收。

MIT 已公开采用；免费签名资格和不在 GitHub Secrets 存放任何 SignPath token 的接入方式仍未批准或确认。继续准备当前源码桌面候选与材料，不将服务商回信记为阶段 B 完成。

### 前一轮探索性记录（2026-10-22）

两路候选 `8ff2891` 的本地及同提交远端 Native 均实际 244/0/3，严格 Clippy/check、快速 CI 和敏感信息门禁通过。优化构建六操作已完成 A/B 后补 B/A 的四次对照，十六份终态与十三源文件保全全部通过；手动串行 81.992–92.928 秒、两路 44.093–52.974 秒，应用恢复串行 108.931–111.942 秒、两路 56.624–63.806 秒。每方案仅两次，近期备份检查仍有波动，详见[当前两路校验对照](docs/V0_26_CURRENT_BACKUP_WORKER_COMPARISON.md)；正式样本与性能门槛继续验证。候选未推广，版本与桌面保持 `0.26.0`。

版本保持 `0.26.0`。首轮 optimized 串行六操作诊断实际 **1/0/0（452.92 秒）**，50k/初始与最终各 10 份有效备份，typed/重开与十三源文件保全通过；手动 **81.99 秒**、到期 **87.65 秒**、应用恢复 **111.94 秒**，rotation 占 **95.96% / 94.15% / 78.53%**，见[实测基线与热点](docs/V0_26_CURRENT_BACKUP_DIAGNOSTIC.md)。两路候选的当前源码整合、完整回归及四次交错观测见上方，不借旧 debug 数字宣布收益。该对照不代表正式性能预算、GUI 或阶段 A 完成。

版本保持 `0.26.0`。当前 `1da1371` optimized Rust 文件库查询/排序完成 50k 必测与 75k 扩展：各 19 场景、每场景 3 连接冷 + 20 warm，共 **38 场景 / 874 样本**，实际 **1/0/0**（160.98 秒）、**0 budget WARN**，见[本轮原生规模查询](docs/V0_26_CURRENT_NATIVE_SCALE.md)。同 SHA 候选 Native 实际 **236/0/3**（993.96 秒）、快速 CI 与敏感门禁成功，main 已正常快进同步。真实编码、IPC/GUI、OS 冷启动、长期内存与可信发布继续按原阶段验收，不将该后端结果写成整个阶段 A 完成。

版本保持 `0.26.0`。4.1-6 缺图/重定位后端闭环已补：真实两 PNG 导入与移动、missing 刷新、缓存/FTS/CJK 重建、drop/reopen、fingerprint 重定位及再次重开/幂等，核对十二表 typed 数据、完整资产记录和双向谱系。首次 `4030249 + commands.rs` 定向实际 **1/0/0**（0.15 秒）、67 项输入相同；整合至 `82c187f` 后完整库回归实际 **236 passed / 0 failed / 3 原有 ignored**（248.98 秒），68 项输入相同，严格 Clippy 通过。身份及 GUI/新进程边界见[缺图与重定位验收](docs/V0_26_MISSING_RELOCATION_ACCEPTANCE.md)。

### 前一批故障补充记录（68 项输入）

版本保持 `0.26.0`。审计定位的备份创建 FULL、当前 Probe 复制、真实 rename 80/183 与 readonly 拒绝/恢复补充验证实际 **5/0/0**（24.40 秒），覆盖 16 个明确故障场景；完整库回归实际 **235 passed / 0 failed / 3 原有 ignored**（223.37 秒），68 项编译/include 输入前后相同，严格 Clippy 通过。A1 核心后端策略已有证明，A2 本次具体路径已补定向和完整回归，后续 4.1-6 后端闭环见首部。测试进程内正式入口与事务核心接点不计 GUI/IPC 或安装验收，产物与源码身份见[故障补充记录](docs/V0_26_BACKUP_FAULT_COMPLETION.md)。

前一批 `4030249` 的[候选 Native](https://github.com/AureliusWu/ImageLore/actions/runs/39712538000)实际 **230/0/3**（901.46 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39712540748)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39712357271)成功并正常快进 main；本批新增故障路径单独验证。两路 worker 候选的本地 233 项和六操作 debug 诊断见[固定提交记录](https://github.com/AureliusWu/ImageLore/blob/0a87dd8ddfce3022df4122576f616b5bbbbaf503/docs/V0_26_BACKUP_GUARDED_WORKER_DIAGNOSTIC.md)，未入 main、未完成正式性能。

### 前一批空间故障记录（67 项输入）

版本保持 `0.26.0`，阶段 A 仍未全部退出。正式恢复入口的空间故障定向门禁实际 **5/0/0**（27.22 秒），覆盖三用途的 18 个故障场景及一个作用域控制；完整库测试实际 **230 passed / 0 failed / 3 原有 ignored**（203.51 秒），67 项编译/include 输入前后相同，严格 Clippy 通过。Windows 112 属于真实部分写入/复制和同步后的受控注入；SQLite FULL 来自本案 8 KiB 有界 memdb 的实际 VACUUM。截断 Commit 持续拒绝并保留完整现场，其余已覆盖故障在撤销注入后正式入口重试；同进程后端重试不计桌面或新进程启动验收。产物复用及源码身份详见[空间故障验收](docs/V0_26_SPACE_FAILURE_ACCEPTANCE.md)。

此前 `c19a2f6` 的 [main Native 复跑](https://github.com/AureliusWu/ImageLore/actions/runs/39397136749)实际 **224/0/3**（1030.72 秒），只覆盖原 13 案源码。后续 31 案 Role 候选 `9284eaf` 的 [Native](https://github.com/AureliusWu/ImageLore/actions/runs/39711150959)实际 **225/0/3**（850.93 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39711213364)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39399396084)成功，不与其他源码混记。性能、桌面、长期内存、安装升级、可信签名、updater 和 RC 按既有计划继续验收。

### 2026-10-20 Role 候选与引擎探针记录

开发与已验收桌面版本仍为 `0.26.0`，阶段 A 未全部退出。本地 Role 候选完成 **31 个真实 Windows 后端进程强杀场景**（应用 pending 11、暂存 pending 10、自动损坏恢复 10），覆盖 DB/WAL 部分移出、部分归位、提交与部分清理，并核对完整原件/raw、业务、正常重试及第二次启动。完整库测试 **225 passed / 0 failed / 3 原有 ignored**（200.57 秒），65 项编译/include 输入前后相同，严格 Clippy 通过，见[存储验收](docs/V0_26_STORAGE_ACCEPTANCE.md)和[pending 暂存安全](docs/V0_26_PENDING_STAGE_SAFETY.md)。

此前 `c19a2f6` 的[候选 Native](https://github.com/AureliusWu/ImageLore/actions/runs/39395025528)实际 **224/0/3**（709.34 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39395033317)和[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39394917151)成功后正常快进 main。该远端结果绑定此前 13 案源码，与新增 31 案的本地结果分开登记。

独立引擎探针已证明有界 memdb 的真实 `SQLITE_FULL / 13`、正常容量控制及源数据保全（1/0/0）；该次尚缺的正式 ImageLore 空间故障定向验证已于 10 月 22 日补充，范围见首部。按原计划验证错误时保全及可恢复行为，可采用受控注入，不追加物理主机断电或耗尽主机磁盘门槛。真实桌面、50k/75k 与冷暖性能、长期内存、安装升级、可信签名、updater 和 RC 继续按既有阶段验收。

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
- 10 月 20 日较早的[Windows 恢复事务](docs/V0_26_RESTORE_TRANSACTION.md)整合完成 193 项本地回归及严格检查；后续 pending-stage 所有权、防重放和真实进程强杀已补充至首部 31 案。193/221/224 项及旧 RED 分别保留来源，A1/A2 的剩余故障、性能和桌面验收继续开放。
- 10 月 18 日按用户要求暂缓桌面验收，继续离线恢复回归和安全同步：最新备份临时不可用时停止替换，解锁后重试最新备份；Windows 权限辅助测试保留原 ACL 精确恢复校验。结果与验证范围见[存储验收](docs/V0_26_STORAGE_ACCEPTANCE.md)。
- 同日补充备份清单 IO 错误传播修复：受控枚举/mtime 错误下的四项真实文件保全回归已完成 RED/GREEN 及同目录重试，完整本地与同源码候选远端 Native 均为 157 项通过，快速 CI 和敏感信息门禁通过，见[存储验收](docs/V0_26_STORAGE_ACCEPTANCE.md)。此项不替代桌面或正式恢复复制/空间不足等故障保全用例。
- 10 月 18 日历史回归实际复现最终写入检查后修复的活动库仍被旧备份覆盖；失败源码与现场继续保留。后续跨替换阶段保护、冲突保全和进程中断结果见首部，不把旧待实现状态延续到当前候选；阶段 A 仍按完整退出标准推进。
- 10 月 15 日[离线恢复补充验证](docs/V0_26_STORAGE_ACCEPTANCE.md)修复已提交恢复遇到临时校验锁时丢失提交标记的问题，并完成真实 Windows 权限拒绝/恢复后重试及加强后的 schema 1–11 文件库矩阵；完整 Rust 测试 152 项通过、3 项原有忽略。桌面、性能及正式发布门槛仍待完成。
- 五组10分钟[内存隔离窗口](docs/V0_26_MEMORY_ISOLATION.md)已完成；连续换图与缩略图滚动仍超过20%预算，保留WARN。坏语义向量的[严格读取与增量重建](docs/V0_26_VECTOR_INTEGRITY.md)已修复，源码CI通过。
- `2276de1`候选的[真实模型矩阵](docs/V0_26_MODEL_MATRIX.md)已完成300图故障恢复、编码/下载取消、编码中关闭及缓存模式下重启读回；300图与5k真实编码均完成各20次热查询和完整持久闭环，p95通过3秒预算。32张冻结自然图的质量记录来自`34baa87`，中文召回有明确局限。50k及同场景冷测尚待验收，不计为全规模通过。
- 50k独立副本的[启动、备份及恢复分阶段测量](docs/V0_26_BACKUP_PHASES.md)已通过，原DB/WAL/SHM保持；备份轮换的逐份完整校验是已测热点，后续优化保留全部校验。
- 10月5日[两路备份校验对照](docs/V0_26_BACKUP_ROTATION_PAIR.md)完成3轮ABBA、12次探索性实测；完整校验和原件保全通过，生产整合、正式性能样本与真实备份/恢复仍待验收。
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
