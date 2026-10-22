# 备份创建与恢复故障路径补充验收

日期：2026-10-22（Asia/Shanghai）。应用版本仍为 `0.26.0`、schema 为 `11`。本记录接续[空间故障验收](V0_26_SPACE_FAILURE_ACCEPTANCE.md)，补齐要求审计定位的创建备份、当前恢复 Probe、实际 rename 失败与只读拒绝的具体后端路径。测试只使用隔离合成文件库，没有启动 GUI 或修改正常用户资料库。

## 当前结果与身份

定向 `conflicts-completion-20261022-01` 实际 **5 passed / 0 failed / 0 ignored**（24.40 秒），退出码 0；原始 stdout 独立核数得到 **16 个 `PASS completion fault` 场景标记**。包含测试代码的严格 Clippy `clippy-completion-20261022-01`（`--lib --tests -- -D warnings`）实际退出码 0。两次执行的 68 项编译/include 输入前后相同，未降低警告等级。

定向门禁实际新编译测试 EXE，SHA256 为 `b955e8a6ace9f18422265885daad7b30b8491d37d5a079dde85756723b5636c4`。本批完整 library suite `full-completion-20261022-01` 实际 **235 passed / 0 failed / 3 原有 ignored**（223.37 秒），退出码 0，68 项输入前后相同。full 沿用上述定向产物，Cargo 报告复用，不声称再次编译；实际独立 EXE 归档 SHA 也与上述值相同。

运行时基线 HEAD 为 `403024923b28d3fb4524d359a2f3962abbdff8a1`，以下五份修改/新增文件另行绑定，基线提交本身不包含这些更改。完整 68 项输入、原始输出、终态回执和实际产物保存在仓库外，公开记录不含本地绝对路径、邮箱或密钥。

| 修改/新增源码 | SHA256 |
| --- | --- |
| `src-tauri/src/backup.rs` | `e606a06aaba0042fe10f4ebfacead4c1ff93c6a70a44f154264646146a2e2179` |
| `src-tauri/src/backup/guarded_restore.rs` | `0ba3920ca932e48b1ffce20c743ee1b8c1d2b39637ad299d6df63d49eddf5510` |
| `src-tauri/src/backup/space_fault.rs` | `31a4850f7de210f4adffe8a06928caafce00183f8a1efbea0e3ba585fb74a169` |
| `src-tauri/src/backup/space_failure_tests.rs` | `c1cf8c6f5c761246ce5b50f354ceb61d6326ba72a6f856c766e8dc7e2c3c369a` |
| `src-tauri/src/backup/fault_completion_tests.rs` | `aa88cf31f7951c677c642106a62a1fa66671f140b74492af6818039a9c4222f0` |

新增接点只在测试编译中启用。已有空间故障文件仅开放七个 helper 的同级可见性及相应格式，不改旧断言和流程；31 案进程终止测试源码未改。此前 **18 个空间故障场景**及 **31 个真实强杀场景**保持各自身份，不算作本批新增场景，也不与五个 Rust 测试数相加。

## 五个测试、十六个场景

| 实际测试 | 场景 | 故障及实际断言 |
| --- | --- | --- |
| `backup_create_real_sqlite_full_keeps_live_wal_and_prior_backup_and_retries` | 创建备份 1 | 正式 `create` 先成功生成独立有效控制备份，再对精确 backups 父目录与名称前缀选出的实际目标执行有界 memdb VACUUM，返回真实 typed `DiskFull / SQLITE_FULL / 13`。活动 DB/WAL 字节、marker/资产/Prompt 业务及已有备份完整指纹保持；失败目标不作为成功备份返回，也不进入备份清单。撤销作用域后同源正式创建成功，验证新备份业务与基线一致。 |
| `source_probe_partial_copy_keeps_original_and_source_and_formal_retry_succeeds` | 三用途 × DB/WAL，共 6 | 在当前 `guarded_restore::Probe::copy` 中，精确匹配工作数据目录、Bundle 类型、输入主库路径和角色；实际复制并同步 31 字节后注入 Windows 112。源候选与原件的完整指纹、原业务保持；不把部分检查副本当活动库，正式外层入口重试成功。 |
| `installed_probe_partial_copy_rolls_back_exact_original_and_formal_retry_succeeds` | 三用途，共 3 | 在安装后的 `Probe::copy_file`，精确匹配 File 类型与已安装目标，真实前缀复制/同步后注入 112。事务回退至原件完整指纹，源保持；撤销注入后外层正式入口重试成功。若部分检查副本作为现场保留，其字节须为严格前缀、完整验证失败；它不是可用备份或活动库。 |
| `install_ready_real_rename_conflict_keeps_every_object_and_formal_entries_refuse` | 三用途，共 3 | 在所有缺失预检通过后、实际 `rename_no_replace` 前创建有效外来目标；钩子返回 Ok，真正 Win32 rename 返回 **80 或 183**。核对外来库业务及完整指纹、staged 健康业务与记录指纹、预留三原件、raw 三角色与源候选。随后三次正式外层入口都拒绝，现场全部文件指纹不变，不猜测成功或删除外来内容。 |
| `readonly_target_rejects_real_write_and_formal_restore_then_retries_after_attribute_restore` | 三用途，共 3 | 仅把隔离 fixture 的目标主库设置 Windows readonly 属性。设置前/恢复后可真实打开写句柄；只读期间写打开和正式恢复均得到真实 **Win32 error 5**。独立只读句柄比较 File ID、volume、大小、mtime、SHA 与字节，源和原业务保持；恢复原属性后正式重试成功，panic 清理也恢复原属性。没有改 ACL、owner 或系统设置。 |

三用途是应用 pending、暂存 pending、已确认损坏的自动恢复。fixture 使用真实已提交但未 checkpoint 的 DB/WAL/SHM；恢复原件沿用 schema 11 的可读索引损伤，健康输出仍走完整验证。

创建备份的 FULL 来自 SQLite 实际执行的唯一命名、8 KiB 有界 memdb，错误原因原样保留，不伪造返回值；它与此前发生在恢复 staged VACUUM 的三案分开。112 属于真实部分 IO 和同步之后的受控错误，不能称为自然 NTFS 满。只读与 rename 错误来自实际 Windows 调用，也没有要求耗尽主机磁盘、物理断电或管理员设施。

rename 故障使用实际操作锁与同一个生产恢复事务核心，之后的三次拒绝经过正式外层入口；该钩子调用不是 Tauri IPC。其余正式入口和重试同样在后端测试进程内执行，**不计完整应用新进程、GUI、安装或桌面重启验收**。测试 EXE 是 Rust library 产物，不能等同应用 EXE。

## 前序同步与独立候选

前一空间故障源码 `4030249` 的[候选 Native](https://github.com/AureliusWu/ImageLore/actions/runs/39712538000)实际 **230 passed / 0 failed / 3 ignored**（901.46 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39712540748)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39712357271)成功，随后正常快进 main 并推送；推送前 1327 项检查及默认 Gitleaks 通过。该远端结果绑定前一批 67 项输入，不能转记为本批新增路径已通过远端。

更早 Role 候选 `9284eaf` 的[Native](https://github.com/AureliusWu/ImageLore/actions/runs/39711150959)已实际 **225 passed / 0 failed / 3 ignored**（850.93 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39711213364)和[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39399396084)成功。此前“运行中”的观察保留为历史时点，现已取得上述终态。

两路校验候选 `0a87dd8ddfce3022df4122576f616b5bbbbaf503` 的本地 **233/0/3** 与 debug 文件库六操作诊断通过，仍是另一份源码、产物和单样本测量，未并入 main，也不是正式性能验收；其[固定提交诊断记录](https://github.com/AureliusWu/ImageLore/blob/0a87dd8ddfce3022df4122576f616b5bbbbaf503/docs/V0_26_BACKUP_GUARDED_WORKER_DIAGNOSTIC.md)单独披露范围。

## 原要求的当前边界

A1 的确认损坏才恢复、未来 schema/迁移/锁错误拒绝，以及原件保全已有当前后端证明。本批为 A2 审计定位的备份创建 FULL、当前 Probe 复制失败、实际 rename 错误和只读拒绝补上了明确范围的定向证据，并完成本地完整回归。计划 4.1-6 的缺图、正式重定位与业务/谱系重开闭环另行定位，不能以已有搜索/缓存子项的通过代替。

当前恢复源码的真实桌面流程与错误提示、50k/75k 与冷暖性能、长期内存、安装升级、签名、updater 和 RC 仍按原阶段验收。没有构建或推广应用 EXE/安装包，也没有更改已验收的 `0.26.0` 桌面运行时或快捷方式。本记录不宣称阶段 A 或 v1.0.0 的全部退出条件已经满足。
