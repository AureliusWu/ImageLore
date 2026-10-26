# Windows 标签发布的签名门禁

记录日期：2026-10-26；应用版本保持 `0.26.0`。这是发布前的拒绝策略与安装载荷绑定，尚未完成签名服务接入、已签名产物验收或 v1.0.0 发布。

## 发现的发布路径

原工作流在固定源码、前端/Native 与升级检查通过后，会直接上传 `v*` 标签的安装包并创建 Release；这条路径没有强制验证内层应用和外层安装器的 Authenticode 签名。构建/升级成功本身不能证明有可信发行者。

本次源码的标签路径先要求非空、格式有效的批准证书清单，避免未配置签名时运行整个打包任务；在上传之前再对实际应用 EXE 和最终 NSIS 文件分别执行完整检查。上传和发布条件均显式包含 `success()`，前置门禁失败即停止后续步骤。

## 验证内容与身份

`src-tauri/windows-signing-policy.json` 只保存产品名与批准签名者**公开叶证书 DER 的 SHA-256**，不是证书的 SHA-1 thumbprint，也不是私钥、文件哈希或服务 token。目前清单为空，当前源码的标签发布会拒绝继续；没有为占位值、Microsoft 运行库或任意有效证书自动放行。

两个文件都须满足：

- 产品名为 `ImageLore`，ProductVersion 与预期完整版本严格一致，FileVersion 的数值部分相符；RC 标识仍须出现在 ProductVersion。
- Windows 返回 `Valid` 的嵌入 Authenticode 签名，签名叶证书摘要在批准清单中，并有时间戳证书。目录签名不能代替项目签名。
- 已安装 Windows SDK 的 Microsoft 签名 SignTool 执行 `verify /pa /all /v /tw`，退出必须为 0；warning 也拒绝。不通过 PATH 中的任意同名程序验证。
- 验证期间只读持有应用和安装器文件，记录完整文件 SHA-256，并在 SignTool 返回后复核同一持有对象。

Microsoft 文档说明 [Get-AuthenticodeSignature](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature)在同时存在嵌入/目录签名时优先返回目录签名；[SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)说明 `/pa` 使用 Authenticode 策略，`/all` 验证所有签名，`/tw` 对缺少时间戳产生 warning，返回 2 表示有 warning。上述检查不调用签名、证书导入、信任库修改或服务端提交。

批准清单需要在实际签名服务、项目身份与证书材料核对后通过源码审查更新；仅改变 JSON 不会让未签名文件通过。服务商资格、私钥与服务 token 的 GitHub 外保存方式继续独立处理，不能据此声明服务已经批准。

## 安装器内层应用的字节绑定

升级脚本新增必须传入的 `CurrentExecutable`，在任何安装/清理前锁定构建输出，并持续持有到验收结束。实际安装的 `ImageLore.exe` 必须与该输出的完整 SHA-256 相同，两次正常关闭后也要保持相同。因此不能在打包后仅改变构建目录应用签名，再把旧内层应用的 NSIS 当作已验收包。

原有托管 runner 限制保留：脚本首先要求 `GITHUB_ACTIONS=true` 与 `RUNNER_ENVIRONMENT=github-hosted`，没有本机或 self-hosted override。旧 schema、七表哨兵、两次正常关闭、运行库实际加载位置与备份/模型字节检查继续执行。

## 本地已执行与剩余范围

本地策略/组件回归实际 **20 项通过**，包括稳定版/RC 的正向策略合约、产品/版本不符、未知或缺失证书、目录/损坏/未签名状态、缺少时间戳、空/错误清单、Microsoft SDK verifier 读取，以及编译出的真实未签名 managed PE 和 CLI 拒绝。正向策略合约使用合成记录，不代表真实签名链已通过。

另以只读方式对已冻结的真实 Tauri 应用和旧源码打包原型作为输入，确认各自 `NotSigned`，门禁在第一个未签名输入处拒绝；两份原件完整哈希保持。它不是新源码安装验收或第二文件的真实签名成功路径。升级脚本的本机调用实际在检查用户目录之前拒绝；四份 PowerShell AST、项目完整性和解析后的工作流步骤顺序通过。

首次测试夹具只有 managed assembly 属性、没有 native VERSIONINFO，控制断言失败，现场与源码已保留。修正测试说明并先检查签名状态后取得上述结果，没有把夹具失败转记为签名成功。

手动分支验证仍默认不上传、不创建 Release；其显式 `upload_installer` 用于分发审查后的评估材料，不是稳定版更新渠道。本轮不启用该输入、不推标签、不切换桌面 current。新源码的托管安装/升级、真实批准签名的正向路径、签名最终产物的匿名下载、updater、GUI、正式性能及 RC 仍待分别验证。
