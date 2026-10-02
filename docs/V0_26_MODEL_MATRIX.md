# v0.26.0 真实模型验收矩阵

本报告记录 2026-10-02 的300张合成图片真实编码与故障恢复、32张COCO自然图片编码与检索质量，以及同产物的已缓存模型取消闭环。各项只有终态、正常关闭与独立持久化核验齐全才记为通过。桌面current于10:36更新为下表同源产物，旧程序保留，见[桌面更新回执](V0_26_DESKTOP_ACCEPTANCE.md)；这不表示v0.26或v1.0.0的全部退出条件已满足。

自然图的功能终态通过；质量报告没有数值通过门槛。冻结 32 图中的英文宏平均 P@1=100%、R@10=95%，中文分别为 12.5%、37.5%。300 图导入记录了三次窗口消息超时，不能把编码和检索期间的响应证据扩大为全流程无冻结。

## 已完成与待补矩阵

| 用例 | 本报告状态 | 已取得的证据与边界 |
| --- | --- | --- |
| 300 图真实模型功能流程 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | PNG/JPEG/WebP 各 100 张，含 6 张大图；实际编码，两个故意失败恢复至 300/300；text/similar 各 20 次 warm；备份、关闭、重启核验通过 |
| 32 图自然图片流程 | `PASS_NATURAL_COCO32_ENCODING_RECALL_REPORT_AND_DURABLE_RESTART` | 32/32 实际编码；16 条冻结查询各返回完整 32 图排名；生成质量报告；备份、关闭、重启核验通过。该 PASS 不包含相关性数值门槛 |
| 已缓存模型哈希阶段取消 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 观察到hash阶段后实际UI取消，114ms观察到同job终态；0向量部分库重启、重新编码300项、备份和完整重启均通过 |
| 已缓存模型初始化阶段取消 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 观察到init阶段后实际UI取消，856ms观察到同job终态；部分库重启、补齐300项及完整持久闭环通过；单次ONNX init仍不可抢占 |
| 下载、单图编码阶段取消与恢复 | 未执行本矩阵的新验收 | 不以取消确认的 IPC 时间替代实际任务终止时间；不复用旧版本结果关闭新来源的门槛 |
| 工作任务期间关闭窗口与恢复 | 未执行本矩阵的新验收 | 已完成的四次 WM_CLOSE 都发生在相应验收工作结束后 |
| 5k / 50k 真实模型规模流程 | 未执行本矩阵的新验收 | 小库计时、已有种子向量大库结果均不代替大库真实图片编码和真实文本推理 |
| 冷模型下载、断网、损坏模型重新下载 | 未执行本矩阵的新验收 | 两次已完成流程使用隔离目录中的可信已缓存模型，不形成冷下载体验结论 |

## 固定程序与模型身份

| 项目 | 两次实际验收共同身份 |
| --- | --- |
| ProductVersion | `0.26.0` |
| 构建来源 | `34baa871dfee85794e30ae2b3e9e112761cd06c7` |
| EXE SHA256 | `3D38BAC9E12AAC61396EF9574C893F224087A23255CFDA01C8F0069ACAAB7B4C` |
| 实际程序 | `<private-evidence>/model-candidate-34baa87-20261002T022355-01db0e06/ImageLore.exe` |
| model_id | `clip-vit-b32-qdrant-v1` |
| 运行目录 | 每次 fresh 独立数据目录、WebView profile 与启动身份记录 |
| 向量来源 | 生产模型编码；`seededVectors=false`，未以种子向量替代 |

`result.json` 保留构建 manifest、来源核验方法、实际程序身份及启动记录。源文件绑定的 SHA256 为：

| 文件 | SHA256 |
| --- | --- |
| `src-tauri/src/semantic.rs` | `0bcd7e4c63fe54313fa72eb5dc5ce46dbd186de9a9fb93c0dadcec88171d8343` |
| `src-tauri/src/semantic/download.rs` | `ab1222908f83dc5ee709fcb73f220fdf06f3771bed41d3b8c0ecf545499b89e0` |
| `src/hooks/useSemanticJob.ts` | `032781d8031e91eb1fb86f3d328da1ea6d4f57fe54b93bc7427ba040076ba714` |
| `src-tauri/Cargo.lock` | `0789c4da87d6bef6c8dfbecf672732dc27978028183aa4b1a277ee99c1641c9e` |

两次最终缓存记录中的实际 ONNX 全文件 SHA 一致：

| 组件 | 实际字节数 | SHA256 |
| --- | ---: | --- |
| vision | 351,686,194 | `c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63` |
| text | 254,102,519 | `4dbe762b11e36488304471e439cde89da053ad7acaddbf9e096745d142ec8d8b` |

辅助文件的记录仅声明有界 JSON 有效性，不声明它们具有已认证的完整文件哈希。最终 `unpublishedStages=[]`，生产状态的模型与辅助文件合计 `model_bytes=608,016,081`。

## 已结束的真实编码与故障恢复

时间均为 2026-10-02 北京时间。任务墙钟来自 IPC 派发至观察到终态，包含实际准备、哈希、初始化和轮询观察；不是独立 ONNX 推理计时。

| 项目 | 300 图功能流程 | 32 图自然流程 |
| --- | ---: | ---: |
| 整体运行起止 | 10:30:23.645–10:32:43.495 | 10:36:22.206–10:38:57.099 |
| 图片字节合计 | 1,550,133 | 5,080,771 |
| 图片 manifest SHA256 | `289d041935f280aa1b4e0d06d899accab3a497696599d5aaf7bfec72fcee71f2` | `fd5d03c7f49d3106eef59415f8f53b08c6772c9475f8de5ab644ee89f98113c9` |
| 首个 worker 墙钟 | 25,450 ms | 4,291 ms |
| 首个 worker 终态 | processed=300、indexed=298、failed=2、skipped=0 | processed=32、indexed=32、failed=0、skipped=0 |
| 恢复任务 | processed=2、indexed=2、failed=0，1,281 ms | 未注入图片故障 |
| 最终索引 | 300/300、stale=0 | 32/32、stale=0 |
| 最终向量字节 | 614,400 | 65,536 |
| 原生 `import_paths` IPC | 2,422.3 ms | 235.5 ms |
| 原生 `create_backup` IPC | 54.4 ms | 46.3 ms |

300 图首次 worker 的失败由夹具主动注入：`fixture000299.jpg` 缺失，`fixture000300.webp` 内容损坏。两张均按原 manifest 恢复，完整大小与 SHA 校验通过后，只重新索引这两项；其余 298 张保持。最终恢复任务和状态证明失败条目可重试，不把预期的两个图片错误隐藏成首次全成功。

| 恢复图片 | 原始文件 SHA256 |
| --- | --- |
| `fixture000299.jpg` | `da61c1d7338c27a2ebdb300dee4e841462d32c2c4ea55be283bb2068e618565e` |
| `fixture000300.webp` | `61ae89e0a668f3c1938c0d13f1fb1dbadef1e8d5bcd88cbeff36c0883dac5f8c` |

300 图此前的一次 observer 启动失败另有 fresh 原始记录保留，不计入这次成功运行，也不归类为模型编码失败；成功运行使用上表的新目录。两次小库的备份 IPC 计时不关闭 50k 库备份性能门槛。

## 20 次 warm 检索计时

每种命令有一次 first-call，再取独立的 20 次 warm。median 为中间两项均值，p95 为 `ceil(0.95×20)` 的近秩值。下表时间来自 WebView 内真实生产 IPC Promise 的 `performance.now()` 差值；不把脚本轮询和观察等待算成原生命令耗时。

| 图库 / 命令 | first-call IPC | warm 样本 | median | p95 | max | p95≤3,000 ms |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 300 / text | 963.4 ms | 20 | 901.15 ms | 1,011.8 ms | 1,024.3 ms | 是 |
| 300 / similar | 4.7 ms | 20 | 4.35 ms | 6.1 ms | 7.3 ms | 是 |
| 32 / text | 1,188.7 ms | 20 | 1,040.55 ms | 1,375.1 ms | 2,070.0 ms | 是 |
| 32 / similar | 3.5 ms | 20 | 3.30 ms | 4.6 ms | 4.9 ms | 是 |

对应 warm 外部观察 median/p95 为：300 text 1,034.5/1,049 ms、similar 261.5/267 ms；32 text 1,164/1,549 ms、similar 264.5/271 ms。similar 外部约 250 ms 的观察周期不能解释为原生检索需要 250 ms。

32 图 text 的 20 次 warm 使用冻结的英文猫查询。其他 15 条质量查询各有实际 IPC 和完整排名，但不把它们宣称为各语言分别完成 20 次 warm。两次 first-call 均发生在可信模型文件已缓存的条件下，不包含本矩阵未执行的冷模型下载。warm 调用仍经过正常生产模型生命周期，不推导持续 session 的初始化省略或 50k 性能。

所有实际检索命中来自本次夹具 ID，分数有限、约在归一化余弦范围内并按降序排列；similar 排除来源图片。此结构检查不判断自然图是否与查询语义相关。

## 生产备份、关闭与完整重启

两次运行的 `verify-snapshot.json`、`verify-full-closed.json`、`verify-full-restarted.json` 均为 `valid=true`、`errors=[]`、schema=11、`integrity=ok`、外键错误数=0。全部 512 维 float32 向量为 2,048 字节、值有限，model_id 与当前图片 fingerprint 匹配；元数据哨兵覆盖 prompt、negative prompt、模型、标签、集合与修订记录。

| 图库 | assets / indexed / norm 检查数 | 最大 L2 norm 偏差 | 三阶段 asset aggregate SHA256 | 三阶段 metadata aggregate SHA256 | 三阶段 vector aggregate SHA256 |
| --- | --- | ---: | --- | --- | --- |
| 300 | 300 / 300 / 300 | `2.998709114354625e-08` | `cb908da9d6f73d113a2c7f320ea159fc75921c5d89a1bccf9895ec651b2a239c` | `8ebbb358823edbaafeafa31f131ab4addc5d18746cb05bc9021866931be4aa0c` | `3388edead9cbb3e0aa5dc77e75dbe98c3c5c1e44772ed239d25494813298d1c1` |
| 32 | 32 / 32 / 32 | `2.962262957151296e-08` | `ff45bad78c4d3af1b21a920cdd0a8219e3f5ce13aa7b383736ca99ada5621d67` | `03fe4bbc6e81df8831886c8edb6d218b3f0b75ceaaf6bf9f89a1943938df2d18` | `67f7e6ba24f01b785d1cf63907f945a305739f16299fa3386bbaff100ff3b1f6` |

snapshot→关闭后→完整重启后三阶段的 11 项比较均 `matched=true`，`different_fields=[]`。typed aggregate 保留 SQLite 值类型、空值与排序规则，覆盖业务记录及可见 FTS 内容；不以行数或“状态显示已就绪”代替向量和元数据持久化一致性。

正常关闭后的源库仍可能带已提交 WAL。独立 verifier 没有用 SQLite 打开源库：持有关闭后源 DB/WAL/SHM 的独占只读文件句柄，保留 raw bundle，源三文件的 SHA、大小与修改时间前后保持。仅在 fresh 工作副本中让 SQLite 重建 SHM、执行 checkpoint；副本前后业务 typed aggregate、schema、完整性和外键结果一致，`queryOnly=true`、checkpoint=`[0,0,0]`，正常关闭后副本 sidecar 消失，再对该副本执行 immutable 核验。

关闭与重启 readback 均为 `PASS_PRESERVED_CLOSED_WAL_CLONE_READBACK`，`sourceSQLiteOpened=false`、`sourceBeforeAfterPreserved=true`、`sourceSidecarsRetained=true`。不得把副本归一化描述为修改源库，或对带 WAL 的源库直接使用 immutable 忽略日志。

| 图库 / 正常关闭 | 实际 PID | WM_CLOSE 请求至观察退出 | 确切进程退出码 |
| --- | ---: | ---: | ---: |
| 300 / 初始进程 | 23948 | 698 ms | 0 |
| 300 / 完整重启进程 | 24328 | 676 ms | 0 |
| 32 / 初始进程 | 18504 | 1,029 ms | 0 |
| 32 / 完整重启进程 | 15080 | 1,129 ms | 0 |

四次使用正常 OS WM_CLOSE，保留进程 handle 通过 `kernel32.GetExitCodeProcess` 取得确切退出码；请求前为 STILL_ACTIVE=259，`forceKill=false`，调试端口释放，窗口 sampler 正常停止。表内时间从实际 WM_CLOSE 请求起算，不混入 helper 排队时间，也不说明正在编码时关闭已验收。

## 窗口响应与已发现的导入限制

独立 sampler 对验收窗口执行 WM_NULL `SendMessageTimeout`，flags=`0x22`，单次 timeout=500 ms，发送后间隔约 100 ms；实际 send-start/end 与耗时保存于 JSONL。准备时间和 helper 排队不计入消息延迟。分组标签是 observer 收到结果时的上下文，不能单凭标签认定任意长区间的原生阶段。

| 图库 / 进程 | WM_NULL 样本 | 未响应样本 |
| --- | ---: | ---: |
| 300 / 初始 | 942 | 3 |
| 300 / 重启 | 6 | 0 |
| 32 / 初始 | 1,060 | 0 |
| 32 / 重启 | 6 | 0 |

300 图的三次未响应均与同一 `import_paths` 观察上下文对应：id=4，观察起点北京时间 10:30:38.841，原生 IPC=2,422.3 ms。实际消息区间如下，保留毫秒壁钟与高精度 elapsed 的区分：

| ordinal | 北京时间 send-start→send-end | elapsed | 结果 |
| --- | --- | ---: | --- |
| 10 | 10:30:38.900→10:30:39.415 | 514.6754 ms | timeout，Win32=1460 |
| 11 | 10:30:39.517→10:30:40.025 | 508.7113 ms | timeout，Win32=1460 |
| 12 | 10:30:40.147→10:30:40.657 | 508.9072 ms | timeout，Win32=1460 |
| 13 | 10:30:40.774→10:30:41.265 | 490.9150 ms | 延迟后响应 |
| 14 | 10:30:41.375→10:30:41.375 | 0.1669 ms | 响应 |

这些采样证明 300 图导入期间存在窗口消息响应受阻。采样间隙、observer 上下文和整数毫秒壁钟不能确定连续冻结的精确起止，不能把三次 timeout 相加后称为全部冻结时长，也不能据此声称整个 2,422.3 ms 都完全不响应。该限制尚需产品修复或后续验收关闭。

300 图编码上下文 218 个样本均响应、max=0.6821 ms；warm text 188 个样本均响应、max=0.9049 ms；warm similar 49 个样本均响应、max=0.2673 ms。32 图导入的一个样本响应耗时 128.535 ms，编码 25 个样本 max=0.3288 ms；warm text 223 个样本 max=3.3669 ms，warm similar 50 个样本 max=1.4077 ms，均无 timeout。它们是相应窗口消息的采样证据，不代替鼠标交互、页面渲染或长时稳定性验收。

内存记录为离散采样：300 图 55 次、32 图 52 次，采集失败均为 0。300 的峰值 Private Bytes=789,159,936，summed Working Set=841,879,552；32 分别为 895,168,512、1,006,985,216。Private Bytes 是已提交私有内存，多个进程 Working Set 相加可能重复共享页；不声明连续峰值、稳态上界或大规模内存门槛。

## 冻结 COCO 32 图的来源与标注

数据为 COCO 2017 validation。准备器只从官方 bucket 的同源区域 HTTPS 地址读取 ZIP 必需的严格 206 Range，提取 `annotations/instances_val2017.json`，未下载完整 ZIP、关闭 TLS 校验、请求 HTTP 或使用第三方镜像：

`https://s3.us-east-1.amazonaws.com/images.cocodataset.org/annotations/annotations_trainval2017.zip`

| 固定证据 | 值 |
| --- | --- |
| 实际 annotation member 大小 | 19,987,840 字节 |
| member SHA256 | `e8c7f7908f1d7278341fae127d0da654f102f11bd7b21d8aeefa635b8c810b6f` |
| 冻结 selection canonical SHA256 | `24e33d9f703ca028a8793abe022b8c6c1438ee28d7b812608d8212f18e522b64` |
| selection 文件 SHA256（含结尾 LF） | `bcdc97d085e7c44898d34407f7722b26fdeadb4fb5e391dad4ada85ee5320bb7` |
| 下载图片 manifest 文件 SHA256 | `7b0cde2a46175cbedea444a48d5278f10863d00fbd989b310acab9452a037700` |
| 32 张图片 | 32 个不同完整文件 SHA，JPEG 实际解码通过，共 5,080,771 字节 |

冻结规则依次为 cat、dog、bird、horse、bicycle、bus、train、airplane，每类选 4 张互不重复图片。按 image ID 升序，排除先前已选图片；anchor 实例须 `iscrowd=0`，官方 annotation area 占图片面积至少 5%。这个条件只用于确定入选 anchor，质量真值保留入选图片的全部 annotation IDs 和全部类别，包括小目标、crowd 和跨 anchor 类别。相关分母不是机械使用 4。

原始失败传输和已完整的前 6 张图片证据保留；续传先检查冻结 selection 身份与既有文件完整 SHA/大小，只下载剩余 26 张，未重新挑图。准备器 readback 记录总计 45 次请求（含 1 次失败传输和 1 次 32,768 字节诊断 probe）、11,708,861 字节已记录 response body；其中 annotation 11 次/6,595,322 字节、成功图片 32 次/5,080,771 字节。该数是响应体账目，不包含网络协议开销，不把失败前准备状态写成自然验收终态。

许可记录保留 annotation 的 CC-BY-4.0 范围及每张图片原 Flickr license/来源。照片许可存在差异，部分含非商业或禁止演绎条件；不能将全部照片概括为 CC-BY-4.0。图片和私有证据未作为发布资产打包，`release_assets=false`。

## 全多标签质量统计与独立复算

固定英文查询为 `a photograph of a {class}`，包括原样的 `a photograph of a airplane`；中文为“一张猫/狗/鸟/马/自行车/公交车/火车/飞机的照片”。16 条查询在看结果前已冻结。中文直接通过同一生产 text encoder，没有翻译替换或看排名后调整措辞。

每条原生查询返回完整 32 图排名。复核确认图 ID 集合完整且无重复、ranks=1…32、分数有限并降序；没有用标签注入排名或过滤候选。独立复算读取保留的官方 annotation JSON、冻结 selection 和实际 `natural-rankings.json`，逐图复核完整 annotation IDs/类别，再用独立整数命中数与有理数运算计算指标，没有调用准备器的 `evaluate_rankings`。所有 per-query 和 macro 值与 `natural-metrics.json`、最终 `result.json` 一致。

对查询 q，`P@k=前k名相关图片数/k`，`R@k=前k名相关图片数/本32图中具有目标类全部官方标注的图片数`。每种语言 8 条查询等权取 macro mean。原报告没有 micro 字段；多标签令一张图片可能是多个查询的相关项，8 类相关图片数之和为 37，不等于 32。

| 类别 | 全标注相关图片分母 | 英文 hits@1 / @5 / @10 | 中文 hits@1 / @5 / @10 |
| --- | ---: | --- | --- |
| cat | 6 | 1 / 5 / 6 | 1 / 4 / 6 |
| dog | 5 | 1 / 3 / 4 | 0 / 0 / 1 |
| bird | 4 | 1 / 4 / 4 | 0 / 1 / 2 |
| horse | 4 | 1 / 4 / 4 | 0 / 0 / 1 |
| bicycle | 5 | 1 / 4 / 4 | 0 / 3 / 4 |
| bus | 4 | 1 / 4 / 4 | 0 / 0 / 1 |
| train | 5 | 1 / 4 / 5 | 0 / 0 / 0 |
| airplane | 4 | 1 / 4 / 4 | 0 / 0 / 0 |

| 语言 / macro | P@1 | R@1 | P@5 | R@5 | P@10 | R@10 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| English | 100% | 22.083333% | 80% | 87.916667% | 43.75% | 95% |
| Chinese | 12.5% | 2.083333% | 20% | 18.958333% | 18.75% | 37.5% |

固定 k 且每条查询权重相等时，micro precision 与表内 macro precision 相等；micro recall 则以全部 37 个“类别—图片”相关项为分母。独立补充计算的 micro R@10 为英文 `35/37=94.594595%`、中文 `15/37=40.540541%`，不能替换上表的宏平均 95% 和 37.5%。

本小库每类只有 4–6 张相关图，因此完美排序的 macro P@10 上限是 `37/(8×10)=46.25%`，R@10 上限为 100%。英文 P@10=43.75% 不应与另一个有不同正例密度的库直接比较，也不能因为未达到 50% 就另造失败门槛。

英文 top10 仅遗漏 dog image `61108`（rank 12）和 bicycle image `10363`（rank 26）。中文八条查询的 top1 都是 cat image `14831`，仅 cat 查询正确；各类 top10 的遗漏如下。遗漏指排名在 10 名以后，不表示图片未索引或结果丢失。

| 查询类别 | 中文 top10 未召回的相关 image ID（实际 rank） |
| --- | --- |
| cat | 无 |
| dog | 17029(11)、22192(16)、29393(18)、61108(21) |
| bird | 56545(12)、100489(15) |
| horse | 12748(32)、16228(20)、23126(11) |
| bicycle | 61108(14) |
| bus | 1584(25)、2006(26)、5037(29) |
| train | 1353(23)、6040(24)、14380(31)、14473(17)、16228(21) |
| airplane | 5477(31)、13348(15)、52017(26)、52412(28) |

这些结果支持“该冻结小库和固定提示词下的中文相关性存在明显不足”，不能从本次结果确定不足的具体原因或宣称英文在用户图库中达到相同质量。32 图是按规则选取的受控、类别 anchor 平衡的小样本，不代表完整 COCO、开放词汇、复杂提示词、不同语言措辞、用户图库或其它设备的泛化质量，也不是 COCO 官方 mAP。

adapter 的自然 PASS 标签表示真实编码、结构有效的完整排名、报告生成及关闭重启持久化流程结束；没有设置 P/R 数值阈值。质量值可能在后续关闭核验之前生成，只有最终状态、`finishedAt`、错误列表和三次 verify 全部齐全时才能记为终态完成。中文质量不足仍应单独保留为产品质量限制。

## 本地证据路径与复核边界

以下路径均相对仓库 `<project>`，位于 `<private-evidence>`，用于本机追溯，不随本报告提交为公开 Git 文件。

| 证据 | 本地目录 |
| --- | --- |
| 300 图成功终态 | `<private-evidence>/native-model-v1/runs/2026-10-02T02-30-23-643Z-300-functional-7fb3794c-0020-4eb7-ab1a-3030e88f5044/` |
| 32 图成功终态 | `<private-evidence>/native-model-v1/runs/2026-10-02T02-36-22-204Z-32-functional-ff65e789-6eb8-4b87-aadd-d3829ce0e6c0/` |
| 冻结原始 annotation / selection / 32 图片许可与 SHA | `<private-evidence>/native-model-v1/natural-coco/runs/20261002T020840Z-ca1b83ceac1e4d4a9fd0aefa379c1231/` |
| 先前 observer 启动失败（未计入 PASS） | `<private-evidence>/native-model-v1/runs/2026-10-02T02-27-37-627Z-300-functional-2be91995-753c-4c4b-a89a-47a75de7da79/` |

两个成功目录均保留 `result.json`、`fixture.json`、`images.jsonl`、`sentinels.json`、两次启动身份、`progress.jsonl`、`process-memory.jsonl`、`window-responsiveness.jsonl`、三份 `verify-*.json` 及 `closed-verification-*` 源 bundle/工作副本记录。自然目录另含 `natural-selection.json`、`natural-rankings.json`、`natural-metrics.json`。数据准备目录保留 `dataset.json` 的早期 annotation-only 状态、`dataset-ready.json`、`private-readback.json`、`selection.json`、`queryplan.json`、原 annotation member、逐图片下载 SHA/许可及失败/续传账目；不同阶段文件保持原始状态，不互相覆盖。

方法代码为 `<private-evidence>/native-model-v1/model-acceptance.mjs`、`verify_closed.py`、`closed_wal.py`、`window-responsiveness.ps1`，以及 `natural-coco/{run.mjs,native_fixture.py,selection.py,range_zip.py,prepare.py,verify_prepared_dataset.py}`。本报告的独立质量复核只读取 JSON/文本，未启动 GUI、编译或用 SQLite 打开验收库；运行和库核验的实际证据来自已经结束的上述原生验收。

## 已缓存模型阶段取消闭环

实际目录为 `<private-evidence>/native-model-v1/runs/2026-10-02T02-39-15-621Z-300-cached-hash-cancel-8661095c-3d61-43f3-9960-c35080a2ffa0/`，身份仍为上表的 `34baa87` / 同一EXE SHA。取消请求是生产UI操作，记录的阶段为 `hash`；从观察请求到同job唯一终态为114ms，终态 processed/indexed/failed均为0、cancelled=true。这是GUI派发和事件传输在内的观察边界，不声称精确的原生函数中断时刻。

四次启动均经OS WM_CLOSE正常退出，retained process handle取得exit0，端口释放；未强杀。`partial-closed`、`partial-restarted`均独立读回300资产、0向量且业务哨兵聚合保持。第三次启动重新真实编码300项，worker墙钟30228ms，indexed=300、failed=0；随后`snapshot`、`full-closed`、`full-restarted`均valid=true，最终stale=0，模型ONNX全文件SHA仍为固定值，无未发布stage，errors=[]。本case各20次warm的原生IPC P95为text1283.3ms、similar6.4ms，均通过3秒预算。

初始化取消另在 `<private-evidence>/native-model-v1/runs/2026-10-02T02-46-05-046Z-300-cached-init-cancel-48e6ae94-c7f2-473d-8383-16b2db05cbc8/` 完成，仍为同源EXE。观察到`init`阶段后UI取消，从观察请求到唯一终态856ms，processed/indexed/failed均为0；单次ONNX init不能被抢占，本次结果不是无条件2秒保证。四次正常WM_CLOSE均exit0，部分关闭/重启两次核验为300资产、0向量，恢复worker23878ms补齐300项、failed=0。后续备份、完整关闭与完整重启核验全valid=true，最终errors=[]；text/similar各20次warm IPC P95为971.0/4.9ms。以上两种阶段取消不代替下载或编码阶段取消。

其余取消阶段与真实模型规模矩阵在取得新终态证据后补充。本报告不将尚在运行的用例、小样本质量、离散内存采样或窗口消息采样提升为尚未取得的版本发布门槛。
