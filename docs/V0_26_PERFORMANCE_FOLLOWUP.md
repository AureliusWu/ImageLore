# v0.26.0 性能与关闭安全复验

执行日期：2026-10-01（Asia/Shanghai）。本记录追加于[桌面验收](V0_26_DESKTOP_ACCEPTANCE.md)，按实际产物区分结果，不覆盖历史测量。

## 程序与检查身份

候选程序来自 `cd220b2a48c2f94a44e88e0c905e3d66781f6d17`，版本 `0.26.0` / schema `11`，SHA-256 为 `760A77637F1CEDCACCB4A1C7033FB8AAF0EAF93BA1260870ED7F54E6A117FAF9`。候选放在私有验收目录，桌面稳定路径仍保留此前 `f61f59b` 的程序，尚未把候选验收结果当作已替换桌面程序的证据。

该候选包含两项产品修改：临时 SQLite 验证连接使用最多 32 MiB 的页缓存；手动备份和恢复准备移入阻塞任务池，关闭窗口等待手动及自动安全任务终态后再保存最后编辑。完整 integrity、外键、备份身份、schema 和关系校验保留。

前端 check / build、Rust fmt / locked check / strict clippy / full lib test 全部通过：85 个命令契约、42 个 Node 行为用例、106 个 Rust passed / 2 ignored。两个 ignored 性能用例必须显式运行，不计入默认 CI 已执行项。源码 `864a662ac07ef693a30a70153fe9a620d003b6a6` 仅追加安装器工作流和验证脚本，产品源码与候选相同；[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/36867577661) 和 [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/36867577689) 均 success。

私有原始证据在 `<private-evidence>/`，不包含用户资料库；此记录只提供可审查的结论和边界。

## 全库校验与启动

同一 50k 文件库，保留完整全库校验，仅改变临时验证连接的页缓存：

| 测量 | 修改前 | 修改后 | 范围 |
| --- | ---: | ---: | --- |
| 复制后完整 integrity，中位数 | 3,379.68 ms | 2,515.15 ms | debug，三次；约 -25.6% |
| 生产备份验证，中位数 | 3,618.82 ms | 2,716.57 ms | debug，三次；约 -24.9% |
| 直接完整 integrity，2 MiB / 32 MiB 缓存中位数 | 2,329.21 ms | 1,285.65 ms | release，三次；约 -44.8% |
| 进程启动至 FCP，中位数 | 3,090.40 ms | 3,126.90 ms | 各三次交错启动；约 +1.18% |

release 的生产复制检查中位数为 1,778.67 ms，生产验证为 1,731.20 ms。验证用例确认测试前后原数据库字节一致；32 MiB 设置只用于临时验证连接，不改变活动图库缓存设置。

启动对照使用同一 50k 库、WebView profile、视口和机器，不清 Windows 文件缓存；不能称为整机冷启动。候选三次 FCP 为 2,470.0 / 3,126.9 / 3,158.1 ms，旧程序为 3,090.4 / 3,027.5 / 3,420.9 ms。FCP 中位数没有改善，建议 3 秒预算仍未通过。首次新路径启动另曾观测约 14 秒，保留在失败的首轮观测中，不从记录中删掉；其失败来自正常退出后的验收脚本进程查询，不代表程序被强制结束。

FCP 表示已绘制内容，另存的首图解码就绪观察上界含采样等待，不作为精确可交互时间。全库校验减少的时间也不能直接等同于 FCP 或实际恢复的改善；恢复仍须单独复测。

原始文件：`storage-profile-{before,after}-v026.txt`、`storage-profile-release-v1.txt`、`native-gui/startup-compare-v1-2026-10-01T13-15-27.021Z/result.json`。

## 原生关闭与内存验收进度

真实界面最初已创建一份约 280 MB 备份，但观测脚本试图包装不可写的 Tauri 内部接口，未获得任务起止证据，因此不计为关闭等待通过。后续改用外部 CDP Network 事件，只记录命令、时间与终态，不修改产品内部状态，不保存认证信息。三项只读 IPC 控制观测通过；管理器加载等待曾超过脚本 10 秒，失败保留且没有盲目重试备份或强制结束程序。

14:04:56 UTC 的新一轮实测观察到：待处理时点击“立即备份”共三次，仅产生一条原生 `create_backup`；20 组只读 IPC 与原生窗口 WM_NULL 探针均完成，窗口全部响应。在手动和自动任务都 pending 时，以真实 Input 编辑后 66 ms 发出普通 WM_CLOSE；自动任务约 11.91 秒、手动任务约 85.50 秒后成功，窗口在两者终态及管理数据刷新之后退出。全程无强杀。独立只读 SQLite 回读确认活动库和唯一新快照各 50,000 项、完整 integrity / foreign keys 通过，最后编辑值正确，新快照也包含该编辑。

该轮旧进程观测器未提前保留进程句柄，`exited=true` 但 `exitCode=null`，错误地报成超时；原始报告保留。现有证据证明 pending 终态后退出及最后编辑/快照正确，但不能推断退出码为 0。后续观测器修复后单独验证原始退出码，不将这一证据缺口藏在全通过结论中。原始目录为 `native-gui/manual-backup-async-cd220b2-2026-10-01T14-04-56-083Z/`；`sqlite-readback.json` 保留实际快照边界及退出码缺失。

候选的新 WebView profile 完成一段 120 秒闲置实测：60 轮，操作、runtime、console 和采样错误均为 0；Private Bytes 起点 400.05 MiB、峰值 422.67 MiB（+5.65%）、末值 365.71 MiB（-8.59%）。仅说明此短窗口没有持续增长，不替代规定的 10 分钟验收。正式验收将分别使用新进程/profile 测量闲置、固定小图切模式、固定 2,200×1,200 图切模式、新图预览和仅滚动缩略图，排除编译并行干扰。

历史连续操作 Private Bytes 末值 +25.24% 的 WARN 保留。不能用短闲置、JS heap 稳定或已解码截图替代真实进程内存门槛；正式结果完成后追加原始窗口、分进程样本与后半段增长。

阶段 A 仍在执行。模型首次准备/取消、全规模真实编码、启动及恢复预算、正式内存窗口未全部关闭前，不进入 `0.27.0` 或标记 `1.0.0` 完成。

## 隔离 Windows 安装升级

源码 `864a662` 的 [Windows installer workflow](https://github.com/AureliusWu/ImageLore/actions/runs/36867695535) 已 completed / success。同一 checkout 先通过六项检查，再构建当前 NSIS 与真实 `v0.19.0` 安装器；一次性 GitHub 托管 Windows runner 完成覆盖升级、唯一安装身份/版本/快捷方式检查、两次正常关闭及完整业务数据、备份和模型哨兵回读。该脚本在本机或自托管 runner 上拒绝执行清理；本地没有运行安装器或删除默认用户目录。

归档的候选 `ImageLore_0.26.0_x64-setup.exe` 为 13,638,019 字节，SHA-256 `BEF568DD756422DB0FE23B14E812A245F70BF235C79CC7AF43BA6AB300F014B3`，Authenticode 状态 `NotSigned`。这证明旧版本覆盖升级流程，通过 artifact 上传留存；GitHub Release 步骤 skipped，没有新增正式 release/tag。它不证明签名、updater、模型推理或新产品源码的后续修改已通过。
