# v0.26.0 原生 CLIP 模型验收证据

验收结论：`PASS_SMALL_REAL_MODEL_FLOW`。在独立数据目录中，实际桌面程序完成公开模型下载与可信校验、三张 PNG 的真实图片编码、文本编码、相似图片检索和生产备份中的向量核验。该结论限于小样本集成流程；v0.26 大图库真实模型吞吐、自然图片召回质量以及下载期间快速取消仍未验收。

## 版本、时间与隔离范围

| 项目 | 证据 |
| --- | --- |
| 测时实际程序 | `desktop-runtime/current/ImageLore.exe`，ProductVersion `0.26.0`；本表固定当时的 source/SHA，后续 current 替换不改变这次模型验收身份 |
| 构建来源 | `035a37ece3b5eb9c4790a3873a208eb8d2b60f99` |
| 程序 SHA256 | `08fec9adba57c4290a907cfc83ddad5b991aa8c32b5d6640e2995388c790afa7` |
| 模型流程 | 2026-09-30 16:48:51 至 16:52:50，北京时间；中途包含验收脚本修复后的续测 |
| 证据复核 | 2026-10-01；只读复核既有 JSON、脚本、模型文件与验收生产备份，没有重新运行模型 |
| 数据目录 | `<private-evidence>/native-model/data`，有独立 acceptance marker |
| WebView profile / CDP | 本任务独立 profile，回环端口 `9223` |

启动器只给验收子进程设置数据目录、WebView profile 和调试端口，移除该子进程的 Hugging Face / Vision 凭据环境变量。准备记录与启动记录均标记 `credentialIsolationVerified=true`；准备阶段记录的公开下载客户端使用独立 cache 与显式空 token。验收未复制正常用户库、用户凭据、已有模型缓存或种子向量，复核也未读取正常用户库或密钥。

三个夹具是程序生成的 `224×224` 红、绿、蓝纯色 PNG。最初只创建 marker，SQLite schema 由实际程序启动生成；`import_paths` 实际导入结果为 added=3、failed=0、skipped=0、duplicates=0。初始 `model_bytes=0`、indexed=0；导入后 total=3、stale=3。

## 取消、重新索引与模型就绪

首次在没有模型缓存的情况下启动任务，并立即请求取消。启动与取消调用合计约 **5.0 ms** 返回 acknowledged=true；此时下载和初始化并未停止。

| 观察点 | 结果 |
| --- | --- |
| 首个准备事件到取消终态事件 | 18,810 ms |
| 脚本观察到取消终态 | 19,156 ms；轮询间隔为 1,000 ms |
| 启动到完成观察的总墙钟时间 | 19,162 ms |
| 取消终态 | done=true、cancelled=true、processed=0、indexed=0、failed=0 |
| 取消后的模型状态 | vision_ready=true、text_ready=false、indexed=0、stale=3 |
| 等待取消终态后重试 | done=true、cancelled=false、processed=3、indexed=3、failed=0 |
| 重试时间 | 启动调用返回后观察等待 1,008 ms；准备事件到完成事件跨度 1,228 ms |
| 图片索引完成时 | 3/3、stale=0、index_bytes=6,144；仅视觉模型就绪 |
| 文本查询完成后 | model_ready=true、vision_ready=true、text_ready=true，3/3 fresh index |

上述证据证明取消状态和重试衔接一致，不能证明下载或初始化可被快速中断。当前取消标志在逐图编码边界检查，不能打断模型下载、初始化或已在执行的单图编码；本次没有生成部分索引，其他时序仍可能在取消期间提交一张已开始编码的图片。不能把 5 ms 的确认时间写成任务终止时间，也不能把观察等待时间写成纯推理时间。

最终 `semantic_status` 的 model_bytes=608,016,081 包含模型与辅助文件，index_bytes=6,144 为三个 float32 向量总字节数。

## 实际模型文件与可信身份

以下 SHA256 为实际落盘 ONNX 重新计算的值，均与既有 trusted manifest、固定 revision 和验收脚本常量一致。证据来自本机下载后的文件，不以网页可访问作为本机下载或可用性证明。

| 组件 | 仓库 / 固定 revision | ONNX 字节数 | SHA256 |
| --- | --- | ---: | --- |
| vision | `Qdrant/clip-ViT-B-32-vision` / `e0c24ed0fa57fa3e4f97f30de74c51d944036ace` | 351,686,194 | `c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63` |
| text | `Qdrant/clip-ViT-B-32-text` / `48ca1db27cb4063eb311ec2aa7f087a808112876` | 254,102,519 | `4dbe762b11e36488304471e439cde89da053ad7acaddbf9e096745d142ec8d8b` |

两个 manifest 的 model_id 均为 `clip-vit-b32-qdrant-v1`，manifest_version=1。模型下载、可信文件身份校验及实际编码流程均完成；未由这些结果推导安装器签名或自动更新能力。

## 生产备份中的真实向量

使用原生 `create_backup` 得到独立快照 `imagelore-1790758320476020900.sqlite3`，文件大小 339,968 字节，SHA256 为 `821c8c616a93adebcd7342edbf5fa183ecba104fd4cf98aaa511ef9341df6645`。验收脚本与本次复核均以 `mode=ro&immutable=1` 读取此快照，未读写正在使用的库：integrity_check=ok、foreign_key_check 无错误、assets=3、semantic_embeddings=3。

三个向量均由实际程序在本次流程中产生，model_id 正确、dimensions=512、blob=2,048 字节、所有 float32 值有限、向量 fingerprint 等于对应当前 asset fingerprint，三个 payload SHA256 不同。

| asset | L2 norm | 向量 blob SHA256 |
| --- | ---: | --- |
| 1 / red-square.png | 1.0000001500122007 | `441580af0f766104fb359ab6886e038deace79352695a283d0254a0c93a76f7b` |
| 2 / green-square.png | 0.9999995926290162 | `8e4200a74527b9a42a81cfec5c1ab102568c20217d6ef1c3409c4c6ff8f48c70` |
| 3 / blue-square.png | 1.000000325705324 | `de61486958660d2a2f3dd1551027f62f310124240f9844fe2393701f3f3caed3` |

归一化最大偏差约 `4.08×10^-7`。独立用备份向量重新计算双精度点积：red/green=0.888057143、red/blue=0.932610746、green/blue=0.918922829，与原生 similar 返回分数的最大绝对差约 `3.29×10^-7`，排序一致。

## 检索正确性与实际 IPC 时间

实际调用生产 `semantic_search_text` 与 `semantic_search_similar`，每种调用共 23 次：三种初始查询/来源检查，另加 20 次交替 warm 采样。所有结果仅包含夹具 ID，无重复 ID，分数有限且位于约 `[-1,1]` 范围并按降序排列。similar 每次排除来源图片并返回其余两张。

| 文本查询 | 返回顺序 | 第一名分数 | 调用耗时 |
| --- | --- | ---: | ---: |
| `a red square` | red → blue → green | 0.27702075 | 18,058.1 ms |
| `a green square` | green → blue → red | 0.30374175 | 916.4 ms |
| `a blue square` | blue → green → red | 0.30118385 | 868.5 ms |

三条查询第一名均与颜色标签一致。这是已记录结果的独立审核结论；现有脚本主要断言候选集合、重复项、分数范围、排序与自匹配排除，不把该三条结果扩大为自然图片召回质量指标。

第一条 text 查询包含真实文本模型下载、哈希校验与初始化，仅有这一次首次调用，不能计算首次下载 p95。warm 是模型文件已有后的生产调用，仍包含真实生产模型生命周期开销；未通过 mock 或种子向量替换文本编码。

| 20 次 warm 采样 | 查询 / 来源 | min | median | p95 | max |
| --- | --- | ---: | ---: | ---: | ---: |
| text | `a red square` | 773.1 ms | 896.75 ms | 1,270.1 ms | 1,290.5 ms |
| similar | blue / asset 3 | 2.2 ms | 2.75 ms | 4.8 ms | 6.1 ms |

median 为排序后中间两项的均值，p95 为 `ceil(0.95×20)` 的近秩值。时间来自原生 WebView 内生产 IPC Promise 完成时的 `performance.now()` 差值，不含脚本随后最多 250 ms 的结果轮询等待。20 次 warm text 的全部 ID/分数均一致，顺序 red → blue → green；20 次 warm similar 全部一致，顺序 red → green，与独立向量点积相符。

最终证据包含 61 条已记录生产操作且无操作错误，runtime_exceptions 为空，最终 errors 为空。启动后立即取消的两次 IPC 在同一表达式中测量，因此不单独计入 operations 数组。

## 首次验收脚本失败与续测

`result-initial-verifier-path-failure.json` 保留第一次结果：status=`FAILED_OR_BLOCKED_REAL_MODEL_FLOW`，唯一错误是 `Native-created embedding snapshot verification failed`。当时实际图片索引已经完成 3/3，stale=0；Python 快照路径边界 guard 没有归一化 Rust 返回的 Windows 扩展路径前缀 `\\?\`，错误拒绝了本夹具内的合法备份路径。

后续仅修正验收脚本对该前缀的处理。`--resume-indexed` 限制续测必须是同一程序 SHA、同一已完成索引和这一种 verifier 错误；没有重置库、重复索引、替换模型或修改产品代码。续测重新调用原生备份、只读核验，然后执行文本/相似检索，最终得到 `PASS_SMALL_REAL_MODEL_FLOW`。初次失败记录保留，不能把它归类为产品模型下载或推理失败。

## 未关闭的范围与后续顺序

2026-10-01 的当前 `f61f59bf86505dadacd15f75da3b75f0ed68fdf3` / EXE `D0AF13E8B229B326CA291EDE454D421482DBA637F97CBC5D271A3D5616271F82` 另完成最终产物回归，证据为 `<private-evidence>/native-model/{launch,smoke}-perf-final.json`。复用原三图库、已可信模型及索引，text/similar 各三次，颜色首位、自匹配排除、生产备份完整性/外键/fresh 512 维向量均通过；OS WM_CLOSE 后自有进程正常退出，状态 `PASS_FINAL_RUNTIME_MODEL_SMOKE`、errors/runtime exceptions 均为空。text 实测为 1,517.4 / 1,154.0 / 1,002.2 ms，similar 为 3.8 / 2.2 / 2.4 ms。每项仅三次，不计算 20 样本 p95，不把旧版首次下载与取消证据转记到新来源。旧模型测试、准备记录和文件保持。

- 本次只有三张纯色 PNG。没有证明 50,000 张真实图片端到端索引吞吐、50k 真实文本推理与大候选检索性能，或长时间模型内存上界。
- 没有自然图片标注集、Recall@K、复杂提示词、跨语言语义质量或跨设备复验。
- 首次下载/初始化期间取消约 19 秒才观察到终态，第一条 text 查询约 18 秒；快速取消和冷模型体验继续保留在 v0.26 验收风险中。
- 没有测试下载断网、部分文件损坏后的重新下载、代理变化与每个网络故障路径，也没有用本次成功下载替代这些故障验收。
- 模型流程通过不等于 ROADMAP 全部退出标准通过；应先结合原生大库、桌面稳定性与存储验收关闭 v0.26 的剩余边界，再进入后续签名、分发和升级阶段。

## 本地原始证据

以下材料位于 `<private-evidence>/native-model/`，用于本机追溯，不随文档提交到 Git：

- `result.json`：最终状态、全部操作耗时、progress、命中结果、模型与快照核验记录。
- `result-initial-verifier-path-failure.json`：原始 verifier 路径失败，保持不覆盖。
- `run_clip.mjs`、`verify_snapshot.py`：真实 IPC 驱动、严格续测约束和只读快照核验方法。
- `fixture.json`、`prepare_fixture.py`、`preparation.json`、`launch.ps1`、`launch.json`：夹具、无种子向量、独立环境与实际运行程序身份。
- `run-console.txt`、`run-resume-console.txt`：取消/索引观察时序与两次验收状态。
- `data/models/semantic/trusted/{vision,text}/trusted-manifest.json` 与实际 `model.onnx`：可信模型身份和本机落盘文件。
- `data/backups/imagelore-1790758320476020900.sqlite3`：实际生产备份中的三组真实向量。
