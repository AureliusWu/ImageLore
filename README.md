# ImageLore v1.0.0

<!-- project-badges:start -->
[![Version](https://img.shields.io/github/v/tag/AureliusWu/ImageLore?sort=semver&style=flat-square&label=version)](https://github.com/AureliusWu/ImageLore/tags) [![Build](https://github.com/AureliusWu/ImageLore/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/AureliusWu/ImageLore/actions/workflows/ci.yml) [![Last commit](https://img.shields.io/github/last-commit/AureliusWu/ImageLore?style=flat-square)](https://github.com/AureliusWu/ImageLore/commits/main) [![License](https://img.shields.io/github/license/AureliusWu/ImageLore?style=flat-square)](LICENSE)
<!-- project-badges:end -->

当前正式版：`1.0.0`，带已知限制发布。Windows 应用与安装器未签名、无自动更新；发行边界见 [v1.0.0 发布说明](docs/releases/v1.0.0.md)。

ImageLore 是一个 local-first 的 AI 视觉生成记忆库，用于保存图片背后的 Prompt、模型、参数、Visual DNA、历史版本与生成谱系，并支持本地语义检索和多参考 Remix。

## 核心能力

- 图片 / 文件夹 / 来源目录导入，SHA-256 去重与后台任务。
- Prompt、Negative Prompt、模型参数、标签与 Revision 历史。
- Semantic Recall：本地 CLIP 图文向量搜索与相似图检索。
- Visual DNA、Prompt Card、Image to Prompt 与 Remix Workspace。
- Collection、Generation Lineage、父子 Prompt Diff 与 Sidecar v3。
- Chrome / Edge「Save to ImageLore」Reference Inbox。
- SQLite FTS / CJK 搜索、虚拟化图库和 WebP 缓存。
- 自动数据库备份、损坏恢复、缺失文件重定位与本地诊断日志。

原始图片保持在原位置；从 ImageLore 删除记录不会删除原图。

## 本地数据

主数据目录：`%LOCALAPPDATA%\app.imagelore.desktop\`

其中包含 `library.sqlite3`、`backups/`、`cache/`、`models/` 与 `logs/`。联网与隐私边界见 [DATA_AND_NETWORK.md](docs/DATA_AND_NETWORK.md)。

## 开发与验证

```bash
npm ci
npm run format
npm run check
npm run tauri:dev
```

Windows 构建：

```bash
npm run tauri:build
```

## AI / 维护者入口

本仓暂未设置根级 `AGENTS.md`，维护时以以下入口为准：

- `VERSION`：当前应用版本唯一事实源。
- `npm run version:sync` / `npm run version:check`：同步和校验 npm、Tauri、Cargo、前端与 README 版本。
- `docs/DATA_AND_NETWORK.md`：本地数据、联网与隐私边界。
- `docs/releases/`：发行说明与已知限制。
- `CHANGELOG.md` / `ROADMAP.md`：历史与后续规划。

不要把逐版本功能流水、性能审计或安装器验收重新堆进 README；详细证据留在 `docs/` 和 CI。不要删除用户原图、绕过本地数据边界或把 Vision API Key 写入持久日志。

## License

原创代码采用 [MIT License](LICENSE)；第三方依赖、模型、字体、图标与其他材料保持各自许可。