# ImageLore v1.0.0 更新方案

编制日期：2026-09-30；执行更新：2026-10-01（Asia/Shanghai）

事实基线：`main` / `9b93bbbabab6ee8795fd6f0b0406acf7dfddccb4` / 应用 `0.25.1` / 数据库 schema `11`。

本轮接手与验证记录：[HANDOFF_BASELINE_2026-09-30.md](HANDOFF_BASELINE_2026-09-30.md)。

执行更新：后续指令已授权版本化桌面快捷方式，并按 ROADMAP 持续推进至 `v1.0.0`。当前开发版本为 `0.26.0`（schema 仍为 11）。A1–A4/A6/A7 的实现与回归见[存储验收](V0_26_STORAGE_ACCEPTANCE.md)、[工作流验收](V0_26_WORKFLOW_ACCEPTANCE.md)；A5 文件库原生测量见[规模记录](V0_26_NATIVE_SCALE.md)，实际桌面程序、保存重启、恢复和性能边界见[桌面验收](V0_26_DESKTOP_ACCEPTANCE.md)，后续全库校验、关闭安全与内存隔离复验见[性能跟进](V0_26_PERFORMANCE_FOLLOWUP.md)。本计划的原始接手基线保留供追溯。阶段 A 的完整退出标准仍未全部关闭，阶段 B 的证书、签名服务及可访问发行渠道尚未配置。

## 1. 版本目标与范围

v1.0.0 的目标是让已有的本地图片生成记忆库能够长期、安全、可维护地使用。沿用 ROADMAP 的 Stable Local Library 定位，以稳定性验收、可恢复性和可信 Windows 分发为主线。

Library、Metadata、Generation Explorer、Semantic Recall、Visual DNA、Image to Prompt、Remix、Reference、Sidecar 和 Lineage 已有实现。本次规划不把这些能力重复列为待开发功能，也不重建测试框架、重写大模块或整体替换界面。保留简体中文与现有 Frutiger Aero 设计，优先改善错误提示、等待反馈和真实工作流的连续性。

接手轮次保持 `VERSION=0.25.1`，后续稳定性迭代已用现有版本脚本同步 `0.26.0`。用户的桌面快捷方式指令需要一个可运行的本地 EXE，因此执行无安装器构建及本地验证。NSIS / GitHub Release 与可信自动更新仍按后续阶段的明确发布需求及退出标准执行。规划中的版本号与工作量不代表已正式发布。

## 2. 接手基线的证据与缺口

下表保留接手时的能力盘点；后续实现和验收进度以独立验收记录为准，不能把原始缺口当作当前仍未修复的清单。真实 CLIP 的三图集成、可信文件与向量独立复核已完成，见[模型验收](V0_26_MODEL_ACCEPTANCE.md)。

| 领域 | 当前已有能力与证据 | v1.0 前仍需补齐 |
| --- | --- | --- |
| 前端与契约 | `npm run check`；85 个 frontend / registered / mock command；Node Preview / Remix / Extension 测试 | 实际切图、编辑保存、关闭、异步失败与跨图状态一致性 |
| Native | fmt / locked check / clippy / library tests，沿用现有 Rust 测试 | 文件数据库上的真实恢复、完整历史 schema 和故障注入矩阵 |
| 搜索规模 | `scripts/test_scale.py` 在内存 SQLite 上构造 50k 记录；语义 smoke 为 16 维模拟向量 | Rust 实际搜索、512 维扫描、模型初始化、IPC、渲染与图片解码的端到端测量 |
| Backup / Restore | WAL checkpoint + VACUUM snapshot、pending restore、启动恢复和原件保留 | 合格 ImageLore 备份身份、外键与业务关系一致性、失败时可回退、长期运行备份策略 |
| Migration | schema 11；多个历史版本的增量迁移单测 | 各 schema 的真实旧版 fixture、迁移中断 / 重跑 / 数据等价验收；未来版本安全拒绝 |
| Windows 升级 | 稳定 identifier、currentUser NSIS、禁止安装器降级、0.19.0 覆盖升级脚本 | 自动更新路径、更新前保存与备份、真实安装版本 / 数据回读、失败恢复 |
| 签名与更新 | 当前 `createUpdaterArtifacts=false`，无 updater plugin / pubkey / endpoint | Windows Authenticode、Tauri updater 签名、渠道与密钥生命周期、更新状态界面 |
| 发布流程 | 快速 CI / Native CI 与 installer workflow 分离 | 安装包构建前绑定同一 SHA 的完整检查；安装包验证成功后发布更新 manifest |

关键代码入口：

- 搜索：`src-tauri/src/search.rs` 的 `library_page`；语义模型与排序：`src-tauri/src/semantic.rs`。
- 数据：`src-tauri/src/migrations.rs`、`db.rs`、`backup.rs`、`lib.rs` 的启动路径。
- 用户流程：`src/hooks/useAssetContext.ts`、`useEditorDraft.ts`、`useCloseGuard.ts`、`useVisionWorkflow.ts` 和 `src/App.tsx`。
- 发布：`.github/workflows/ci.yml`、`native-ci.yml`、`windows-release.yml`、`src-tauri/tauri.conf.json`。

接手时定位的 raw 保全错误被忽略、来源失败摘要不足、Sidecar 直接写入、陈旧 embedding 和仅启动清理缓存等风险，已分别在 A2/A7/A5 修复并加入回归。真实桌面连续操作又复现了切图取消分页的竞态，随后分离图库与选图序号。自动备份重校验移入阻塞任务池后，进一步用独立备份锁将它与活动图库连接分离；小图 Fit/full 共享不缩小的资源，源 mtime 纳秒精度修复同秒替换的旧预览。各项最终验收范围与未关闭的性能门槛仍以独立记录为准。

## 3. 实施顺序

| 阶段 | 建议版本 | 核心交付 | 进入下一阶段的条件 | 初步工作量 |
| --- | --- | --- | --- | --- |
| 接手 | 0.25.1 保持 | 同步 main、完整基线、修复已证实的小范围问题、形成任务表 | 本地门禁通过，遗留风险有明确记录 | 本轮 |
| A：数据与规模验收 | 0.26.0 | 恢复安全、历史 schema 矩阵、50k+ 原生测量、关键用户流程 | 无未解决的数据损失问题；性能基线与测试结果可复现 | 6–10 个工作日 |
| B：可信分发 | 0.27.0 / beta | Authenticode、updater、更新界面与渠道、发布工作流门禁 | 密钥/证书/渠道已就绪；签名与失败路径验收通过 | 4–7 个工作日，不含证书等待 |
| C：发布候选 | 1.0.0-rc.1 | 升级 / 灾难恢复 / 长时间运行 / 离线验收，用户说明与回滚演练 | Windows 原生候选验收通过；发布证据完整 | 3–5 个工作日 |
| D：正式发布 | 1.0.0 | 同一提交的签名安装包、校验和、更新 manifest、发布说明 | RC 问题关闭；按已授权的持续更新任务执行发布 | 1–2 个工作日 |

预计总工程量约 14–24 个工作日，须在阶段 A 的原生测量后重估；证书采购、发布账号和渠道权限的等待不计入。数据验收与性能测量可以并行；自动更新必须等恢复保护与更新前保存策略稳定后再开放。

## 4. 阶段 A 的任务与验收

| ID / 优先级 | 任务与实现边界 | 验收证据 |
| --- | --- | --- |
| A1 / P0 | 自动启动恢复仅处理已确认的数据库损坏；未来 schema、锁、权限、迁移代码错误应停止并提示 | 临时文件库：高版本库 + 旧备份不发生替换；锁错误不恢复；损坏库仍恢复并保留原件 |
| A2 / P0 | 验证备份属于受支持的 ImageLore schema；恢复前后检查 integrity / foreign keys / 关键关系，遇错误保持活动库及候选原件 | 非 ImageLore SQLite、未来 schema、截断备份、最新坏/较旧好、只读/锁/磁盘失败用例；失败后重启回读 |
| A3 / P0 | 完整历史迁移矩阵；每个 schema fixture 来源固定到历史 commit，迁移重跑与中断后恢复可验证 | schema 1–11 → 11；各版本核心数据计数、portable ID、Prompt/Revision、标签、集合、关系、Session、DNA、Reference/Remix 语义等价 |
| A4 / P1 | 明确自动备份语义：当前为启动时判断最新备份 mtime 是否超过 24h；补长期开机定时/重新聚焦检查并复用同一备份入口 | 可控时间测试、重复触发去重、失败提示与重试、保留 10 份策略，手动备份不被并发覆盖 |
| A5 / P1 | 合成图库原生性能基线；沿用 Rust 与 Python 入口，测量后才优化热点 | file-backed WAL + Rust 真实分页/搜索/512维语义路径 + Windows 原生操作记录，50k 必测、75k 扩展 |
| A6 / P1 | Preview / Reference / Remix / 编辑保存与关闭的异步一致性 | 先复现再补现有 Node/Rust 回归；快速切图、迟到请求、失败、保存Revision、关闭flush和取消后的重启状态 |
| A7 / P1 | 长期数据一致性与边界 | Inbox重复导入合并来源；来源目录重复同步/取消；多参考Remix结果谱系；Sidecar跨库往返；派生索引清空重建前后命中等价 |

A1 与 Reference 切图回归首先在接手轮次完成。后续 A2–A7 已开展实现与回归，结果以上方独立记录为准；不能用局部自动测试代替真实模型、桌面长时间操作及安装器升级的整项验收。

历史 fixture 候选来源：schema 1 `c2a06a5`；3 `0bfe4a3`；4 `6a60ba0`；5 `d3d74f8`；6 `60b7353`；7 `2395967`；8 `4b2dcb5`；9 `fdadeec`；10 `c2720fb`；11 `e93857c`。建立 fixture 时记录完整 SHA、生成命令与数据摘要。schema 2 应按历史 `1299f7f` 的实际初始化/迁移流程生成：该版本 `schema.sql` 的静态标记仍是 1，不能只修改标记伪造版本 2。未来 schema 必须安全拒绝，不尝试降级迁移。

测试只使用临时目录、合成图片、合成元数据和独立测试账户/VM。桌面合成验收使用受标记保护的 `IMAGELORE_TEST_DATA_DIR` 和独立 WebView2 profile，不打开正常用户资料库；默认用户路径及旧版迁移行为保持原有语义。

### 4.1 恢复矩阵

1. 正常 WAL 中有未 checkpoint 的已提交写入：备份 → 改写 → stage → 重启应用恢复 → 回读。
2. 当前库损坏：选择最近可验证的受支持备份；损坏 DB/WAL/SHM 的保留失败应中止替换并提示。
3. 最新备份损坏：跳过，选择较旧可用备份；全坏或缺失：安全停止。
4. 未来 schema、非 ImageLore 数据库、权限不足、忙/锁：保留原库，不静默改成旧数据。
5. 恢复暂存/复制/rename/磁盘空间不足/进程中断：下一次启动仍有可用原件，且不把不完整文件当成功库。
6. 原图路径缺失：元数据与谱系保留，显示缺失并支持重定位；重建缓存和搜索索引不改变手工数据。

数据库 Backup 保存元数据与关系。图片字节仍在原路径，丢失原图需要用户另行备份；模型与缓存也不能被声称已经包含在 SQLite snapshot 中。文档与恢复界面应解释恢复点时间和可能失去的备份后编辑。

### 4.2 性能测量

固定 Windows 版本、CPU/RAM/SSD、WebView2、构建模式、视口、图片格式/大小分布、库大小、schema、模型revision与向量维度。真实模型下载与首次初始化单独计时；模型已可信缓存时仍保留冷/暖初始化测量。

- 文件 SQLite：50k / 75k 记录，512维合成向量、固定seed和已知查询答案；加入重复、缺失图片、长Prompt、多语言、DNA-only、Reference-only记录。真实模型编码、吞吐与质量按独立模型矩阵验证，三图集成不能替代规模验收。
- 分页：首屏、OFFSET 4800 / 24000 / 48000，包含实际总数查询、排序、组合筛选与返回字段。
- 关键词：英文、1/2/3+字CJK、混合多词、双引号、零结果、DNA/Reference专属命中。
- 语义：文本/相似图，冷模型、暖模型、索引缺失/取消、结构化过滤，正确结果与遗漏检查。
- UI：10分钟连续滚动、Fit/100%与快速切图、缓存命中/失效、导入进度/取消、保存/关闭、重新启动。
- 工作流：浏览器采集 → Inbox重复来源合并 → 多图Remix → 导入结果/谱系 → Sidecar往返 → 重启回读。

冷场景至少3次；暖场景至少20次，保存原始样本、中位数、p95、最大值与错误数。分项记录 SQL/锁等待、模型加载/扫描、IPC、渲染/解码、进程总内存。当前 Native 语义路径读取全候选并排序且存在100,000候选上限；须验证上限是否会遗漏所支持容量内的记录，再决定限制提示或算法修改，不先承诺无上限。

以下为待基线校准的建议预算，均非已测承诺：

| 场景 | 建议预算 |
| --- | --- |
| 模型不参与的冷启动首屏 | ≤3s |
| 暖分页/关键词/3+字CJK | p95 ≤500ms |
| 1–2字CJK | p95 ≤1s |
| 暖语义检索 | p95 ≤3s |
| 缓存预览 / 冷2200px预览 | p95 ≤300ms / ≤1.5s |
| 连续滚动10分钟 | 无持续内存增长；总进程内存增长建议 <20% |
| 取消 | 可中断阶段通常 ≤2s；单文件处理或模型下载不可抢占阶段单列 |

记录大于200ms的UI阻塞并定位原因。不能把 SQL-only smoke 结果写成端到端体验结论；浏览器 Mock 也不能代替 Native 读写验收。

## 5. 阶段 B：签名与自动更新

### 5.1 两种签名分别实现

- Windows Authenticode：签应用EXE与NSIS安装器，验证发行者、签名链及可信时间戳。需要可用代码签名证书或受支持签名服务；不能承诺签名后立即获得SmartScreen信誉。
- Tauri updater：独立签名密钥，客户端内置公钥，发行包配套签名。验签证明更新包来自发行者；不能替代Windows代码签名。私钥只进入受保护发布环境，禁止提交、打日志或放进客户端。

实施前确定签名服务/证书、发布账号、Secrets、密钥备份与轮换责任。密钥轮换要设计旧客户端到新公钥的升级桥接；丢失密钥不能通过关闭验签解决。

官方依据：[Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/)、[Tauri updater](https://v2.tauri.app/plugin/updater/)。实施时再次核对锁定依赖与当时文档。

### 5.2 推荐渠道与用户流程

源码仓库当前为私有仓库。推荐独立的发行仓库或HTTPS静态分发渠道，托管 updater manifest 与签名产物；在选定渠道前验证普通已安装客户端可以下载。不能假定私有GitHub Release可匿名访问，不能把共享仓库Token写进应用。

沿用NSIS currentUser与稳定identifier；接入Tauri updater和最小所需能力。设置页提供当前版本、检查更新、更新说明、下载进度/取消、错误原因与重试。启动检查不阻塞本地资料库，断网仍可使用；稳定渠道默认不升级到预发布版。安装更新前复用编辑flush和任务协调，创建并验证备份，得到用户安装意图后关闭/重启。

更新验收：无更新、合法新版本、网络超时/断网、坏JSON、不支持平台、签名不符、包损坏、空间不足、下载取消、安装失败、未保存编辑、活动导入/索引、未来schema、更新后重启回读。全程不放宽验签、不扩大文件协议权限。

v0.25.1尚无updater，因此从该版本首次进入带updater的版本需要手动安装；只有完成这次过渡安装的客户端才进入自动更新路径。

### 5.3 发布顺序

1. 固定目标SHA，验证 VERSION/tag/lockfile；检查前端与Native门禁是该SHA的成功结果，或在同一release workflow中执行完整门禁。
2. 在明确授权的打包任务中生成NSIS与updater产物，完成两种签名及时间戳校验。
3. 在隔离Windows环境执行0.19.0/0.25.1→首个带updater候选的手动覆盖安装；自动更新验收从首个带updater版本或后续稳定版本→新候选开始。
4. 读取真实程序版本和同一组哨兵业务记录，检查唯一安装项、快捷方式、备份/模型保留、未创建第二数据目录。
5. 上传不可变版本产物、签名和校验和；验证下载链接与内容；最后更新manifest，避免客户端发现尚未就绪的包。
6. 保存SHA、workflow链接、artifact摘要与原生验收记录；更新失败时撤回manifest入口并保留坏包证据。

代码回滚与数据回滚分开处理：已迁移数据库不能由旧程序强行打开。优先发行修复版；需要恢复备份时说明备份之后的编辑影响，保留当前库原件并由用户选择。禁止为回滚将 `allowDowngrades` 改为 `true` 或绕过数据库未来版本保护。

## 6. RC 与 v1.0.0 退出标准

全部满足才允许宣称v1.0.0验收完成：

- 原有六项门禁全部通过，已有测试不删除，Rust保持 `--locked` 与Clippy `-D warnings`。
- Command四处一致：frontend API、Rust command、handler注册、browser mock；现有85为本轮基线，新功能若增加命令须同步变更。
- schema矩阵、备份/恢复故障矩阵及关键用户流程无未解决的数据损失或跨图覆盖问题。
- 50k原生性能达经校准预算；75k与长时间运行的范围/限制明确披露。
- 签名与更新失败矩阵通过，隔离Windows上的真实升级/重启/数据回读有证据。
- 离线使用、模型下载失败、Vision配置缺失、来源图片失效都给出可恢复提示；API Key不进入数据库、Sidecar或日志。
- README / ARCHITECTURE / WINDOWS_INSTALLER / CHANGELOG与实际版本、渠道和恢复边界一致。
- 同一SHA的门禁、产物、发布版本与更新manifest可追溯；回滚演练完成。

最低开发门禁继续使用：

```bash
npm run check
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
git diff --check
```

## 7. 下一批工作与外部依赖

已依次执行 A2/A3 的存储与历史迁移保护，并并行完成 A5 测量和 A6/A7 一致性修复。真实 CLIP 下载、可信校验、初始化、三图编码及文本/相似检索已完成小样本集成验收，首次等待与取消仍有明确风险。下一批继续关闭阶段 A 的启动/恢复性能、WebView 稳态内存、真实模型大规模吞吐/召回质量、复杂图片分布、导入/索引取消和长时间/离线验收。阶段 A 未通过退出标准前，不把 `0.27.0` 或 `1.0.0` 标为已完成。每个提交只对应一个目的：先回归、再最小修复、再相关门禁；验证报告区分单元测试、原生操作、安装器升级和正式发布。

10 月 1 日桌面稳定路径的 `f61f59b` EXE 已完成 300 轮/10 分钟原生操作、保存关窗/重启/恢复和三图模型复验；稳定就绪模式切换 20 次 p95 47.09 ms。后续 `cd220b2` 候选保留全部校验并加大临时连接缓存，完整校验耗时下降，但交错启动 FCP 中位数 3.13 秒仍未通过建议 3 秒，尚未替换稳定桌面程序。恢复此前为 84.37 秒，须单独复测；WebView 历史 Private Bytes 末 +25.24%、后半仍增长，短闲置窗口不能关闭门槛，继续分别隔离固定图切模式、新图预览、仅滚动缩略图和闲置，再检查复杂大图。每次先保存原始窗口/样本与身份，避免仅以百分比、画面已解码或旧源 CI 声称门槛关闭。

模型准备的可中断网络等待、终态监听、旧查询状态竞态及 100001 项静默遗漏已完成源码修复与回归，见[取消与状态记录](V0_26_MODEL_CANCELLATION.md)。新产物的原生模型、规模和复杂图片验收仍须单独执行，不沿用旧三图 EXE 结论代替。

阶段B前须落实：代码签名证书/服务与费用、发行渠道的访问权限、updater私钥保管、候选Windows测试环境。隔离 GitHub Windows runner 的旧版覆盖升级已通过，但签名及 updater 尚无证据。当前不需要为这些依赖暂停阶段A，也不在缺少它们时生成“已签名/已自动更新”的结论。
