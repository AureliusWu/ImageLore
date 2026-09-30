# v0.26.0 原生规模与派生索引验收记录

日期：2026-09-30（Asia/Shanghai）。本记录是阶段 A5 的 Rust 原生 SQL/向量扫描证据，GUI、图片解码和真实模型另行记录。

## 实现与回归

- 语义检索与相似图参考向量必须匹配当前资产 fingerprint，并排除 missing 资产。先加入两个回归，旧实现分别返回旧/缺失资产 `[3,2,1]` 和旧参考向量；修复后均通过。候选摘要与 embedding 在同一 AppState 数据库锁内读取，减少并发资产更新造成的跨状态结果。
- 清空并使用生产 `db::reindex_asset` 重建 FTS/CJK 后，Prompt、DNA-only、Reference-only、多语言命中保持一致，宽高未知值仍为 `None`。
- 预览缓存生成后节流执行后台清理（60 秒，每库同一时刻仅一个清理任务），沿用 1 GiB 软预算。仅处理完成的 `.webp`，保留正在写入的 `.tmp` 和刚返回的预览；新增两个边界回归已通过。此策略由持续生成触发，非定时扫描所有磁盘。

## 原生性能协议

入口仍是 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`，规模用例 `native_scale_tests::file_backed_native_scale` 标为 ignored，需显式执行。

- 本机：AMD Ryzen 5 3600 / 12 逻辑线程；约 16 GiB RAM；Windows 11 `10.0.26200`；`rustc 1.97.0 (2d8144b78 2026-07-07)`。
- 构建：两轮均为标准 `debug-unoptimized`。基线为 SQL 优化之前、语义 freshness 修复之后；复测版本 `0.26.0`。
- file-backed SQLite / WAL，schema 11；50,000 与 75,000 行分别约 258.4 / 387.9 MiB；固定 seed `0x496d6167654c6f72`，重复名称/内容指纹、缺失记录、长 Prompt、多语言、DNA-only 与 Reference-only 数据。
- 两轮复用同一数据库路径和数据。19 项/规模：4 个分页深度、英文、CJK 1/2/3+、混合、引号、DNA-only/Reference-only（英文/CJK）、零结果、组合参数、exact seed、两种 512 维 ranked 扫描。所有 COUNT、返回 ID 顺序、摘要字段、NULL 语义与语义排名都有独立已知答案。
- 每项 3 个连接冷样本与 20 个暖样本，暖预热一次不计入。冷仅表示重新打开 SQLite 连接，未强制清空 Windows 文件缓存；计时包括实际生产 Mutex/COUNT/摘要查询/排序或向量扫描，不包括 fixture 构建、连接初始化、断言与 JSON 写入。
- 512 维向量为固定 seed 的稠密合成向量；使用真实生产 `semantic::ranked` 扫描和排序。它不证明模型编码、下载、初始化、IPC 或 WebView 性能。75k 最高 ID 正确命中；现有 100,000 候选上限保持，不能外推为无限容量。

## 测量后的小优化

移除当前过滤条件不需要的 generation/DNA LEFT JOIN；CJK 使用生产迁移/reindex 已保证的 FTS `rowid=asset_id` 联接，避免每个命中读取 UNINDEXED `asset_id` 所需的长文本内容。LIKE 复核和全部筛选条件保持。前后 EXPLAIN 仍以 FTS 为驱动表，assets 走主键。

以下单位为 ms；预算为计划的待校准建议，表中使用 warm p95 判断：

| 场景 | 50k p95 前→后 | 75k p95 前→后 | 建议预算/结论 |
| --- | ---: | ---: | --- |
| 首屏 COUNT+摘要 | 50.0→19.3 | 65.1→29.6 | 500，PASS |
| OFFSET 48000 | 58.3→23.3 | 63.3→33.7 | 500，PASS |
| 英文多词 | 439.2→337.1 | 615.2→547.2 | 500，50k PASS / 75k WARN |
| CJK 单字 | 84.6→66.0 | 105.3→104.5 | 1000，PASS |
| CJK 双字 | 76.9→68.4 | 433.6→100.3 | 1000，PASS |
| CJK 3+ | 530.5→266.4 | 652.1→455.3 | 500，PASS |
| CJK/英文混合 | 574.0→321.8 | 744.8→490.0 | 500，PASS |
| 双引号多词 | 459.4→336.9 | 567.1→513.1 | 500，50k PASS / 75k WARN |
| 512 维无结构过滤 | 1599.0→1568.0 | 2335.8→2594.2 | 3000，PASS |
| 512 维组合过滤 | 336.0→333.5 | 433.2→528.2 | 3000，PASS，75k尾延迟波动 |

两轮各 874 个计时样本，正确性断言无错误。优化后 50k 所有场景落在建议预算内；75k 扩展仅英文与双引号略超 500 ms，不放宽预算或掩盖。75k ranked 的 median 仅约 +1.4% / +4.6%，但 p95 约 +11.1% / +21.9%，保留尾延迟 WARN，待 release 模式及真实 Native UI 测量进一步判断。

Rust 测试进程的全程 peak working set 约 241.9→240.1 MiB；原始 JSON 同时保留即时 working set/private bytes。该数字不代表模型或 WebView 进程内存，不证明长期无内存增长。

## 本地原始证据

- `<private-evidence>/semantic-before.txt` / `semantic-after.txt`：语义 freshness 先失败后修复。
- `<private-evidence>/native-scale/native-scale-1790750603.json`：优化前完整样本。
- `<private-evidence>/native-scale/native-scale-1790751595.json`：同库复测完整样本。
- `<private-evidence>/native-scale/native-scale-comparison.json`：38 项前后 cold median / warm median / p95 / max / delta / verdict。
- `<private-evidence>/native-scale/cjk-query-plans.json`：SQL 查询计划。
- `<private-evidence>/native-scale/source-hashes-before.json` / `source-hashes-after.json`、基线源码副本。
- 对应 `native-scale-1790750603-{50000,75000}.sqlite3` 为原生基准原件，保留供复测，不直接修改为 GUI 库。

复现示例（PowerShell）：

```powershell
$env:IMAGELORE_SCALE_EVIDENCE = '<独立本地证据目录>'
$env:IMAGELORE_SCALE_KEEP_DIR = '<保留大测试库的目录>'
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib native_scale_tests::file_backed_native_scale -- --ignored --nocapture
# 相同测试库复测时，显式指向上一次的 JSON：
$env:IMAGELORE_SCALE_REUSE_REPORT = '<上一轮 native-scale-<stamp>.json>'
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib native_scale_tests::file_backed_native_scale -- --ignored --nocapture
```

`IMAGELORE_SCALE_ASSET_DIR` 可让 fixture path 指向真实合成图片根，文件名规则 `<id>.png`。GUI 使用另一个隔离库；若重复 fingerprint 对应不同图片字节，应在 GUI 副本中重算 fingerprint，防止制造无效缓存碰撞。真实模型下载/编码与 GUI/滚动/保存/重启验收不可用本记录代替。

## 原生 GUI 与真实模型验收

本节由隔离 Windows 原生应用验收另行补充。上述 Rust 结果不能替代 WebView2 的滚动、快速切图、Fit/100%、保存/关闭、重启回读、模型下载/编码或长期内存测量。
