# 本地数据与联网说明

更新日期：2026-10-26；适用于当前 `0.26.0` 桌面源码及 Save to ImageLore 扩展。本说明描述已核对的源码行为；本轮尚未完成新源码的真实桌面与网络观察。

## 什么内容保存在本机

ImageLore 将图片路径、Prompt、生成参数、标签、集合、谱系、来源、AI 分析结果及语义索引保存在本地资料库。导入记录引用原始图片，预览和缩略图另建缓存；图片预览、关键词检索和本地 CLIP 推理在本机进行。当前代码没有将整个图库自动同步到维护者服务器的功能。

默认数据目录为 `%LOCALAPPDATA%\app.imagelore.desktop\`，包含数据库、备份、缓存、模型和诊断日志。随图片保存或导出的 `.imagelore.json` Sidecar 可能包含 Prompt、来源 URL、页面标题与谱系；分享图片文件夹时也请核对这些文件。

## 哪些操作会联网

| 操作 | 发送或取得的内容 | 控制方式 |
| --- | --- | --- |
| 首次使用或修复本地语义模型 | 从固定 Hugging Face/Qdrant revision 下载模型及配套文件。接收方及下载 CDN 会收到常规网络请求信息；该下载请求不携带图库图片或检索文字 | 可信完整缓存可离线使用，下载任务可取消 |
| 点击当前图片的 Image to Prompt 分析 | 向你配置的 OpenAI-compatible 服务发送模型名、分析指令及图片 JPEG。最长边限制为 1600 像素，JPEG 质量 88；配置 Key 时通过认证头发送 | 由你发起分析，普通导入、换图和保存编辑不会触发这项请求 |
| 扩展右键“保存到 ImageLore” | 浏览器从选中的图片地址下载文件，适用浏览器既有 cookie、代理和网络策略；图片、来源页面 URL、标题和时间写入本机 Downloads/ImageLore Inbox 与 Sidecar | 由右键菜单显式采集；桌面启动或聚焦时同步这个本地目录 |
| 点击来源链接 | 将所选 http/https 地址交给系统浏览器，后续连接适用浏览器和目标网站的政策 | 由你点击链接 |
| 安装时需要 WebView2 | 安装包内置 Microsoft bootstrapper，缺少运行时的机器可能联网取得 WebView2 | 当前配置不是完整离线运行时安装包，见 [Tauri 官方说明](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options) |

外部 Vision 服务如何保存或使用请求，取决于你选择的服务和配置。使用前请查看该服务的隐私及数据保留条款；ImageLore 的本地备份或删除操作不能清除第三方已经收到的数据。

扩展还在浏览器本地存储最近采集的状态和错误信息；清理桌面数据库不会同时清除这些浏览器记录。

## Key 和诊断日志

Vision 的专用 API Key 字段仅保存在当前进程内存，也可由 `IMAGELORE_VISION_API_KEY` 环境变量提供；数据库设置保存 Base URL 与模型名。请把密钥填入专用 Key 字段，避免写入会持久保存的 URL、模型名、Prompt 或来源标题。

常规 `imagelore.log` 按约 1 MiB 阈值轮换，最多五份；启动失败还会另写 `startup-error.log`。这些文件可能含本机路径和错误上下文，日志模块不提供通用内容脱敏。当前项目代码没有日志自动上报入口；手工分享日志、截图或数据库前，请先核对并脱敏。

## 删除与备份的范围

数据库备份和 Sidecar 可能保留较早版本。删除当前记录不会同时清除已有备份、Sidecar、原始图片、浏览器下载记录或第三方数据；这些副本需要分别管理。当前 SQLite 文件没有应用提供的数据库加密，本机资料的访问也取决于操作系统和存储位置的权限。

## 核对范围

源码核对覆盖 [Vision 请求](../src-tauri/src/vision.rs)、[显式分析入口](../src/hooks/useVisionWorkflow.ts)、[模型下载](../src-tauri/src/semantic.rs)、[浏览器采集](../browser-extension/background.js)、[来源打开](../src-tauri/src/references.rs)、[本地日志](../src-tauri/src/diagnostics.rs)与[安装配置](../src-tauri/tauri.conf.json)。这些文件与已核对的 `152d2a4` 产品源码相同；本轮没有向外部 Vision endpoint 发送图片。

操作系统、浏览器/WebView2、代理及第三方服务各自的网络行为不由这份源码审查认证。新源码的受控网络观察、安装界面告知及桌面操作仍须分别验收；本说明不表示签名服务资格或 v1.0.0 发布已完成。
