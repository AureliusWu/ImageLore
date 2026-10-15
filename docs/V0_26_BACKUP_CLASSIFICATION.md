# 备份不可用错误与损坏错误的分类

日期：2026-10-03（Asia/Shanghai）。这是阶段 A 的备份保全修复；没有发布新桌面程序或验收备份性能。

## 已复现的问题

旧轮换代码把所有校验失败都当作备份损坏，可能在临时目录不可用时隔离一个有效备份。另一个实际 SQLite 反例在完整性检查结束后持有独占锁：真实错误为 `database is locked`，旧校验器却返回“缺少 schema 标记”。

Windows 临时目录反例使用单独子进程，把其 TEMP/TMP 指向自建普通文件，不改变父进程环境。首轮测试在检查带末尾分隔符的 Windows 临时路径时失败，尚未执行产品轮换；这份失败不作为产品反例。修正测试的文件检查路径后，旧轮换返回 `Ok(())`，有效主 DB 被移走，WAL/SHM 留在原处。第二份实际执行记录才证明轮换问题。原始失败及修正后证据均保留在仓库外。

## 修复行为

内部校验结果区分 `Rejected` 与 `Unavailable`，现有调用接口继续返回 `Result<(), String>`。

| 实际观察 | 分类与轮换行为 |
| --- | --- |
| SQLite 明确 CORRUPT/NOTADB；已完成检查发现完整性、外键、业务关系异常 | Rejected，沿用原隔离路径 |
| 明确缺少必要表/列、schema 标记缺失或内容非法、版本不支持 | Rejected，沿用原隔离路径 |
| BUSY/LOCKED、权限或 I/O 错误、临时副本无法创建、未知 SQLite 错误 | Unavailable，停止轮换并保留该候选原路径与文件 |

schema 标记必须是有效 UTF-8 TEXT；不能用 BLOB、数值或 NULL 冒充版本。查询失败保留实际 SQLite 原因，只有完成检查后确认标记缺失才返回缺失错误。原全库 integrity、foreign-key、必需 schema 和孤立业务记录检查保留，连接缓存仍为 32 MiB。没有增加并发校验、放宽历史 schema 或跳过慢检查。

## 实际回归

两个真实反例修复后通过：临时副本不可创建的有效 DB/WAL/SHM 保留原字节、mtime、目录项与哨兵；实际锁错误保留原因。另加扩展 SQLite 错误分类、12 个真实标记异常数据库，以及持有实际独占锁时的轮换保全回归。现有备份测试结果为 35 passed、2 ignored，子进程自身还有一次通过记录，不能把它重复计入父测试数量。

当前工作区同时含新增资产 FTS 优化，完整本地 Native 门禁在该联合源码上通过：fmt、locked check、Clippy `-D warnings`、147 passed / 3 ignored。前端 `npm run check` 与 `npm run build` 通过，Node 回归为 59 项，命令契约保持 85。源码与原始日志摘要保存在 `<private-evidence>`，本地工作区结果不代替最终提交的 CI 或新 EXE 验收。

## 2026-10-15：无辅助文件的 WAL 候选

真实桌面对照发现：已经 checkpoint、只剩主文件的 WAL 格式候选，仍可能被 SQLite 的 `READ_ONLY` 连接在原目录生成空 WAL 和 SHM。源文件头的读写版本为 `2/2`；实际 `VACUUM INTO` 新备份为 `1/1`。不能用 immutable 连接返回的 `journal_mode` 代替这一物理格式观测。本次观察未发现九份保留旧备份的主文件字节、mtime 或文件身份改变，新备份的完整逻辑校验也通过，但原目录新增辅助文件的保护缺口需要修复。

`needs_probe_copy` 现在同时识别已有 WAL/SHM 和 SQLite WAL 文件头，令校验与生成快照都复用既有临时 DB/WAL 副本。普通回滚日志格式仍直接只读校验；短文件和非 SQLite 内容继续由原 SQLite 路径分类，其他文件读取错误保持不可用。没有修改原件、清理已有辅助文件或放宽完整性、外键和业务关系检查。

新增两项真实文件回归：`wal_header_without_sidecars_validation_preserves_source`、`wal_header_without_sidecars_snapshot_preserves_source`。fixture 由 SQLite 实际写入 WAL、checkpoint、正常关闭后仅复制主文件生成，不伪改文件头。测试比较源字节、mtime、目录项、相邻哨兵及无新 WAL/SHM；新快照另回读已提交的 marker、Prompt 和素材计数。原生产实现的两项测试均因新增辅助文件失败；修复后完整 Rust 库回归为 **149 passed / 0 failed / 3 ignored**。最终结果已绑定重新编译的测试产物及测试前后相同的源码 SHA；原始失败和完整日志保留在 `<private-evidence>`。

同一源码的 fmt、离线 locked Clippy `-D warnings`、85 项前端/Native/mock 命令契约及 tracked 敏感信息检查也通过。

本记录冻结时，补丁仍在独立本地分支，未合入 main、未构建或推广新的桌面 EXE。WAL 候选增加的副本 I/O 需要在固定新源码上测量；旧运行时和两路候选的性能数据不转记为本补丁结果。该回归不关闭真实桌面配对、恢复矩阵、长期内存或 v1.0.0 发布门槛。

## 仍需独立验证的边界

停止轮换只保证当前不可用候选不被修改，不会回滚此前已合法处理的其他候选。新快照可能暂时使备份数超过保留上限。已有隔离和保留清理只操作其原有主文件路径，本修复没有新增损坏 DB/WAL/SHM 的原子隔离，也没有解决外部进程并发修改的路径竞争。

[既有阶段测量](V0_26_BACKUP_PHASES.md)属于旧实现。不能把其中的耗时重记为当前源码的性能结果；后续优化仍须保留完整校验和同场景对照。
