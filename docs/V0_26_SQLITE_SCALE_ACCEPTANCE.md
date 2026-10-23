# v0.26 SQLite 安全升级后的原生规模查询

日期：2026-10-23（Asia/Shanghai）。应用版本 `0.26.0`、schema `11`。实际归档由源码 `4d8451fd1fc46340267bd0c464731f18cfd8e26a` 编译；执行时文档 HEAD 为 `1dfe36999fd63eacdb26259da7ffde0a6f0a836b`，68 项编译/include 输入完全相同。本轮使用新 rusqlite 0.40.2 / libsqlite3-sys 0.38.2；不继承[旧引擎规模结果](V0_26_CURRENT_NATIVE_SCALE.md)。

## 实际范围和结果

独立显式 ignored 规模测试实际 **1 passed / 0 failed / 0 ignored**，**137.99 秒**，其余 238 项过滤；完整库另已实际 **236/0/3**，二者不累加为不同测试数。重新创建 50k 与 75k 合成 WAL 资料库，各 19 场景，每场景 3 次新连接与 20 次 warm，共 **38 场景 / 874 个计时样本**；另有 38 次 warm-up 不计统计。38 项既有建议预算均通过，没有删去或替换样本。

受测程序和 68 项输入前后相同。按原始数组独立重算中位数、nearest-rank p95、最大值及预算判定，与报告一致。测试对每次分页核对总数、完整页 ID 顺序、路径、fingerprint 和部分 nullable 尺寸；ranked 核对独立已知 ID 结果及降序。向量为合成 512 维单位轴查询，不涉及真实 encoder 或自然召回。

下表为 warm p95，单位毫秒；预算使用测试既有值，没有因升级而放宽。

| 场景 | 50k p95 | 75k p95 | 原建议预算 |
| --- | ---: | ---: | ---: |
| `page_first` | 13.23 | 54.18 | 500 |
| `page_offset_4800` | 14.24 | 61.97 | 500 |
| `page_offset_24000` | 15.62 | 35.28 | 500 |
| `page_offset_48000` | 16.39 | 24.40 | 500 |
| `english` | 198.79 | 422.99 | 500 |
| `cjk_one` | 56.04 | 89.32 | 1000 |
| `cjk_two` | 40.60 | 84.19 | 1000 |
| `cjk_trigram` | 190.02 | 250.55 | 500 |
| `mixed_terms` | 203.21 | 264.46 | 500 |
| `quoted_terms` | 249.68 | 288.11 | 500 |
| `dna_only` | 53.41 | 64.75 | 500 |
| `dna_only_cjk` | 40.97 | 57.77 | 500 |
| `reference_only` | 56.67 | 59.87 | 500 |
| `reference_only_cjk` | 58.20 | 60.36 | 500 |
| `zero_results` | 0.34 | 0.23 | 500 |
| `combined_parameters` | 29.12 | 35.14 | 500 |
| `exact_seed` | 40.32 | 51.61 | 500 |
| `ranked_512_unfiltered` | 572.99 | 718.80 | 3000 |
| `ranked_512_combined` | 253.17 | 416.13 | 3000 |

## 实际写入版本与边界

测试正常退出、所有连接关闭后，仅以原始字节读取两份新 fixture 的 100 字节 DB 头，再计算完整文件 SHA；没有由 Python SQLite 打开这两个数据库。两份均无 WAL/SHM/journal，offset 96 为 **3053002（3.53.2）**，offset 92 的 version-valid-for 与 offset 24 的 change counter 均为 2，字节数与 mtime 在核对前后相同。按 [SQLite 官方文件格式说明](https://www.sqlite.org/fileformat.html#write_library_version_number_and_version_valid_for_number)，这是最后修改该 DB 的 SQLite 库版本。结合当前 Native 新建 fixture 的过程与实际构建选中记录，可确认本轮 fixture 的写入版本；不将它转述为已经执行 `SELECT sqlite_version()`，也不把 lib test 结果转记为桌面应用运行证明。

“cold”只指新 SQLite 连接及 statement/page cache，未刷新 OS 文件缓存。构建、打开、断言与报告写入均不在查询计时内；机器可用内存、背景负载、时间及缓存未受同等控制。本轮不与旧引擎的单次总耗时作稳定提速比较，不关闭 IPC/WebView、解码、完整应用冷暖、滚动、长期内存或签名/updater/RC 验收。

## 当前桌面与发布状态

文档 HEAD `1dfe369` 的默认前端构建与真实 Tauri `--no-bundle` 构建均成功，当前隔离候选 EXE SHA256 为 `f32d25ace7ebd358b486f3907a20cc0246eda2480dac5e88c47ce5ebd589bdad`，产品/文件版本 `0.26.0`，只读检查为 `NotSigned`。编译输入与原稳定运行时文件前后相同，尚未推广为真实桌面运行时。

已准备十二张独立合成 PNG、空白且带 marker 的数据根及新 WebView profile 计划。首次启动在两分钟资源检查内未达到 3 GiB 可用内存，控制器终态为“未启动”，没有应用 PID 或 profile，也没有打开普通用户库；两个已知默认目录下六个 DB/WAL/SHM 元数据检查均前后不存在。该结果不计成功或失败的应用启动样本，也不证明全部用户文件保全。资源允许时继续隔离桌面导入、编辑、重启及其后原计划验收。

[候选源码 Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39908775792)已从原始日志核对实际 **236/0/3（1093.75 秒）**；[main 快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39911210262)和[main 敏感门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39911210285)通过，[main Native 复跑](https://github.com/AureliusWu/ImageLore/actions/runs/39911210425)原始日志另确认 `1dfe369` 实际 **236/0/3（1126.82 秒）**。随后[备份与恢复 profile](V0_26_SQLITE_BACKUP_ACCEPTANCE.md)独立确认运行时 SQLite 3.53.2；源已正常快进同步到公开 main，这些结果不等于正式 Release。

## 证据身份

| 证据 | SHA256 |
| --- | --- |
| 本轮规模原始报告 | `3a2e71b2943957eabc0532db5a3ed164dbfb008c885a687947d040004085011b` |
| 实际执行终态 | `eeb591f946919aea2c375cb681fff005aeeead22dc09b50372dff51e58988240` |
| 统计与 DB 头核对 | `6b3d45a15928d9ea6ca313d22ea2c8ed1747dbfb71dae858466ab3c8523bda19` |

原始报告、完整日志与合成 DB 均保存在仓库外；公开文档不包含本地绝对路径、实际用户图片、邮箱、凭据或账号安全设置。当前仍执行阶段 A，阶段 B/C/D 和 v1.0.0 发行保持开放。
