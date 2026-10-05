# v0.26 备份候选校验两路对照

复核日期：2026-10-05。状态：`COMPLETE_EXPLORATORY_ONLY`。新一轮 3×ABBA 共 12 次实测已完整结束，探索性等价检查通过；只能支持继续审查两路候选方案，尚未通过项目正式性能门槛、生产整合或 GUI 验收。

## 来源与测量范围

外部实验代码导出自 `77dbf70d7b945f3a4655cf5a1d308d28407b7e37`，该 HEAD 的生产实现沿用 `ec8874f`。`<project>` 的生产文件在实验前后保持相同 SHA/size/mtime，生产 `rotate` 未改动。仅外部 checkout 的 test 模块加入实验 helper；独立 release Rust lib test EXE 已编译并实测，不能转记为桌面 EXE 或新生产候选已验证。

| 已绑定输入/产物 | SHA-256 |
| --- | --- |
| `77dbf70` 导出的 source archive | `e73cc0a03223e62f931309cb876358d7c6671100b9ae00adca178b6600763604` |
| 原生产 `<project>/src-tauri/src/backup.rs` | `daddb47e3557dafd5b8eb7d3045f549f60464f4dc046260706164e8177afbc2f` |
| 外部编译 `backup.rs`（仅 test include 差异） | `d9eff220fbae63e8ab351825e0eebe400ca30357fbd7a770c69ad3a01b82ae89` |
| 外部编译 `db.rs` | `4780c85d349b2aefa2c34cacb427d3b638089088406f6e9e810333fae1c1c9c2` |
| 外部 test-only helper | `33ad3122bf89d460959a9b4787c421c47f923734bf3801cc654f745f4667606f` |
| 独立 test EXE（37,291,008 bytes） | `1c5430b155eee8f8335b40ea5b7dc382975f56decba654968cbfb6eeb72c130b` |

输入为封存的合成 50k 快照的新副本，共 11 个候选，每个含 50,000 个资产。10 份直接副本保留源 mtime；第 11 份采用最新快照的相同字节/SHA，复制时 mtime 刻意加 1 秒，用于建立最新 ordinal。运行期间保全比较基于这份新 fixture 的初始状态，不能把这次排序设置写成源文件变更。

A 每 batch 1 个 worker，B 每 batch 最多 2 个 worker；两者均以独立 Connection 执行原完整 `validate_typed`，每 Connection cache 为 32 MiB。每 batch 全部 join 后按既定 ordinal 消费结果，只生成 retention/quarantine 的 dry plan，未调用真实 `rotate`、quarantine 或 delete。

每个样本的 `wallMs` 从实验 `owned_run` 内创建 Instant 开始，覆盖 worker 创建、完整校验、join、按序分类与 dry plan 汇总，至返回测量结果前采样。前后完整文件哈希和 typed 业务摘要在该计时区间外。各 worker 保留自身 TLS trace；并行子 trace 不加到父子 span 或 exclusive 时间，不能用 worker duration 之和代替 wall time。

## 全部完成样本

顺序固定为每轮 A、B、B、A。下表直接列出 12 个落盘样本的 `wallMs`，未筛选快样本或混入旧记录。

| Round | Slot | Variant | wallMs |
| --- | --- | --- | ---: |
| 1 | 1 | A | 54775.3391 |
| 1 | 2 | B | 51347.0587 |
| 1 | 3 | B | 64990.2338 |
| 1 | 4 | A | 103805.4837 |
| 2 | 1 | A | 99589.9628 |
| 2 | 2 | B | 55782.4402 |
| 2 | 3 | B | 65139.1270 |
| 2 | 4 | A | 101677.3929 |
| 3 | 1 | A | 102372.8662 |
| 3 | 2 | B | 26947.4418 |
| 3 | 3 | B | 20881.9143 |
| 3 | 4 | A | 55924.5327 |

| Round | A mean（s） | B mean（s） | `(B/A−1)×100%` |
| --- | ---: | ---: | ---: |
| 1 | 79.290411 | 58.168646 | -26.64% |
| 2 | 100.633678 | 60.460784 | -39.92% |
| 3 | 79.148699 | 23.914678 | -69.79% |

| Variant | 实际完成数 | Mean（s） | Median（s） | Worst（s） |
| --- | ---: | ---: | ---: | ---: |
| A | 6 | 86.357596 | 100.633678 | 103.805484 |
| B | 6 | 47.514703 | 53.564749 | 65.139127 |

总体 median 差异为 -46.77%，仅作本轮描述性统计。OS 缓存和后台资源未清空、固定或测量归因，未证明全量集合处于 warm/cold 状态；三轮及同轮差异的原因尚未隔离，不能将上述降幅承诺为真实使用提速。

## 完整性与保全

12 份样本各有 11 个有效 outcome，实际共 132 次完整 validator 调用；每个候选的 `validation.total`、integrity、schema tables、foreign keys、business relations 阶段各出现一次。前后 typed 业务摘要（含向量 BLOB 和可见 FTS 行）、SHA/size/纳秒 mtime、WAL/SHM 存在性均与基线相同。typed 摘要本身不覆盖 SQLite internals/FTS shadow；本轮同时比较原始文件 SHA 保全字节。

独立契约样本验证了明确 Rejected、真实 exclusive lock 导致 Unavailable、worker panic 导致保全并中止，以及按序消费和 dry plan 一致。同 batch 后序只读校验可能已完成，但 Unavailable 之后不消费。文件不变，删除仅是计划；真实生产 retention/quarantine/delete 回归仍需候选整合后执行。

原始样本文件写在 Rust assertions 之前，单独存在不能证明测试通过。本轮后续样本、最终 Rust summary 与终态 `exit 0 / 1 passed / 0 failed` 共同确认全部 assertions 完成。封存回执同时确认原始 frozen tree 的 47 份文件元数据、5 个生产源码输入及外部编译输入不变。导出文件字节审计覆盖 159 个 tracked 文件，唯一既有文件差异为 test include，helper 单独绑定 SHA；未用 SQLite 打开 frozen 源，未删除源 fixture，未重启 GUI，未触及真实用户库。

资源上限为新授权的 1,800 秒。`run-result` 的 1196.6743985 秒从 Cargo Process.Start 前立即启动 Stopwatch，到进程终态、stdout/stderr drain 和日志写入后采样；不是 whole flow，也不是单个校验耗时。watch 起点为 `2026-10-05T14:58:26.3711472Z`，Cargo start 为 `14:58:26.3734963Z`，run-result 写入为 `15:18:23.0480736Z`。预备 50.7949231 秒、最终 verify 14.8823841 秒和 seal 9.4261521 秒另有计时，后两项并行执行，不能机械相加为总耗时。

RAM 仅有 Cargo 启动前（含随后编译阶段）的空闲物理内存 2,865,392 KiB 与终态封存时 2,954,864 KiB 两个上下文点，总可见内存均为 16,718,540 KiB。它们不是精确 test 起点、进程峰值或 20% 内存门槛证据。

## 证据与尚待完成

私有回执位于 `<private-evidence>/backup-readonly2worker-20261005T145457-eeec49e5/`。公开文档只保留脱敏结果，原始日志与 payload 不入库。

| 回执 | SHA-256 |
| --- | --- |
| `final-report.md` | `17dff76fab6101eefcb218d76d34ec23b4e5217b27dc457408ab4c07c2ae241f` |
| `final-analysis.json` | `f2a915a7f646f56b6c3abcbc4d3155ecf0ccf3949d30478feaed650909f3c249` |
| `final-review-manifest.json`（绑定 40 个审查产物） | `1248d7037d53bf5c1731aafd7f5ade957820f200b86d93d6d9f7c5fb5f30697a` |
| `results/summary.json` | `f5a564645fc2bf0f18ac6dcd1c1d316f888d673e7f525476b8e3e6d417c22c58` |
| `sealed-artifacts.json` | `3d58dfc60cb4bb8d75b01100d1fe7eb6f31bfc56e43780f38382c1d78ec4d4e0` |
| `source-preservation.json` | `ea15074cf1045fc246d216bde3ff86378dca7ba30d1ec134834f58a4f21c8973` |
| `timing-scope.json` | `3b25450f9d8ff744cfbffa6df74f54cb0777a65012136fc55aad704a5fba6c8c` |

旧 1,200 秒资源看门狗中止的 partial 实验独立保留为 `INCOMPLETE`，不能并入本轮统计或升级为通过。每 variant 计划 6 次与实际落盘 6 次分别确认；无论哪一种，都不等价于项目 warm≥20/cold≥3 门槛。

仓库外两路 production candidate 已准备，但尚未整合、编译或测试。仍须完成代码与 `workerTraces` 消费兼容审查、Clippy/全 lib 与必要回归、正式性能样本、真实 manual backup/restore 和 GUI 验收；缺失父 TLS validation phase 不能被旧消费者解释为 0 ms。当前不生成安装包，不构成 formal release，也不关闭 v1.0 未完成门槛。
