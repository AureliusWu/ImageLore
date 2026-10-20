# Windows pending 候选暂存与旧候选防重放

日期：2026-10-20（Asia/Shanghai）。本记录接续[恢复事务验收](V0_26_RESTORE_TRANSACTION.md)，覆盖 Windows 的 `stage_restore_at`、`finish_pending_stage` 及其与活动恢复事务的入口顺序。应用版本保持 0.26.0；后端回归不代表桌面、安装升级或 v1.0.0 已发布。

## 原始反例

基线暂存代码在检查 pending 不存在后移动 previous，在预留旧 pending 后覆盖目标名称，安装后按路径删除 previous。三个回归先验证普通恢复与连续换点，再触发真实 SQLite 写入，实际结果为 **0 passed / 3 failed**：新 pending 被覆盖两次，同一 previous 对象上的新提交被删除一次。

原始 RED 源码 SHA-256 为 `a74011481112d2e43f5dba92acfe3c43b3ba5c5a705a340ddc22f1fd40cbb581`，测试 EXE 为 `7b23a847bfd8bf2d807217b7d32e9b4f663db9f7ccd62dbab6c5b8ff1888d2ac`。完整失败日志、源码、补丁、产物与现场冻结在仓库外的 `<private-evidence>/pending-stage-safety-20261020-01`，没有用后续 GREEN 覆盖原证据。

## 暂存事务

暂存复用已持有对象、完整指纹、raw 保全、禁止覆盖移动、Prepared/Commit 与续作的同一套实现。可信调用方选择目标，记录不能自行选择写入位置：

| 操作 | 固定目标与事务 | 源候选 |
| --- | --- | --- |
| 应用用户选择 | 活动库 / `restore.transaction`，模式 `pending` | 固定 pending，提交清理时按完整指纹消费 |
| 已确认损坏的自动恢复 | 活动库 / `restore.transaction`，模式 `recovery` | 备份保持不变 |
| 暂存新的用户选择 | `restore.pending.sqlite3` / `restore.pending.transaction`，模式 `stage_pending` | 备份保持不变 |
| 接管旧 previous | 同一 pending 目标与事务 | previous 保持不变，提升前完整源指纹须仍匹配，最终目标三角色必须全空 |

两种事务均只识别原有六个角色名称。pending 暂存没有另一套宽松回滚或按路径删除逻辑。旧 pending 的 raw 副本位于数据目录下的 `recovery/pending-stage`，不算普通备份点。事务清理只删除已持有且完整指纹匹配的对象，raw 永久保留。

保留区原件与已安装对象从最终获取到写入提交记录期间持续持有。原始 RED 中清理前可成功提交的写入，现在在持有期间会被 Windows 拒绝；这类测试只证明写入被阻止，没有声称产生了新提交。另有释放句柄后的真实 SQLite COMMIT 回归，在同一 File ID 上写入新 marker 和额外资产；尚未提交与已提交两条收尾路径都必须停止并保留新内容。

## 持久操作锁与旧格式接管

Windows 的 apply、recover 和 stage 在首次查询或续作前取得数据目录内的 `restore.operation.lock`。AppState 已有的备份锁仍在外层；内部函数借用同一个操作守卫，不重复获取 share0 文件锁。锁通过完整签名临时文件写入同步、同句柄禁止覆盖发布获得；进程退出由系统释放句柄，固定文件留存以供下次复用，不通过删除“陈旧锁”取得所有权。外来、截断、目录或重解析点锁对象会停止并保留。

旧 `restore.pending.previous.sqlite3` 的 DB/WAL/SHM 不再移动或删除。接管记录 `restore.pending.legacy-adopted.json` 绑定数据目录身份与三个角色的完整指纹，严格限制格式、字段、长度和校验和，并通过完整临时文件同步后禁止覆盖发布。缺失主库但存在侧车不是“无旧候选”。

只有 previous 时，使用事务生成 pending 副本，再发布接管记录；已有 pending 时，持续持有双方、在副本上验证完整 ImageLore schema 后发布。匹配记录存在时，previous 永远只作保全证据，即使 pending 已消费，也不再次提升。记录损坏、未知、目录身份不符、缺失角色或同一对象的新内容都停止，不重写记录。未发布的协议临时文件保留，不当成最终授权记录；其真实名称与两个事务、锁、候选名称均被 raw 路径解析排除。

续作前先检查两个事务名称。两者同时被任意对象占用时整体停止；活动事务与未接管 previous 并存也停止，避免先消费 pending、再把 previous 提升而重放。最终接管记录一旦存在，任何事务续作前都先严格校验。活动事务加匹配接管记录允许正常续作，pending 事务加 previous 且尚无接管记录也允许完成合法的旧候选提升。

同一次操作会记住已经验证或发布的接管记录；外部进程随后移除该记录时，后续步骤必须报错，不能退回 previous-only 提升。该保护不等于能够推断下一次独立启动前被外部永久删除的协议历史；不要手工删除或改写接管记录。

## 验收与边界

本地完整 Rust 库测试为 **221 passed / 0 failed / 3 原有 ignored**（116.88 秒），实际进程退出码 0。记录为 `full-green-20261020-02-terminal.json` 及对应原始 stdout/stderr；64 项编译与 include 输入前后相同，并已逐文件归档。Cargo 报告测试产物本轮新编译；实际 EXE 与独立归档 SHA-256 均为 `1dfc1569bab2119e4280be44e61307541ed3bb02cfb0edb595e76bad67fa37cd`。

最终同一编译输入的严格 Clippy `--lib --tests -- -D warnings` 为 `clippy-green-20261020-04`，实际退出码 0。原始三个窗口的定向 GREEN 为 3/0，已包含在完整套件中，不重复累加。新增回归还覆盖六个事务步骤故障后精确回滚与正常重试、独立进程占锁/退出、完整锁签名发布前故障、旧候选三角色与损坏接管记录、合法已接管事务续作、双事务冲突、pending 消费后多次启动不重放，以及本次操作中接管记录被外部实际删除的 pending 尚在/已消费两种状态。

测试使用独立合成资料库、真实文件指纹与 SQLite 业务行；混合状态和再次启动检查比较完整文件树、字节、修改时间及身份，不能仅凭返回 Err 视为保全。原始 full 首轮 **215 passed / 5 failed / 3 ignored**（119.67 秒）及其产物继续保留：三项新 fixture 错读活动事务目录，在进入待测续作前即失败；另两项精确快照未预先计入新增持久锁文件。修正 fixture 与基线快照后保留完整保全断言，另完成审查发现的接管记录消失保护。严格 Clippy 首轮发现两项 Windows 不再使用的旧快照 helper，现仅在非 Windows/测试条件编译，没有降低警告等级。

上述原始日志与现场均位于 `<private-evidence>/pending-stage-safety-20261020-01`。所有 Cargo 门禁使用 `--offline --locked --jobs 1`，没有启动 GUI 或构建应用 EXE/安装包。远端检查以本轮实际源码提交另行绑定，不能借用基线 193 项 CI 的通过结论。

源码 `01b5697eb01dc4be1a3f9ffdd66e312ff77ba685` 的[候选 Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39392292515)实际为 **221 passed / 0 failed / 3 ignored**（818.78 秒），format/check/Clippy 均成功；[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39392304237)及[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39392245399)也成功。源码随后正常 fast-forward 同步 main，推送前 1293 项检查与默认 Gitleaks 通过，没有创建 PR 或调整保护规则。原始 CI 日志分别保存在同一私有证据目录的 `ci-candidate-01b5697-{native,fast}.log`。下列强制中断新增测试与该 221 项源码身份分别绑定。

## 真实 Windows 子进程强制终止矩阵

后续在同一生产逻辑上添加仅测试编译的持久步骤钩子，使用实际测试 EXE 启动独立子进程。子进程取得真实操作锁、执行真实事务，在指定边界写入并同步位于数据目录外的 ready 记录后暂停。父进程核对 nonce、PID、操作与边界，确认子进程仍存活且操作锁实际 Busy，再调用 `Child.kill()` 并 wait/reap；不使用回调 Err、panic、正常退出或 Rust Drop 代替强制终止。

| 边界 | 应用 pending | 暂存 pending |
| --- | --- | --- |
| Prepared 已同步，尚未移动原件 | 通过 | 通过 |
| 原件三角色已预留 | 通过 | 通过 |
| staged 已安装，尚无 Commit | 通过 | 通过 |
| Commit 已同步，尚未清理 | 通过 | 通过 |
| 已删除第一份保留区原件 | 通过 | 通过 |
| 已消费 pending 主库，侧车仍在 | 通过 | 不消费源备份，不适用 |
| Prepared 已删除，只剩自含 Commit | 通过 | 通过 |

最终定向记录 `conflicts-matrix-20261020-02` 共实际终止 **13 个子进程**，退出码均为 Windows `1`；逐案 PID、退出码及数据断言 PASS 保存在原始输出中，耗时 25.24 秒。两个父测试完成全部 13 案；另一个普通测试计数是 exact child 入口在未设置子进程参数时的空操作，不重复计作额外强杀案例。

每案均使用实际 DB/WAL/SHM：在唯一播种连接保持空闲时复制已提交、未 checkpoint 的三角色，然后关闭播种连接。独立 main-only 副本仍为旧 marker/1 个资产，而完整 DB/WAL 副本读出新 marker/2 个资产，证明业务提交确实依赖 WAL。检查只打开数据目录外的一次性副本；不会在待比较原件上通过 SQLite 重建 SHM。

强杀后核实持久操作锁完整指纹未变且可以重新获取，核实真实 journal 所处边界、原件/候选/安装角色及 raw 三角色。未提交状态续作后精确恢复原件完整指纹；已提交状态继续清理而保留安装库。raw 字节、指纹与已提交业务保持；暂存不消费源备份、不改活动库。随后正常 apply/stage 重试和第二次启动再次回读业务与完整指纹，pending 消费后不会复活。

完整套件 `full-matrix-20261020-02` 为 **224 passed / 0 failed / 3 原有 ignored**（149.14 秒），65 项编译/include 输入前后相同。它沿用最终定向门禁实际新编译的同一测试 EXE，SHA-256 为 `01b8c14eb75b2f1fd1c8c74e7e3079f15bb07834db04a20c6e1b87e9c626f082`，另有独立产物与逐文件源码归档；没有声称 full 又编译了一次。严格 Clippy `clippy-matrix-20261020-02` 通过。原始证据保存于 `<private-evidence>/restore-process-interruption-20261020-01`，先前定向通过、Clippy 命名警告及修正后的复跑均保留，未降低警告等级。

源码 `c19a2f60b786ebc3e511fe36b0ce2197738c94d3` 的[候选 Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39395025528)实际 **224 passed / 0 failed / 3 ignored**（709.34 秒），format/check/Clippy 成功；[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39395033317)和[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39394917151)成功，之后正常快进 main。源码与 65 项已测 Git blob 字节逐项一致；推送前 1301 项检查与默认 Gitleaks 通过。原始 Native/Fast 日志在同一私有目录中，后续新增场景不转记为这次远端已通过。

## 部分角色移动与自动损坏恢复

新一轮在前述 13 案之外，增加三种用途的 DB/WAL 逐角色移出和逐角色归位中断，并补齐自动损坏恢复的持久边界，共 **31 个实际强杀场景**：应用 pending 11、暂存 pending 10、自动损坏恢复 10。

移出钩子在对应原件的同句柄禁止覆盖移动实际成功后触发；归位钩子在回滚实际移动原件并更新所持角色后触发。选择 DB 和 WAL 两处，覆盖三角色只完成一项或两项的状态；三项全移已由原 Reserved 场景覆盖。归位测试先在 Installed 之后返回测试错误，进入真实回滚，再由父进程在指定角色归位后强制终止；前面的错误只用于选择回滚分支，不算作强杀证据。

自动恢复使用明确的物理索引损伤而非仅写入 NOTADB 字符串：完整 schema 11 上建立独立单叶索引，在真实 WAL 提交后复制三角色并关闭播种连接；证明 WAL 的帧没有覆盖该索引页，再只改变主库索引 payload 的一个字节。主库 ID/长度保持、SHA 改变，WAL/SHM 完整指纹不变。专用副本明确读出 schema 11、指名该索引的 integrity 非 ok，以及正确 marker、两项资产和 Prompt；损坏原件和 raw 不被误称为完整健康库。提交后的新库仍走完整 schema/完整性/业务验证。

最终定向 `conflicts-roles-20261020-01` 实际为 31 个强杀退出与 31 个逐案 PASS（67.82 秒）；三个父测试与一个空参数 child 入口计为 4 个普通测试，不重复累加案例数。回滚前后严格比较所有角色、staged 与 raw 的完整指纹。自动恢复未提交状态先恢复损坏原件，再通过正式 `recover_latest_valid_backup` 正常重试；源备份三角色在续作、恢复与第二次启动后都未被消费或修改。

完整本地回归 `full-roles-20261020-01` 实际 **225 passed / 0 failed / 3 原有 ignored**（200.57 秒），65 项编译/include 输入前后相同，并逐项校验后另存源码快照。沿用最终定向新编译的测试 EXE，封存 SHA-256 为 `ef57576853bbaf8047b289ec3d24d162b8ab10a416435c0eaeb3fd9189aee6cc`。严格 Clippy `clippy-roles-20261020-02` 通过，首轮只影响测试钩子的 `unused_enumerate_index` 警告与修正日志分别保留。原始证据位于 `<private-evidence>/restore-role-interruption-20261020-01`；本地 225 项不转记为远端 CI 或桌面验收。

## 证据范围与剩余验收

上述锁只串行遵守新协议的恢复操作；备份创建/轮换仍有其既有 AppState 锁和源文件保护，旧程序也不识别新接管记录，不能声称跨旧版并发或降级兼容已经验证。旧 `restore.rollback` 仍需保留并人工核对。原计划要求恢复暂存、复制、rename、空间不足和进程中断的故障保全及再次启动；空间不足故障注入仍须取得独立证据。物理主机断电和耗尽主机磁盘不在这些测试的证明范围，也不作为新增发布门槛。桌面进程重启、安装升级、规模与长期内存继续独立验收。所有原始现场、邮箱、凭据及本机路径留在仓库外。
