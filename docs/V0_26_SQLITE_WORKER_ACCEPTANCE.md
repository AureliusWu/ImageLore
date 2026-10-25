# SQLite 安全升级后的两路备份校验候选

记录日期：2026-10-25。开发版本 `0.26.0`，schema `11`，阶段 A 仍在进行。

## 来源与范围

候选提交为 `8eeabcbc152e0c01417445b95dbe0af29ee9571a`，位于 `codex/backup-worker-sqlite-security-20261023`。它以 main `f2bc509` 为基线，将此前两路轮换校验整合到已经升级 SQLite 的源码中。生产轮换每批最多两个校验线程，按原记录顺序处理结果，等待本批线程结束后再移动或删除文件；线程启动失败或 panic 按不可用处理。

静态核对保留了原有 61 个备份测试函数体，以及此前八个 worker 回归用例。另将 Native CI 的严格 Clippy 扩展到测试目标，并按新工具链提示调整一个既有损坏向量测试的分块读取方式；原有断言保留。

候选未合入 main，也未替换桌面运行时。历史旧引擎的 40 个交错样本及启动尾延迟风险继续见[预处理对照](V0_26_PRECONDITIONED_BACKUP_SERIES.md)，不转记为本候选的性能结论。

## 远端完整回归

| 检查 | 同提交证据 | 结果 |
| --- | --- | --- |
| 普通 Native | [39918270591](https://github.com/AureliusWu/ImageLore/actions/runs/39918270591) | 244 passed / 0 failed / 3 ignored；0 filtered；1229.45 秒 |
| 优化 Native | [39918274753](https://github.com/AureliusWu/ImageLore/actions/runs/39918274753) 的 `optimized-library / optimized-library` | 244 passed / 0 failed / 3 ignored；0 filtered；993.81 秒 |
| 前端检查与构建 | 同一 [39918274753](https://github.com/AureliusWu/ImageLore/actions/runs/39918274753) 的 `verify` | success，含 Windows ACL fixture 生命周期与版本同步幂等检查 |
| 敏感信息门禁 | [39918134009](https://github.com/AureliusWu/ImageLore/actions/runs/39918134009) | success |

上述结果已从原始日志独立复核。优化任务枚举 247 个实际可执行测试，执行完整套件，三项既有 ignored 用例仍需各自显式验收。七个子步骤的退出码与日志哈希一致；构建前后 188 个受版本控制文件相同，并与指定提交逐文件核对（187 项直接字节相同，一项为 CRLF checkout）。

优化构建使用 Rust `1.99.1`、`opt-level=3`，关闭 debug assertions；Cargo 的 `fresh=false` 表示本次重新生成了测试产物。其 EXE SHA-256 为 `37acff7fd8eb117d92c3ec63f6699233ec46c3f03a867bf0e43b6037bc9fff65`。工作流只归档 JSON 与日志，未上传 EXE/DLL。证据 ZIP 的服务端 digest 与下载字节 SHA-256 均为 `353a300d5ea94d3cfebb86d12d1e681d42818558a18a95936979916f9799503c`。

首次候选 `73d3194` 的 [Native 检查](https://github.com/AureliusWu/ImageLore/actions/runs/39916546433) 因测试目标新增 Clippy 提示失败，测试未执行；该失败记录保留。上表属于修正后的 `8eeabcb`，不覆盖早期失败。

## 本地候选与后续验收

10 月 23 日的本地构建中断，缺少终态与归档，保留为未证明完成的尝试。10 月 25 日在新证据目录重新构建，实际编译并归档优化库测试 EXE，68 项编译输入前后相同，SHA-256 为 `c4b59f62724520e237cc1f19a3eb6a1f47dc4c3077a47e3e59138a84f70c0936`。该身份与远端工具链产物分别登记。

该本地产物的完整库回归实际 **244 passed / 0 failed / 3 ignored、0 filtered（195.97 秒）**，68 项输入与归档身份在执行前后相同；测试列表为 247 项，使用单测试线程完整执行。构建回执、实际进程退出码、原始 stdout/stderr 与哈希均已保留。

## 当前引擎六操作诊断

使用上述已完成完整回归的同一归档，单独执行备份诊断：实际 **1 passed / 0 failed / 0 ignored、246 filtered（228.80 秒）**，`SELECT sqlite_version()` 返回 **3.53.2**。完整库回归中的三个 ignored 用例未被统一改为通过；这里只显式执行其中的六操作 profile。

| 后端操作 | 本次耗时（ms） | 完整校验次数 | worker 报告数 |
| --- | ---: | ---: | ---: |
| 正常启动 | 1764.9249 | 0 | 0 |
| 检查近期备份 | 1728.8338 | 1 | 0 |
| 手动创建 | 16085.0322 | 12 | 11 |
| 到期自动创建 | 66541.1075 | 13 | 11 |
| 暂存恢复 | 9385.6442 | 2 | 0 |
| 应用待恢复备份 | 55957.6426 | 13 | 10 |

50,000 条资产、初始与最终各 10 份有效备份、26 张表的 typed 内容，以及关闭连接后恢复和重开检查均通过；恢复前修改状态保存在一份 raw 中。十三个原始源文件在准备阶段及整个 Native/consumer 阶段以只读共享句柄保护，前后字节数、SHA-256 和时间戳相同，SQLite 仅打开工作副本。

独立复核确认六项根操作成功、实际 Native 退出码为 0、原始日志与每份报告哈希一致。父线程和 worker 的校验次数分别核对后求和；每个完整校验包含唯一的全部 SQL/业务阶段，所有被记录的 SQL 结果均为成功。worker 报告均已消费且无 failure origin；跨线程时钟不相加，报告本身不证明 join 与消费的先后时间，顺序保护另由同源码回归用例验证。

这些是**每操作一个样本**。复制、哈希、checkpoint 与固定操作顺序会影响缓存，且与 10 月 23 日[串行基线](V0_26_SQLITE_BACKUP_ACCEPTANCE.md)的运行日期和主机负载不同。正常启动本身也有明显差异，因此不将两轮耗时直接相除作为并行收益。正式 20 warm / 3 cold、完整应用、长期内存和候选推广仍未证明；到期备份与恢复耗时也继续保留为待优化观察。

本地证据位于私有验收目录：`backup-worker-security-native-build-20261025-01`、`backup-worker-security-native-validation-20261025-01`、`backup-worker-security-diagnostic-20261025-01` 和 `backup-worker-security-review-20261025-01`。原始数据库、日志、截图和机器路径不随本文上传。关键结果哈希：

| 记录 | SHA-256 |
| --- | --- |
| 本地构建回执 | `e399494ee076083f10d8c91b384083583f63cb2d4a2b2fc2933c3cf4c2dd1145` |
| 本地完整回归 stdout | `3cf2f4a655eb03065d68fe002cf28b6d5c08f7db1ea5b6a1a62e6c07d105201e` |
| 六操作 Native 回执 | `6802d10506b5fbdd67fcd896ef9e6cf7a15ea5d7357039f6d69f1eb707877bc0` |
| 六操作来源绑定 | `b5e14398ea92939eb41088dbbe7d1963957819475781a6dbc29b9533faddc181` |

## 桌面现场与发布边界

独立桌面程序仍绑定安全升级源码 `1dfe369`，不是本页 worker 候选：EXE SHA-256 `f32d25ace7ebd358b486f3907a20cc0246eda2480dac5e88c47ce5ebd589bdad`。它在内存准入满足后实际启动并渲染空白隔离库，随后正常退出，退出码为 0。关闭后原 DB/WAL/SHM 以只读共享方式持有，将完整副本交给 SQLite checkpoint 后检查：schema 11、integrity `ok`、外键错误 0、资产/提示词/版本记录均为 0；原始三文件及十二张合成图保全。

文件夹选择交互受窗口控制画面不一致影响，未完成导入。后续桌面操作由用户延后，导入、编辑、保存重启、备份恢复及完整应用性能仍待验收。既有运行时和快捷方式哈希保持不变；正常用户资料库检查仅覆盖列出的六个 DB/WAL/SHM 路径，不扩大为全盘数据审计。

本记录不关闭真实桌面、完整应用冷暖、长期内存、签名、安装升级、updater、RC 或 v1.0.0 发布条件。继续按 ROADMAP 的 A → B → C → D 顺序推进。
