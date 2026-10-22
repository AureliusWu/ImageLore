# v0.26 当前优化构建六操作诊断

日期：2026-10-22（Asia/Shanghai）。应用版本 `0.26.0`、schema `11`，受测源码 `1da13713040485a2919bfb26258425624b0fafaa`。本轮使用实际归档的 optimized Rust libtest（`opt-level=3`、`debug_assertions=false`），运行当前串行生产后端；未构建或运行应用安装包。

原生运行、报告消费、诊断汇总及原始源保全四类终态均已实际通过，stdout 为 **1 passed / 0 failed / 0 ignored，452.92 秒**。这是六操作各一次的正确性诊断；手动创建 **81.99 秒**、到期自动备份 **87.65 秒**、应用恢复 **111.94 秒** 是本轮实际成本，当前备份/恢复性能仍需优化，没有取得正式性能预算通过或提速结论。

## 六操作单样本

下表为各操作唯一 root span 的实际 wall 时间，单位 ms，显示一位小数；完整原始报告保留未舍入值及嵌套 span。每项 `report.ok` 和 root `resultOk` 均为 true。观察到的 wrapper 数仅计 `validation.total` span，不是完整 validator 总调用数。

| 操作 | root wall（ms） | 观察到的 validation.total wrapper 数 | 实际行为与结果 |
| --- | ---: | ---: | --- |
| startup_normal | 1668.8 | 0 | 正常打开文件库，未恢复/替换，备份名称清单不变 |
| auto_recent | 5736.5 | 1 | 近期自动备份返回 None，备份名称清单不变 |
| manual_create | 81992.4 | 12 | 真正创建备份，备份与活动库的 typed 业务数据保持基线 |
| auto_due_create | 87647.0 | 13 | 到期后真正创建备份，验证及 typed 业务数据与基线一致 |
| stage_restore | 9983.5 | 0 | 真正暂存恢复，pending 可验证且保持基线，活动库数据不变 |
| apply_pending_restore | 111942.3 | 10 | 修改当前 Prompt 后关闭连接并真正应用恢复，重开回到基线，替换前原件的 raw 仍可回读 |

整个测试的 452.92 秒包含未计时断言等工作，不能当作任何单项操作延迟；父子 span 的 inclusive 时间也不能直接求和。正常打开仅计 `open_library_connection` 后端，排除全局 `prepare_state` 的目录处理、pending 应用、缓存清理和诊断初始化，以及 Tauri/WebView、IPC、FCP、library-ready、blocking-pool 排队和 UI。互斥锁无竞争，本轮不测并发等待。

## 本轮热点分解

只读复核同一批原始事件，按 parent 链划定 `rotation.total` 子树，再汇总该子树内 `validation.integrity_check` 的 exclusive wall；比例使用未舍入的 rotation inclusive / 操作 root wall。单位 s，下表显示三位小数。

| 操作 | 操作 root wall（s） | rotation inclusive（s） | 占操作 root | rotation 内 integrity_check exclusive 合计（s） |
| --- | ---: | ---: | ---: | ---: |
| manual_create | 81.992 | 78.681 | 95.96% | 75.168 |
| auto_due_create | 87.647 | 82.520 | 94.15% | 78.944 |
| apply_pending_restore | 111.942 | 87.905 | 78.53% | 84.589 |

这些 integrity_check span 已包含在 rotation 时间中，不能与其父 span 重复相加；exclusive wall 是扣除子 span 后的经过时间，不是 CPU 时间。手动与到期操作的 `rotation.remove` 分别只有 **30.9 ms / 36.7 ms**；apply 本次为十份有效备份、无淘汰，未记录 remove span，不能写成 0 ms。本轮主要成本落在轮转中的完整性检查，不能归因为删除文件。

apply 另外直接观测到 `restore.apply_pending_total` 的直属 validation span 合计约 **6.584 s**，以及该 guarded 阶段自身 exclusive 约 **17.452 s**。后者尚无更细 span 归因，不能直接写成某一项复制、hash 或同步耗时；它与 rotation 及直属 validation 的范围分别保留。现有事件未完整包装直接 guarded validation，所以下述计数边界仍适用。

## 数据保全与实际闭环

本轮创建单次使用的新隔离文件库 fixture，**50,000 资产、初始 10 份有效备份，最终 10 份有效备份**，SQLite `3.46.0`，validator cache 为 `32768 KiB`。这项 cache 配置不等于进程内存测量。每份最终备份均实际执行验证。

`profile_business` 枚举 main 中非 SQLite 内部、非 shadow 的表，对每列区分 Null、Integer、Real 位值、Text 和 Blob，排序逐行摘要并保留行数；本 fixture 共记录 26 表，含实际为空的表。手动/到期备份、pending 与恢复后的活动库按该 typed 基线比较，保留资产、Prompt、标签、DNA、Reference、集合、生成索引、合成 embedding 及搜索表的实际数据范围，不扩大为任意数据的无条件保证。

恢复前实际修改一条 Prompt，确认其 typed 结果不同于基线；关闭 AppState/连接后调用正式 `apply_pending_restore`，再经 `db::init_db` 重开、验证并确认恢复到基线。pending 与 rollback 不再存在，**1 份保留原件 raw** 仍可验证且 typed 内容等于修改后的状态，证明恢复目标与原件分别保留。此处是同一测试进程内关闭连接再重开，没有启动新桌面进程。

原始 DB/WAL/SHM 加十份来源备份共 **13 文件**。准备阶段从只读独占句柄复制 raw 与工作副本，仅工作副本执行 SQLite checkpoint；运行阶段重新取得并持续持有十三个只读独占句柄，覆盖原生 workflow 与 consumer。最终 source-terminal 记录十三文件前后 SHA256、大小和 mtime 均相同，并与准备清单的前后值/raw 摘要相符；原始源未由 SQLite 打开。准备与运行之间没有连续持有句柄的声明，漂移须在运行前拒绝。此次复核只消费小型回执和报告，未另行重读大数据库或测试 EXE 干扰计时。

## 来源与未记录范围

68 项编译/include 输入、归档和脚本绑定当前 `1da1371` 源码，并衔接此前完整库回归的源码清单。optimized 构建回执为 build-only，构建本身执行测试数为 0；本次独立 native 子进程实际执行测试，退出码 0、verificationErrors 为空。公开记录不包含本地绝对路径、邮箱、数据内容或密钥。

| 证据 | SHA256 |
| --- | --- |
| 实际 optimized libtest 归档 | `aea1b80bb3c2987781e5a3b4ebc4e6474b864362127dca6322e2086eda0e71cd` |
| 原生运行终态 | `7f70c348da7e47b78a52df9970b9b2c79293a536b7ad1a29bb9f6708998bb9d8` |
| 诊断消费汇总 | `460e74b8ef6290ad7b8c596b5e87b54197b4364f813097e54c75db7c2d3499ad` |
| 原始源保全终态 | `cd62869ca353975d9c30a6efffc439005492e3f6b7b60a526220a1e442a6bc55` |

四类实际状态分别为 `PASS_SINGLE_RELEASE_NATIVE_WORKFLOW`、`PASS_SINGLE_RELEASE_CONSUMER`、`PASS_SINGLE_RELEASE_SIX_OPERATION_DIAGNOSTIC` 与 `PASS_ORIGINAL_SOURCE_READ_HELD`。六份报告、summary、stdout/stderr 的摘要已分别核对，consumer 对原始报告的 wall 值与本次独立读取一致。

当前 legacy serial producer 的 `fullValidationCalls` 上报值就是上述 wrapper 数；直接 guarded validator 调用未全部包装，因此完整 validator 总调用数和 worker 数在消费结果中保留为 **null / 未记录**。五类 validation span（open、configure、integrity_check、portable_ids、foreign_keys）的 `resultOk=null` 数按表中顺序为 **0、5、60、65、10、65**；null 只表示未记录，不能写成 SQL 成功、失败或零次执行。操作 root/返回值及 typed/reopen 断言分别提供本次结果证据，未记录 span 不代表零耗时，独立 worker 候选的计数不借入本轮。

## 下一步与验收边界

本轮各操作只有一个样本。复制、hash、clone checkpoint、启动和未计时断言会预热缓存，操作顺序固定，没有控制 OS 缓存冷；不构成 20 warm / 3 cold 分布、GUI/IPC、长期内存、原生资源、安装升级或可信发布验收，也不支持跨候选提速比较。此前 38 场景/874 样本的 Rust 查询规模验证与本六操作诊断各自保留范围，不能合并为阶段 A 或 v1.0.0 整体验收完成。

下一步以当前已回归源码为基线，在隔离候选中兼容现有两 worker 轮转方案，针对上述实际热点优化，并以同来源、50k 数据、初始十份有效备份及相同观测范围复测，同时保留既有保全与拒绝策略。新的 worker-current 隔离源码正在准备，尚无本轮测试或性能通过结论；旧 worker 的 debug 单样本不能与当前 optimized 串行样本折算收益。正式性能结论仍须按原冷暖样本与预算建立，不能凭本轮基线直接推广或宣布提速。

文档提交 `41bb619` 的[候选敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39720297342)通过后已正常推送 main；main 的[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39720439249)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39720439259)成功。这些源码/文档门禁不替代上述真实原生 workflow 与原件保全回执。
