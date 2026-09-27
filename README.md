# ImageLore v0.14.1

ImageLore 是一个 local-first 的 AI 视觉生成记忆库，用来保存图片背后的 Prompt、模型、参数、历史版本、集合与生成谱系，并让这些信息在几个月以后仍然可以继续使用。

## 当前核心

- 简体中文 Frutiger Aero 桌面界面
- 图片 / 文件夹 / 原生拖拽导入
- 后台导入任务、实时进度与取消
- SHA-256 精确重复内容检测与自动跳过
- Prompt、Negative Prompt、模型、标签自动保存
- Prompt Revision 历史版本
- Collection 与多父 Generation Lineage
- 全库搜索式父图选择器
- 父子 Prompt Diff
- Tag 重命名 / 合并 / 删除
- Collection 重命名 / 删除
- SQLite FTS + CJK 子串搜索
- 虚拟化图库 + 持久化 WebP 缓存
- 本地 asset protocol 图片传输，不再通过 Base64 IPC 搬运大图
- 1 GB 缓存治理
- A1111 与 ComfyUI Metadata Adapter
- ComfyUI Prompt / Negative / Model / Seed / Steps / CFG / Sampler 等提取
- 每 24 小时自动数据库备份、手动备份、完整性验证与安全恢复
- 缺失文件重定位
- .imagelore.json Sidecar
- Windows NSIS 一键安装包

## 数据位置

主数据库：

`%LOCALAPPDATA%\ImageLore\library.sqlite3`

备份：

`%LOCALAPPDATA%\ImageLore\backups\`

缓存：

`%LOCALAPPDATA%\ImageLore\cache\`

原始图片始终保留在原位置，不会复制进 SQLite；从 ImageLore 删除记录也不会删除原始图片。

## 快捷键

- `Ctrl+F` 搜索
- `Ctrl+I` 导入图片
- `Ctrl+S` 保存当前编辑并建立 Prompt 版本
- `Ctrl+Shift+C` 复制 Prompt
- `F6` 聚焦 Prompt
- `F7` 切换适应窗口 / 100%
- `Alt+↑ / Alt+↓` 前后切图

## 开发

普通提交只运行快速 CI；只有手动发布或版本标签才生成 Windows 安装包。

```bash
npm ci
npm run check
npm run tauri:dev
```

正式 Windows 构建：

```bash
npm run tauri:build
```

## 版本

`VERSION` 是唯一版本源：

```bash
npm run version:sync
npm run version:check
npm run version:patch
npm run version:minor
npm run version:major
```
