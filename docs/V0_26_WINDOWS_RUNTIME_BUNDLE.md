# Windows 原生运行库的打包修复

记录日期：2026-10-25。应用版本保持 `0.26.0`；本记录区分现有候选字节、隔离打包原型与仍待执行的真实安装验收。

## 实际依赖与遗漏

现有应用候选来自 `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`，EXE SHA-256 为 `f32d25ace7ebd358b486f3907a20cc0246eda2480dac5e88c47ce5ebd589bdad`。只读 PE 导入表读取及已安装 Microsoft dumpbin 的独立比较一致，应用直接导入 `DirectML.dll`、`MSVCP140.dll` 和 `MSVCP140_1.dll`。这只是导入依赖证据，不把 Cargo 依赖图当作完整静态链接图。

对该归档 EXE 与已核对的 DirectML 副本运行真实 Tauri 2.12 NSIS 打包，原配置的生成脚本没有 DLL File 指令；仅开启 VC 选项时新增十个 VC DLL，仍没有 DirectML。因此开发机上能启动，不能证明原安装包覆盖新机器的运行库要求。

## 修复路径

- Windows 目标显式开启现有 ort 的 `copy-dylibs` 特性，使 provider DLL 随原生构建进入输出目录；FastEmbed 关闭的默认特性没有提供这一步。Cargo.lock 的 599 个包及版本均未变化，只增加项目到已有 ort 的依赖关系。
- 启用 Tauri 内置 `bundleVCRuntime`，由它从构建机 Visual Studio redistributable 目录收集 DLL，并安装到应用目录。对应[固定 CLI 实现](https://github.com/tauri-apps/tauri/blob/447fa9f3f993fe77724189e355078b38ce20baea/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs)与[官方选项](https://v2.tauri.app/reference/config/#bundlevcruntime)已核对。
- `npm run tauri:build` 先完成无安装器的原生构建，再由 `bundle_windows_installer.ps1` 读取 Cargo 实际输出目录，支持独立 `CARGO_TARGET_DIR`。它核对 ort-sys 锁定版本及 `windows-runtime.json` 中的 DirectML 大小/完整哈希，复制到独立构建输入目录并只读持有，用临时资源映射交给 Tauri standalone bundle。
- 升级验收要求三个已导入 DLL 都位于安装目录；启动后检查实际加载模块路径，不能由 runner 的全局 DLL 满足。DirectML 的已安装字节还须匹配同一清单，再执行正常关闭和原有业务读回。

VC++ 与 DirectML 保持各自许可。应用目录中的 VC++ DLL 随应用版本维护，不能依赖 Windows Update 自动更新这些副本；依据 [Microsoft 本地部署说明](https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp)及[分发条件](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files)准备后续正式分发材料。

## 已执行的本地打包原型

下列比较只改变打包配置/脚本，使用归档应用副本；它不是新源码完整编译或真实安装的证明。

| 变体 | NSIS 编译 | 生成脚本中的 DLL File 指令 | 安装包字节数 |
| --- | --- | ---: | ---: |
| 原配置 | 成功 | 0 | 13,461,950 |
| 仅内置 VC 选项 | 成功 | 10 | 13,939,118 |
| VC 选项 + 哈希绑定的 DirectML 资源 | 成功 | 11 | 22,753,287 |

第三项包含两个必须的 MSVCP DLL 与 DirectML，全部资源输入的 SHA-256 已核对；DirectML 为 **18,527,776 字节**，完整 SHA-256 为 `9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1`。缺少和篡改 DLL 的两个负例均在调用 bundler 前拒绝，没有生成安装包。首次脚本的 Windows PowerShell 哈希命令与多 Node 路径兼容失败分别保留，后续用 .NET 哈希和明确选择命令修复，未覆盖失败现场。

原始应用/DLL 字节、原源码工作区均保持不变。所有安装包、编译脚本、原始日志与机器路径保存在仓库外；本机没有安装或启动这些原型，没有推广桌面 current 或改动用户资料库。

## DLL 签名证据与后续验收

PowerShell 对 DirectML 返回 `Valid / Catalog`；[官方说明](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature)明确有目录签名时优先返回它。另以已安装 Windows SDK SignTool 执行 `verify /pa /all /v /tw`，不使用目录搜索 `/a`，对同一文件的嵌入签名验证实际退出 **0**，**1 个签名、0 warning、0 error**；签名者为 Microsoft Corporation，时间戳验证为 2024-10-26。完整文件哈希与 Authenticode 摘要属于不同范围，不混用。

这项证据补充了 DirectML 的 Microsoft 身份，不代表 ImageLore 已签名，也不代表验证了整个 NuGet 包。现有 ImageLore EXE 的证书表为空且 `NotSigned`，Windows Authenticode、服务商资格、updater 与正式 Release 条件继续独立处理。

下一步使用新源码的现有 Windows 工作流做原生检查、应用构建、0.19.0 升级及安装目录真实加载验证。手动工作流默认验证模式，跳过安装包上传，只有显式 `upload_installer` 才启用分发复核后的上传；手动运行不会创建 Release。该远端验收的实际结果另行登记，不以本地打包成功代替。阶段 A 的桌面连续操作、正式性能及长期内存门槛仍未关闭。
