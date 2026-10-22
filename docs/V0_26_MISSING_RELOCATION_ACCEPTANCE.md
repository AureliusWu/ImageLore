# 缺图、重定位与重开保全验收

日期：2026-10-22（Asia/Shanghai）。应用版本保持 `0.26.0`、schema 为 `11`。本记录针对 v1.0.0 方案 4.1-6：原图路径缺失时保留元数据和谱系，正式重定位及缓存/搜索重建后仍可回读手工数据。它接续[恢复故障补充验收](V0_26_BACKUP_FAULT_COMPLETION.md)，没有启动 GUI 或修改正常用户资料库。

## 实际结果与源码身份

`commands::tests::missing_relocation_rebuild_and_reopen_preserve_manual_metadata_and_lineage` 的首次定向 `conflicts-missing-20261022-01` 实际 **1 passed / 0 failed / 0 ignored**（0.15 秒），退出码 0。运行时基线为 `403024923b28d3fb4524d359a2f3962abbdff8a1`，加上 `commands.rs` 的委托 helper 与新增测试；67 项编译/include 输入前后相同。实际新编译测试 EXE 的 SHA256 为 `8cdfd128176ea6f521472be5e581632c486c1654f741a2892cf854a2f14eaf22`，不与其他候选产物混用。

随后工作树正常快进到 `82c187fdcf8385f21f42723690e14a9aece080c0`，纳入上一批备份故障测试，保留同一 `commands.rs` 字节。该文件 SHA256 为 `30e86582fe5131e808e8a531b0b6704b4af61316802f819c23c7912e374e75d2`；下述整合测试运行时，该文件改动尚未提交。之后源码已提交为 `1da13713040485a2919bfb26258425624b0fafaa`，main 正常快进并完成推送。现有 main 的 68 项编译/include 输入与下述整合完整回归回执逐项 SHA256 相同，`commands.rs` 保持上述身份；提交不改变既有实际运行的产物与基线归属。

整合后的严格 Clippy `clippy-missing-integrated-20261022-01`（`--lib --tests -- -D warnings`）与 locked/offline check `check-missing-integrated-20261022-01` 均实际退出码 0，68 项输入前后相同。整合完整 library suite `full-missing-integrated-20261022-01` 实际 **236 passed / 0 failed / 3 原有 ignored**（248.98 秒），退出码 0，68 项输入及 HEAD 前后相同；原始 stdout 单独列出本例为 ok。该次实际新编译测试 EXE，SHA256 为 `0ca9ee92ea6639da6eb3fa6deff01df856b0448259610709f9755a840e844502`，独立归档实际 SHA 与之相同。首次 67 项定向的 `8cdf...` 产物与本次 68 项整合的 `0ca9...` 产物分别绑定，没有转记运行结果。

原始日志、终态回执、逐项源码绑定和测试产物保存在仓库外；公开记录不包含本地绝对路径、邮箱或密钥。本轮未构建或推广应用 EXE、安装包，也未改桌面快捷方式。

## 生产行为与实际闭环

`refresh_missing` 与 `relocate_missing` 只各增加一层薄委托：Tauri `State` 取出同一 `AppState`，重定位的原 String 以 `Path::new` 借用给 helper。原生产函数体保留，参数语义及返回值不变，未增加 IPC command 或修改注册。测试调用这两个生产状态函数，图片导入、指纹查找、数据库更新与重建都走现有实现；没有伪造缺图或重定位结果，也没有执行 Tauri IPC。

1. **真实导入。** 在新建、带标记的隔离目录生成两张颜色不同的 3×3 PNG，经 `importer::run_import` 实际导入，断言 `(added, skipped, duplicates, failed) = (2,0,0,0)`。父、子指纹不同，子图 fingerprint 与 portable ID 非空。
2. **固定手工数据。** 写入 Prompt/negative/model、Revision、Tag、手工 DNA、Reference、收藏、Generation Index、Collection 与父子 `derived_from` 谱系。保存十二表有序 typed Value：assets 中除允许变化的 path/name/missing/updated_at 外的列，以及 prompt_state、prompt_revisions、tags、asset_tags、visual_dna、reference_sources、relations、collections、collection_assets、generation_index、app_meta；每个关键阶段持续比较。
3. **实际预览。** 在原路径生成缓存并读取其 3×3 尺寸，生成前后 DB/WAL/SHM 字节相同，两张原 PNG 字节不变。
4. **真正移走。** 文件系统 rename 把子图移到另一目录并改名，旧路径实际缺失。生产 `refresh_missing_for_state` 返回 1，完整子记录只把 missing 改为 1；父记录完整 JSON 保持。missing view 总数/ID、双向谱系父/子 ID 与名称都断言。
5. **缓存与搜索重建。** `prune_cache(..., 0)` 实际删除旧预览，前后库三角色和原图字节保持。清空真实 FTS/CJK 表，先确认 Prompt 搜索为空，再经 `db::reindex_asset` 重建；九组英文/CJK Prompt、negative/model/tag、DNA 与 Reference 查询恢复正确子图，十二表快照保持。
6. **关闭重开后重定位。** drop AppState 与 SQLite 连接，再经 `db::init_db` 打开同一文件库。先回读 missing/手工数据/谱系，再执行实际 fingerprint 重定位，返回 1；只有允许的 path/name/missing/updated_at 变化，原 portable ID 与元数据关系保持。旧文件名不再命中，新文件名命中原资产 ID。
7. **新预览、再次重开及幂等。** 在新路径重建预览，核对尺寸与库/图片字节保全。再次 drop/reopen 后，refresh 与 relocate 均返回 0，重定位后的完整资产 JSON 不变，十二表快照、父记录、谱系和搜索保持。

四个允许变化的 assets 字段另用完整 AssetRecord JSON 对照预期，没有从验证中省略；只有重定位后的 updated_at 采用该次真实值，最后幂等调用仍要求完整 JSON 相同。谱系既比较表行，也用现有关系查询检查父/子 ID 及随重定位变化的名称。

## 前序结果与范围

`4030249` 的 [main Native 复跑](https://github.com/AureliusWu/ImageLore/actions/runs/39714529622)实际 **230/0/3**（894.91 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39714529608)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39714529637)成功；它不覆盖新增的 commands 测试。故障补充候选 `82c187f` 的 [Native](https://github.com/AureliusWu/ImageLore/actions/runs/39716471483)实际 **235/0/3**（1123.68 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39716474855)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39716364009)成功。

包含本次缺图测试的 `1da1371` 同 SHA 候选 [Native](https://github.com/AureliusWu/ImageLore/actions/runs/39717614898)实际 **236 passed / 0 failed / 3 原有 ignored**（993.96 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39717618259)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39717537934)成功，Native 原始日志 SHA256 为 `911af59165d2e801588f74bcbdd22b769c89f6fd51e063e1ea81a6da1a1172d4`。随后 main 正常快进到该提交并完成推送，推送前门禁 1346 项与 Gitleaks 通过；main 推送后的重复 Native 结果未在此预写。

两路 worker 候选 `0a87dd8` 的 [Native](https://github.com/AureliusWu/ImageLore/actions/runs/39714869314)实际 **233/0/3**（952.23 秒），[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39714873165)与[敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39714599823)成功。该候选仍未并入 main，六操作 debug 诊断仍是单样本，不能视为正式性能通过；其[固定提交记录](https://github.com/AureliusWu/ImageLore/blob/0a87dd8ddfce3022df4122576f616b5bbbbaf503/docs/V0_26_BACKUP_GUARDED_WORKER_DIAGNOSTIC.md)单独披露范围。

本测试为 4.1-6 的文件库后端闭环提供明确证据，不把十二表快照扩大为所有应用数据的无条件保证。两次重开是在同一测试进程中关闭连接后重新初始化 SQLite，没有启动全新 ImageLore 进程。当前源码的 50k/75k Rust 查询/排序规模结果已另记于[本轮原生规模查询](V0_26_CURRENT_NATIVE_SCALE.md)；实际缺图显示、文件选择器/重定位窗口流程、桌面关闭重启、真实编码与 IPC/GUI 性能、长期内存、安装升级、签名、updater 与 RC 继续独立验收。既有 A1/A2 用例保留原源码和产物身份，不因此宣称 v1.0.0 全部退出条件完成。
