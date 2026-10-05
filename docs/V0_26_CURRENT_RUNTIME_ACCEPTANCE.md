# v0.26.0 当前桌面运行时验收

日期：2026-10-03（Asia/Shanghai）。当前 `ec8874f` 产物已完成单轮 300 张合成图片的真实 Native 生产 IPC、CLIP 编码、暖检索、备份和关闭重启持久核验，并于北京时间 23:41 推广到桌面稳定路径。功能回执为 `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART`，推广回执为 `PASS_NATIVE_ACCEPTED_DESKTOP_RUNTIME_PROMOTED`。阶段 A 和 v1.0.0 的完整退出标准仍未关闭。

## 当前产物与证据身份

| 项目 | 实际身份 |
| --- | --- |
| 应用版本 / schema | `0.26.0` / `11` |
| EXE 构建来源 | `ec8874f9421deb184e17e833347d2a70bf5b0238` |
| EXE SHA256 | `10381121a5ea89c71d629f93c6f5e2879ca829612706e50c5ba82e16bd50e01c` |
| EXE 大小 | `47863296` bytes |
| 证据冻结时主仓库 HEAD | `177958a390d724dc65fb0e1433e70029022a46f0`；生产源码与候选来源保持相同，冻结前工作树干净 |
| 稳定目标 | `<project>/desktop-runtime/current/ImageLore.exe`；已有桌面 `ImageLore v0.26.0.lnk` 的实际目标已验证 |
| 功能 run | `<private-evidence>/native-current-v1-20261003-ec8874f-e724041d/runs/2026-10-03T15-32-30-509Z-300-functional-6d477f5a-1fc2-47fa-983b-5e981bb33829` |
| 功能 run 时段 | `2026-10-03T15:32:30.510Z` 至 `15:34:20.638Z`，北京时间 10 月 3 日 23:32:30.510 至 23:34:20.638 |
| 推广 manifest 时间 | `2026-10-03T15:41:05.5079495Z`，北京时间 10 月 3 日 23:41:05.5079495 |

功能 run 的 `result.json` SHA256 为 `34d8640ea92c96f2297624d318130d05ad4b8a0a35c84e1af0e21f1a20711235`。同一私有证据目录的 `final-native-functional300-receipt.json` SHA256 为 `15e9d036243e3ab14992dcdf54166d967dfb7664241c8c486a1bd32a676e2188`；`final-native-freeze-receipt.json` 于 `2026-10-03T15:39:07.3103390Z` 冻结来源、脚本与回执，确认没有补跑其他 case，原生进程已全部正常结束。文档后续提交不改变 EXE 的构建来源。

本轮与 [FTS 配对实验](V0_26_FTS_IMPORT_PROFILE.md)分别记账。FTS 实验的 release opt3 测试 EXE、SQL/事务样本及 50k 背景不属于这个桌面 EXE 的模型规模验收。历史 `2276de1` 的 5k/50k 模型和桌面记录继续保留原 source/EXE 身份，见[模型验收](V0_26_MODEL_ACCEPTANCE.md)。

## 真实编码与预期故障分类

素材为 300 张合成栅格图片，未预置 seeded embedding；运行真正的 `clip-vit-b32-qdrant-v1` vision/text 模型及生产编码路径。最终状态为 `indexed=300`、`total=300`、`stale=0`，模型就绪，全部向量为真实 512 维，向量总字节 `614400`。模型状态报告 `model_bytes=608016081`；本轮使用独立测试缓存，不声称重新完成首次网络下载或物理离线验收。

| 阶段 | 终态 | worker 观察墙钟 |
| --- | --- | ---: |
| 初次 300 项 | `processed=300`、`indexed=298`、`failed=2`、`done=true`、`cancelled=false` | `19074` ms |
| 修复后补齐 2 项 | `processed=2`、`indexed=2`、`failed=0`、`done=true`、`cancelled=false` | `1054` ms |

初次失败恰为预先注入的缺失 JPEG 与损坏 WebP；原始失败终态保留，修复这两张测试图片后只补齐对应两项。最终 300/300 不是删除失败样本后的结果，两个 worker 的处理数也不累加成 302 个独立资产。

worker 墙钟由生产 IPC 发起任务到观察到终态，包含 prepare、文件哈希、模型初始化和编码。阶段事件为接收时刻，不能推断精确 ONNX 入口或隔离推理吞吐；两段墙钟不作为同场景冷测或完整导入速度的证明。

## 暖检索与计时边界

text/similar 各保留一次 first-call 和 20 次 warm，共 42 个查询样本。项目暖语义检索预算保持 p95 ≤3000 ms；两条命令的 20 次 Native IPC warm p95 均通过。

| 查询 | first-call Native IPC / observer ms | warm 样本 | Native IPC median / p95 / max ms | observer median / p95 / max ms |
| --- | ---: | ---: | ---: | ---: |
| `semantic_search_text` | `856.9 / 1049` | 20 | `785.05 / 803 / 806.5` | `1032.5 / 1046 / 1055` |
| `semantic_search_similar` | `4.2 / 261` | 20 | `3.9 / 4.6 / 4.8` | `262 / 268 / 269` |

Native IPC 计时在 WebView 内包围生产 invoke，包含 IPC、Native 命令及响应，不包含外部观察工具的调用与等待。observer 是工具侧观察墙钟，两种口径分列；不把 observer 与 IPC 相减后归因为某个产品阶段。first-call 排除在 20 次 warm 汇总之外，它不是清除 Windows 文件缓存后的冷模型测量。该单轮小库查询不能外推到 50k、自然图片召回质量、完整渲染或每次物理输入延迟。

## 备份、关闭和 fresh-profile 重启

生产备份 `snapshot`、首次正常关闭后的 `full-closed` 和一次 fresh-profile 完整重启再关闭后的 `full-restarted` 三阶段都取得 `integrity=ok`、foreign-key errors `0`、schema `11`、资产/向量 `300`、向量字节 `614400` 和 3 个元数据哨兵。三阶段 typed 业务摘要完全一致：

| 摘要 | 三阶段相同的 SHA256 |
| --- | --- |
| asset | `848275810bc5cb45b60e25ebf66b24b61043c832d87e472107a676b6033e99f4` |
| metadata | `9aada6db53a0deb5b0ca34a94c1916161e6884bf4c55f5ead9cba5c28a83b28e` |
| vector | `0921ca0c76031e3271494f4ce689ed811a4ec339af8b4367f4a9f1a72723b5d3` |

两次关闭均实际发送 OS `WM_CLOSE`；以关闭前保留的 exact process handle 读取退出码，两次均 `exitCode=0`、端口已释放，未 force kill。闭库验证默认只让 SQLite 打开独立 DB/WAL/SHM clone，源测试 DB/WAL/SHM 字节、大小与 mtime 在验证前后相同，sidecar 原件保留。clone 中的 checkpoint 不作用于源库；typed 摘要一致也不等于不同阶段数据库文件逐字节相同。这次备份与重启没有执行恢复故障矩阵，不能关闭历史恢复耗时 WARN。

## 短窗观察与尚未覆盖的门槛

本轮 56 个进程内存样本无采集失败；Private Bytes 峰值 `922267648` bytes，summed Working Sets 峰值 `1063460864` bytes。Private Bytes 是私有提交量，Working Sets 求和可能重复计算共享页。这只是当前功能窗口的峰值观察，没有长期基线或增长率，不计为 <20% 内存预算通过。

首轮与重启窗口分别有 750 / 6 个 `WM_NULL` 探针，non-responsive 均为 0，两个采样 helper 正常结束。窗口消息响应不能代替 render/input、物理选择框或复杂图片长期交互验收。

阶段 A 仍须完成当前产物的 50k 及更多真实工作流、至少三次可比冷测、长期内存、自然召回、复杂图片和物理离线验收。已有启动、恢复、内存和历史编码关窗的 WARN 保留；本轮没有验证取消场景。签名、安装器/用户实机升级、updater 和正式 v1.0 发布仍有各自门槛，不能用此次 Native 功能 PASS 关闭。

## 桌面推广、保全与回退边界

推广回执保存在 `<private-evidence>/runtime-candidate-20261003T054617Z-3493c745ebc14b03ab9ae25c7e53bf6b/desktop-promotion-receipt.json`。current manifest、EXE SHA 与已验收候选一致；已有版本化快捷方式的目标已复核，链接和管理 state 的字节与 mtime 保持。未生成安装器，`formalV1Release=false`。

原 current 的 `2276de179da95cf29a889a0a6e7d9a01d940ad8a` / EXE SHA256 `077027cdce55a9ffca55d18e00b84d50290ff876f9c708b721bc31bae532b82b` 完整目录保存在 `<project>/desktop-runtime/previous-20261003-154105-8664e52888984d39bf61c845d8b1dd44`。回执确认 previous 中的旧文件与替换前逐字节相同，作为程序回退材料；本次没有实际执行回退演练。

推广前后仅检查列明的六个当前/legacy 默认用户 DB/WAL/SHM 路径元数据，六项均不存在且检查结果相同；没有通过 SQLite 打开默认用户库或读取其 payload。该范围不扩大为全部用户目录或全盘数据保全证明。程序回退与数据恢复分开：不覆盖当前资料库，不让旧程序绕过未来 schema 保护；需要数据恢复时仍按[阶段 A 恢复方案](V1_0_0_PLAN.md)保留原件并验证备份身份。

所有原始回执、日志、fixture 和测试资料库保留在仓库外 `<private-evidence>`；公开文档只保留必要产物身份、汇总数字和验收边界。
