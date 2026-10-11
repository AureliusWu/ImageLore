# ImageLore v0.26.0 备份校验候选

记录日期：2026-10-11。候选源码为 `c88162e77a5ca0c1222155835706d947bfe86d02`，尚未推广至主分支或桌面稳定目标。本记录不关闭阶段 A，也不代表 v1.0.0 发布。

## 实现与保留的规则

轮转按既有候选顺序分成至多两个工作线程的批次。每批全部线程 join 后才按 records 顺序消费结果并改名或删除文件；完成先后不改变消费顺序。每份仍执行完整验证，连接缓存上限仍为每连接 32 MiB。保留最新十份有效备份；Rejected 的既有隔离策略保留，Unavailable、线程 panic 或启动失败停止消费，其后未消费文件保持。

新增七项实际 Rust 回归覆盖完整验证计数、最新十份与坏候选、工作线程数量、逆序完成、真实不可用前缀、panic/启动失败及 WAL 原件保全。内部 trace 和私有导出均为 `cfg(test)`，默认不开启；生产桌面程序不会因此产生这些报告。

## 同一提交的 CI

- [Fast CI](https://github.com/AureliusWu/ImageLore/actions/runs/37952891597)：成功。
- [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/37952905952)：fmt、locked check、严格 Clippy 和 library tests 成功。实际 157 项：154 通过、0 失败、3 跳过；backup 模块 44 项中 42 通过、2 个显式 profiling 测试跳过。新增七项轮转回归全部通过。
- [敏感信息检查](https://github.com/AureliusWu/ImageLore/actions/runs/37952808514)：成功。

三个跳过项保持原有身份，不改写为通过；下方本地两项普通回归也不等于执行这些跳过项。

## 本地真实报告与消费

独立构建的 libtest SHA256 为 `03467ce97e94dc6a54378695064414b04bb809c9dea15304b02ebc37528e8000`，构建来源为上述完整候选提交；`backup.rs` SHA256 为 `12ff1dd9affd5abb752491a46598eabf82283898329662f38fa4e4587d78852b`。两个指定回归实际运行，2 通过、0 失败、155 filtered，测试时长 5.64 秒。它们创建四份真实 schema 2 报告，再由固定来源的 Node 消费器严格核对。

| 场景 | worker trace 数 | 完整验证调用数 | 候选 started | 候选 consumed |
| --- | ---: | ---: | ---: | ---: |
| normal | 3 | 3 | 3 | 3 |
| panic-before | 2 | 1 | 2 | 1 |
| panic-after | 2 | 2 | 2 | 1 |
| spawn-failure | 2 | 1 | 1 | 2 |

四报告共 13 个独立会话、7 次完整验证调用。正常 operation 成功，三个注入失败 operation 均按预期失败；“失败 trace 被正确记录”不能记为 operation 成功。各线程的单调时钟原点独立，worker 时长不相加到父线程时长；JSON 不直接记录 join-before-consumption 时间关系，这项由独立实际 Rust 并发断言证明。

首轮本地 ORT 链接失败与导出路径保护失败均保留。之后修正本地环境及移开非仓库的空目录，未改候选源码或锁文件，实际重跑及消费成功。原始报告和日志仍在 `<private-evidence>`，没有上传 GitHub、Issue、PR 或 CI artifact。私有联合回执 SHA256 为 `5618611579a5802b0eb613ce1dfd5aa3b31abdbec5c643b98112ec059456ff7b`；已独立核对其 15 个引用及四报告、实际测试产物和源码哈希。

## 仍需完成

10月11日已从同一 `c88162e` 完成私有 release / no-bundle 桌面构建，自然退出码 0；EXE 版本 `0.26.0`、SHA256 `cfe34332e142507531342c06dc3e74fe7a155c7ac8b2da09d7949ca4c0d318b8`，源码及两份锁文件不变。私有执行包另附验证缓存的 DirectML `1.15.4`，SHA256 `9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1`。原 current 包只有 EXE；静态导入相同不证明历史运行时加载了同一 DLL。正式配对须让 A/B 使用相同的私有依赖副本，并记录实际加载观察范围，保持原 current 不动。构建及静态导入检查不替代 GUI 启动、备份或恢复验收。

这是一组小型真实回归和报告契约验收。它不证明 50k 桌面备份变快，也不替代三次 cold / 二十次 warm、真实轮转文件集与资料等价、编辑保存/重启/恢复、安装升级或长期内存验收。生产桌面 profiling 关闭，其内部调用数只能按源码推导，不能把本轮 7 次 trace 调用移作桌面运行的实测计数。

此前[三轮 ABBA 探索性对照](V0_26_BACKUP_ROTATION_PAIR.md)保持原来源与范围。下一步在两个独立合成资料目录中，对现有 `ec8874f` 桌面程序和本候选执行实际 Native 备份与恢复对照；通过后才评估合入主分支及桌面推广。可信签名、updater 和正式发布继续按 [v1.0.0 更新方案](V1_0_0_PLAN.md)的阶段顺序推进。
