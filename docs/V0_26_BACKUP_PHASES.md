# v0.26.0 备份与恢复阶段测量

日期：2026-10-02（Asia/Shanghai）。这是一次独立文件库的 Release 模式诊断，定位完整校验和备份轮换的耗时；没有测量 GUI、任务池排队或冷启动首屏。

## 实际执行与数据保全

受测产品源码为 `a58f55b02e9f5f6133f40447a95403f44bc24576`，加上本次仅测试启用的阶段记录。编译输入 `backup.rs` SHA256 为 `F1C03C1200B095ACF7B452EA587284A36BBB382B2043401AE919286B349CAFB8`，`db.rs` 为 `8F4F60538C63E135DD7E6006E9B09DFAFB3CCE4D62DE9CB3E25F17F3A60C219C`；生产校验、SQL、锁顺序与错误处理保留。

准备器对原 50k 合成图库的 DB/WAL/SHM 和九份备份持有独占只读句柄，先保存原始 bundle，再生成独立工作副本。原 DB/WAL/SHM 包含最后一次已提交编辑，不能仅复制主 DB 或删除 WAL。原文件的 SHA、大小、mtime 前后相同；原库没有通过 SQLite 打开。只有工作副本执行 checkpoint，要求全部业务表的类型化行摘要、可见 FTS 行、schema、完整性与外键检查前后相同，结果为 `[0,0,0]`，正常关闭后副本 sidecars 消失。

实际命令为现有 Rust 库内显式 ignored 测试：

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml --release backup::tests::profile_file_backed_backup_phases -- --ignored --exact --nocapture --test-threads=1
```

测试通过，六个实际操作均成功；资产数 50,000，初始九份备份、最终十份。每次操作之后核对全部业务行，恢复前的改写原件也按字节保留。总测试 168.43 秒包含未计入各操作的业务/文件摘要检查。

## 操作与热点

| 实际操作 | 操作用时 | 完整 ImageLore 备份校验次数 | 轮换候选数 | 轮换用时 |
| --- | ---: | ---: | ---: | ---: |
| 正常启动的后端数据库路径 | 2.054 秒 | 0 | 0 | — |
| 已有近期有效备份，自动判断 | 2.233 秒 | 1 | 0 | — |
| 手动创建备份 | 22.965 秒 | 11 | 10 | 19.538 秒 |
| 超过24小时，自动创建 | 26.181 秒 | 13 | 11 | 20.416 秒 |
| 暂存恢复 | 5.375 秒 | 2 | 0 | — |
| 关闭活动连接后应用恢复 | 63.644 秒 | 17 | 11 | 44.604 秒 |

“备份校验次数”只统计 `validate`，不包含启动的独立 integrity probe。正常启动实际执行复制探测，SQLite integrity check 1.877 秒、DB/WAL复制 0.125 秒、迁移 0.000249 秒。不能据此把启动的完整性检查省略，或把后端 2.054 秒当成已通过 3 秒 GUI 首屏门槛。

手动备份的 11 次 integrity check 共 18.126 秒，foreign-key check 共 2.601 秒，VACUUM INTO 1.583 秒。轮换占本次操作约85%；热点来自逐份完整校验，迁移和文件复制不是本次主要耗时。下一步比较有界的两路完整校验，并保持按原有排序处理隔离和十份保留策略；没有跳过检查、持久缓存“已验证”标记或扩大 SQLite cache。

## 证据边界

完整证据目录：`<private-evidence>\storage-phase-profile-20261002T021151-e7258501`，保留新建单次 fixture、原始 bundle、clone checkpoint 结果、六份操作 JSON、完整 stdout 和 `profile-summary.json`。

复制、哈希及前后业务校验都会预热 OS 缓存，操作顺序固定。连接按原32 MiB缓存执行，没有强制清空缓存；这是一次诊断样本，不能与此前 GUI 85.50 秒备份或84.37秒恢复直接构成性能改善比较。记录的 parent/child span 为包含关系，不能相加重复计时。当前数据库锁等待无竞争，不包含主窗口响应、任务池排队、Tauri/WebView或首屏。

本批 Rust fmt、locked check、lib Clippy `-D warnings` 和默认库回归通过：137 passed、3 ignored；本次另外执行其中一项重型测试并通过。此前同产品前端的49项 Node 与 build门禁通过，后续仍按提交绑定CI及新EXE原生验收。阶段 A 的启动、恢复性能、内存和真实模型门槛保持独立。
