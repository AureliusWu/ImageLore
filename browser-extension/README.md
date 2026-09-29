# Save to ImageLore 浏览器扩展

适用于 Chrome / Edge 的 Manifest V3 扩展。

## 开发模式安装

1. 在 ImageLore 中打开「资料库管理 → 来源目录」，点击「准备浏览器 Inbox」。
2. Chrome 打开 `chrome://extensions`，Edge 打开 `edge://extensions`。
3. 开启「开发者模式」。
4. 选择「加载已解压的扩展程序」，指向本仓库的 `browser-extension` 目录。
5. 在网页图片上右键：
   - **保存到 ImageLore**
   - **保存到 ImageLore · Remix 参考**

扩展不会连接外部 ImageLore 服务，也不会读取你的 ImageLore 数据库。它只把图片和一个同名 `.imagelore.json` Sidecar 写入系统下载目录下的 `ImageLore Inbox`。

ImageLore 会把该目录登记为 Source Folder，并在启动或重新聚焦桌面应用时同步。图片仍使用现有 SHA-256 去重逻辑；同一图片从不同网页再次保存时，来源记录会合并到已有资产。
