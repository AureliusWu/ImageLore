# v0.26 存储与恢复验收

日期：2026-09-30；执行更新：2026-10-20（Asia/Shanghai）。范围：A1 启动保护、A2 备份/恢复、A3 历史迁移、A4 自动备份后端。全部故障库与哨兵数据位于独立临时目录，未打开用户正常资料库。

## 2026-10-20 当前进度

本地 Role 候选完成 **31 个真实 Windows 子进程强制终止场景**：应用 pending 11、暂存 pending 10、自动损坏恢复 10。覆盖持久记录、DB/WAL 部分移出与部分归位、安装、提交及部分清理；父进程在确认子进程存活和实际占锁后强杀，31 次退出与逐案 PASS 保存在 `conflicts-roles-20261020-01` 原始输出中（67.82 秒）。原件、候选和 raw 使用真实含未 checkpoint 提交的 DB/WAL/SHM；自动恢复原件为 schema 11 的单叶额外索引损伤，以专用副本同时证明 integrity 非 ok 和业务仍可读，提交后的健康库仍通过完整验证。详见 [pending 暂存与进程终止记录](V0_26_PENDING_STAGE_SAFETY.md)。

本轮完整 library suite `full-roles-20261020-01` 实际 **225 passed / 0 failed / 3 原有 ignored**（200.57 秒），进程退出码 0，65 项编译/include 输入前后逐项相同。完整套件沿用该 Role 候选已编译的同一测试 EXE，SHA256 为 `ef57576853bbaf8047b289ec3d24d162b8ab10a416435c0eaeb3fd9189aee6cc`，不声称又编译一次；包含测试代码的严格 Clippy `clippy-roles-20261020-02` 通过。原始 stdout、terminal、源码绑定和独立测试产物留在仓库外 `<private-evidence>/restore-role-interruption-20261020-01`。

远端证据单独绑定此前源码 `c19a2f6`：[候选 Native](https://github.com/AureliusWu/ImageLore/actions/runs/39395025528)实际 **224 passed / 0 failed / 3 ignored**（709.34 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39395033317)和[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39394917151)成功，之后正常快进 main。该远端结果覆盖此前 13 案源码，不能转记为新增 31 案候选已通过远端。

另有独立 SQLite 引擎探针 `probe-01` 实际 **1 passed / 0 failed / 0 ignored**（0.01 秒）：SQLite 3.46.0、`USE_URI=1`，8 KiB 有界 memdb 的 VACUUM 实际返回 `DiskFull / 13`，独立 8 MiB 正常控制成功且源数据未变。它是仓库外的引擎容量探针，不是 ImageLore 恢复用例；原始回执保存在 `<private-evidence>/sqlite-full-probe-20261020-01`。

A1/A2 整项仍未关闭。正式恢复中的新副本探针与空间不足故障保全用例仍待补齐，可在独立文件库的受控 IO/SQLite 边界注入并核对实际文件副作用与同目录重试。原计划要求这些失败时保全和可恢复行为，不要求物理主机断电或耗尽主机磁盘，不把后二者追加为发布门槛。真实桌面重启、50k/75k 与长期内存、安装升级、签名、updater 和 RC 仍按原计划各自验收；本轮没有运行 GUI、推广应用 EXE 或更改桌面快捷方式。

## 2026-10-20 较早 Windows 恢复事务记录（193→221→224 项）

该日较早的 pending 暂存修复完成本地 **221 passed / 0 failed / 3 原有 ignored**（116.88 秒），64 项编译输入及实际新编译测试产物分别绑定，严格 Clippy 通过。暂存复用同一所有权事务，新增持久操作锁与旧 previous 接管记录；覆盖外来 pending、释放后的真实新提交、消费后防重放及混合事务保全。该批当时未包含真实进程强杀，后续 13 案和首部 31 案分别补充；下列 193 项及“三个 RED 待处理”段落保留为对应旧基线。

后续补充真实 Windows 后端子进程强制终止：应用/暂存两种事务的 13 个持久与部分清理边界均通过，实际 DB/WAL/SHM 保留未 checkpoint 业务提交，父进程核对存活/占锁后强杀并验证续作、raw、重试和第二次启动。完整库测试 **224 passed / 0 failed / 3 原有 ignored**（149.14 秒），65 项输入前后相同，严格 Clippy 通过；[完整范围](V0_26_PENDING_STAGE_SAFETY.md)保留各源码身份，不能转记为物理断电、桌面或安装验收。

Windows 恢复替换现已接入持续独占句柄、禁止覆盖的同句柄移动和持久身份记录。新协议使用 `restore.transaction`，不复用缺少对象身份的旧 `restore.rollback`。完整协议、旧格式处理和验证边界见[Windows 恢复事务](V0_26_RESTORE_TRANSACTION.md)；下文旧替换流程保留为历史记录，不能据此推断当前 Windows 的清理行为。

隔离候选实际新编译的完整 library suite 为 **193 passed / 0 failed / 3 原有 ignored**（89.67 秒），61 项构建输入在检查前后保持一致，封存测试 EXE SHA256 为 `a03d2b50d1b6b5444fb720ee7ac2060ca8b05866c1280fde270bfaacc042b898`。当前源码的 locked/offline check、包含测试代码的严格 Clippy `--lib --tests -- -D warnings`、fmt 和 diff 检查通过。原始输出、源码绑定、失败现场及测试 EXE 均在仓库外 `<private-evidence>/restore-guarded-transaction-20261018-01`；最终记录为 `full-full-20261020-05`、`clippy-clippy-20261020-05`、`check-check-20261020-04`。

先前四项真实文件丢失 RED 的冻结源码和产物继续保留。GREEN 区分两类结果：最终守卫取得前修复的健康库会阻止自动恢复；守卫取得后的原件修复、安装库写入及改名被实际拒绝，不能声称此时产生了新提交。Reserved 后真实创建的外来库或 WAL 则保留双方，并证明重复两次重启处理仍不改变全部活动/预留/暂存/pending/raw/记录文件的字节与 mtime。独立测试进程实际尝试 SQLite 打开、文件写入、改名和 raw 父目录改名均被拒绝；正常恢复结束后实际写入成功。

补充直接覆盖释放句柄后的间隙：在提交前失败与已写入提交记录两种状态下，真实 SQLite 对同一已安装文件提交新 marker 和资产，确认 File ID 不变、SHA 改变。正式续作与两次再次启动均拒绝删除或回退这些新编辑，活动库、pending、原件、raw 和记录保持。此前 `full-full-20261020-03` 的 192 项通过及独立产物继续作为中间结果保留。

首轮完整测试 **184 passed / 7 failed / 3 ignored** 单独保留，暴露属性访问目录句柄没有阻止改名、部分 UTF-16 长度下重命名目标错误及原件保全错误缺少上下文。修复加入真正参与共享检查的目录读取权限、显式字符串终止空间和八种中文目标长度回归；未通过放宽数据保全断言消除失败。

在上述 193 项基线时，旧 `finish_pending_stage` / `stage_restore_at` 的 pending/previous 入口尚需独立并发所有权保护，真实进程中断也尚未执行；后续暂存修复与 13/31 案见首部，不把这些旧缺口继续当作未实现功能。旧 `restore.rollback` 无身份记录时仍保留现场并要求人工核对。A1/A2 整体剩余故障、性能和桌面门槛保持；该批没有构建或推广应用 EXE，也没有更改当前桌面快捷方式或默认用户资料库。

源码提交 `7aacb97` 的[远端 Native](https://github.com/AureliusWu/ImageLore/actions/runs/39386940463)实际 **193 passed / 0 failed / 3 ignored**（496.45 秒）；[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39386940101)及[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39386881359)均成功，随后正常 fast-forward 同步 main。另一个隔离工作区已取得 pending-stage 三个实际丢失 RED（0/3，3.34 秒），分别证明旧 previous 复原和新候选安装可覆盖外来 pending，旧清理可删除同 File ID 新提交；[完整边界记录](V0_26_RESTORE_TRANSACTION.md)保留失败身份，继续阻止阶段 A 退出。

## 2026-10-18 离线补充验证

用户暂缓桌面验收期间，完成最新恢复候选临时不可用的文件库回归与修复。生产源码提交为 `525258bbbc71c1e2da382f034009a7d003aeec67`，`backup.rs` SHA256 为 `3cc0fc9460314565802d10a62ebdb5d9a639ec3ff25adc4273c6bb061a6e8dcc`。原始日志、RED/GREEN 源码、测试产物及终态回执保留在仓库外 `<private-evidence>/recovery-offline-20261018-01`。

- **最新候选临时被锁。** 两个独立合成目录各含损坏活动库及新旧两份有效备份；固定不同 mtime 并核对排序。真实 SQLite `BEGIN EXCLUSIVE` 使最新备份校验返回 `Unavailable`。旧实现跳过最新备份，在报错前用较旧库替换了活动库，新增字节保全断言实际失败，测试退出码 101。修复在任何替换之前保留原因并停止；活动库及两份候选的字节和 mtime 保持，没有恢复通知、raw/rollback/next 工件。释放锁后，同一目录成功恢复最新备份的业务哨兵，并保留损坏原件。独立解锁控制场景也成功选中最新备份。
- **明确无效仍可跳过。** 选择候选时保留 typed 分类，只有 `Rejected` 继续查找较旧候选。既有“最新损坏、较旧有效”和无有效候选测试保留并通过；临时不可用不能被当作损坏授权回退。
- **权限辅助进程的原 ACL 写回。** `005bc4e` 的[远端 Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/38709607553)为 151 passed / 1 failed / 3 ignored；仅显式标记原 Access 待写回后，`fd78810` 的[复验](https://github.com/AureliusWu/ImageLore/actions/runs/39100493061)仍为 152 passed / 1 failed / 3 ignored。本地成功不能覆盖这些失败。新增[快速诊断](https://github.com/AureliusWu/ImageLore/actions/runs/39101387788)确认远端 DACL 标志为 `4 → 1028`，仅 AutoInherited 位变化，DACL 字节、ACE 顺序与集合均相同；本地真实 AI=false 基线也复现同一严格失败。[Windows 自动继承模型](https://learn.microsoft.com/en-us/windows/win32/secauthz/automatic-propagation-of-inheritable-aces)会调整该标志，原件规则没有残留 deny。
- **保留严格校验的修复与确定性回归。** 保留 `Set-Acl` 常规恢复路径，仅原始 AutoInherited=false 且完整 Access SDDL 不匹配时，使用仅 DACL 的原生写回恢复旧继承状态；这个测试专用分支不写 owner/SACL。最终仍要求完整原 Access SDDL 逐字相同、恢复 ACK 和子进程正常退出，不通过忽略标志放宽断言。Fast CI 提前运行默认/明确 AI=false 两个真实权限用例；控制端另取完整 SDDL 作独立比较，并确认恢复后可写。旧 fixture 的确定性 Legacy 用例已实际 RED，新 fixture 两用例均 GREEN。

首次本地完整 Rust suite 为 **153 passed / 0 failed / 3 原有 ignored**，实际测试 48.55 秒；该次为新编译产物，EXE SHA256 为 `4721242aeddea038b055d25a19a924c687d0e1d3991bdbc66257e3d89708b246`。locked/offline check、严格 Clippy `-D warnings`、fmt、`npm run check`、前端构建和 diff 检查通过。测试前后五份源码/锁文件 SHA 相同，RED 与修复后产物分别封存。该次发生在上述继承控制标志修复之前，后续验证单独绑定；合成快捷方式及运行目录测试没有运行 fixture EXE。

继承状态修复后的完整本地复测仍为 **153 passed / 0 failed / 3 原有 ignored**，实际测试 49.94 秒，新编译测试 EXE SHA256 为 `2191a76fc6e4bf803b2ebd667d2b4f6baa7d93676459b52d8ecf9fedbf0e64e6`。该次 fixture SHA256 为 `333d07ccd393e026880ed65d9d95586234428c6988b4557acb8258bba936867f`，独立双场景 driver 为 `cdcc4fc75963beb30a7338a56eeef65c280edab1cf8c0b381d3ada8e10733fff`；测试前后源码不变，完整日志与产物另存为 `acl-fixed-*`。两种继承基线的实际拒绝、严格子进程/控制端回读、恢复后可写和自有目录清理均通过。

另在独立 worktree 将既有两路校验候选移植到 `525258b`，兼容候选为 `ef782ad750f587ab708f64306b8c63c1e79a3cb3`。四个原提交三方合并无冲突，完整测试 **160 passed / 0 failed / 3 原有 ignored**（59.24 秒），包含测试代码的严格 Clippy 通过；十一项关键安全函数/测试逐段核对保留。其测试 EXE SHA256 为 `ddcc235fa4745ef1f1767a7da1e2c7bd62b63f92ad00173005218f15e6e841dd`，与主线测试产物分开封存。两路实现仍未合入 main；此项兼容回归不替代真实 Native 备份/恢复对照或性能验收，旧冻结候选保持。

上一批修复同步至 `main / 48689a62631f85fb30a0a2de28ee681e918b5fe2` 后，[Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39102234537)、[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39102234647)及[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39102234595)均成功。该次实际远端 Rust 测试为 **153 passed / 0 failed / 3 原有 ignored**（332.67 秒），与此前失败及候选分支的结果分别保留；下述清单修复有独立验证身份。

### 备份清单 IO 错误传播

在独立合成目录的真实 SQLite 文件上，对目录逐项枚举和 mtime 读取边界分别注入受控 IO 错误，并调用正式自动恢复与轮转入口。四个独立无故障控制均通过；旧实现的四项保全回归为 **0 passed / 4 failed**，恢复实际替换了活动库，轮转实际删除了候选，原始失败现场保留。这里复现的是受控输入下的真实文件副作用，不是操作系统磁盘或目录枚举故障的现场复现。

最小修复逐项传播枚举错误及 mtime 读取错误，不再丢弃失败项或把读取失败当作时间 0；保留早于 Unix epoch 时间的既有回退、成功清单排序和十份有效备份策略。修复后四项 GREEN 为 **4 passed / 0 failed**：错误发生后全部原件字节、mtime 和目录名称保持，原因不丢失；移除注入后均在同一目录成功重试，并核对最新恢复点或正确保留的十份候选。

最终完整本地测试为 **157 passed / 0 failed / 3 原有 ignored**（71.48 秒），locked/offline check、包含测试代码的 strict Clippy `--lib --tests -- -D warnings`、fmt、diff 及敏感信息扫描通过。扩大 Clippy 范围时实际发现两条既有测试写法警告，等价改用布尔断言及 `is_multiple_of` 后复测，失败日志保持。最终 `backup.rs` SHA256 为 `d6081ffa387d18cfbe6cc170413ae4ab0a1c376cbe40eed09b57e2d2b81549aa`，新编译测试 EXE 为 `9fcd63ca1bba3bc93a12aa492aa24890b90abb566c0d5f0768b0aaab1de9fa96`；源码、RED/GREEN、完整日志和产物封存在仓库外 `<private-evidence>/backup-discovery-errors-20261018-01`。

该项同源码候选 `9f6da5bdaf10f025d1655a373ed8efdc74c99d95` 的[Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39103518879)完整 Rust 测试为 **157 passed / 0 failed / 3 原有 ignored**（405.95 秒），四个 discovery 用例实际通过；[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39103520348)及[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39103483976)均成功。快速 CI 中默认和 Legacy 两项真实 ACL 生命周期均通过。后续仅追加本节远端结果，不改变已验证源码；上述结果不计为桌面或灾难恢复整项验收通过。

### 两路候选与清单修复的隔离兼容验证

将清单错误传播修复与上述未关闭竞态记录移植到既有两路候选，得到独立 `82bb027bba3f4073105b0f79e4128263d014dfcc`；旧候选和旧证据保持不变。重新采集全部 32 份编译输入及 3 份 fixture/gate 输入和原字节快照，新编译完整本地测试为 **164 passed / 0 failed / 3 原有 ignored**，包含测试代码的严格 Clippy、check 通过。新测试 EXE SHA256 为 `19d18bfc61af4a3daf89208d80650be3156105095ca5d0803f3571107346d8c1`，`backup.rs` 为 `9bd34c3c07ebf635d076291cb10dbd04a209ee0a0fec766b7d2990c2422d8e49`。

同一新产物再实际执行两项 profile 导出，四份新报告由字节未变的固定消费者验证，得到 13 个 session、7 次完整 validator 调用；全部 35 份输入、二进制与报告身份保持。新回执保存在仓库外 `<private-evidence>/backup-worker-discovery-compat-20261018-01`，不继承旧产物或报告的运行结果。此候选仍未合入 main 或推广为桌面程序，未关闭真实 Native 对照、性能和下述并发恢复门槛。

### 2026-10-18 历史 RED：最终检查与替换之间的并发修复

以 `48689a6` 的恢复逻辑，在独立 worktree 增加仅测试用的最终写入检查后钩子。真实 schema 11 文件通过 `BEGIN IMMEDIATE`、版本查询并实际报告索引完整性损坏，独立普通恢复控制先成功；随后在检查结束、预留文件之前，将活动库修复为有效库，提交新的业务哨兵和第二项资产后关闭连接。实际恢复返回 `Ok(())`，业务哨兵却被旧备份覆盖，保全回归 **0 passed / 1 failed**，退出码 101。此结果证明受控调度下真实文件的数据丢失，不代表已启动第二个应用进程或完成真实桌面并发测试。

该旧基线的 Windows 独占句柄及 SQLite 事务均未跨越完整替换过程。仅再检查一次不能证明检查之后的安全；普通重命名可能替换新建目标，回滚也需保护来源不明的新活动库。此项当时是 A1/A2 未关闭的数据保全门槛，未计入当时通过项。失败源码、现场、测试 EXE 及回执继续保留在仓库外 `<private-evidence>/restore-final-check-race-20261018-01`；后续持续句柄、禁止覆盖移动、冲突回归和进程中断结果分别见本页首部及[恢复事务](V0_26_RESTORE_TRANSACTION.md)，不删除或改写这次真实失败，也不以其旧待办描述当前实现。

本节仅证明文件库后端及受控 Windows 权限行为，没有启动 ImageLore GUI，也未构建或推广桌面 EXE、安装包、签名或 updater。真实桌面/性能/安装升级与阶段 A 退出标准保持待验收；现有桌面运行时仍绑定其原构建来源。

## 2026-10-15 离线补充验证

基于 `main / 69299a4` 补充故障回归并修复已提交恢复的临时不可用分支。最终测试源码的 `backup.rs` SHA256 为 `52b4f97aa3c1d488feee6429cd85acf66f27f8f4a956bd96bd48490b824b7a8c`，`migrations.rs` 为 `734741faa4ea886f921b133c829ddd2c6491a85283f26784c352630dda4330bd`。原始日志、失败现场和测试 EXE 保留在仓库外 `<private-evidence>/backup-permission-recovery-20261015-01`，不上传 GitHub。

- **已提交恢复遇到真实锁。** 新用例先证明实际 SQLite `BEGIN EXCLUSIVE` 使已安装库校验返回 `Unavailable`。旧实现实际删除了 `COMMITTED`，新回归因此失败；原失败及现场保留。修复将临时锁、IO 等不可用错误与明确无效的安装分开处理，在删除 marker 或移动文件前停止，保留错误原因。`pending` / `recovery` 两种合法 marker 的 DB、pending、reserved 原件及 marker 字节和 mtime 均保持；解锁后再次执行，保留有效安装并按原语义清理。
- **缺失或已确认损坏的安装仍回退。** 两种合法 marker × 缺失/明确 `Rejected` 的四个文件库场景均回读预留原件，pending 候选保持可重试。未来 schema 的既有拒绝保护继续通过。此修复没有增加跨进程持续锁或文件集合原子替换保证。
- **真实 Windows 权限拒绝。** 仅对唯一标记的合成目录 `backups`，为当前 SID 设置不继承的 `CreateFiles` deny。实际创建文件得到 `PermissionDenied`，恢复停止，原 DB/WAL/SHM/pending 字节和 mtime 不变，无 rollback/next 残留。辅助进程在显式请求或控制端 EOF 时恢复独立保存的原 Access ACL；显式恢复逐字核验 ACL 并正常退出后，同一 pending 重试成功，三份原始 raw 字节均保存。没有修改默认用户目录或全局权限。
- **历史矩阵补齐 oracle。** 本地 Git 对象核验 11 份历史 SQL 和 schema-2 历史迁移源码均匹配冻结 manifest。既有文件库矩阵补充七个资产属性的非默认哨兵与带类型快照，校验 schema-2 实际 include 的 SQL 副本，关闭并重新 `init_db` 后比较完整业务快照、portable ID 与最新版本标记。保留原有迁移重跑、故障回滚重试、外键及完整性断言。

最终 `cargo test --offline --locked --lib --jobs 1 -- --test-threads=1` 为 **152 passed / 0 failed / 3 原有 ignored**；locked/offline check、strict Clippy `-D warnings`、fmt 与 diff 检查通过。完整测试前的一次 ACL 辅助启动失败单独保留：Windows PowerShell 继承了不兼容的模块搜索路径；辅助脚本显式导入本宿主 `$PSHOME` 的 ACL 模块后，单项及完整复测通过，该失败不计作 PASS。

本节证明当前源码的合成文件库后端与真实 Windows 文件权限行为，未启动 ImageLore 桌面程序，未验证物理断电、真实磁盘耗尽、跨进程完整恢复、安装器/updater 或完整 GUI 故障矩阵。当前桌面 EXE、性能样本与先前构建身份保持各自记录，阶段 A 和 v1.0.0 的完整退出标准仍未关闭。

## 已执行的验证

- cargo test --locked --manifest-path src-tauri/Cargo.toml --lib migrations::tests -- --test-threads=1：15/15 通过。包含 schema 1–11 文件数据库升级、完整业务记录快照、确定性 portable ID、迁移重跑、最终步骤故障回滚后重新打开/重试、未来及非法 schema 拒绝、fixture 来源 hash。
- cargo test --locked --manifest-path src-tauri/Cargo.toml --lib backup::tests -- --test-threads=1：首轮 24/24；独立备份锁调整后 26/26。包含带 WAL 候选 DB/WAL/SHM 字节不变、首次安装失败、pending 暂存中断、原 DB 损坏且仍被占用的 Windows 回归，以及近期有效检查不等待图库连接、备份外部损坏后的重新验证。
- cargo test --locked --manifest-path src-tauri/Cargo.toml --lib tests::startup_ -- --test-threads=1：9/9 通过（7 项启动回归与 2 项备份启动回归）。
- 复制活跃 SHM 曾真实触发 Windows os error 33（文件部分被锁定）。最终副本探针仅复制 DB 与 WAL，在独立目录重建派生 SHM；没有放弱原有“合法 WAL 写锁下仍可读、但不可恢复”回归。所有最后修改均已由上述 backup/startup 目标复测通过，全项目门禁由本轮总体验收记录汇总。
- 自动备份 async 包装调整后，完整 native 门禁通过：`cargo fmt --check`、`cargo check --locked`、`cargo clippy --locked -- -D warnings`、`cargo test --locked --lib`（95 passed / 1 ignored）。日志为 `<private-evidence>/backup-async-cargo-*-v026.txt`；此轮未重复运行已单独完成的 50k/75k ignored 规模用例。
- 独立备份锁与预览缓存修复后的当前 `f61f59b`，完整 library suite 为 104 passed / 1 ignored；相关检查与 strict clippy 通过。日志为 `<private-evidence>/cargo-{clippy,test}-perf-final-v026.txt` 和 `startup-backup-lock-{check,clippy}.txt`。历史 50k/75k ignored 用例的查询/排序代码未变，证据继续绑定其实际运行来源。

恢复用例覆盖：真实未 checkpoint 的已提交 WAL 写入→独立备份→后续编辑→pending 恢复→重新打开回读；最新坏备份跳过选择较旧有效库，最新临时不可用则停止并在解锁后重试同一最新候选；无有效备份停止；空/普通 SQLite、未来 schema、孤儿关系、截断候选拒绝；五个恢复步骤的 IO/rename/磁盘失败注入后原件可读且 pending 可重试；原 DB/WAL/SHM 保全失败必须中止；独占锁及活跃 WAL writer 不发生替换；中断预留回滚。自动备份覆盖 8 个并发请求只生成 1 份、24h 前后边界、坏文件不能推迟有效备份、失败后重试、手动备份独立名称、保留 10 份有效备份。

故障注入及中断用例是临时文件库上的受控状态模拟；没有声称真实磁盘耗尽、物理断电或隔离 Windows 安装器升级已验收。

## 自动备份 IPC 调度

启动时前端立即检查自动备份，之后每 15 分钟与窗口 focus 再检查；即便已有 24 小时内的备份，也先执行完整性、身份、外键与业务关系验证，不只读取 mtime。50k 桌面测试资料库的备份约 280 MB，该同步 IO/扫描此前直接在同步 IPC handler 中执行。

第一步把 `ensure_auto_backup` 命令包装改为 async：拥有的 AppHandle 移入 `tauri::async_runtime::spawn_blocking`，在 closure 内取得同一个 AppState，再调用 `ensure_auto_at`。该版本仍以数据库 Mutex 包裹备份扫描，后续定位了图库查询等待同一锁的问题。

第二步增加独立 `backup_operation` Mutex。自动/手动创建及恢复暂存统一先获取该操作锁；只有实际 `VACUUM INTO` 需要活动数据库锁，快照完成便释放，随后完整验证、sync 与轮换。已有近期备份的校验不再占用图库连接，且每次重新验证，不缓存“曾经有效”的结论。State 与锁 guard 不跨 await，24 小时边界、无效候选处理及 8 个并发请求只产生一份的语义保持。返回值仍为 BackupRecord 或 null，内部业务错误与 JoinError 均拒绝原 Promise，前端 pending/finally 与错误显示不变。

确定性回归先让测试主线程持有 db 锁，旧实现的近期有效备份检查超时；修复后独立返回 None。另先成功检查、再外部损坏同一备份，后续检查仍生成验证合格的新快照并保留损坏字节。证据：`<private-evidence>/startup-backup-lock-{before,after,check,clippy}.txt`；26 项备份回归及 fmt/check/clippy 均通过。

锁定依赖为 Tauri 2.12.0 / tauri-macros 2.7.0 / Wry 0.57.0。宏默认同步分支直接调用命令，async 分支把响应交给 runtime；spawn_blocking 使用专用阻塞池。实际需要新备份时 VACUUM 仍会占用数据库锁，启动副本探针及安全验证均保留。因此不能仅凭锁回归保证启动 FCP 达到 ≤3 秒；最终程序的正常启动与交互 profile 由桌面验收记录另报，恢复时安全复制开销也独立记录。

## 恢复安全边界

SQLite 的 READ_ONLY 连接仍可能重建 SHM。本轮回归真实捕获了损坏 DB 配合任意 WAL/SHM 时的原始 SHM 改写。启动现在先复制 DB/WAL 到独立临时库，在副本目录重建 SHM 并检查完整性，再决定打开原库或启动恢复；探针复制/权限/IO失败不会授权恢复。恢复在任何活动库 SQLite 操作之前保全 raw 原件，任意原件保全失败就停止。WAL 候选的验证及 VACUUM snapshot 同样在副本上读取，保留候选原始字节。

副本探针不持有原库写锁，不能把它当作独占状态证明。替换前在 Windows 以 share_mode(0) 原 DB 句柄探针排除仍在占用的文件，再以零等待 BEGIN IMMEDIATE 检查原库写锁，BUSY/LOCKED/权限错误始终拒绝；只有已经确认且无占用的 CORRUPT/NOTADB 可进入保全后的损坏恢复流程。损坏头部可能在 SQLite 锁检查之前返回 NOTADB，所以 Windows 句柄检查是独立的一层保护。健康库仍须通过身份、外键与业务关系检查。

自动恢复和用户主动 pending 恢复分别处理：自动恢复在替换前必须仍确认活动库损坏。最终复制探针之后，成功取得 BEGIN IMMEDIATE 的同一实际原库连接再次读取 schema 与 integrity；若原库已经恢复健康，自动恢复停止并保留新写入。副本 probe 在两次复核之间遇到并发写入而不一致，不能授权覆盖后来已健康的库。主动 pending 恢复可覆盖受支持的健康旧库。活动库消失或无法确认时自动恢复停止。

可读未来/非法 schema 无论 integrity 是否报错都拒绝覆盖；finish_or_rollback 在处理 COMMITTED 或回滚原件之前也执行该保护，保留未来 main、旧 reserved 和 marker。确定回归使用只修改额外合成索引叶子页的故障，readonly integrity_check 明确报告 index/table 不一致，schema 12 仍可读。SQLite 的 readonly integrity_check 会跳过 CHECK 异常，因此没有拿 favorite=2 作为已经确认物理损坏的证据。修复竞态回归在 Staged/Probed hook 后插入新的健康库与资产，验证自动恢复不消费候选、不丢失新写入。

候选资格采用历史真实业务结构：schema_version 必须在 1–11 内，按版本验证核心字段和新增表；执行 integrity_check、foreign_key_check、业务孤儿关联与重复 portable ID 检查。历史库没有 application_id，因此不把单个版本字符串当成身份凭据。候选验证不会创建缺失数据库，未来或普通 SQLite 不能成为活动库。

替换顺序：

1. 保留 pending/备份候选，对候选生成目的目录同文件系统的 restore.next 临时 snapshot 并验证。
2. 保全原 DB/WAL/SHM 到 raw，健康原库另创建可恢复的 pre-restore snapshot；失败中止。
3. 在活动 DB 同目录创建固定 restore.rollback，把原 DB/WAL/SHM rename 预留到该目录；失败把已预留文件回退。
4. 仅通过 rename 安装已验证 snapshot；不使用覆盖活动路径的 copy 回退。安装或验证失败恢复原件，pending 保持可重试。
5. 再验证安装库，写入并同步 COMMITTED。有效提交清理 pending 和 rollback；未提交、撕裂 marker 或坏安装在下次启动先回退原件。首次无原库的失败候选移至 recovery/*.raw 后重试。

所有恢复目录、候选名称与 marker 路径在程序内固定生成；marker 只允许 pending/recovery 两种状态，不接收可执行命令或外部绝对文件路径。恢复路径不会从恢复记录中解析任意目标。stage_restore 只使用备份文件 basename，并以固定 previous 候选文件保护 pending 暂存中断。

无效候选在轮转时移入 backups/rejected/*.raw 保留证据，10 份上限只用于已验证的有效备份。raw、隔离候选及 probe 都不算普通恢复点。SQLite 备份含元数据、编辑与业务关系，不含原图字节、模型或缓存；恢复会回到该恢复点，备份之后的编辑可能丢失，但当前库原件仍保全。

## 历史 fixture 来源

来源文件：[manifest.json](../src-tauri/tests/fixtures/migrations/manifest.json)；未修改历史 SQL。每个固定 SHA 的 git show 命令与 SHA-256 均记录在 manifest；Rust 测试核对内容 hash。schema 2 另冻结 1299f7f 的原始 Rust migrations.rs，并运行其真实 apply 初始化/迁移。该版本静态 schema.sql 仍标记 1，测试没有改标记伪造 schema 2。

| schema | 完整历史 commit | schema.sql SHA-256 |
| --- | --- | --- |
| 1 | c2a06a5256b46797fbb37aabc924024a219e629d | 457692197923f582be10c3f262268f7424a9ba562d8e298e6974ce32e27be933 |
| 2 | 1299f7fa40a4fc0efd024a069ed101983ffa0241 | 6b59af9f39e31a4351579e60e3feecde296fe542967df53ef8cdfc332c43aadf |
| 3 | 0bfe4a307a28fde4990ad7e6a35ed44f0af7b0fb | c38771413c64cb892ba515b3651bd298ab4c070531f356c8403e31506bd7d800 |
| 4 | 6a60ba08088ad8b8501afedf1fb7c7709fa6126c | 340337d389fe39402f9d6d684a6e654f56506d59d888784e4e3da21aa74cf957 |
| 5 | d3d74f80883f2c1ccdda326db24de0136233197e | bbec75e5abebacfd17b8855b23a3fd9ea42a0400d92da8b5191076f26fb558c7 |
| 6 | 60b7353c370afa81ea5e63ea53f394d1a5196283 | b85ef74beab7f363d0afdc6a6736791f4aba5b987a664d0b4cdd89d3579dfb97 |
| 7 | 23959672b340524b6db9cf97298a4118f4668efb | 56e65afaac1a795c2e6f821639dccfb4b0c854b0fea4f8c86dfc32955518d1d0 |
| 8 | 4b2dcb53e10779bd8e03b66fa615ff29fa8f92ae | 137d9eab2ffa8d5143721cb6292ebe909ac48429332e1dff91caac95aacc3d45 |
| 9 | fdadeece7d46e7889387b8cb4d0d493702d666dc | 1cad7d09bbbe48a4e59f0d0e23120fc8a92688d1f34c71c1aa19148940cbb44b |
| 10 | c2720fb5cbd11cdff156e2aeb693cecfebf15dac | ac18a61bf5f303c4e0d2b5425577f7d6b96bc9800e51381447a168b549a5f752 |
| 11 | e93857cb7176606b11d732db276ee9d1daff303b | a9bd924fa7749fd128cdf01c9f94234cb6a245043297ee907b1f93913598b2b4 |

Schema 2 原始迁移文件 SHA-256：8740675970c11883dd752c7ae197a592c1484ccfd73ee22cdf2a43ed3c717448。

每个版本使用其历史实际可用的表插入合成哨兵。全版本保留 2 个资产与 Prompt、1 个 Revision、标签/集合成员和父子谱系；schema 3 起检查 portable ID、Session、alias、saved filter、待解析关联；4 检查来源目录；5 检查 Generation Index；6 检查 embedding/settings；8 检查手工 DNA；9 检查 Vision 配置/分析；10 检查 Remix 草稿/跨资产来源；11 检查 Reference URL、标题与 metadata。比较的是完整有序业务行，而不只有计数；派生搜索索引也需命中中英文哨兵。迁移 savepoint 将 DDL、回填和 schema marker 一起提交，末步故障不留下半升级数据，重开后可再次执行。
