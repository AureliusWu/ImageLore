# ImageLore 接手与基线记录

日期：2026-09-30（Asia/Shanghai）

用户指定本地路径：`<project>`

仓库：`AureliusWu/ImageLore`（私有）

接手远端 SHA：`9b93bbbabab6ee8795fd6f0b0406acf7dfddccb4`

分支：`main`；应用版本：`0.25.1`；schema：`11`。

## 同步与范围

目标目录最初为空，以远端main克隆后，执行 `git fetch origin`、`git checkout main`、`git pull --ff-only origin main`；工作树在基线开始前干净。本轮未发现适用的AGENTS.md。

已阅读用户交接说明以及README、ROADMAP、ARCHITECTURE、测试脚本、CI、Windows安装器文档与相关代码。沿用现有Node/Python/Rust测试体系，不新增独立框架。本轮新增小范围回归保护和v1.0规划，不升级版本，不运行NSIS/Windows Release，不生成安装包或GitHub Release，不启动应用接触真实用户资料库。

## 原始main基线

| 项目 | 结果 | 说明 |
| --- | --- | --- |
| `npm ci` | PASS | 按lockfile安装；73 packages |
| `npm run check` | PASS | 版本/结构/schema/scale/contract/Node/Biome/TS全部通过；32.78s |
| Command contract | 85 / 85 / 85 PASS | frontend / registered / mock |
| Preview / Remix / Extension | 5 / 3 / 3 PASS | 原始main测试数量；0 failed |
| 50k Python smoke | PASS | FTS 0.0032s；CJK 0.0013s；Generation 0.0213s；16维semantic 0.3005s |
| `npm run build` | PASS | 19.98s；JS 364.43kB / gzip110.37kB；CSS53.58kB / gzip11.61kB |
| Rust fmt check | PASS | 0.83s |
| Rust locked check | PASS | 162.34s，含首次依赖下载/编译 |
| Rust locked clippy `-D warnings` | PASS | 10.73s |
| Rust locked library tests | 53 / 53 PASS | 首次编译135.62s，执行0.13s |

远端观察：

- 当前HEAD的[Frontend CI](https://github.com/AureliusWu/ImageLore/actions/runs/36655427920)成功。
- 最近的[Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/36655119313)成功，SHA为`646a9a3`。`9b93bbb`仅修改前端CI，未触发路径过滤的Native workflow；本轮仍对当前HEAD在本地执行完整Native验证，不把旧SHA的远端结果当当前HEAD测试。

本机环境：Node24.14.0、npm11.9.0、Python3.14.4、Rust/Cargo1.97.0、x64 Windows MSVC、VS2022 BuildTools、Windows SDK10.0.26100.0。仓库CI使用Node22，二者分开记录。测试链接阶段有MSVC生成`.lib/.exp`的stdout信息warning，未影响退出码、Clippy或测试结果。

50k smoke使用内存SQLite与16维模拟向量；这些数值不能证明生产512维语义检索、原生GUI或原图处理的端到端性能。

## 本轮修复

### 切图来源残留

`useAssetContext`切换资产时清空DNA/Analysis/Remix，却未清空Reference Sources；请求等待期间新图片可显示上一张图来源。

先在现有Preview Node入口中新增实际Hook回归：旧代码6通过/1失败，来源长度实际1、预期0。修复只在切图初始化增加 `setReferenceSources([])`。新增测试同时覆盖空选择和旧请求迟到不覆盖新图上下文；修复后Preview 7/7通过。

测试通过现有TypeScript transpileModule与Node vm注入React/API受控状态，加载实际Hook源码；它是异步状态单元回归，不替代原生WebView操作验收。

### 启动恢复范围

原始 `prepare_state` 对已有数据库的任意初始化失败都会尝试历史备份恢复，包括数据库未来schema、锁、权限和迁移错误。修改只允许已确认数据库损坏进入自动恢复：初始化失败后以只读SQLite连接执行完整性probe，依据typed SQLite错误分类；正常初始化路径不增加probe。公开Command/API、schema和版本保持一致。

新增5项Rust回归：未来schema拒绝并保持当前记录/schema12/旧备份；exclusive锁不进入恢复；完整性正常但迁移失败保持原库；确认损坏仍恢复并保留原始字节；typed BUSY/LOCKED/PERM/READONLY/CANTOPEN/IOERR不构成损坏证据。旧恢复逻辑的4个场景1通过/3失败；修复后5/5目标回归通过。权限覆盖为typed错误分类，未做Windows ACL实机验收。

备份身份验证、原件保全失败、pending手动恢复边界与迁移中断矩阵仍列在后续A2/A3中；本次修复不代表整个灾难恢复验收已经完成。

## 修改后的完整复验

| 门禁 | 结果 | 说明 |
| --- | --- | --- |
| `npm run check` | PASS | 10.87s；全部入口通过，Preview7、Remix3、Extension3，0 failed |
| Command contract | 85 / 85 / 85 PASS | 未新增Command |
| `npm run build` | PASS | 5.13s；JS364.44kB / gzip110.36kB；CSS53.58kB / gzip11.61kB |
| `cargo fmt --all … -- --check` | PASS | 0.64s |
| `cargo check --locked …` | PASS | 4.37s |
| `cargo clippy --locked … --lib -- -D warnings` | PASS | 6.31s |
| `cargo test --locked … --lib` | 58 / 58 PASS | 21.88s含编译，测试执行7.16s；0 failed |
| `git diff --check` | PASS | 无空白问题；version sync未改变版本文件 |

测试链接阶段仍有同一MSVC stdout信息warning；未通过关闭warning或降低Clippy绕开任何门禁。构建耗时受首次编译与缓存影响，不作为性能提升结论。

数据可靠性结论：`CONDITIONALLY TRUSTED`。本轮保护有合成库与单元回归证据；完整恢复矩阵、跨版本业务一致性及真实原生工作流仍待验收。v1.0正式发布门槛尚未达成。

## v1.0方案与未验收门槛

方案见 [V1_0_0_PLAN.md](V1_0_0_PLAN.md)，并已接入ROADMAP。

推荐顺序：v0.26数据安全/历史迁移/原生50k验收 → v0.27可信分发与updater beta → v1.0 RC隔离Windows升级/恢复/长期运行 → 明确授权后的正式发布。

未完成项包括：全schema兼容矩阵、受支持备份身份与保全失败路径、真实50k+原生性能、长期开机备份/缓存策略、数据派生一致性、Windows双签名、自动更新、安装器与真实升级验收。代码签名证书/服务、发行渠道及密钥责任是后续外部依赖；私有源码仓库不能被默认当可匿名下载的更新渠道。

SQLite Backup保存元数据，不能恢复外置原图字节。恢复与性能验收只允许使用临时合成数据或一次性VM/隔离Windows账户；现有Windows升级脚本会操作固定用户数据目录，不在当前用户环境直接运行。

## 本地证据

完整命令输出保留在本地 `<private-evidence>/`，不提交日志或构建产物：

- `original-handoff.md`：用户提供的交接原文。
- `npm-check.log` / `npm-build.log`及result：原始前端基线。
- `native-baseline-20260930/`：原始Rust环境与四项门禁、生成的Tauri schema产物归档。
- `final-npm-check.log` / `final-npm-build.log`及result：修改后前端完整门禁。
- `final-cargo-fmt.log` / `final-cargo-check.log` / `final-cargo-clippy.log` / `final-cargo-test.log`及result：修改后Native完整门禁。
- `github-initial-runs.json`：接手时远端CI观察。

构建生成的`src-tauri/gen/`由本地`.git/info/exclude`排除；应用源文件和本轮修改均显式审查。
