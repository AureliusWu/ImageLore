# 新增资产 FTS 索引的配对测量

日期：2026-10-03（Asia/Shanghai）。这是阶段 A 的合成 SQL/事务测量；最终六对实验与数据保全检查为 PASS。50k 背景下，500 条新增资产的完整索引 helper 总耗时中位数由 replacement 的 10630.9915 ms 降至 append 的 66.1083 ms。该结果不代表完整文件导入、桌面体验或阶段 A 已验收完成。

## 实现边界与实际查询计划

导入路径仅在同一事务中确切 `INSERT` 新 canonical asset 后调用 `index_created_asset`。已有资产刷新、重复来源合并、编辑与索引修复继续使用 `reindex_asset` 的 replacement 路径；append 不能用于派生索引孤立项或重复项修复。两条路径共享完整的 English/CJK 索引写入内核。

English FTS 的 `asset_id` 为 `UNINDEXED`。项目锁定的 rusqlite bundled SQLite 实际执行计划表明，`DELETE FROM asset_search WHERE asset_id=?1` 使用 `SCAN asset_search VIRTUAL TABLE INDEX 0:`，即使该新 ID 尚不存在也要扫描 FTS 表。append 省去这次 English 删除；其余 helper 工作仍执行。CJK 删除使用 `rowid=?1`，实际计划为 `SCAN asset_cjk_search VIRTUAL TABLE INDEX 0:=`，保留 rowid 约束。

English FTS 的历史 rowid 独立于 canonical asset ID；本次没有改成用 canonical ID 强行覆盖 English rowid。历史 rowid 碰撞保全、已有资产陈旧重复项修复和重复 reindex 的幂等性由既有 Native 回归 `created_asset_append_preserves_legacy_rowids_and_existing_replacement` 覆盖，与下面的性能配对样本分别记账。

## 来源与测量口径

生产来源为 `ec8874f9421deb184e17e833347d2a70bf5b0238` / 应用 `0.26.0`。以下四份捕获生产源的 raw SHA256 与该 HEAD 的 Git blob 字节 SHA256 完全相等；测量任务没有修改生产源。

| 生产源 | raw SHA256 = HEAD Git blob 字节 SHA256 |
| --- | --- |
| `src-tauri/src/db.rs` | `4780c85d349b2aefa2c34cacb427d3b638089088406f6e9e810333fae1c1c9c2` |
| `src-tauri/src/importer.rs` | `bb03c6b229a493cae8c976c0e9e37af796ea37eede9048f5b9d97d0820a352ef` |
| `src-tauri/src/migrations.rs` | `c060afe1520a8bcb1f4203d3e0d77e86484d0fade1f435d37ab54be1d866e3ff` |
| `src-tauri/src/backup.rs` | `daddb47e3557dafd5b8eb7d3045f549f60464f4dc046260706164e8177afbc2f` |

隔离衍生源码仅在外部 `db.rs` 加入 `cfg(test)` include，并加入独立 `owned_fts_profile.rs`。实际编译身份为 release 优化测试二进制：`opt_level=3`、`test=true`、`debug_assertions=false`，EXE SHA256 为 `0b6d4a7c6465bb3d16fc3a481c3b25091fc09c06179ef7a36835e42484b096b9`。它是实验测试 EXE，不能当作发布运行时 EXE 或桌面候选验收。profiler SHA256 为 `b9b2918d87916a3b45efb6b18ca6adf355ed6c972bf3a0719339af336723c40a`。

A 为 append，B 为 replacement。5k、50k 两种合成背景各完成三对，执行顺序固定为 A/B、B/A、A/B；每个成员在独立副本中新增 500 条资产，共六对、12 个成员、6000 条唯一样本。配对双方使用相同背景和合成输入；SQLite 实现来自项目锁定的 rusqlite bundled 库，没有用系统或 Python SQLite 代替。每个成员开启新的 SQLite 连接，但准备、复制、哈希与完整校验已使 OS 缓存变暖，属于 warm OS-cache 测量。

`index 总耗时` 是成员内 500 次完整索引 helper 的计时之和，包含资产读取、标签/DNA/Reference 文本取得及 English/CJK 索引操作，不能把全部差值归因于一条单独计时的 DELETE。`batch` 为合成 canonical/context/index/commit 批次墙钟，包含 JSON/Instant 测试采样开销。背景生成、复制、校验、初始化及 EXPLAIN 分开记录，不混入 batch。降低比例按每对 `(replacement - append) / replacement` 计算；汇总列取这三个配对比例的中位数。

## 全部六组结果

下表保留所有完成的配对，耗时单位为 ms；没有挑选最快成员或旧实验前缀。对号从 1 开始展示，对应私有摘要的 0–2。

| 背景资产数 | 对 | 顺序 | append index 总耗时 | replacement index 总耗时 | index 降低比例 | append batch | replacement batch | batch 降低比例 |
| ---: | ---: | :---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 5000 | 1 | A/B | 64.7276 | 624.8205 | 89.6406% | 659.3911 | 1217.6889 | 45.8490% |
| 5000 | 2 | B/A | 64.1769 | 637.6326 | 89.9351% | 666.9817 | 1259.2169 | 47.0320% |
| 5000 | 3 | A/B | 63.7765 | 647.3132 | 90.1475% | 654.9631 | 1255.1632 | 47.8185% |
| 50000 | 1 | A/B | 65.7975 | 10681.0648 | 99.3840% | 959.3671 | 11663.3459 | 91.7745% |
| 50000 | 2 | B/A | 66.1808 | 10571.7747 | 99.3740% | 962.9992 | 11560.9879 | 91.6703% |
| 50000 | 3 | A/B | 66.1083 | 10630.9915 | 99.3782% | 967.8336 | 11641.1201 | 91.6861% |

| 背景资产数 | append / replacement index 总耗时中位数 ms | 配对 index 降低比例中位数 | append / replacement batch 中位数 ms | 配对 batch 降低比例中位数 | append / replacement 最慢 batch ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 5000 | 64.1769 / 637.6326 | 89.9351% | 659.3911 / 1255.1632 | 47.0320% | 666.9817 / 1259.2169 |
| 50000 | 66.1083 / 10630.9915 | 99.3782% | 962.9992 / 11641.1201 | 91.6861% | 967.8336 / 11663.3459 |

三对用于方向性比较，不能证明统计显著，也不用于推断生产 p95。本实验没有另设产品速度通过预算；表中的 PASS 指完整实验与数据验证通过，不改变计划中既有性能阈值或 WARN。

## 数据保全与真实失败回滚

全部六对的 focused 与全业务 typed 状态比较通过；SQLite integrity、foreign-key、English/CJK FTS integrity、索引基数与实际查询检查通过。独立 JSON 复核取得 28 份完整 integrity 记录、56 项双 FTS 基数检查。每张 FTS 表均验证无孤立行、行总数等于 canonical 总数、匹配的 distinct canonical 主键数等于 canonical 总数，保证每个 canonical asset 恰有一行索引。

跨成员逻辑比较仅排除各次新生成的 `created_at`/`updated_at`，新 context 时间值另按执行区间核验；旧业务时间字段精确保留。FTS5 shadow 表字节不进入跨成员逻辑摘要，但两张 FTS 的实际 `integrity-check` 均执行。原始背景 DB 的 raw SHA、大小、mtime 保持不变，成员内旧数据与旧搜索结果也已检查，不能把 typed 等价写成全部数据库文件逐字节相同。

回滚用例在同一未提交事务中插入新 canonical/context 后，实际删除合成 CJK 表，使 append 在完成 English 插入后收到真实 SQLite `no such table` 错误。事务 drop 后，完整业务 typed 状态（含全部时间字段）、sequence、CJK 表及旧查询恢复；SQLite/FK/双 FTS 完整性再次通过。这是实际 SQLite 写入失败后的事务保全证据。

## 失败记录与证据保存

旧准备器的完整基数校验过慢，实验主动中止并保留为 INCOMPLETE；旧 5k 三对与未完成的 50k 前缀、原草稿、首份摘要和原始日志全部保留。停止后的读回含一条 50k `beforeMeasurement` 事件，所以不能声称 50k 测量完全未开始。其样本均未纳入本报告，最终结果来自全新、完整的六对。

准备器随后以等价的完整不变量校验替换慢的逐资产分组校验，保留所有 SQLite/FK/FTS、全业务 typed、旧查询与样本验证。首轮 opcode 分析曾因连接地址差异而断言失败，也完整保留。最终语义 opcode 比较只允许 `VOpen`/`VUpdate`/`VBegin` 的 P4 精确 `vtab:<hex>` 连接指针不同；其他 opcode 结构与同背景查询计划一致，原始 opcode 地址和行均未改写。

原始日志、JSONL、fixture、源身份和冻结清单仅保存在仓库外 `<private-evidence>`。公开报告依据 `FTS_PUBLIC_SANITIZED_CONCLUSION.md`、`FTS_FINAL_PROVENANCE_20261003.md`、`paired-summary-final.json` 和 `fts-final-freeze.json`；最终摘要 SHA256 为 `caa8f81427cd1c08cb2a54214cab6f680d1569354eafbc3fabbc0b1ab37ad7fc`，冻结时间为 `2026-10-03T05:50:09.889879Z`。原摘要与原始证据均保留且没有覆盖，不上传这些私有文件。

## 阶段 A 仍待关闭的范围

本实验不覆盖完整文件导入的读取/解析/哈希、Session/父关系、IPC、GUI、图片解码、模型、安装升级或用户资料库。它不是三次可比冷测、长期内存观测、自然召回验收或 v1.0 完成证明。当前桌面产物已完成下述独立 300 项 Native 模型工作流，更多当前源码工作流、三次可比冷测和长期内存仍须按[阶段 A 方案](V1_0_0_PLAN.md)验收；既有启动、内存及恢复 WARN 保留。

`ec8874f` / EXE SHA256 `10381121a5ea89c71d629f93c6f5e2879ca829612706e50c5ba82e16bd50e01c` 的桌面产物已独立完成 300 项真实 512 维模型编码、预期坏/缺失两项修复、text/similar 各 20 warm 检索及备份/正常关闭/fresh-profile 重启，并于 `2026-10-03T15:41:05.5079495Z`（北京时间 10 月 3 日 23:41:05）推广到稳定目标，见[当前运行时验收](V0_26_CURRENT_RUNTIME_ACCEPTANCE.md)。该桌面 EXE 与本实验测试 EXE 分别记录，300 项结果不扩大为当前 50k、长期内存或物理 UI 验收。历史 `2276de1` 的模型、桌面和安装器证据保持其原始 source/EXE 身份。备份错误分类的联合源码回归另见[备份不可用错误与损坏错误的分类](V0_26_BACKUP_CLASSIFICATION.md)，不重复计入本实验样本。
