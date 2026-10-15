# 随版本更新的桌面快捷方式

桌面链接名为 `ImageLore v<实际程序版本>.lnk`。版本读取自 EXE 的 ProductVersion；脚本不会给旧 EXE 标注仓库的新版本。

本地构建与更新：

```powershell
npm run desktop:build
```

该命令构建无安装器的原生应用，发布到 `desktop-runtime/current/ImageLore.exe`，再更新桌面链接。旧运行目录保留为 `desktop-runtime/previous-<时间>-<随机标记>`；推广或快捷方式更新失败时回退运行目录。请先退出正在运行的旧程序，Windows 文件占用会安全阻止替换。

仅重建链接：

```powershell
npm run desktop:shortcut
```

程序启动时也会刷新已有的受管快捷方式，因此以后启动新版本的安装版时，链接可跟随新的 EXE 路径。启动旧版本不会自动把链接降级；删除受管链接表示退出自动维护，再次显式运行上述命令可恢复。

脚本使用每个状态文件的跨进程互斥，只清理状态文件记录的旧链接，并同时核对桌面目录、名称、描述和 EXE 目标。普通手工链接不受影响，同名但不属于 ImageLore 的链接会被拒绝覆盖。状态文件为 `%LOCALAPPDATA%/app.imagelore.desktop/desktop-shortcut.json`。

状态文件显式按 UTF-8 读写。快捷方式使用 Windows [IShellLinkW Unicode 接口](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw) 与 `IPersistFile` 读写目标、工作目录、图标和描述；避免英文 Windows 的旧接口拒绝中文目标路径。该封装内嵌在脚本中，保持本地发布和应用启动时的脚本自包含，不新增外部依赖，也不调用目标解析或启动 EXE。此项不扩展 Windows 长路径支持范围。

本地运行目录不纳入 Git；图片和资料库仍使用独立数据目录。`desktop:build` 不生成 NSIS、不触发 GitHub Release。签名安装包和自动更新仍按 ROADMAP 的后续阶段验收。

本轮已经实际生成桌面 `ImageLore v0.26.0.lnk`，目标为 `<project>\desktop-runtime\current\ImageLore.exe`。程序身份、发布回退与隔离原生操作证据见[桌面验收](V0_26_DESKTOP_ACCEPTANCE.md)。快捷方式的版本跟随已安装/已构建 EXE；下载远程更新属于后续 updater 阶段。

回归命令 `npm run test:shortcut` 使用临时中文目录、临时桌面及带版本资源的合成 EXE，不运行这些 EXE，不接触实际桌面。测试核对目标/工作目录/图标的 Unicode 回读，保留全部版本、所有权、删除退出、锁定回滚及并发场景；并发子进程保留句柄并要求实际退出码为 0。`test:runtime` 的合成链接也使用相同 Unicode 接口，保留原有推广、碰撞拒绝与回退断言。
