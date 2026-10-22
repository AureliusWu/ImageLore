# v0.26 当前源码两路校验兼容验证

日期：2026-10-22（Asia/Shanghai）。应用版本仍为 `0.26.0`、schema `11`。本记录对应隔离候选，不代表已经推广到 main、桌面程序或正式版本。

## 实际门禁与身份

在基线 `41bb6194b5404428649a5caa3ec3b61643822fcd` 上无冲突移植 `0a87dd8ddfce3022df4122576f616b5bbbbaf503` 的两路校验补丁。运行时修改尚未提交，以下源码 SHA 与完整输入清单单独绑定，不能把基线提交当作已经包含补丁。

- 完整 library suite 实际 **244 passed / 0 failed / 3 原有 ignored**，267.59 秒，退出码 0。
- 包含测试代码的严格 Clippy（`--lib --tests -- -D warnings`）及 locked/offline check 均实际通过。
- 68 项编译/include 输入在各次执行前后相同；完整测试实际新编译，归档 SHA256 为 `746c11aa1951cf7aebeb1442a119a7290693cf68a9b7391f91c5bc6884016b18`。
- 本轮归档是 debug Rust lib test EXE，不是应用 EXE，也不是优化构建性能测量。

| 修改源码 | SHA256 |
| --- | --- |
| `src-tauri/src/backup.rs` | `1a8336d488d09562cc7f1bcdad3d88b64311e218e29e7249f65cf4d6c43f104d` |
| `src-tauri/src/backup/guarded_restore.rs` | `2876dacd94961a7da111bbe8bf3a08fe7dd39b8176816147b1209b5c347f7c5f` |

其余 66 项输入与当前串行源码保持一致。原 61 个 backup 测试函数体逐字保留，新增原候选已有的 7 项轮换回归及 1 项 guarded profile 回归。主线空间故障、创建 FULL、Probe 部分复制、实际 rename/readonly、31 案进程中断和缺图重定位相关源码保持；没有删除旧断言。

## 实现与边界

每批最多两个校验 worker，保留原完整 integrity、schema、portable ID、外键及业务关系检查；所有已启动任务 join 后，按原 records 顺序消费结果。Unavailable、panic 或启动线程失败停止后续批次，轮换计划没有获得不完整结果时的隔离或删除授权。保留十份备份的语义与现有恢复原件保护不变。

新增回归覆盖反序完成后的有序消费、Unavailable 后的消费前缀、最新十份有效候选、WAL 副本保全、panic/spawn 失败、独立 profile session 计数、投机结果不被消费，以及直接 guarded 校验计数不遗漏、不重复。

旧候选的 233 项回归与一次 debug 六操作诊断仍保留在[历史记录](V0_26_BACKUP_GUARDED_WORKER_DIAGNOSTIC.md)，不是本轮新集成的远端或性能证据。本轮同样不能由完整回归通过推出提速、GUI 或正式发布通过。

## 下一步

先封存本轮受测源码并完成同源远端门禁，再构建优化后的 lib test，在相同合成源、全新副本及相同六操作流程下测量。串行基线见[固定提交的六操作记录](https://github.com/AureliusWu/ImageLore/blob/47cdc435fdb0f05d6e36a629dd27d25e8155a7e5/docs/V0_26_CURRENT_BACKUP_DIAGNOSTIC.md)。旧 debug 数字不参与改善率计算；单样本对照也不关闭正式冷暖、GUI、内存或安装发布门槛。

原始库、副本、完整日志、逐项输入清单与测试产物均保存在仓库外。本记录不包含本地绝对路径、邮箱、密钥或资料库内容。桌面验收按用户要求暂缓，当前桌面版本与快捷方式保持已验收的 `0.26.0`。
