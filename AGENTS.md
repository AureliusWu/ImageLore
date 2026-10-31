# AGENTS.md

本文件定义 ImageLore 的 AI / 自动化维护规则。README 面向用户与项目入口；详细历史、发行记录和设计说明放在 `docs/`、`CHANGELOG.md` 与 GitHub Release。

## 项目定位

ImageLore 是本地优先的视觉生成资料与提示词管理工具，主要技术栈为 React + TypeScript + Vite + Tauri 2，并包含浏览器扩展与本地桌面运行时。

## 事实源

- 当前版本：`VERSION`。
- npm 依赖与脚本：`package.json` / `package-lock.json`。
- 桌面端配置：`src-tauri/`。
- 浏览器扩展：`browser-extension/`。
- 版本历史：`CHANGELOG.md`。
- 长期设计、迁移和发行说明：`docs/`。

不要在多个文件里复制一套新的版本事实；版本变化应通过现有版本同步脚本维护。

## 修改前最低要求

```bash
npm ci
npm run version:check
npm run lint
npm run build
```

涉及完整工作流、桌面运行时、浏览器扩展或发布边界时，优先执行：

```bash
npm run check
```

## 维护约束

- 保持 local-first：用户图片、提示词、历史记录和本地索引不得默认上传到远端。
- 不提交真实凭据、令牌、用户私有数据、构建产物或本机绝对路径。
- 数据结构或持久化格式变更必须考虑旧数据兼容与迁移。
- 浏览器扩展、Web 与 Tauri 桌面端共享逻辑时，避免形成三个互相漂移的实现。
- 不把 Mock、截图或浏览器模拟写成真实桌面 / 实体环境验收。
- 除非任务明确要求，不生成、打包或发布 Windows EXE / Installer。
- 不随意放宽现有安全扫描、版本一致性和稳定性门禁。

## 文档边界

README 只保留产品定位、当前版本、核心能力、最小开发入口和本文件链接。历史迭代、长篇验收、发布证据和路线规划继续留在 `docs/`、`CHANGELOG.md`、`ROADMAP.md` 和 Release。
