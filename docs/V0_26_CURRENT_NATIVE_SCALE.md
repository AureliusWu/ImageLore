# v0.26 当前优化构建原生规模查询验收

2026-10-23 后续：SQLite 安全升级后的独立 50k/75k 重测已完成，实际 1/0/0、38 场景 / 874 样本、0 budget WARN，见[新引擎规模验收](V0_26_SQLITE_SCALE_ACCEPTANCE.md)。本页保留旧引擎原始源码和数字，不将其转记为新引擎结果。

日期：2026-10-22（Asia/Shanghai）。版本 `0.26.0`、schema `11`，受测源码 `1da13713040485a2919bfb26258425624b0fafaa`。本轮直接运行实际归档的 optimized Rust library 测试，`opt-level=3`、`debug_assertions=false`；没有构建或运行应用安装包。

实际终态为 `PASS_OPTIMIZED_NATIVE_SCALE_CORRECTNESS`，**1 passed / 0 failed / 0 ignored，160.98 秒**。50k 与 75k 两个新建文件 SQLite WAL 库各 19 场景，共 **38 场景、874 个计时样本**；全部正确性断言通过，所有 warm p95 都在原建议预算内，**budget WARN 为 0**。正确性结论与预算结论分别保留，这不是阶段 A 或 v1.0.0 全部验收完成。

## 方法与实际结果

两规模均使用固定 seed `0x496d6167654c6f72`、512 维合成向量及独立已知查询答案，保留真实分页、COUNT、组合筛选、关键词/CJK、DNA/Reference 专属命中及 `semantic::ranked` 评分排序路径。未复用旧 fixture、旧报告或用户资料库。

每场景 **3 次连接冷样本 + 20 次 warm 样本**；连接冷指重新建立 SQLite 连接及 statement/page caches，操作系统文件缓存没有清空。生产 Rust 调用单独计时，排除库构造、重开、断言与 JSON 写入；160.98 秒是整个测试运行时间，不能当作单次查询延迟。

下表由原始 JSON 配对生成，显示 warm p95，单位 ms；判定使用未四舍五入值。原始报告保存全部 cold/warm 样本、median、p95、max 与 errors；对全部 874 个样本独立重算统计并核对 38 组预算，结果一致、errors 全为 0。

| 场景 | 50k warm p95（ms） | 75k warm p95（ms） | 建议预算（ms） |
| --- | ---: | ---: | ---: |
| page_first | 27.8 | 49.4 | 500 |
| page_offset_4800 | 45.6 | 44.3 | 500 |
| page_offset_24000 | 26.8 | 32.1 | 500 |
| page_offset_48000 | 26.2 | 37.1 | 500 |
| english | 269.2 | 422.1 | 500 |
| cjk_one | 54.7 | 82.9 | 1000 |
| cjk_two | 66.9 | 84.7 | 1000 |
| cjk_trigram | 221.1 | 307.6 | 500 |
| mixed_terms | 229.8 | 357.9 | 500 |
| quoted_terms | 240.9 | 413.5 | 500 |
| dna_only | 57.4 | 102.1 | 500 |
| dna_only_cjk | 65.4 | 94.3 | 500 |
| reference_only | 57.3 | 90.0 | 500 |
| reference_only_cjk | 53.1 | 86.3 | 500 |
| zero_results | 0.1 | 0.3 | 500 |
| combined_parameters | 46.0 | 56.6 | 500 |
| exact_seed | 45.0 | 75.7 | 500 |
| ranked_512_unfiltered | 528.9 | 1210.1 | 3000 |
| ranked_512_combined | 279.4 | 437.9 | 3000 |

## 来源与保留边界

实际构建回执为 `PASS_RELEASE_LIB_TEST_BUILD_ONLY`，构建阶段未执行测试；规模测试另有真实子进程终态、原始 stdout/stderr 与完整报告。运行前后 HEAD、Git 状态、68 项编译/include 输入、运行脚本、构建回执和实际测试归档身份均一致。归档为 Rust lib test EXE，不能作为已验收应用 EXE。

| 证据 | SHA256 |
| --- | --- |
| optimized 测试归档 | `aea1b80bb3c2987781e5a3b4ebc4e6474b864362127dca6322e2086eda0e71cd` |
| build-only 终态 | `7081baf6f5b9a0f002375e3eaef2607e96e4dd10ef3f17d3850be9a701f4d5fa` |
| 原始规模报告 | `de0a38abe4292cf639a27097d3a0b95d2c678d260ee1fa3b2cc0da07e102e7f0` |
| 规模测试终态 | `4e9fa4a49c7f89d2394b2008979ff6310afda4e739009485d3994f4df1180721` |

此处的 512 维检索使用合成查询向量，未执行真实 encoder、模型下载、IPC、WebView、图片解码、完整桌面启动、长期内存或原生资源性能验收，也不外推操作系统缓存冷启动体验。本轮关闭的是当前源码上述 Rust 查询/排序场景的规模与建议预算验证；真实备份/恢复及资源性能、GUI 与安装发布仍按各自要求继续。

前序故障候选 `82c187f` 的 [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39716471483)实际 **235/0/3**（1123.68 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39716474855)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39716364009)成功。本轮 `1da1371` 同 SHA 候选的 [Native](https://github.com/AureliusWu/ImageLore/actions/runs/39717614898)实际 **236 passed / 0 failed / 3 原有 ignored**（993.96 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39717618259)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39717537934)成功。随后 main 正常快进到 `1da1371` 并完成推送，推送前门禁 1346 项与 Gitleaks 通过；此处远端结果指上述同 SHA 候选运行，不预写 main 推送后的重复 Native 结果。
