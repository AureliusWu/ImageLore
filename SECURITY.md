# 敏感信息与上传防护

适用范围为 ImageLore 的源码、Git 历史、构建输入、CI 日志和发布材料。此规则同时适用于私有仓库；仓库可见性不能替代敏感信息保护。

## 存储边界

真实密钥、访问令牌、密码、登录 Cookie、带鉴权参数的 URL 和私钥只保存在仓库外的 `<private-storage>`、受控环境变量或操作系统凭据库。禁止把这些私有材料上传到 GitHub，包括 GitHub Secrets。环境变量不得打印或写入诊断报告。签名使用本地密钥或外部受控签名服务；GitHub 只接收公开公钥和已签名产物。

真实资料库及其 WAL/SHM、用户图片、原始截图、HAR、内存转储、原始验收数据和扫描报告保存在仓库外。不要把这些材料放入源码树、`.git`、测试夹具、日志、Issue、PR 或 CI artifact。公开验收说明只保留脱敏结论、产物来源、校验值和必要边界；使用 `<private-storage>` 表示私有证据位置，不记录个人用户名或绝对私人目录。

前端构建会暴露客户端使用的配置。`VITE_`、`TAURI_` 前缀变量以及打包进前端、浏览器扩展或桌面 EXE 的字符串都不能用于保密。密钥不得通过前端环境变量传递。

`.env.example`、`.env.*.example` 和对应 `.template` 文件可以跟踪，但只能使用明确占位值。合成测试数据可以跟踪；不得使用真实凭据、用户资料或截图作为测试反例。不要为了绕过检测而给真实敏感内容添加例外标记。

## 提交前与 CI

`.gitignore` 排除常见私有文件和目录，但不能保护已经跟踪的文件，也不能阻止 `git add -f`。提交前必须检查暂存区内容；只扫描当前工作区会漏掉与工作区不同的暂存字节。

本地提交钩子应运行项目敏感信息检查器的暂存区扫描。钩子需要在每个克隆中安装，并可被 `--no-verify` 绕过，所以还要使用 CI。安装钩子前检查既有 `core.hooksPath` 和已有钩子，保留原有保护。

在新克隆中检查并保留已有钩子后，可安装项目钩子：

```text
git config --local core.hooksPath .githooks
git config --local imagelore.gitleaksBinary <verified-local-gitleaks-executable>
```

`pre-commit` 检查真实暂存字节；`pre-push` 同时检查可达历史和 Git 传入的实际待上传 SHA，防止直接推送未被本地分支引用的旧提交。缺少已验证的本地 Gitleaks 时拒绝推送。钩子使用本地配置的扫描器路径，路径不得写入公开项目配置。

本地 Git 的 noreply 邮箱不能覆盖 GitHub 自动生成提交时的账号邮箱选择。维护者应在账号 Settings → Emails 开启 `Keep my email addresses private` 与 `Block command line pushes that expose my email`，并核实新生成提交实际使用 noreply；[GitHub 官方说明](https://docs.github.com/en/account-and-profile/how-tos/email-preferences/blocking-command-line-pushes-that-expose-your-personal-email-address)。未验证该设置时，不创建或重新打开会生成测试合并提交的 PR，也不通过网页生成合并提交；可以将已通过全历史检查的 noreply 候选提交快进到主分支。

手工核验入口：

```text
python scripts/test_sensitive_guard.py
python scripts/check_sensitive.py --staged
python scripts/check_sensitive.py --tracked --history
```

`.github/workflows/sensitive-information.yml` 在所有分支推送、Pull Request 和手工触发时执行检查器回归、已跟踪文件及历史扫描。它完整拉取 Git 历史，并使用固定 Gitleaks 8.30.1；Windows x64 压缩包的 SHA256 在解压和执行前必须与固定官方校验值一致。

Gitleaks 使用 `.gitleaks.toml` 的项目配置及默认规则扫描所有已获取引用的可达历史，输出使用 `--redact=100`。配置例外必须对应可审查的合成占位数据，不能宽泛排除业务源码、Git 历史或整类敏感信息。

检查器输出不得包含命中原文。扫描报告只能写到运行器临时目录，不能打印完整报告、上传为 artifact 或提交到仓库。检测通过说明已检查范围没有规则命中，并不证明所有敏感信息都可被自动识别；编码、压缩、二进制或规则外个人信息仍需审核。

`main` 已把 `sensitive-information` 设置为必需状态检查，适用于管理员，并禁止强制推送和删除。更新先在基于最新主分支的候选分支通过检查，再更新主分支；不得通过临时关闭保护来上传未检查内容。独立 CI 失败不会撤销已经推送的提交，也不会自动阻止另一个发布工作流；发布前必须确认对应源码的检查已通过。

## 打包与发布

上传前检查实际文件清单和构建输入，只允许明确列出的发布文件。不要压缩或上传整个工作目录、`.git`、本地运行时备份或 `<private-storage>`。`.gitignore` 不限制手工压缩、复制、artifact 上传或第三方工具的文件收集。

Windows CI artifact 和 GitHub Release 只应包含指定安装器；不得附带资料库、原始日志、截图或扫描报告。安装器和前端构建仍需确认没有嵌入凭据或私有数据。`package.json` 的 `private: true` 保护 npm 发布，但不保证手工打包或其它上传入口安全。

## 发现泄露后

先通过维护者确认的私密渠道报告，只提供规则类别、脱敏位置和影响范围，不在公开 Issue、PR 或日志中粘贴秘密。对于已泄露的凭据，优先撤销或轮换，并核对访问记录；只删除文件或提交不能让凭据失效。

清理 Git 历史需要维护者确认和协作，处理相关分支、标签、PR 引用及远端缓存，并通知持有旧克隆的协作者。同步检查 Actions 日志、artifact、Release 附件和其它复制品。历史重写不能追回已下载的副本；不得把清理完成当作凭据无需轮换的依据。
