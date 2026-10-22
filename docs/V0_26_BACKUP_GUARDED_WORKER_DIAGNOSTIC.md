# v0.26 受保护恢复与两路备份校验整合诊断

日期：2026-10-22（Asia/Shanghai）。状态：`PASS_SINGLE_DEBUG_DIAGNOSTIC`。两路候选校验已整合到具有恢复事务、文件身份保护和进程中断续作的候选源码，并完成一次独立文件库的六操作实际诊断。完整业务和保全断言、报告消费者均通过；这是每个操作一个样本的 debug 后端诊断，正式性能、GUI、内存、安装升级和发布验收仍未完成。

## 受测源码与实际执行

测量时源码为 `9284eaf0ab0b29e8e9358bd8abb5ba4a74944d44`，加上 `backup.rs` 与 `backup/guarded_restore.rs` 两处候选更改；当时完整二进制 Git diff 的 SHA-256 为 `9a1c3db3764c1bd8cff022e99460d9f826c109f89a0d122eb069a347b67c10fe`。仅该基线提交不能代表已经包含本次整合。编译和执行回执同时绑定 65 个输入的字节、SHA、大小与 mtime，并封存源码快照及独立测试 EXE。

| 受测输入或产物 | SHA-256 |
| --- | --- |
| 编译 `src-tauri/src/backup.rs` | `a817f112dd3f8e8494b7b5422c1b5d993186664dbd878c24e19eb98969516a10` |
| 编译 `src-tauri/src/backup/guarded_restore.rs` | `5879855deafc5a61f424dc44ad58613cfcc784441d2495d520aa9670188934c0` |
| 编译 `src-tauri/src/db.rs` | `4780c85d349b2aefa2c34cacb427d3b638089088406f6e9e810333fae1c1c9c2` |
| 新编译、归档的 debug Rust lib test EXE | `d905ff4439af53e93ffb432dbef8cf1e60afcf540348617af654aa62ead9e416` |

同源门禁实际通过：`cargo check --offline --locked --lib`、`cargo clippy --offline --locked --lib --tests -- -D warnings`，以及未过滤的默认库回归 **233 passed / 0 failed / 3 ignored，193.83 秒**。实际命令同时指定 `--manifest-path src-tauri/Cargo.toml --jobs 1`；回归另带 `--message-format=json -- --test-threads=1`。三个 ignored 重型测试没有被默认回归计入通过数。

随后直接运行上述归档 EXE 中的重型测试，不再经过 Cargo，也没有启动应用 EXE：

```text
<archived-debug-lib-test.exe> backup::tests::profile_file_backed_backup_phases --ignored --exact --nocapture --test-threads=1
```

原生终态为 `exit 0 / 1 passed / 0 failed / 0 ignored`，六操作断言全部完成；独立报告消费者随后 `exit 0`。运行前后复核同一 full receipt、测试 EXE、65 个编译输入和诊断脚本身份。准备器、native 流程与消费者使用新建、单次使用的输出；失败或中断产物不会覆盖、删除或重用。

## 原源保全与独立工作库

源为仓库外标记的合成 50k 验收库，包含原 DB、已提交 WAL、SHM 和十份有效备份。准备器先对这 13 个源文件全部取得持续持有的独占只读句柄，再分别生成 raw bundle 与独立工作副本；源文件 SHA、大小和 mtime 前后相同。没有通过 SQLite 打开原源库，没有丢弃已提交 WAL。

raw bundle 保存 DB/WAL/SHM 原字节。工作副本保留 DB/WAL，SHM 由 SQLite 在副本内重新生成。仅工作副本执行 checkpoint，结果为 `[0,0,0]`；schema 11、全部业务表的类型化行摘要、向量 BLOB、可见 FTS 行、schema 定义、完整性和外键检查前后相同，正常关闭后工作库的 WAL/SHM 消失。类型化业务摘要不单独证明 SQLite 内部页或 FTS shadow/posting 的物理等价；原始字节保全另有 raw SHA 校验。

六操作使用真实生产后端函数，完成实际创建、轮换和恢复。初始十份、最终十份有效备份。手动和自动备份的业务摘要与基线相同；暂存后活动库不变；关闭活动连接后先观察到测试改写，再应用 pending 恢复并回到原业务基线。恢复前改写原件保留为 raw，之后也核对其业务内容。测试未使用真实用户库。

## 六个操作的单样本结果

下列 `wallMs` 为该操作根 span 的实际时长，单位毫秒。完整 validator 调用数同时包含当前线程与独立 worker 的完整调用，由消费者逐次核对七个校验阶段及五个 SQL 结果；调用数与 worker 报告数不是相同口径。

| 实际操作 | wallMs | 完整 validator 调用数 | worker 报告数 |
| --- | ---: | ---: | ---: |
| 正常启动的后端数据库路径 `startup_normal` | 2405.0285 | 0 | 0 |
| 已有近期有效备份的自动判断 `auto_recent` | 9437.7692 | 1 | 0 |
| 手动创建备份 `manual_create` | 24191.8543 | 12 | 11 |
| 到期自动创建备份 `auto_due_create` | 25825.2573 | 13 | 11 |
| 暂存恢复 `stage_restore` | 87165.4018 | 2 | 0 |
| 关闭活动连接后应用恢复 `apply_pending_restore` | 286681.4832 | 13 | 10 |

启动的零次完整 validator 调用不表示零次完整性检查：启动仍走独立复制探测路径。各个已调用 validator 保留完整性、schema、portable ID、外键和业务关系检查，没有跳过验证或复用持久“已验证”标记。每连接校验 cache 保持 32 MiB。

两路轮换每 batch 最多两个 worker，使用独立连接完成原完整校验，全部 join 后按原排序消费结果。真正的排序、隔离和十份保留逻辑已经进入此次实际操作；没有把旧 dry plan 当成本次真实轮换。

`workerTraces` 保持各自 TLS session 和时钟；消费者验证所有本次 worker 为 `Valid`、被消费，完整 validator 的阶段没有遗漏或重复包裹。trace 的 `joinBeforeConsumption` 仍为 **`not-recorded`**，不能伪造为 trace 已直接证明 join。该顺序和有界并发由同源实现与默认回归中七项 rotation 测试验证：反序完成、实际 Unavailable 后仅消费前缀、最新十份有效候选、独立副本保全 WAL、panic/spawn failure 保全、独立 session 计数和失败后未消费的投机结果。另有一项 guarded stage profile 回归核对直接 probe 的完整调用数，防止遗漏或双计。

父 span 包含子 span，不可相加重复计时；worker 独立时钟不能叠加为父线程耗时。受保护 Windows 恢复路径未被现有阶段记录覆盖的细项仍为“未记录”，不能沿用旧 TLS 阶段名解释成 0 ms。该轮可以观察上述总时长与校验次数，尚不能把恢复耗时归因到未记录细项。

## 性能结论与后续边界

这是固定顺序下每操作一次的 **debug** 样本。准备复制、源/raw/clone 哈希、checkpoint、运行前完整哈希及未计时的业务校验均会预热 OS 页缓存；没有控制或证明冷缓存，也没有生成正式 warm 样本。`formalWarmSamples=0`、`coldSamples=0`、`guiActions=0`。

本轮的构建模式、源码恢复保护、初始备份数和工作范围与 [10 月 2 日 Release 单样本](V0_26_BACKUP_PHASES.md)及[10 月 5 日 ABBA 只读 dry plan](V0_26_BACKUP_ROTATION_PAIR.md)不同。旧 Release 的 168.43 秒是含额外检查的全测试时长，不能与本轮单个操作或 debug 全流程相除；ABBA 的 worker/dry-plan wall time 也不是本轮真实备份/恢复 wall time。这里不计算改善率，不声称已经提速。

后端启动 2405.0285 ms 不包含 Tauri/WebView、任务池排队、首屏或库就绪 UI，不能据此关闭 GUI 小于 3 秒的门槛。项目仍需同条件的正式至少 20 次 warm、至少 3 次 cold 样本、内存与真实桌面验收。此次整合不能替代安装升级、签名、自动更新、RC 或 v1.0.0 发布证据。

## 脱敏证据索引

原始库、raw bundle、私有路径、日志和完整 payload 均在仓库外保留。公开文档只记录结果和身份摘要。证据位于 `<private-evidence>/backup-worker-role-compat-20261020-01/`、`<private-evidence>/backup-worker-diagnostic-20261020-01/` 与 `<private-evidence>/storage-phase-profile-worker-debug-20261020-01/`。

| 回执 | SHA-256 |
| --- | --- |
| `full-worker-role-20261020-01-terminal.json` | `6fba0a4440ec09122ad637d3c09687a55c5b6d27108cd7a886d0e204e6de93a1` |
| `diagnostic-binding.json` | `30c288c92638cd563459fa78ab8cf45091ebc3bbbe1096845d69babc3d5dd6ad` |
| `native-terminal.json` | `159649d80dfa614215aca4faee0a80e03300e0ee0a55a11ee54a2d6ace9c19f0` |
| `consumer-terminal.json` | `03d5826e96742486a8649ef3a47a9212d6e8bb21cebc0b02ae2a95f2eeb02438` |
| `diagnostic-consumer-result.json` | `332e7cc7b2293e29467958d12ca277e861b893aa1fc9c044b3027bee372f9ce7` |
| `fixture-manifest.json` | `1ab5045e44c2f606bcf925c37dc958009df38af0b9a61a4708b83b74c14f5f60` |
| `profile-summary.json` | `af8d623ba971a8830e1879fb536ae3720eab4332c699d509a3ce40c50dd921f2` |

六份操作报告的 SHA 由 native receipt 列出，消费者逐份读取并核对；原生 stdout/stderr 也由回执绑定。旧 `82bb027` 候选、旧消费者验证、失败和中断实验继续封存，各自保留当时的范围和结论，不并入本次 233 项回归或单次诊断通过数。
