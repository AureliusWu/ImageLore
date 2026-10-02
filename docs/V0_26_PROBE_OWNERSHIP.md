# v0.26 完整性检查临时目录所有权修复

2026-10-02，独立回归先复现旧行为：完整性探测的 `create_dir` 遇到已有目录时返回 `AlreadyExists`，后续错误清理仍删除该目录。正常流程的时间戳名称很少碰撞，但这不能授权删除未由当前调用创建的目录。

旧逻辑只提取为可指定目标路径的函数，未改变创建、复制或清理顺序。回归先创建带有二进制哨兵的已有目录，调用探测复制，再核对原件；实际结果为 1 failed / 1 passed、Cargo exit 101。失败是原件已经不见的 `NotFound` 断言，编译成功。另一项正向控制让源数据库路径成为带有哨兵的目录，确认复制失败时可以清理本调用新建的目标，同时保留源内容。

修复把原子 `create_dir` 放到有条件清理之外：只有成功创建目录后才有清理权限。名称增加进程 ID 和原子序号；遇到占用最多重试 8 次，然后返回错误，不接管已有目录。SQLite 完整校验、外键和业务检查、DB/WAL 复制范围、SHM 重建、备份数量与恢复策略均保留。

同两项回归修复后 2 passed / 0 failed、Cargo exit 0。固定的 `backup.rs` 字节 SHA256 为 `8FEC01F565696E2A52FE50F2377CADF2C24F3F899B5188F296A518824BC56CE5`。这是文件所有权故障回归，不是备份轮换性能提升证明；[先前的50k阶段测量](V0_26_BACKUP_PHASES.md)仍属于其原始源码，不能把旧耗时转记为修复后产物的结果。

本机原始证据在 `<private-evidence>/`，不提交夹具：

- `probe-ownership-old-20261002-{start,result}.json` 和 `.log`：旧逻辑源码 SHA、失败与正向控制。
- `probe-ownership-fixed-20261002-result.json` 和 `.log`：修复字节 SHA 与两项通过结果。

随后统一Native门禁通过：fmt、locked check、lib clippy `-D warnings`，以及139 passed / 3 ignored的默认库测试。日志为 `<private-evidence>/import-close-native-{fmt,check,clippy,test}-20261002.log` 与对应门禁结果JSON。这仍不替代新的原生产物验证；两路有界备份校验尚未实施。
