# 前端分发范围与声明草稿

记录日期：2026-10-25。应用版本仍为 `0.26.0`，阶段 A 未关闭；本记录补充分发材料，不代表签名或正式发布。

## 对照的应用与前端字节

对照应用来自 `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`，归档 EXE SHA-256 为 `f32d25ace7ebd358b486f3907a20cc0246eda2480dac5e88c47ce5ebd589bdad`。其正常 Tauri 无安装器构建已记录前端输出哈希，候选尚未推广为桌面 current。

本轮使用同一隔离源码及已锁定依赖，按 [Vite JavaScript build API](https://vite.dev/guide/api-javascript#build) 在新目录构建；仅增加读取 [Rollup generateBundle 元数据](https://rollupjs.org/plugin-development/#generatebundle)的观察插件，没有修改输出内容。三个文件与原应用构建后的前端文件逐字节相同：

| 文件 | SHA-256 |
| --- | --- |
| `index.html` | `e9d3f1789c6be6e1eaa40b584c2af54f90d66fe6dd98e7fe3c1d348f0ef21180` |
| `assets/index-Bzrl0l6h.css` | `c76e8151469cfbe436f8ee0a1144e5571528107869dfb3260d1e09cc49b3a4e5` |
| `assets/index-DzMZ5FeH.js` | `571250864845f49bd367ee3ec6c3bc582c971329f244153a1a0d537a60dad6fe` |

Node 构建退出码为 0。184 项受版本控制输入、51 个实际物理输入文件及原 dist 文件的哈希保持一致，隔离工作区仍干净；归档 EXE 身份再次核对相同。观察没有打开 ImageLore 或正常用户资料库。

## 实际 chunk 中的第三方包

该 JS chunk 记录 66 个模块，外部 chunk imports 和 dynamicImports 均为空；模块长度为构建器观察值，不作为版权或最终压缩字节的归属判断。物理输入中涉及下列五个 npm 包，每个包均有非零 rendered 模块：

| 包 | 版本 | 包内许可声明 | 不同物理源码文件 |
| --- | --- | --- | ---: |
| `@tauri-apps/api` | 2.12.0 | Apache-2.0 OR MIT | 8 |
| `@tauri-apps/plugin-dialog` | 2.8.0 | MIT OR Apache-2.0 | 1 |
| `react` | 19.3.0 | MIT | 4 |
| `react-dom` | 19.3.0 | MIT | 4 |
| `scheduler` | 0.28.0 | MIT | 2 |

从 npm 官方 registry 取得五个确切版本的元数据及原始 tarball，完整 SHA-512 SRI 均与 package-lock.json 相同。19 个物理源码文件、五份 package.json 和七份随包声明/许可文件，共 **31 个文件**，逐一与已安装输入比较，字节全部相同；没有安装或执行下载的包。React 等包的 registry gitHead 仅作为发布者元数据，不提升为密码学源码证明。

这份实际输出记录缩小了前端范围；完整 npm lock 的 133 项、已安装声明的 73 项仍是不同范围，不能直接作为客户端全部包含的组件。

## 前端声明草稿

本地已准备五个组件及生成 helper 的独立声明草稿，保留 **10 份原样文本**和逐文件哈希/来源清单，尚未作为完整应用声明公开发布。

`@tauri-apps/plugin-dialog` 的发布包只附 `LICENSE.spdx`，其中列出许可和版权声明，没有 MIT/Apache 条款全文。本轮依据确切 npm 版本的 gitHead `a2364a5f216324439feedeb25b2db74e7b1eba90`，取得同一上游提交的 [MIT 全文](https://github.com/tauri-apps/plugins-workspace/blob/a2364a5f216324439feedeb25b2db74e7b1eba90/LICENSE_MIT)与 [Apache 2.0 全文](https://github.com/tauri-apps/plugins-workspace/blob/a2364a5f216324439feedeb25b2db74e7b1eba90/LICENSE_APACHE-2.0)。上游 SPDX 文件与发布包字节相同，原 SPDX 声明与两份全文一起保留；没有补造权利人或年份。

另有两个无物理源文件路径、非零 rendered 的生成模块：Vite modulepreload polyfill 与 CommonJS helper。草稿同时保留锁定 Vite 7.3.7 的完整 LICENSE.md，其中包括 plugin-commonjs 的组件许可声明；这不表示整个 Vite 构建工具进入客户端。

完整原始包、模块路径和报告保存在仓库外。独立复核重新核对三份输出、五个包的完整 SRI、31 个相关文件及十份草稿文本，均通过。原生库的实际链接范围、DirectML 条款和身份、模型/分词器及其他应用资产继续按[依赖来源与许可准备](V0_26_DISTRIBUTION_DEPENDENCIES.md)核对；本记录不关闭这些项目、桌面验收或签名/安装升级/updater 门槛。
