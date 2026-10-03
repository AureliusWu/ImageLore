# 桌面运行目录的来源与发布回归

日期：2026-10-03（Asia/Shanghai）。本批修复来源记录并验证隔离发布；没有替换真实桌面程序或发布安装包。

## 来源绑定

以前发布脚本总把当前 Git HEAD 写入 `builtFrom`。构建后发生文档提交时，它可能把旧二进制错误地标记为新源码产物。

候选目录含 `runtime.json` 时，发布脚本要求完整 40 位 `builtFrom`、64 位 EXE SHA256、与项目 VERSION 一致的版本及 `ImageLore.exe` 文件名。实际 EXE 版本与 SHA256 都须匹配，复制到 staging 后再次计算 SHA256；记录保留候选的实际声明来源，不用当前 HEAD 替代。

明确传入 `SourceDirectory` 必须有该清单。默认构建目录为旧 `desktop:build` 保留兼容路径，但没有清单时只允许 tracked-clean 工作区，并标记 `builtFromSource=legacy-clean-head`。这是构建命令立即发布时的干净 HEAD 假设，不能独立证明历史二进制确实来自该源码。

来源清单与字节匹配也不构成密码学来源认证，没有验证完整 DLL 集合或供应方签名。可信安装器、Authenticode 与 updater 仍是阶段 B 的独立门槛。

## 实际隔离回归

`scripts/test_publish_desktop_runtime.ps1` 已接入 `npm run check`，也可独立运行 `npm run test:runtime`。测试在新建、持有所有权标记的临时项目中运行；合成 EXE 仅供版本与文件操作检查，从未执行。

七组场景实际通过：

1. 文档 HEAD 超前时保留候选 commit 与 EXE hash。
2. 更新后旧 EXE/清单准确保留，测试专用版本快捷方式正确更新。
3. 错误或缺失的来源字段、hash、版本及文件名在 staging 前拒绝。
4. staging 复制漂移拒绝提升，当前运行目录与候选源保持。
5. 不属于应用的快捷方式冲突触发回滚，原目录/状态/链接保持，失败候选保留。
6. 实际独占文件句柄阻止当前 EXE 替换，原件保持。
7. legacy 默认目录拒绝 tracked changes，干净 HEAD 兼容路径通过。

共观察 13 次预期拒绝。713 次断言包含所有权、路径与清理检查，不是 713 个独立测试。Windows PowerShell 5 的最终执行继承原环境，测试显式加载宿主 Utility 模块，不修改父进程模块路径。较早的两份失败属于隐藏目录清理和宿主模块加载问题，原始结果保留，没有计入通过记录。

真实桌面目录、当前 EXE、运行清单、默认快捷方式状态和既有链接的 SHA、mtime 与名称前后一致。测试清理前验证所有路径和所有权 token，拒绝 reparse point，成功后仅删除自有临时目录。原始 stdout/stderr、结果 JSON 与源码 SHA 保存在仓库外 `<private-evidence>`；合成回归不替代新 EXE 启动、实际升级或 50k 原生验收。
