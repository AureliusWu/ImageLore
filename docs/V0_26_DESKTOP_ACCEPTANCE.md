# v0.26.0 桌面原生验收

日期：2026-09-30；本记录主体固定2026-10-01（Asia/Shanghai）的 `f61f59bf86505dadacd15f75da3b75f0ed68fdf3` 程序，版本 `0.26.0`，schema `11`。历史连续使用和恢复复验属于 `99b2699`，真实模型小样本属于 `035a37e`；各轮按实际 source/SHA 保留身份，不混用。

2026-10-02最新桌面稳定路径已替换为 `2276de179da95cf29a889a0a6e7d9a01d940ad8a` / EXE SHA256 `077027CDCE55A9FFCA55D18E00B84D50290FF876F9C708B721BC31BAE532B82B`，仍为 `0.26.0`。实际发布时刻为`2026-10-02T03:06:29.1460572Z`（北京时间11:06:29），快捷方式仍为`%USERPROFILE%\Desktop\ImageLore v0.26.0.lnk`，指向`desktop-runtime/current/ImageLore.exe`。

同源两条CI：[run 36957688387](https://github.com/AureliusWu/ImageLore/actions/runs/36957688387)与[run 36957688415](https://github.com/AureliusWu/ImageLore/actions/runs/36957688415)均completed/success。新产物已完成300图functional（text/similar各20warm p95=964.0/4.8ms，预算通过）、编码取消与恢复、编码期间正常关窗与恢复、失败proxy环境下缓存分支，以及随后实际下载阶段取消；证据见[模型矩阵](V0_26_MODEL_MATRIX.md)和[直接导入关闭保护](V0_26_IMPORT_CLOSE.md)。三个缓存控制用例各3warm仅smoke，不关闭performance；失败proxy分支不等于物理断网。

2026-10-02新增的derivative和Remix受控picker直接UI导入闭环均实际PASS，source/EXE绑定上面的完整2276de1身份。每个case均正常WM_CLOSE、最新编辑与关系/Session/谱系闭库持久回读、独立fresh-profile重启及第二次正常关闭；共四次retained-handle exit0/portReleased，无force kill或恢复重试。

| 直接UI case | 实际覆盖 | 初次 / fresh-profile重启 PID | 结果 |
| --- | --- | --- | --- |
| derivative | 持有真实import Response期间关闭，后续derived_from/Session及最新parent Prompt保全 | 21900 / 23240 | PASS，完整闭库typed business/schema SHA两轮一致 |
| Remix | 持有真实apply_lineage Response期间关闭，base source谱系/Session、child draft Prompt fallback及最新编辑保全 | 2300 / 23460 | PASS，完整闭库typed business/schema SHA两轮一致 |

薄传输仅包装可写fetch，业务全部原fetch→真实原Response延迟→原Tauri回调；唯一synthetic为精确single fixture picker path，`physicalPickerExercised=false`，真实物理选择框仍pending。没有SQLite打开源库，原DB/WAL/SHM bytes/size/mtime before==after；只在独立验证clone读取WAL/checkpoint并比较typed hash。相对本地run目录为`<private-evidence>/native-model-v1/runs/2026-10-02T03-21-04-091Z-direct-derivative-e9f92077-fc2c-41c6-8aab-7826af82a39f`及`2026-10-02T03-22-05-912Z-direct-remix-a3cdd80a-7e01-4cc5-94b9-bb29159717f3`（同一runs父目录）。完整source/EXE SHA、闭库typed SHA与OS发送/renderer事件/Response释放/owned exit/端口确认分开的时刻见[直接导入关闭保护](V0_26_IMPORT_CLOSE.md)。helper排队不当作产品latency；这两case不补充模型warm样本，也不覆盖Remix多参考DNA、任意IPC或直接destroy。

旧`34baa87` current实际保留在`desktop-runtime/previous-20261002-030628-7cbe07beab44440380ad007809f7bf45`，发布回执为`<private-evidence>/desktop-publish-2276de1-20261002.json`。回执保留六个默认DB/WAL/SHM路径检查，均不存在；该范围只涉及列出的六个文件，不扩大为扫描全部用户目录。发布记录`formalV1Release=false`。新2276de1的导入observer上下文19个WM_NULL样本全部响应、max0.5179ms；采样不等于render/input。编码期间关窗完整观察到终态2821ms仍超过2秒，详见上述矩阵。

此前10:36发布的`34baa871dfee85794e30ae2b3e9e112761cd06c7` / EXE `3D38BAC9E12AAC61396EF9574C893F224087A23255CFDA01C8F0069ACAAB7B4C`、其自然32图质量以及哈希/初始化取消均保留原身份。它替换的更早current保留在`desktop-runtime/previous-20261002-023612-af4e65cf993146319588539fa37ca904`，原回执`<private-evidence>/desktop-publish-34baa87-20261002.json`保持。下表与后续历史性能、内存及自然质量指标不转记为2276de1的结果；新来源已补齐上述两项受控选择输入导入关窗闭环，真实物理选择框和其它模型规模门槛保持各自验收状态。

已完成实际桌面快捷方式和原生构建；保存/关闭/重启、备份恢复及分页修复均有真实程序证据。真实 CLIP 已通过三图小样本集成验收；启动、内存和全规模模型等退出标准尚未关闭，不将本轮开发更新标为阶段 A 全部通过或 v1.0.0 正式发布。

## 10月1日程序与门禁

| 项目 | 结果 |
| --- | --- |
| 桌面快捷方式 | `%USERPROFILE%\Desktop\ImageLore v0.26.0.lnk` |
| 实际目标 / ProductVersion | `<project>\desktop-runtime\current\ImageLore.exe` / `0.26.0` |
| 产物 | SHA-256 `D0AF13E8B229B326CA291EDE454D421482DBA637F97CBC5D271A3D5616271F82`；原生 release 无安装器构建 |
| runtime manifest | builtFrom `f61f59bf86505dadacd15f75da3b75f0ed68fdf3`；publishedAt `2026-10-01T11:49:02.9511496Z`；每次替换的 previous 目录保留 |
| 本地门禁 | 前端 check / build，Rust fmt / locked check / strict clippy / full lib test 均通过；85 command 契约、33 Node 行为用例（9 + 3 + 3 + 18）、104 Rust passed / 1 ignored；规模用例另行执行 |
| CI | 当前 `f61f59b` 的 [ImageLore CI](https://github.com/AureliusWu/ImageLore/actions/runs/36857820830) 与 [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/36857821109) 均 completed / success |
| 分页复验（f61f59b） | 4 / 4，通过真实滚动和每轮四连选：240→480→720→960→1200；末张身份、解码与 fit/actual 模式保持正确 |
| 正常启动 | `f61f59b` 三次进程重新启动：3,471.5 / 3,254.2 / 3,277.7 ms 到 FCP，全部超过建议 ≤3 秒，保留 **WARN** |

当前证据为 `<private-evidence>/desktop-build-perf-final-v026.txt`、`frontend-check-perf-v026.txt`、`cargo-{clippy,test}-perf-final-v026.txt`、`startup-backup-lock-{check,clippy}.txt`、`native-gui/perf-final-runtime-identity.json`、`pagination-perf-final.json` 与 `startup-after-perf-{1,2,3}.json`。文档追加提交不改变上述程序源 SHA 或产物身份。

历史 `99b2699` EXE 的 SHA-256 为 `EA0251C953FEF20AC06A9AD5ADEB7AE5D011E47719FF2676E6D57BE076950EFE`；`035a37e` 为 `08FEC9ADBA57C4290A907CFC83DDAD5B991AA8C32B5D6640E2995388C790AFA7`。从前者到后者，唯一产品代码修改是公共 CLIP 下载器直接以本库 cache 构造客户端并显式禁用 token；固定模型 revision/大小/SHA 保持。两者的 UI 代码相同，历史 10 分钟测量仍绑定原 EXE。`035a37e` 另外通过实际启动、Revision 保存、输入后 26 ms OS 关窗、进程退出后 SQLite 最新值回读及三图真实模型流程。历史 `99b2699` 和 `035a37e` 的两套 CI 均已 success，不能代替当前 source 的 CI。

本次实现三项小修复：独立备份操作锁避免近期备份验证占用图库连接；按源文件真实尺寸统一无需缩小的小图 Fit/full 缓存；使用纳秒源时间戳修复同秒同大小替换后的旧预览。全部先复现后回归，不削弱验证、原件保全或缓存预算。

启动采样保留同一隔离资料库和 WebView2 profile，不清 Windows 文件缓存，也不称为整机冷启动。修改前 `035a37e` 的三次 FCP 为 3,671.1 / 3,227.0 / 3,232.4 ms；修改后为 3,471.5 / 3,254.2 / 3,277.7 ms。中位数约 3,232.4→3,277.7 ms（+1.4%），没有证明 FCP 改善，也没有通过 ≤3 秒预算。首次近期备份检查曾与编译回归并行，顺序采样只有各三次，差异不作统计显著性结论。FCP 只表示出现内容，不代表全部图片和交互就绪；观测到首图就绪的上界另保留在 JSON，不能冒充精确首次可交互时间。真正需要新快照时 VACUUM 仍持库锁，启动安全探针和恢复开销单列保留。

## 首轮基线与修复来源

| 项目 | 首轮结果与边界 | 证据 |
| --- | --- | --- |
| 本地门禁 | check、clippy、前端检查/构建及原生 release 构建通过；Rust 95 passed / 1 ignored，规模用例单独执行。此结果属于首轮代码；后续修复的独立门禁见上表。 | `<private-evidence>/*-v026.txt`；Rust 明细为 `cargo-test-v026.txt` |
| 同提交 CI | `6c51b98` 的 Native CI、ImageLore CI 与 OG Preview 均 completed / success。 | [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/36684213606)、[ImageLore CI](https://github.com/AureliusWu/ImageLore/actions/runs/36684213669)、[OG Preview](https://github.com/AureliusWu/ImageLore/actions/runs/36684213716) |
| 实际桌面发布 | 首轮 `desktop-runtime/current/ImageLore.exe` 为 0.26.0；桌面 `ImageLore v0.26.0.lnk` 指向该稳定路径，描述含版本。首轮 SHA-256 见下文；当前产物以最新身份表为准。 | `<private-evidence>/desktop-publish-v026.txt`、`desktop-shortcut-real-v026.json` |
| 发布事务测试 | 初次发布、升级保留 previous、非本工具拥有的新版本链接碰撞回滚、独占文件锁阻止替换，4 组通过；失败后旧 EXE、state 与链接保持。仅在 GUID 临时目录使用合成 C# EXE，未运行这些 EXE，未触碰实际桌面或用户数据。 | `<private-evidence>/test-publish-runtime-v026.txt` |
| 原生就绪观察 | 真正 Tauri WebView，1520×940 / DPR 1.25；资料库 50,000 项，观察时加载 240 项、DOM 14 张卡片且有预览。该就绪快照不提供首屏完整交互完成时间。 | `<private-evidence>/native-gui/ready-state.json` |
| 首次 FCP | **WARN：启动记录到 FCP 4,819.5 ms**，超过 ≤3 秒的建议门槛。仅 1 次启动观察，不能称为 3 次冷启动 p95。 | `native-gui/launch.json`、`ready-state.json` |
| Native API / 预览 | 真实 WebView 调用生产 Tauri IPC；6 项分页/筛选与 seeded 相似图全部零错误，预览生成、asset 协议获取与解码成功。计时范围见下一节。 | `<private-evidence>/native-gui/benchmark-ipc.json` |
| 10 分钟 GUI 交互 | **局部失败**：600,252 ms、299 轮，action/runtime/console errors 均为空，最终缩略图 12/12 已载入、预览可用；但分页停在 **480/50,000**。无异常不代表滚动功能通过。随后真实无点击/连点对照确认共享请求序号竞态，已分离图库与选图序号，并加入 4 条正式 App callback 回归。 | `native-gui/soak-2026-09-30T07-46-35-238Z/result.json`、`analysis.json`、`raw.jsonl`、`pagination-repro.json` |
| 存储与历史迁移 | backup 24/24、migration 15/15、startup 9/9；覆盖 schema 1–11、WAL 快照、损坏原件保全、锁拒绝、失败回滚与 pending 重试。各过滤套件有交叉，不累加为独立用例数。 | [存储验收](V0_26_STORAGE_ACCEPTANCE.md) |
| 编辑与跨库工作流 | 首轮前端受控 harness 14/14；importer 4/4、Sidecar 5/5、sources 2/2。真实 SQLite 文件跨库导出导入回读通过；后续 GUI 关闭重启实测见下文。 | [工作流验收](V0_26_WORKFLOW_ACCEPTANCE.md) |
| Rust 原生规模 | 50k / 75k 文件库均完成已知答案验证；75k 英文/引号查询超过建议 500 ms，向量扫描尾延迟退化为 WARN。此为 debug 构建的 Rust API 数据，不能替代桌面渲染与真实模型测试。 | [原生规模验收](V0_26_NATIVE_SCALE.md) |

首轮桌面 EXE SHA-256：`D3407F53D0B14174AA3676822C5DA06D81D08A15B02FEB977002332A61C8F45B`。

首轮启动时间为 `2026-09-30T07:37:47.661Z`（北京时间 15:37:47.661）。WebView `timeOrigin` 比启动记录晚 775.5 ms，Paint Timing 的 FCP 为 4,044 ms，合计 4,819.5 ms。FCP 只表示出现内容，不保证首屏数据、所有图片与交互均已准备好。

## 首轮 Native IPC / 预览测量

| 50k 场景 | 样本数 | warm p95（ms） | 结果 |
| --- | ---: | ---: | --- |
| 第一页 COUNT + 摘要 | 20 | 24.5 | PASS，API 建议预算 500 ms |
| offset 48,000 深分页 | 20 | 28.8 | PASS |
| 英文筛选 | 20 | 325.8 | PASS |
| CJK trigram 筛选 | 20 | 188.1 | PASS |
| DNA-only / Reference-only | 各 20 | 64.7 / 52.9 | PASS |
| 512 维 seeded 相似图 | 20 | 453.1 | PASS，仅合成向量排序 |
| 2,200×1,200 预览端到端 | 3 冷 / 20 暖 | 冷 40.8 / 暖 3.0 | PASS，仅当前图片与缓存协议 |

API 计时在 Native WebView 内以 `performance.now()` 包围 `__TAURI_INTERNALS__.invoke`，包含 IPC、Rust API 与响应反序列化，排除 CDP 传输、断言及 React 渲染。预览端到端包含 IPC、asset 获取与图片解码；每个冷样本删除对应派生 WebP、清浏览器资源缓存并加 nonce，但未清 Windows 文件缓存。6 项列表查询及相似图测量均为 20 个暖样本，不能外推为所有查询、整屏响应或无限容量的保证。

图片是 **50,000 个 synthetic solid-color PNG**：100 个 2,200×1,200、16,633 个 128×96、33,267 个 64×80，不能证明真实照片、大文件、复杂纹理与各类输入格式的解码性能。512 维 embedding 为固定 seed 的合成向量；该测试使用生产扫描/排序，但**不证明真实 CLIP 模型的下载、初始化、编码、召回质量或真实模型端到端耗时**。

测试程序使用 `<private-evidence>/native-gui/data` 中的独立资料库副本；50k 基线库保持不变，副本完整性为 `ok`，foreign-key errors 为空。首轮 WebView 为 Edge 154.0.4258.37，测量结束于 `2026-09-30T07:43:07.819Z`。所有原始日志为本地验收证据，CI 链接对应明确提交，不使用旧 `1a701ea` 的 CI 记录证明 `6c51b98`。

## 连续交互、保存与恢复

`99b2699` / EXE `EA0251…950EFE` 的第二轮连续操作为 600.273 秒、299 轮、1,195 次点击。加载数量 1,200→1,440→…→3,600，每页稳定追加 240；图库 DOM 卡片 18–20 个，可见 8–10 个。299 次末张选择均验证身份/fingerprint和当前图 decode；60 次图像快照中的 838 张图全部完成并解码。Runtime、console、action errors 均为 0。

包含 CDP 输入与两帧/解码等待的滚动 p95 为 19.04 ms，四连选及末图解码 p95 为 197.49 ms，明确等待新模式 cache src 的切换/解码 p95 为 67.02 ms。首轮四连选最大 2,296.16 ms 保留；不能把 warm p95 外推成每次交互都满足预算，也没有遍历全部 50,000 张。JSHeapUsed 为 5.215–6.627 MiB，末值 6.126 MiB，Nodes 1,001–1,251；这不能替代全进程内存。此前记录的 5.47–6.95 / 6.42 为十进制 MB，本记录已统一成 MiB。

全进程采样只含 root PID 13780 与 6 个 WebView 子进程，基线为开始前 8.788 秒，正式 10 分钟窗口内 35 个样本，末样早于结束 18.092 秒：

| 指标 | 起点 MiB | 峰值 MiB / 增长 | 末样 MiB / 增长 |
| --- | ---: | ---: | ---: |
| 进程 Working Set 之和 | 469.29 | 569.43 / +21.34% | 560.73 / +19.48% |
| Private Bytes 之和 | 313.43 | 457.63 / +46.01% | 445.47 / +42.13% |

**内存为 WARN**，私有提交量和驻留峰值均未通过建议 <20% 增长门槛。增长主要来自 WebView；后半段 Private Bytes 净变化为 −6.72 MiB，但回归斜率仍略正，不能声明长期稳定或无泄漏。Working Set 可能重复统计共享页，Private Bytes 是私有虚拟提交量而非驻留 RAM；约 30 秒轮询不能捕获全部瞬时峰值。首末未覆盖的秒数、窗口外样本与后续模型测试均不并入指标。证据：`native-gui/memory-summary.json` 和两份 process-memory JSONL。

真实保存验证使用 `.prompt-box` 输入与保存版本按钮。`99b2699` 第二轮在输入后 17 ms 发送 OS WM_CLOSE，窗口/进程正常退出，SQLite readonly 读取最新文本一致；重新启动后再次读取一致。随后通过生产 IPC 创建 `imagelore-1790757296148178700.sqlite3`，修改该记录、暂存恢复、关闭/重新启动，资产 25893 的文本恢复到原哨兵。恢复启动 FCP 为 38.236 秒，**单列慢恢复 WARN**，不混入正常启动样本；raw 原库 DB/WAL/SHM 与恢复前快照均保留。

历史 `035a37e` EXE 又独立验证 Revision 保存、输入后 26 ms OS 关窗和退出后的最新文本回读。证据：`native-gui/save-close.json`、`shipped-closed-readback.json`；前一轮进程重启/恢复证据为 `backup-stage.json`、`restore-verified.json`、`startup-restore-final.json`。Backup 保存 SQLite 元数据与关系，原图、模型与 cache 不包含在快照内。

所有大图库操作使用受 marker 保护的 `IMAGELORE_TEST_DATA_DIR` 和独立 WebView profile。正常用户路径的 `library.sqlite3`/WAL/SHM 验收前后均不存在，未把测试库写入该位置；正常目录新增/维护的是显式要求的 desktop-shortcut 状态。对照证据：`native-gui/user-library-{before,after}.json`。

## 真实模型小样本

`035a37e` 程序完成固定 revision 的 vision/text ONNX 下载和 SHA/尺寸验证，真实编码三张 224×224 纯色 PNG，生成三组 fresh 512 维向量；生产备份独立读取、归一化、点积与排序复核通过。20 次暖文本检索 p95 为 1,270.1 ms，相似图 p95 为 4.8 ms。首次文本查询约 18.058 秒，冷模型取消约 19.156 秒才观察到终态，继续保留等待/取消风险。详见[模型验收](V0_26_MODEL_ACCEPTANCE.md)；三图集成证据不能替代 50k 吞吐、自然图片召回质量和完整断网矩阵。

## 10 月 1 日当前 EXE 复验

`f61f59b` / `D0AF…71F82` 的连续交互窗口为 `2026-10-01T11:51:04.479Z` 至 `12:01:04.758Z`，工作流计时 600.282 秒。300 轮、1,198 次点击、300 次末图身份/fingerprint/当前元素解码检查通过；60 次快照的 838 张图全部完成并解码。图库持续从 1,200 追加至 3,600，DOM 18–20、可见 8–10，Runtime/console/action errors 均为 0。没有遍历全部 50k，素材仍为上述纯色合成分布。

滚动 p95 19.04 ms；连点及末图解码 p95 205.55 ms，最大 386.83 ms。模式采样 p95 17.64 ms 只覆盖画面模式、资源身份与解码：小图两种模式都用同一 full URL，300 个模式 URL 均为 full，其中 150 次为 Fit。300 个总模式快照中有 258 个状态仍为“正在加载预览…”。它不能与历史等待不同 URL 的 67.02 ms 当成完整加载的同口径加速。随后独立 20 次切换在点击后等待两帧、连续两次就绪并校验连接中的图像 decode，全部通过，median 42.45 / p95 47.09 / max 50.19 ms。此为单张小图暖缓存，不外推复杂大图。

分钟 JS heap 为 5.563–9.159 MiB，末 6.217 MiB；Nodes 1,119–1,247。全进程正式窗口含 root PID 14508 和 6 个 WebView 子进程，60 个约 10 秒采样；基线早于开始 10.048 秒，首样晚 0.157 秒，末样早于结束 1.202 秒，3 个结束后样本排除：

| 指标 | 基线 MiB | 峰值 MiB / 增长 | 末样 MiB / 增长 |
| --- | ---: | ---: | ---: |
| Working Set 之和 | 533.35 | 586.79 / +10.02% | 539.29 / +1.11% |
| Private Bytes 之和 | 375.83 | 503.90 / +34.08% | 470.71 / +25.24% |

**内存仍为 WARN**。基线到首样 Private 已回落至 347.22 MiB；相对首样末值增长 35.56%，不能只选更高基线美化结果。后半 Private 增加 38.59 MiB，回归斜率 +6.76 MiB/min；基线至末样主进程 −3.30 MiB、WebView +98.18 MiB。不同轮次起始状态和 cache 不同，不能把百分比变化写成已消除增长或证明缓存是唯一原因；未放宽 <20% 建议预算。

本轮输入后 19 ms OS 关窗，进程正常退出，SQLite readonly 回读资产 25998 的最新文本一致；再次启动也一致。生产创建的 `imagelore-1790856241402461800.sqlite3` 已独立 immutable 检查 integrity/FK 和该文本。首个验收脚本在 30 秒 CDP 超时后退出，但原生备份继续完成；没有把超时当作取消或删除备份。延长本地验收等待并复用这份完成且验证过的快照，修改记录、stage、关窗重启，恢复文本一致。恢复启动 FCP **84.369 秒 WARN**，不混入正常启动；本轮恢复点数量增加，不能据此作单因素前后归因。

当前 EXE 另以原三图隔离库和已有可信模型执行 text/similar 各三次、生产备份向量复核及 OS 关窗，状态 `PASS_FINAL_RUNTIME_MODEL_SMOKE`。这证明当前产物仍能使用真实模型，不重复声称首次下载或 20 次暖采样属于该产物。所有验收窗口已正常关闭，正常用户 DB/WAL/SHM 仍不存在。

本地证据：`native-gui/soak-canonical-2026-10-01T11-51-04-428Z/{result,analysis,raw}.json*`、`memory-summary-perf-final.json`、`process-memory-perf-final.jsonl`、`mode-stable-perf-final.json`、`save-close-perf-final.json`、`closed-readback-perf-final.json`、`backup-independent-perf-final.json`、`backup-stage-perf-final.json`、`restore-verified-perf-final.json`、`startup-restore-perf-final.json`、`user-library-after-perf-final.json` 与 `native-model/smoke-perf-final.json`。历史报告和证据均保留，不覆盖为新来源。

## 阶段结论与后续门槛

本轮已同步源码、交付可运行 `0.26.0` 桌面程序和随实际 EXE 版本维护的快捷方式，完成存储/迁移安全回归、原生分页修复及实际保存/关闭/重启/恢复验证。derivative/Remix受控picker UI导入关闭、关联/Session/谱系与最新编辑持久化、fresh-profile重启已各自实际PASS；真实物理选择框仍pending。**阶段 A 保持进行中**：正常启动、恢复启动与WebView内存增长保留历史WARN；新产物编码关窗完整观察2821ms仍超过2秒。最新300功能、编码取消/关窗恢复、下载取消以及5k真实编码/20warm/备份关闭重启已取得各自终态，旧来源自然图和哈希/初始化取消不转嫁。5k text/similar IPC p95为989.2/56.2ms，均通过3秒预算；内存观测保留一次采样缺口，不推算50k耗时。50k真实模型、真实物理选择框交互、Remix多参考DNA、下载取消后的重新下载恢复、物理离线及复杂图片长期稳态仍待验收。阶段B的证书/签名服务、公开可访问发行渠道和updater密钥管理尚未配置，后续按A→B→RC→1.0顺序推进。
