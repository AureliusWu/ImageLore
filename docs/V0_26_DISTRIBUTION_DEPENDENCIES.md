# Windows 分发依赖的来源与许可准备

记录日期：2026-10-25。版本仍为 `0.26.0`，阶段 A 继续验收，签名、安装升级、updater 与正式发布保持各自门槛。

本轮实际应用候选来自 `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`，EXE SHA-256 为 `f32d25ace7ebd358b486f3907a20cc0246eda2480dac5e88c47ce5ebd589bdad`。对照文档输入 main `177283b`，选定产品源码、锁文件和 LICENSE 相同；本轮没有推广该候选到桌面 current。

## 原生分发包的字节与来源证明

锁定的 `ort-sys 2.0.0-rc.13` 将 Windows x64 + DirectML 映射到 Pyke `ms@1.28.0` 分发。重新取得的压缩包为 **31,076,494 字节**，完整 SHA-256 与锁定表一致：

`f7c654b3729cb9e5ad2a36a0c38e5b48e63bf4eed22968931aed33a0ad0b527d`

按锁定解码器的 raw LZMA2 / 64 MiB 字典读取 tar，逐成员流式计算哈希，没有提取或运行成员：

| 成员 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `onnxruntime.lib` | 341,152,186 | `c3f5bb800fd19c05d3bd13614654f0c37ea3db1fbfba4d10963fa678cdb6cdd0` |
| `DirectML.dll` | 18,527,776 | `9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1` |

静态库与只读持有期间观察到的当前缓存相同；DLL 与应用候选旁置字节相同。分发包没有独立 LICENSE/NOTICE 文件，相关文本另行收集。

使用已安装的 [GitHub CLI 官方验证命令](https://cli.github.com/manual/gh_attestation_verify)，对实际完整包执行来源证明的密码学验证，退出码为 0。验证约束为：

- 仓库 `pykeio/ort-artifacts`。
- 签名工作流 `.github/workflows/build-runner.yml`。
- 签名与来源源码 digest 均为 `25d68de62d14300838a44c104261e0a773d44f8d`。
- 拒绝 self-hosted runner；证书实际声明 `github-hosted`。
- 默认 SLSA provenance v1，实际 subject digest 与上述压缩包相同，并有已验证的 Rekor 时间记录。

验证输出将该包绑定到 [Pyke 构建运行 30151376321 / attempt 1](https://github.com/pykeio/ort-artifacts/actions/runs/30151376321/attempts/1) 与[固定构建源码](https://github.com/pykeio/ort-artifacts/tree/25d68de62d14300838a44c104261e0a773d44f8d)。这是上游依赖的构建身份与字节证据，不是 ImageLore 的 Authenticode 签名。

静态库的内嵌构建字符串声明 `rel-1.28.0 / da9b5e3`。官方 API 将此前缀解析为 Microsoft [固定提交 `da9b5e364c465de65c49d91e696cd6485270757f`](https://github.com/microsoft/onnxruntime/commit/da9b5e364c465de65c49d91e696cd6485270757f)，与此前收集的官方 LICENSE/ThirdPartyNotices 来源一致。这个交叉印证仍保留“二进制自报告”的边界；Pyke 补丁、实际最终链接组件及确定性重建没有据此统一判为通过。

## 当前候选的许可声明清单

本轮从实际隔离应用构建目录重新读取锁文件和安装声明：

| 范围 | 结果 |
| --- | --- |
| 完整 Rust lock 图 | 599 包；573 份 SPDX 声明可读取，25 项本地源缺失或不唯一，项目自身未在 Cargo 元数据填写 SPDX 字段 |
| Windows x64 normal/build 解析图 | 404 包；403 个第三方包的本地 SPDX 声明均可读取，项目许可另由已采用的根目录 MIT LICENSE 证明 |
| npm lock 图 | 133 项；73 个已安装项的声明可读取，60 项未为本平台安装 |
| 初次本地文本候选池 | 750 份原样文本；当时 11 个 Rust 包和 4 个 npm 平台工具包未附可收集的顶层许可文本 |
| Rust 固定源补充 | 9 个包的 14 份固定提交文本候选，包含完整许可及短声明；另取得一份 Mozilla 官方 MPL 2.0 全文，适用范围分别核对 |

Windows 图由 `cargo tree --locked --offline`、目标 `x86_64-pc-windows-msvc`、`normal,build` 边及 `tauri/custom-protocol` 特性解析。它包括构建工具，不能等同于最终 EXE 的链接图；npm 安装项也不能仅按 dev 标志推定是否进入 Vite bundle。

同一源码、目标和特性的只读补充解析结果为：`normal` **380 项**，`normal,no-proc-macro` **304 项**。`selectors 0.38.0` 与 `simd_helpers 0.1.0` 均在前者出现、后者缺席；`zune-inflate 0.2.54` 在两者均出现。这些图说明依赖路径，尚不能证明最终链接范围，过程宏生成的代码也需单独考虑。全部输入哈希及干净工作区状态前后一致，没有触发编译。

三项 crate 的缓存压缩包完整 SHA-256 均与 Cargo.lock 相符；逐项读取包内文件，分别核对 **22 / 7 / 16 个文件**，与缓存源码全部相同：

- `selectors` 的 16 个 Rust 源文件头含 MPL 声明；[固定源码](https://github.com/servo/stylo/blob/572ecba2d1600e7c3d490586692a209faf703baa/selectors/lib.rs)指向 [Mozilla 官方 MPL 2.0](https://www.mozilla.org/en-US/MPL/2.0/)。已分别归档原始声明和官方全文，源码取得方式与第 3 节告知要求继续按实际分发范围核对。
- `zune-inflate` 的包内主页补充了仓库位置，版本身份仍取 crate 自带固定 VCS 提交。取得该提交的 [LICENSE-ZLIB](https://github.com/etemesi254/zune-image/blob/69502ce83fdfecdd0beefd677e2abb3781b29d98/LICENSE-ZLIB) 和 [LICENSE.md 短声明](https://github.com/etemesi254/zune-image/blob/69502ce83fdfecdd0beefd677e2abb3781b29d98/LICENSE.md)；后者不能替代 MIT/Apache 全文，本轮没有将它计为两份完整许可。
- `simd_helpers` 的[固定 Cargo.toml](https://github.com/lu-zero/simd_helpers/blob/ca1a2f84aa386d758e98f8a609d990263932fb85/Cargo.toml)声明 MIT 与 `proc-macro = true`，但包和已核对的固定仓库根目录仍无完整许可文本；该文本与版权声明缺口保留，没有补造权利人或年份。

四个 npm 平台工具包分别与 `esbuild`、`rollup`、`@tauri-apps/cli` 的锁定版本、同版本父包 optionalDependency 和仓库字段相符；父包的 LICENSE 候选已在本地池内。这个关系补齐了文本查找路径，尚不替代对生成代码及最终分发范围的核对。

后续[前端输出与声明草稿](V0_26_FRONTEND_DISTRIBUTION_SCOPE.md)已按原候选三个文件的完全相同字节绑定实际 chunk 模块，识别五个 npm 包及两个生成 helper。五个发布包的完整 SRI 和 31 个相关文件核对通过，插件缺失的条款全文由固定上游来源补充，本地草稿保留十份原样文本。这个范围与完整 lock、构建工具图及尚待确认的原生链接范围分别记录。

主目录曾检出 `source-map-js` 和 `vite` 的已安装版本落后于锁文件，首轮清单正确停止，没有输出清单结果。实际应用使用独立构建目录；本轮已改从该目录取证，并确认其产品源码与所记录 main 相同。该停止记录与主目录安装缓存均保留。

上述是许可声明、文本及来源材料，未将“有 SPDX 字段”直接当作分发许可全部完成。当前仍须完成最终产物包含范围、部分缺失文本及版权声明、模型与资产的署名范围；DirectML 的官方 NuGet 文本、其整包签名/成员身份验证与 SignPath 的 System Libraries 例外分别处理。项目 MIT 不会替第三方组件重新许可。

## 当前性能与发布边界

当前引擎的两路校验已有[完整回归与六操作证明](V0_26_SQLITE_WORKER_ACCEPTANCE.md)。新一轮 ABBA 对照在首次预热期间观察到并行 Cargo 编译，按预先规则停止；这次预热的 Native、consumer 和原件保全通过，**正式统计仍为 0**，现场完整保留，没有替换为一次更快观测。

原始依赖包、完整验证输出、许可文本池和机器路径均保存在仓库外。服务商资格、维护者条件、GitHub 外服务凭据接入、Windows 应用/安装器签名和 updater 仍需独立完成。本轮没有接受服务条款、付费或发布二进制。
