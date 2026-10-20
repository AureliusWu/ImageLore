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

上述锁只串行遵守新协议的恢复操作；备份创建/轮换仍有其既有 AppState 锁和源文件保护，旧程序也不识别新接管记录，不能声称跨旧版并发或降级兼容已经验证。旧 `restore.rollback` 仍需保留并人工核对。真实进程异常退出矩阵、物理磁盘耗尽、桌面重启、安装升级、规模与长期内存仍须独立验收。所有原始现场、邮箱、凭据及本机路径留在仓库外。
