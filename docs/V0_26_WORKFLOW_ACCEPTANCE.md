# v0.26 A6 / A7 工作流与数据一致性回归

验证日期：2026-09-30（Asia/Shanghai）。开发基线为 `main / 1a701ea / 0.25.1`；当前更新目标为 `0.26.0`。本记录描述工作区修复与回归，全项目发布门禁由阶段总验收记录汇总。

## 已修复的行为

- 编辑器初始化曾让草稿与保存基线引用同一对象，修改字段也会修改基线，导致自动保存和关闭前 flush 无法识别脏数据。现在基线保持独立副本；迟到的旧资产保存结果也不能改写新资产基线。
- flush / Revision 保存失败现在向调用方传播错误，保留草稿供重试。关闭窗口先等待保存及任务取消；导入/语义任务取消失败也会传播到关闭保护，失败保持窗口并显示错误，重复关闭请求继续受到保护。独立取消按钮消化已经显示的失败，取消失败不会继续清空索引或删除模型。
- 图库刷新、选图和元数据重读采用编辑序号与真实脏状态保护：等待请求期间的新输入不会被返回的旧记录覆盖。历史恢复和元数据重读若迟到，重设服务端保存基线后再次保存新输入，防止界面保留新输入但数据库被迟到写入覆盖。历史恢复检查 Revision 资产归属，先保存草稿并处理失败；Sidecar 与手动备份也先等待编辑保存。
- Vision 分析、DNA 写入与后续记录刷新都校验发起时的资产上下文。切图及切走再返回都使旧请求失效；属于其他资产的分析不能载入 Prompt、应用 DNA 或保存 Revision。
- 导入不再把缺失目录或目录枚举错误隐藏为成功。损坏 JSON、无法读取或不受支持 schema 的既有 Sidecar 会计入失败，保留图片及 Sidecar 原件，不合并不完整上下文。
- 来源同步只有完整批次成功且未取消时才更新 `last_scan_at`。当前导入摘要没有逐来源成功标记，因此任何失败会保留整批上次成功时间；扫描时间戳以事务提交，提交失败也计入结束事件的失败数。
- Sidecar 导出先在同目录创建独立暂存文件，写入并 sync 后再替换目标文件。写入或替换失败保持旧 Sidecar，并清理本次暂存文件。

## 已通过的回归

| 验证入口 | 结果 | 覆盖 |
| --- | --- | --- |
| `node --test scripts/test_stability_workflows.mjs` | 14 / 14 | 运行正式 Hook 的受控 React/API harness；Vision 迟到及来回切图、错误资产分析、编辑保存失败重试、Revision 失败重试、旧图保存迟到、保存期间继续编辑、延迟读取期间的新输入与切图保护、迟到服务端写入后的基线重设与新输入再次持久化、关闭失败及重复关闭；正式 Import/Semantic Hook 取消失败传播和重试，以及真实取消 Hook 与 CloseGuard 的失败衔接 |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib importer::tests` | 4 / 4 | 保留原有枚举/预取消回归；新增真实导入的跨库工作流和来源取消/失败矩阵 |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib sidecar::tests` | 5 / 5 | 原有解析；暂存写入故障、完整替换、Windows 无共享文件锁导致替换失败且旧内容保留 |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib sources::tests` | 2 / 2 | 原有行映射；取消/失败批次保持全部旧时间；第二来源 UPDATE 故障使第一来源 UPDATE 一并回滚 |
| `npm run lint` | 通过 | TypeScript strict |
| 所改 Rust 文件 rustfmt；所改前端及新增测试 Biome；`git diff --check` | 通过 | 未删除既有测试、未新增测试框架或 Native command，未放宽 `--locked` / Clippy 要求 |

Rust 构建输出有标准 Windows linker stdout warning；测试执行没有失败。

## 真实文件数据库工作流

全部数据位于独立系统临时目录；不调用用户资料库定位入口，不读取或更改用户正常数据库。

跨库测试用正式图片编码器生成三张不同 PNG，经过正式 importer、SHA-256 去重、Reference merge、Remix draft/save/apply_lineage 和导出入口：

1. 导入基础图与光线参考图，保留 Prompt、标签、Session、DNA 与首个 Reference 来源。
2. 将基础图复制为另一文件，带第二个网页来源连续导入三次：不新增资产，两种来源保留且不重复。
3. 导入结果图并应用两图 Remix 两次：`derived_from` 与 `reference` 关系各一条，备注保留实际使用的主体/风格、光线/环境字段；已有结果 Prompt 保持；Session 继承。
4. 导出三张 Sidecar，在第二个文件数据库中先导入结果，再导入两张父图，验证 pending 关系最终完整解析。
5. 三次重复导入后关闭并重开数据库连接，再导出，逐字段比较 portable ID、fingerprint、Prompt、标签、DNA 内容、多来源 Reference、Session 与谱系。

比较仅排除本应变化的图片路径及 DNA 来源/更新时间；三张图片在另一目录内沿用文件名和内容。Sidecar 保留结果谱系，不承诺搬运未应用的 Remix 草稿。

来源故障测试包括预取消、中途取消、缺失根目录、SQLite trigger 注入的上下文合并失败、坏 JSON 与未来 Sidecar schema。失败事务不会留下 Session / DNA / Reference；已完成的导入记录保留，成功扫描时间不被伪造，关闭并重开数据库连接可回读；随后成功扫描可以更新成功时间。

## 验收边界

上述证明是正式 Rust 路径的临时文件数据库回归，以及运行正式前端 Hook 的受控 Node 回归。数据库连接重开不等同于完整应用或机器重启；写入故障注入不等同于真实磁盘耗尽/断电；三次重复同步不等同于长时间持续运行。

实际 Windows WebView2 操作、真实窗口关闭/进程重启、10 分钟连续使用、所有派生索引清空重建前后命中等价，以及全项目六项门禁仍需在阶段总验收中分别记录。本报告本身不把 A6/A7 全部验收或 v1.0 正式发布标记为完成。
