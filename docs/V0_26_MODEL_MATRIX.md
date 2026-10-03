# v0.26.0 真实模型验收矩阵

本报告按实际 source/SHA 分开记录 2026-10-02 的模型验收，2026-10-03 更新既有50k报告的只读复核。历史 `2276de1` 产物完成300图、5k、50k功能闭环、编码期间关窗与恢复、编码取消与恢复、失败代理环境下的缓存分支，以及实际下载取消和重新下载恢复。旧 `34baa87` 的自然32图、哈希取消和初始化取消保留原身份；这些结果也不能转记为隐私重写后源码的验收。各项只有终态、正常关闭与独立持久化核验齐全才记为通过；功能通过不等于质量或全部版本门槛通过，三次可比冷测仍未完成。

桌面current于北京时间11:06:29更新为 `2276de1` / EXE `077027CD…32B82B`，旧 `34baa87` 保留，见[桌面更新回执](V0_26_DESKTOP_ACCEPTANCE.md)。这不表示v0.26或v1.0.0的全部退出条件已满足。

旧 `34baa87` 自然图的功能终态通过；质量报告没有数值通过门槛。冻结32图中的英文宏平均 P@1=100%、R@10=95%，中文分别为12.5%、37.5%。旧产物300图导入记录了三次窗口消息超时；新 `2276de1` 功能流程的导入观察上下文19个WM_NULL样本全部响应、max=0.5179ms。该采样不等同渲染或输入体验。新产物编码期间关窗的完整观察到终态为2821ms，超过2秒，保持单列。

## 已完成与待补矩阵

| 用例 | 本报告状态 | 已取得的证据与边界 |
| --- | --- | --- |
| 300 图真实模型功能流程 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 两个故意图片失败恢复至300/300；text/similar各20次warm p95=964.0/4.8ms，预算通过；三阶段持久核验通过 |
| 300 图真实模型功能流程 / 旧34baa87 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | PNG/JPEG/WebP各100张，含6张大图；实际编码、故障恢复、各20warm及持久核验；保留旧导入timeout事实 |
| 32 图自然图片流程 / 旧34baa87 | `PASS_NATURAL_COCO32_ENCODING_RECALL_REPORT_AND_DURABLE_RESTART` | 32/32实际编码，16条冻结查询完整排名和质量报告；该PASS不包含相关性数值门槛，新产物未重测自然图 |
| 已缓存模型哈希阶段取消 / 旧34baa87 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 114ms观察到同job终态；0向量部分库重启、重新编码300项、备份和完整重启均通过 |
| 已缓存模型初始化阶段取消 / 旧34baa87 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 856ms观察到同job终态；部分库重启、补齐300项及完整持久闭环；单次ONNX init仍不可抢占 |
| 编码阶段UI取消与恢复 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 113ms观察到终态；10项已提交索引保持，恢复290项至300/300；仅3warm smoke |
| 编码期间WM_CLOSE与恢复 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART`，时间WARN | 完整观察2821ms；47项部分库重启保持，恢复253项；全部关闭exit0，仅3warm smoke，不掩盖>2秒观测 |
| 失败代理环境下缓存分支 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 已可信缓存编码300/300，备份及重启核验；仅3warm smoke，不表示物理断网 |
| 实际下载阶段UI取消 / 新2276de1 | `PASS_OBSERVED_DOWNLOAD_CANCEL_AND_DURABLE_CLOSE` | 观察暂存92,828,085字节后取消，112ms观察到终态；0向量部分库重启，未发布组件且无未发布stage；未执行重新下载或编码恢复 |
| 5k真实模型规模流程 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 5000/5000真实编码、各20warm及备份/关闭/重启核验通过；详见下节 |
| 50k真实模型规模流程 / 历史2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 50000/50000实际编码、failed=0；各20warm IPC p95=1350.6/486.1ms；备份/两次关闭/一次fresh重启持久核验一致；合成图库非自然召回 |
| 下载取消后重新下载并恢复 / 新2276de1 | `PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART` | 独立空缓存中实际取消、部分库重启，再真实下载可信组件及编码300项，五阶段核验和四次正常关闭通过；仅3warm smoke |
| 物理断网、损坏模型重新下载 | 未完成本矩阵验收 | 已完成下载重试不代替损坏模型重试；失败代理缓存分支不等于物理断网 |
| derivative / Remix 导入中物理关窗 | 受控选择输入的原生UI闭环通过 | 真实关系/Session/谱系和最新编辑完成后正常退出，fresh-profile读回一致；fixture picker bypass及其它边界见[导入记录](V0_26_IMPORT_CLOSE.md) |

## 历史2276de1产物的独立终态

实际来源 `2276de179da95cf29a889a0a6e7d9a01d940ad8a`，EXE SHA256 `077027CDCE55A9FFCA55D18E00B84D50290FF876F9C708B721BC31BAE532B82B`，ProductVersion仍为0.26.0。实际候选路径为 `<private-evidence>/import-candidate-2276de1-20261002T025343-32ea0c6b/ImageLore.exe`。下列五项均有对应最终`finishedAt`、`errors=[]`、独立隔离目录和实际启动身份；未使用种子向量。

| 模式 | 终态结束（UTC） | 持久核验及正常关闭 |
| --- | --- | --- |
| functional | `02:59:06.957Z` | snapshot/full-closed/full-restarted均valid，300索引；两次WM_CLOSE确切exit0 |
| close-encoding | `03:01:04.442Z` | partial-closed/partial-restarted为47索引，恢复253项后完整三阶段为300；四次WM_CLOSE确切exit0 |
| encoding-cancel | `03:04:50.944Z` | 两个部分核验为10索引，恢复290项后完整三阶段为300；四次WM_CLOSE确切exit0 |
| offline-cached | `03:07:42.588Z` | worker24386ms，完整三阶段为300；两次WM_CLOSE确切exit0 |
| download-cancel | `03:11:28.096Z` | 两个部分核验为300资产、0索引、0向量字节；两次WM_CLOSE确切exit0；未执行完整编码流程 |

前四项最终stale=0、vector bytes=614400；每项自己的snapshot→完整关闭→完整重启asset/metadata/vector aggregate SHA相等，11项比较匹配、无差异。两个带恢复的控制用例先分别校验47或10项的部分关闭/重启SHA一致，再实际编码剩余项，不把部分索引写成全量完成。各verify均integrity=ok、外键错误=0；源DB/WAL/SHM保留，SQLite只在fresh副本中核验。下载取消的两份部分报告也均valid，11项比较匹配；它没有snapshot/full-closed/full-restarted，不补造这些证据。

functional三阶段SHA固定为asset `30f5ead6a827a6da7004a066ac689b4c643f67a17a2a820d7cabb6c674be6b43`、metadata `757a2fd893fd215ff35e43abeef7dd72e7fbd4a4add98193d9afbc1d653a4501`、vector `1c86925d11131d3e054ea8c097f85818f9b4c711c5212413de63bcf406bfdea9`；300向量最大norm偏差为`2.998709114354625e-08`。

| 模式 | 每命令warm数 | text / similar原生IPC统计 | 门槛解释 |
| --- | ---: | --- | --- |
| functional | 20 | median897.0/4.25ms，p95 964.0/4.8ms，max1009.2/5.4ms | 两项20样本p95均通过3000ms预算 |
| close-encoding | 3 | max858.1/4.7ms | 仅功能smoke，`sufficientWarmSamples=false`、`projectWarmGatePassed=false` |
| encoding-cancel | 3 | max1034.0/5.3ms | 同上，不关闭performance门槛 |
| offline-cached | 3 | max928.5/5.3ms | 同上，不关闭performance门槛 |
| download-cancel | 0 | 未执行文本/相似检索warm | 不具有编码或检索性能结果 |

functional的导入IPC为1936.6ms。初始863、完整重启6个WM_NULL样本均无timeout，其中导入observer上下文19个样本均响应、max0.5179ms，实际发送范围北京时间10:57:16.326–10:57:18.317。它们在可证明的observer外包区间10:57:16.250–10:57:18.318内；后界来自下一`library_page`观察起点，导入的精确观察完成时刻没有单独落盘。标签在等待轮询和删除操作时仍保留，不能将19个样本全部硬归入原生1936.6ms的执行区间，也不等同render/input体验。详见[直接导入与关闭保护](V0_26_IMPORT_CLOSE.md)。

close-encoding在观察到encoding且processed=10时选择关窗。完整观察请求为`1790909990342`，helper排队为`1790909990456`，实际OS WM_CLOSE为`1790909993025`，终态事件receivedAt=`1790909993137`、观察终态=`1790909993163`。因此完整观察到终态2821ms，helper queued→WM_CLOSE为2569ms，实际WM_CLOSE→收到事件112ms、→观察终态138ms。**保留2821ms超过2秒的结果**；后两项只能说明实际OS请求后的事件/观察边界，不能把它们冒充完整用户等待，也不能删除helper排队时间。期间继续提交到47项后唯一取消终态，正常退出后部分库读回47项；单图ONNX和部分文件/SQLite操作仍不可抢占，不提供无条件2秒保证。

encoding-cancel为实际UI取消，观察请求到终态113ms，clickSentAt→观察终态102ms；processed/indexed保持10、failed=0，部分库关闭重启后保持，再编码290项。close-encoding与encoding-cancel的用途和起点分别记录，不互换数值。

offline-cached仅在验收子进程环境设置失败的回环HTTP/HTTPS/ALL_PROXY，`NO_PROXY=''`，启动记录`offlineProxyGuard=true`。已有可信组件可继续生产编码和检索，关闭与重启均通过；未关闭物理网络、未改变全局proxy，也不证明任意客户端都遵循这些环境变量或没有网络包。该结果只关闭本次缓存分支，不关闭完整离线矩阵。

download-cancel在无已发布vision/text组件的fresh目录中实际进入download阶段，观察暂存文件共92828085字节后点击取消；观察请求到唯一终态112ms，processed/indexed/failed均0、cancelled=true。关闭后的`cacheAfterCancelledClose`两个组件均`published=false`、`unpublishedStages=[]`。部分库重启保持0向量；这个PASS限定下载取消与持久关闭，没有重新下载成功、模型初始化、编码恢复或冷下载性能结果。

五项本地目录均在 `<private-evidence>/native-model-v1/runs/`：

- `2026-10-02T02-57-03-885Z-300-functional-f3640808-be4e-4cec-907b-eda611ea9bda/`
- `2026-10-02T02-59-30-969Z-300-close-encoding-a61f2a7c-0d35-4fbf-82e9-d5b496cb58b1/`
- `2026-10-02T03-03-00-973Z-300-encoding-cancel-2aa68e41-8c85-42a8-9c6a-c1ffa53ab29a/`
- `2026-10-02T03-06-30-340Z-300-offline-cached-a5c9a740-1c8c-4278-87c5-574ee42ee0e8/`
- `2026-10-02T03-10-54-699Z-300-download-cancel-1b68885a-acc9-4f66-bf3a-ed50159976a8/`

各目录的`result.json`、实际launch身份、`progress.jsonl`、`window-responsiveness.jsonl`与相应`verify-*.json`保留原始观测；不以本报告改写原始终态。

## 下载取消后的重新下载恢复

独立run为`<private-evidence>/native-model-v1/runs/2026-10-02T03-23-26-334Z-300-download-cancel-retry-9e376e9f-d564-4ef1-aa1c-bd8dde5b5638/`，来源仍为2276de1与同SHA EXE。2026-10-02T03:25:23.119Z完整终态`PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART`、errors为空。开始时未复制缓存，实际下载暂存72602865字节后UI取消，121ms观察到唯一取消终态，indexed=0。部分库正常关闭和独立profile重启均校验300资产、0向量。

随后另一个fresh profile在相同隔离数据目录启动，真实重新下载并校验vision/text ONNX，分别351686194/254102519字节，完整SHA与固定production pin一致；support JSON仍只具有已有revision及大小/结构校验边界，不扩大为独立认证的support hash。恢复worker32447ms包含下载、哈希、初始化和实际编码，300/300 indexed、failed=0。snapshot/full-closed/full-restarted全部valid、integrity=ok、FK=0，向量与资产/元数据摘要各自保持；四次正常WM_CLOSE确切exit0、端口释放、未强杀，最终stale=0且无残留未发布stage。每命令3次warm只作功能smoke；单次完整下载恢复不构成三次可比冷测或下载性能承诺。

## 5k真实模型规模流程

2026-10-02T03:20:24.571Z终态为`PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART`，`errors=[]`。来源仍为上节`2276de1`与同SHA EXE。独立目录为`<private-evidence>/native-model-v1/runs/2026-10-02T03-11-50-390Z-5000-functional-aad6ac57-bced-45fa-8471-c5c48e84fbff/`。5000张受控PNG/JPEG/WebP（含6张大图）全部实际编码，没有种子向量；本场景不衡量自然图片召回质量。

| 项目 | 实际结果 |
| --- | --- |
| UI调度至worker终态 | 367059ms；5000 processed/indexed，failed/skipped=0 |
| 整个worker吞吐 | 13.6218图/秒，包含模型准备、哈希和初始化；非独立推理吞吐，不外推50k耗时 |
| 文本检索20warm IPC | median886.95 / p95 989.2 / max1096.4ms；3000ms门槛通过 |
| 相似图检索20warm IPC | median49.4 / p95 56.2 / max58.7ms；3000ms门槛通过 |
| 首次文本/相似查询 | 913.2 / 46.1ms；单次首次调用不是三次可比冷测 |
| 原生备份IPC | 290.8ms；只适用于本5k库，不替代50k多备份轮换结果 |
| 窗口WM_NULL | 初始4299、重启6次，无timeout；不代替render/input和长期稳态内存门槛 |
| 两次正常WM_CLOSE | retained-handle确切exit0，端口释放，未强杀；OS请求至观察退出463/547ms，为采样上界 |
| 完整索引 | 5000项、10240000向量字节、512维有限单位向量、当前fingerprint，stale=0 |

snapshot/full-closed/full-restarted均`valid=true`、integrity=ok、外键错误=0，三阶段asset/metadata/vector aggregate分别保持`942b6a5a1ab8449393f7aa635be8c480f77e936e3d4b201a171a2036eeaf9b1e`、`6231fe9d5264900d3ae0a9665d8149762d89a6fd62c1e88625e2e5342fe4ffe5`、`0607ea090ce038a47bfe42c679805fccaba33344fc52781b8388a14c5e79798e`。关闭副本核验保留原DB/WAL/SHM，源SQLite未打开。

编码期间记录236份进程采样，观测Private Bytes峰值826478592字节；sample81中的PID1696在采样时退出或不可访问，保留一次collection failure，未补零或删除记录。这是带已知采样缺口的观测峰值，不声明完整进程树的真实最大值，也不计为10分钟内存预算通过。模型功能、索引及持久闭环的PASS不覆盖该内存缺口。

## 50k真实模型规模流程

2026-10-02T04:56:14.437Z终态为`PASS_REAL_MODEL_ENCODING_AND_DURABLE_RESTART`、errors为空，绑定上节完整`2276de1` source和EXE SHA；不是隐私重写后源码的重新验收。独立run为`<private-evidence>/native-model-v1/runs/2026-10-02T03-25-53-021Z-50000-functional-ddfcbc16-302f-4ee9-be09-b180b7bd778c/`。50,000张合成PNG/JPEG/WebP（含6张大图）真实编码，无种子向量；本项使用已有可信缓存，不表示自然图片召回或冷下载验收。

| 项目 | 实际结果 |
| --- | --- |
| UI调度至终态观察 | worker 3882340ms；processed/indexed=50000，failed/skipped=0、cancelled=false |
| worker吞吐口径 | 12.8788图/秒，包含prepare/hash/init及终态观察；不是隔离ONNX推理吞吐 |
| 文本检索20warm nativeIpcMs | min1170 / median1251.55 / p95 1350.6 / max1439.2ms；3000ms门槛通过 |
| 相似图20warm nativeIpcMs | min408.5 / median474.7 / p95 486.1 / max489.1ms；3000ms门槛通过 |
| 首次文本/相似检索 | 1727.1/441.3ms，各一次；三次可比冷测仍未完成 |
| 向量核验 | 50000项有限512维float32、102400000字节、当前fingerprint、stale=0；norm最大误差3.221689715005027e-08 |
| 正常关闭与持久化 | 一次fresh-profile重启、两次WM_CLOSE retained-handle确切exit0，端口释放，无强杀；三份核验摘要一致 |

20个warm原始样本已独立重算，median取中间两项均值、p95取`ceil(.95*20)`近秩，与报告相符；nativeIpcMs不包含外部轮询。snapshot/full-closed/full-restarted全部valid、integrity=ok、FK=0，资产/元数据/向量三组完整SHA见[50k验收记录](V0_26_MODEL_ACCEPTANCE.md)。两closed报告各11字段比较无差异，WAL只在新独立副本读取，clone checkpoint为(0,0,0)、前后typed业务/schema保持；源SQLite未打开，原DB/WAL/SHM的bytes/size/mtime/SHA保持并全保留。

北京时间两次实际WM_CLOSE请求/观察退出为12:55:24.713/12:55:25.006、12:55:54.492/12:55:54.813；293/321ms为观察边界，helper queued→实际请求约1.1s单列，不归成产品阻塞。内存仅采 initial worker/查询窗口11:49:12.660–12:55:11.049：2711份、collection failure=0，观测Private Bytes峰953008128字节（908.86MiB）、summed Working Sets峰969887744字节（924.96MiB）。采集本身median1387/max3416ms，可能漏瞬态；不覆盖导入准备或重启，Working Sets可能重复共享页，不能当长期内存上界或滚动预算通过。功能、持久与warm预算PASS不关闭自然召回、三次冷测或阶段A全部门槛。

## 旧34baa87的固定程序与模型身份

下文300功能、32自然图、哈希和初始化取消证据仍绑定旧 `34baa87`；以下两组详细计时和质量结果不属于最新2276de1。

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

这些采样证明旧34baa87的300图导入期间存在窗口消息响应受阻。采样间隙、observer上下文和整数毫秒壁钟不能确定连续冻结的精确起止，不能把三次timeout相加后称为全部冻结时长，也不能据此声称整个2422.3ms都完全不响应。新2276de1的后续修复与WM_NULL复核见上节；新样本不抹去旧产物的历史事实，也不形成完整渲染或输入体验结论。

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

以下路径以仓库外的 `<private-evidence>` 为抽象根，用于本机追溯；该根不是仓库子目录，原始材料不随本报告提交为公开 Git 文件。

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

历史2276de1的下载取消/重试、编码取消、关窗恢复及5k/50k真实模型闭环见前节；旧34baa87的哈希、初始化和自然图证据仍仅属于旧来源。受控选择输入的原生UI导入关闭闭环见[导入记录](V0_26_IMPORT_CLOSE.md)。50k已取得完整终态和独立持久核验，三次可比冷测仍待完成；不将合成规模、小样本质量、离散内存或窗口消息采样提升为完整版本发布门槛，也不转记到隐私重写后的新源。
