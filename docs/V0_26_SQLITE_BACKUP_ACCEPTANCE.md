# v0.26 SQLite 安全升级后的备份与恢复诊断

日期：2026-10-23（Asia/Shanghai）。应用版本 `0.26.0`、schema `11`。实际受测程序由 `4d8451fd1fc46340267bd0c464731f18cfd8e26a` 编译；执行时文档 HEAD 为 `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`，68 项编译/include 输入完全相同。本轮继续使用串行生产后端，没有合入两路校验候选。

## 实际结果

归档 optimized libtest（opt-level 3、debug_assertions=false）独立执行既有显式 profile 测试，实际 **1 passed / 0 failed / 0 ignored，495.82 秒**，其余 238 项过滤。原生运行、报告消费、六操作汇总和原始源保全终态全部通过。这不与完整库的 236/0/3 累加为不同测试数。

在新建、单次使用的隔离副本中，正常后端连接实际执行 `SELECT sqlite_version()`，返回 **3.53.2**。不是仅从锁文件或 DB 头推断运行版本；这仍然是库测试程序的运行证据，未启动桌面应用。

50,000 资产、初始与最终各 10 份有效备份，validator cache 为 32768 KiB。下表为各操作唯一 root span 的 wall，完整原始值保留，未删除慢样本。每项 report.ok 与 root resultOk 均为 true。

| 操作 | 单次 wall（秒） | 观察到的 validation.total wrapper 数 | 实际行为 |
| --- | ---: | ---: | --- |
| startup_normal | 7.090 | 0 | 正常打开文件库，备份名称清单不变 |
| auto_recent | 8.148 | 1 | 近期自动备份返回 None，清单不变 |
| manual_create | 89.464 | 12 | 真正创建并验证手动备份，typed 数据保持基线 |
| auto_due_create | 87.885 | 13 | 到期后真正创建并验证自动备份 |
| stage_restore | 11.361 | 0 | 真正暂存恢复，pending 保持基线、活动库不变 |
| apply_pending_restore | 124.685 | 10 | 修改 Prompt、关闭连接、应用恢复并重开，恢复基线且保留替换前 raw |

## 数据保全与来源

typed 基线记录 26 张表，逐列区分 Null、Integer、Real 位值、Text 和 Blob，并保留行数与排序后的逐行摘要；实际空表也记录。备份、pending、恢复后活动库按此基线核对。恢复前一条 Prompt 的修改先被确认不同于基线；应用恢复并通过正式初始化重开后回到基线，另有一份保留原件 raw 的 typed 内容仍等于修改后的状态。该闭环覆盖本合成 fixture，不扩大为任意数据的保证。

原始 DB/WAL/SHM 加十份来源备份共十三文件。准备阶段从只读独占句柄复制，仅工作副本执行 checkpoint；运行阶段重新取得十三个只读独占句柄，持续覆盖原生 workflow 和 consumer。最终十三文件的 SHA256、大小和 mtime 均与运行前及准备清单相同，原始源没有由 SQLite 打开。准备与运行之间没有连续持有句柄的声明；漂移由运行前核对拒绝。

68 项编译输入、实际归档、完整库回归及本轮适配脚本分别绑定。新适配器直接检查实际 optimized 完整库 236/0/3 回执，不将其改写为旧 debug 回执。六项辅助守卫检查包括正常证据接受，以及错误归档、错误 HEAD、过滤后的完整套件、失败套件和代码变化冒作文档差异的拒绝；这些合成守卫结果与真实原生运行分别记录。

| 证据 | SHA256 |
| --- | --- |
| 实际 optimized libtest | `8dc3843193e4e1ce097ae13ff4f039517dc28628244689ee7d2a06e881634bfd` |
| 原生运行终态 | `505bb3dd8946f1e053b5d7f5448287ed3c06f72e9a511a7caf040d66c07203ae` |
| 六操作消费汇总 | `fa141993e4e81a777e1c9cddf6684dc06bef9ab00254e81666a24bfbdf3bd82c` |
| 原始源保全终态 | `af5bcf34746fba70a206ce7e5a46c1cc30730b5636fdb1831aa299fe5bcf0377` |
| 原生 stdout | `586dcb6af769f20f3695bc7a80eb5d0b6399ca50a4508806d8b5115e6e066833` |

[main Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39911210425)另从原始日志核对为 `1dfe369` 的 **236/0/3（1126.82 秒）**，无过滤，与本地优化套件及本 profile 分别记录。

## 性能与验收边界

这轮每操作只有一个样本。复制、hash、checkpoint、正常启动及未计时断言预热缓存，操作顺序固定，OS 缓存、背景负载和可用内存未受同等控制。正常打开仅计 `open_library_connection` 后端，排除全局 prepare_state、Tauri/WebView、FCP、IPC、UI 和阻塞任务排队。总测试时间还包含未计时断言，不能当成任何单项操作延迟。

wrapper 数不等于完整 validator 总调用数；直接 guarded 校验未全部包装，完整计数和 worker trace 仍为未记录。五类 legacy SQL span 的 resultOk=null 数依次为 0、5、60、65、10、65，只表示没有记录 SQL 返回状态，不能转写为成功或零次执行；操作返回值、root 与 typed/reopen 断言分别提供本轮结果证据。

手动、到期与应用恢复仍需数十秒至两分钟，不能宣布备份性能已达标。新旧引擎单次耗时不构成回归或提速的受控比较；[旧引擎诊断](V0_26_CURRENT_BACKUP_DIAGNOSTIC.md)与[旧引擎两方案分布](V0_26_PRECONDITIONED_BACKUP_SERIES.md)保留原身份，不移记为本轮性能证明。真实桌面、正式冷暖、长期内存、安装升级、签名/updater 和 RC 继续按原阶段验收。新桌面候选尚未运行或推广，阶段 A 与 v1.0.0 仍未完成。

原始日志、报告、数据库和本地路径均保存在仓库外；公开文档不包含用户图片、个人邮箱、凭据或账号安全设置。
