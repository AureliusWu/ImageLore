# ImageLore v0.13.0

ImageLore 是一个 local-first 的 AI 视觉生成记忆库，用来管理图片背后的提示词、模型、参数、历史版本、集合与生成谱系。

## 当前核心

- 简体中文 Frutiger Aero 桌面界面
- 图片 / 文件夹 / 拖拽导入
- Prompt、Negative Prompt、模型、标签自动保存
- Prompt Revision 历史版本
- Collection 与多父 Generation Lineage
- 父子 Prompt Diff
- SQLite FTS + CJK 子串搜索
- 虚拟化图库 + 持久化缩略图缓存
- 缺失文件重定位
- A1111 / ComfyUI 元数据读取
- .imagelore.json Sidecar
- Windows NSIS 一键安装包

数据保存在：
`%LOCALAPPDATA%\ImageLore\library.sqlite3`

原始图片不会复制进数据库，也不会因为从 ImageLore 移除记录而被删除。

## 快捷键

- `Ctrl+F` 搜索
- `Ctrl+I` 导入图片
- `Ctrl+S` 保存当前编辑并建立 Prompt 版本
- `Ctrl+Shift+C` 复制 Prompt
- `F6` 聚焦 Prompt
- `F7` 切换适应窗口 / 100%
- `Alt+↑ / Alt+↓` 前后切图

## 开发

```bash
npm install
npm run tauri:dev
```

正式 Windows 构建：

```bash
npm run tauri:build
```

也可以直接运行 `BUILD_WINDOWS_EXE.bat`，或使用仓库中的 GitHub Actions。

## 版本

`VERSION` 是唯一版本源：

```bash
npm run version:sync
npm run version:check
npm run version:patch
npm run version:minor
npm run version:major
```
