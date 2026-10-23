# v0.26 安全依赖升级与兼容回归

日期：2026-10-23（Asia/Shanghai）。开发版本仍为 `0.26.0`、schema `11`。受测源码为 `4d8451fd1fc46340267bd0c464731f18cfd8e26a`，仅升级依赖及适配既有故障测试中的整数读取；未将两路备份候选合入。

## 改动与实际验证

| 依赖 | 原锁定版本 | 新锁定版本 |
| --- | --- | --- |
| rusqlite | 0.32.1 | 0.40.2 |
| libsqlite3-sys | 0.30.1 | 0.38.2 |
| rustls | 0.23.45 | 0.23.46 |
| rustls-webpki | 0.103.15 | 0.103.16 |
| webpki-roots | 1.0.9 | 1.0.10 |
| Vite | 7.3.6 | 7.3.7 |
| source-map-js | 1.2.1 | 1.2.2 |

SQLite bundled 头文件现在为 **3.53.2**；Windows 实际优化构建记录选中了新 `libsqlite3-sys` 和 `rusqlite`，未选中新增的 wasm 专用依赖。首次完整编译因六处测试直接读取 `usize` 失败，改为 `i64` 读取后使用 `usize::try_from` 检查转换；原损坏注入、WAL、索引页及恢复断言保留，没有启用额外全局整数 feature 或降低测试要求。失败尝试与之前因控制器中断而无终态的尝试均独立保留，不算通过。

修正后优化 lib test 编译通过；实际执行归档的完整套件为 **236 passed / 0 failed / 3 原有 ignored**，**157.19 秒**，无过滤。68 项编译/include 输入和程序归档在执行前后相同。独立 `cargo check --release --offline --locked --lib` 与 `cargo clippy --release --offline --locked --lib --tests -- -D warnings` 通过；`cargo fmt --check` 通过。该套件覆盖既有迁移、备份、受控空间/权限故障及恢复保护，但三个原有显式规模/profile 用例仍需单独执行。

前端按新锁执行 `npm ci --no-audit --no-fund`、`npm run check` 与 `npm run build` 全部通过，锁文件未改写。快捷方式及运行时发布测试使用独立临时目录和合成文件，没有运行 fixture EXE、修改真实桌面或推广本轮应用。

同源码候选[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/39908778908)与[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/39908775709)已成功；[Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/39908775792)已从原始日志核对实际 **236/0/3（1093.75 秒）**，与本地优化构建的耗时各自记录。

## 公告与许可范围

2026-10-23 对实际新锁的 **598 个 Rust registry 包、133 个 npm 锁定项**进行了不带凭据的公开公告查询。OSV 无待翻页；npm bulk 返回空记录，旧 TLS 命中不再返回。此结果是指定时间的公告快照，不是无漏洞或最终分发许可保证，也不是 `cargo audit` 的运行结果。

仍返回以下 Rust 记录，按实际 Windows 构建而非整个跨平台 lock 区分：

- [glib RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)：迭代器实现 unsound；本次 Windows 构建和目标图均未观察到，未声称修复其他平台。
- [paste RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)：停止维护；本次构建观察到该过程宏依赖，保留维护风险，不把它改写为已证实的运行时漏洞或已修复。
- [proc-macro-error RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)：停止维护；本次 Windows 构建和目标图均未观察到。

SQLite 原生代码另参照 [SQLite 官方 CVE 说明](https://www.sqlite.org/cves.html)，没有仅依赖 Rust 包公告推断其安全性。后续 50k/75k 原生查询已独立执行，38 场景 / 874 样本、0 budget WARN；两份新 fixture 的原始文件头均记录最后写入库为 3.53.2，详见[新引擎规模验收](V0_26_SQLITE_SCALE_ACCEPTANCE.md)。尚未独立执行 `SELECT sqlite_version()` profile，也不从该 DB 头或头文件推断桌面程序已运行。

六个本次新增/升级且被 Windows 目标图选中的 Rust 包已核对本地 crate 归档 checksum 与新锁一致，并收集声明及许可文本；另收集新 Vite/source-map-js 的实际安装许可文件。Windows 构建共观察到 404 个 package 身份，构建选中不等于最终链接或实际安装分发。未下载、未作为 Windows 依赖验收两个 wasm 专用归档。最终应用/NSIS、ORT/DirectML、其他未变依赖的通知及来源仍按原分发审查继续，MIT 仅覆盖原创代码。

## 证据身份和发布边界

| 证据 | SHA256 |
| --- | --- |
| 优化 lib test 归档 | `8dc3843193e4e1ce097ae13ff4f039517dc28628244689ee7d2a06e881634bfd` |
| 成功编译终态 | `df9fb6fbad76497332ba36c254282c647ab69811bdc1fca2863745c80b3c2277` |
| 完整库测试 stdout | `8ba0a5105f4d0c1a6fee7b3b89c681f364bc556346d1d048d354f526077bf533` |
| 新 Cargo.lock | `2c2f6c213178ae542a8e778fd6eb9256c98c7c30a998fb7e8699247074d61c77` |
| 新 package-lock.json | `70fc9296badcbef71dca249b4527ad988a329604bfc93c3447e81a16a9df649d` |

完整日志、未改写的失败尝试和许可材料均位于仓库外。以上证明当前依赖的后端兼容回归，尚未验收本轮实际应用 EXE、GUI、长期内存、安装升级、签名或 updater。已验收桌面继续绑定其原始源码；只有新候选完成隔离桌面验收后才推广运行时与快捷方式。

MIT 已公开采用；免费签名资格尚未批准，禁止将 SignPath 提交 token 存入 GitHub Secrets 的接入方案仍未获服务商确认。MFA 与信誉材料也未记为已满足。继续原阶段顺序，不发布 `v1.0.0` 标签或宣称可信分发已经完成。
