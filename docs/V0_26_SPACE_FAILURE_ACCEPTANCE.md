# Windows 恢复空间故障验收

日期：2026-10-22（Asia/Shanghai）。本记录接续[pending 暂存与进程终止验收](V0_26_PENDING_STAGE_SAFETY.md)，只覆盖隔离合成资料库的 Windows 后端恢复入口。应用版本保持 `0.26.0`、schema 保持 `11`；没有启动 GUI、修改正常用户资料库或推广桌面产物。

## 本批结果与源码身份

在 `9284eaf0ab0b29e8e9358bd8abb5ba4a74944d44` 的 Role 候选之上增加测试专用故障注入和五个测试。定向门禁 `conflicts-space-20261022-01` 实际 **5 passed / 0 failed / 0 ignored**（27.22 秒），退出码 0；原始 stdout 包含 **18 个正式入口故障场景的 PASS 标记**，另有一个精确范围/清理控制测试。场景数与 Rust 测试数分别登记，不相加。

定向门禁实际新编译测试 EXE，其 SHA256 为 `b7f7c54beb0f1c8a671e625c7abe347144389b6c3fb1f6c94ae1846d90fa3915`；67 项编译/include 输入及 HEAD 在执行前后保持相同。包含测试代码的严格 Clippy `clippy-space-20261022-01`（`--lib --tests -- -D warnings`）通过。完整 library suite `full-space-20261022-01` 实际 **230 passed / 0 failed / 3 原有 ignored**（203.51 秒），退出码 0，67 项输入前后相同；它沿用定向门禁新编译的同一测试 EXE，Cargo 报告产物复用，不声称完整套件又编译一次。独立产物归档 SHA 与该 EXE 一致，67 项输入均另作逐文件归档。

本批源码绑定中的六个修改/新增文件如下，完整 67 项输入记录、原始 stdout/stderr、终态回执及产物另存于仓库外。公开记录不包含用户邮箱、密钥或本地绝对路径。

| 文件 | SHA256 |
| --- | --- |
| `src-tauri/src/backup.rs` | `a040d596348759a55c099c37a84938267715da0bd6e1a1ff4c25cf5209b48edb` |
| `src-tauri/src/backup/guarded_restore.rs` | `7e78f2837cd33897de46ff421ade9f7e018712a0c4425f008dfdbfcde6ed5a35` |
| `src-tauri/src/backup/owned_handle.rs` | `ed5c324797ce0bee0f9f49681786b7bd51f6635ba4077abea6ca6572879e1690` |
| `src-tauri/src/backup/restore_process_exit_tests.rs` | `c96ded8c289c67014a9604a95a6f8ce1dc5eb902d492749caf905a5f535a4ed2` |
| `src-tauri/src/backup/space_failure_tests.rs` | `f33c7ce7b8476f321b2fcdab41b87fed6531cb906e9d41bcdeecfc5e995c6ecd` |
| `src-tauri/src/backup/space_fault.rs` | `d7e62154d7d1dd83401bc3fca3530a00e98cab92ffd9a9fa75c7463b2eb399e3` |

2026-10-20 的首次定向尝试只留有 claim/started，没有 terminal 或可证明执行完成的原始结果。2026-10-22 复查原会话句柄已不存在、对应 PID 和 Cargo/rustc/libtest 进程均已退出，认定为**中断且结果未知**；既不算通过，也不改记为测试失败。原文件与现场保留，本次使用新的证据后缀执行。

## 故障边界与实际文件断言

三种用途分别调用正式的应用 pending、暂存 pending、自动损坏恢复入口。fixture 使用真实文件 SQLite DB/WAL/SHM，业务提交依赖未 checkpoint 的 WAL；自动恢复原件沿用 schema 11 可读、额外索引单叶损伤的 fixture。读取业务和完整性时使用独立副本，避免 SQLite 改写待比较原件的侧车。

| 故障 | 场景数 | 实际动作与核验 |
| --- | --- | --- |
| PREPARED 部分写入 | 3：每种用途一案 | 创建记录并实际写入、同步 17 字节后返回 Windows 错误 112；只清理本次尚未登记的自有记录与事务。原件和源候选完整指纹保持，移除注入后同目录正式入口重试成功。 |
| raw 部分复制 | 9：三用途 × DB/WAL/SHM | 新建唯一 raw 并实际复制、同步 31 字节后返回 112；检查输出确为相应原件的严格前缀，保留该对象的字节、身份和 mtime。原件与源不变，raw 不进入普通备份候选；正式重试成功后部分 raw 仍保持。 |
| COMMITTED 部分写入 | 3：每种用途一案 | 新库安装并完整验证后，实际写入、同步 Commit 的 17 字节前缀再返回 112。严格解码拒绝截断 Commit；已安装库、预留原件、raw、源候选和记录保留。随后三次正式入口调用都拒绝，全部现场文件的完整指纹保持。 |
| SQLite 容量不足 | 3：每种用途一案 | 在正式暂存 VACUUM 边界，针对唯一命名、8 KiB 上限的内建 memdb 执行实际 `VACUUM INTO`；返回原始 typed `DiskFull`、扩展码 `SQLITE_FULL / 13`。错误原因保留，原件、源与原文件现场不变，移除注入后正式重试成功。 |

Windows 112 是在真实前缀写入/复制及 `sync_all` 成功后注入的 IO 错误，不能称作自然发生的 NTFS 磁盘满。SQLite 13 则由有界 memdb 的实际引擎操作产生，也不代表耗尽主机磁盘。此前独立引擎探针的 8 KiB 失败、8 MiB 正常控制与源数据保全，继续按其自身 1/0/0 的身份保留；本批三案是接入 ImageLore 正式入口后的新证据。

故障计划仅编译到 Windows 测试，以精确目标路径和 IO 类型匹配，线程局部一次消费，并由 RAII 恢复之前作用域。raw 在正式保全流程确定唯一实际目标名后才绑定该路径；SQLite 上限只影响本案的唯一命名 memdb，不改全局 SQLite 或系统容量设置。控制测试证明不同路径、不同 IO 类型不会消费计划，离开作用域和 panic unwind 后正常复制/写入可完成。

截断 Commit 的正确结果是停止并保全，不能为了得到重试成功而猜测提交状态、删除协议记录或回退已安装内容。其他可清理故障在撤销注入后使用同一测试进程内的正式入口重试；这证明后端返回、释放和再次进入的行为，**不算新进程启动、桌面保存重启或 GUI 验收**。

## 与已有证据及剩余验收的关系

此前 Role 候选的 **31 次真实 Windows 子进程强杀**与完整本地 **225/0/3**（200.57 秒、65 项输入）继续按[pending 暂存与进程终止记录](V0_26_PENDING_STAGE_SAFETY.md)分别绑定。2026-10-22 02:29 UTC 观察时，该候选 `9284eaf` 的[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39711213364)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39399396084)已成功，[Native](https://github.com/AureliusWu/ImageLore/actions/runs/39711150959)仍在运行、待终态。此状态只绑定 Role 候选，不能替代本批空间故障源码的远端验证。

更早 `c19a2f6` 的 13 案源码已有候选 Native **224/0/3**（709.34 秒）；同源码的 [main Native 复跑](https://github.com/AureliusWu/ImageLore/actions/runs/39397136749)实际 **224 passed / 0 failed / 3 ignored**（1030.72 秒），原始日志已另行归档。它们不覆盖新增 31 案或本批空间故障源码。

本批补上明确范围的恢复空间故障证据并完成本地完整回归；A1/A2 整项仍须按原要求复核完整矩阵及剩余验收。后续仍须按计划完成真实 Native 备份/恢复性能对照、50k/75k 与冷暖测量、长期内存、桌面保存/重启、安装升级、可信签名、updater 和 RC。没有把物理主机断电或耗尽主机磁盘追加为新发布条件，也没有将本地后端通过转记为阶段 A 退出或 v1.0.0 发布完成。
