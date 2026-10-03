# v0.26 直接导入的后台执行与关闭保护

2026-10-02状态：`2276de1`新EXE的derivative、Remix两项直接UI导入关闭闭环已实际PASS，包含真实OS WM_CLOSE、最新编辑/关系/Session/谱系持久化及独立fresh-profile重启回读。文件选择为明确披露的fixture picker bypass，`physicalPickerExercised=false`；真实物理选择框仍pending。源码门禁和已有模型功能/取消/缓存结果保持各自证据范围。

旧 `34baa87` 程序300图导入耗时2422.3ms，同一窗口在调用期间出现3次超过500ms的实际消息响应超时，详见[原始模型矩阵](V0_26_MODEL_MATRIX.md)。三个直接导入命令同步运行整个批次；本次新图没有生成缩略图，不能将停顿归因于缩略图。没有内部I/O/SQL分项计时，也不从总耗时推导其中各部分占比。

`import_paths`、`import_folder`、`import_dropped_paths` 改为async，在blocking pool内复用原导入函数，保留逐图事务、去重、结果和错误契约。普通批量导入仍沿用既有后台job流程；直接导入不注册无法由UI等待的隐藏job ID。

关闭保护覆盖三个App直接导入工作流，跟踪从首次flush/选择框到导入后的关系、Session、Remix谱系和最终刷新。原始Promise在其首个微任务前登记；关闭后不接受新入口或迟到选择框返回的新导入。已经开始持久写入的流程继续完成后续关系，避免导入完成却遗漏关联。关闭等待全部已登记操作结束后再抛原始失败；失败保留窗口、显示错误并允许重试。最后一次保存期间若输入继续变化，依据编辑epoch继续flush，取消后台任务结束后再保存。

所有App后台启动入口共用关闭守门，覆盖来源自动同步、Inbox焦点同步、手动同步和导入完成后的自动索引。关闭无条件调用两个hook的取消等待，利用既有starting ack/唯一终态契约，覆盖同一JS turn内刚启动、React active尚未渲染的任务。没有改动hook或模型取消实现。

原代码实际执行新增的前9项竞态测试时为0 passed / 9 failed；该轮只保留在工具输出，未另存原始文件日志，也未保留当时9项测试文件的精确SHA。不能用后续10项文件的SHA冒充旧采集。事后校准回执为 `<private-evidence>/import-close-audit-20261002.json`，明确标注此证据范围。

修复后的完整本地门禁由根任务重新采集到文件：

- `import-close-frontend-gates-20261002.log` 与 `import-close-frontend-gates-20261002-result.json`：85个frontend/registered/mock命令一致；59项Node测试，其中44项stability；快捷方式回归、格式、TypeScript均通过。
- `import-close-frontend-build-20261002.log` 与 `import-close-frontend-build-20261002-result.json`：正式前端构建通过，JS370.13kB、gzip112.40kB。
- `import-close-native-{fmt,check,clippy,test}-20261002.log` 与 `import-close-native-gates-20261002-result.json`：fmt、locked check、lib clippy `-D warnings`，默认库测试139 passed / 3 ignored。

新增10项行为回归涵盖首微任务前关闭、derivative/Remix迟到选择框、导入后关系/Session/刷新等待、两项并发中一项失败仍等待另一项、关闭后的自动索引和Inbox入口、启动确认尚未渲染、最后保存期间的新编辑。用例执行实际App/hook回调，时间与IPC由可控deferred依赖驱动；它们不是物理原生验收。

未增加依赖、变更schema或命令数量。修复不提供任意外部IPC或直接`window.destroy`的全局原生关闭协调。旧候选的模型与关闭证据不转记到新构建。

## 新产物的实际窗口消息与模型关闭复验

实际来源为`2276de179da95cf29a889a0a6e7d9a01d940ad8a`，EXE SHA256为`077027CDCE55A9FFCA55D18E00B84D50290FF876F9C708B721BC31BAE532B82B`，ProductVersion仍0.26.0。它已替换桌面current，旧34baa87保留，发布身份见[桌面验收](V0_26_DESKTOP_ACCEPTANCE.md)。

300图functional运行的`import_paths`原生IPC为1936.6ms。WM_NULL初始进程863次、完整重启6次均无timeout；导入observer上下文19个样本全部responsive、`timedOut=false`、Win32=0，max=0.5179ms。实际send-start/end第一条为北京时间10:57:16.326→16.326，末条为10:57:18.316→18.317；整数毫秒壁钟相同不表示高精度耗时为零。

这19次均位于可证明的observer外包区间10:57:16.250–10:57:18.318：前者为`currentOperation`导入观察起点，后者为下一`library_page`观察起点。导入精确观察结束时刻没有单独落盘。分组标签来自接收消息时的上下文，且在轮询结果、删除操作记录时仍保留；不可将所有19次硬写成发生在原生1936.6ms的handler执行区间。WM_NULL支持此次窗口消息处理响应的结论，不替代实际render/input或derivative/Remix操作体验。旧34baa87三次导入timeout的历史证据保持，不与新EXE混算。

同一新产物的functional最终恢复两个故意图片错误至300/300，snapshot、正常关闭、完整重启三阶段typed aggregate一致、verify均valid，两个WM_CLOSE确切exit0。text/similar各20warm p95为964.0/4.8ms，均满足3000ms预算。编码取消、编码期间关闭与失败代理缓存三个控制用例各只有3warm，全部仅作为smoke，`projectWarmGatePassed=false`；不据此补足性能样本。

编码期间关闭的完整观察请求→终态为2821ms，超过2秒。helper queued→实际OS WM_CLOSE为2569ms；OS WM_CLOSE→收到终态事件112ms、→观察终态138ms。后两项不冒充完整用户等待，helper排队不被隐藏。处理从请求观察时10项继续到47项后取消，正常退出0；部分库关闭和重启均读回47项，再真实编码253项至300，完整备份/关闭/重启一致，四次正常WM_CLOSE均确切exit0。UI编码取消的另一独立用例为113ms观察到终态，保持10项并恢复290项。单图ONNX和部分FS/SQLite操作不可抢占，不作无条件2秒保证。

失败代理缓存用例只给验收子进程设置回环失败proxy，保留可信已缓存组件，实际编码300项和持久核验通过。它不代表关闭物理网络，也不声明任意客户端都遵守proxy或没有网络包。

原始材料为`<private-evidence>/native-model-v1/runs/`下的`2026-10-02T02-57-03-885Z-300-functional-f3640808-be4e-4cec-907b-eda611ea9bda`、`2026-10-02T02-59-30-969Z-300-close-encoding-a61f2a7c-0d35-4fbf-82e9-d5b496cb58b1`、`2026-10-02T03-03-00-973Z-300-encoding-cancel-2aa68e41-8c85-42a8-9c6a-c1ffa53ab29a`、`2026-10-02T03-06-30-340Z-300-offline-cached-a5c9a740-1c8c-4278-87c5-574ee42ee0e8`目录；详细终态和后续下载取消见[模型矩阵](V0_26_MODEL_MATRIX.md)。

## 直接UI导入、正常关闭和fresh-profile重启实测

两case串行执行，实际source均为`2276de179da95cf29a889a0a6e7d9a01d940ad8a`，EXE SHA256均为`077027CDCE55A9FFCA55D18E00B84D50290FF876F9C708B721BC31BAE532B82B`。derivative的orchestrator原始工具输出为`9dbea7`、Remix为`57c17f`，两次CLI均exit0。每个case有独立marker资料库、两张不同32×32 PNG、真实原生parent Prompt/Session seed，随后通过App实际UI点击导入。seed步骤不作为UI工作流验收。

Tauri原`invoke`和`window.__TAURI_INTERNALS__`对象保持不变，仅对可写`window.fetch`安装薄传输wrapper。业务调用全部delegate原fetch，扣留的是取得的真实原Response，释放后仍由原Tauri回调继续App工作流；没有fake业务Summary、成功或错误。唯一synthetic响应是`plugin:dialog|open`返回本次精确single fixture path，`physicalPickerExercised=false`。额外close-event listener只记录事件，不preventDefault或destroy；通过要求App自己的关闭保护完成真实后续持久写入并正常退出，不能凭窗口仍在通过。

derivative扣留真实`import_paths`成功Response（added1、duplicates0、failed0、child ID2），取得时刻为03:21:15.126Z；关闭请求送达后实际输入最新parent Prompt，再释放Response。App原流程完成`add_relation → set_asset_session → refresh`。Remix通过实际UI保存draft ID1，只使用base source；扣留03:22:18.163Z取得的真实`apply_remix_lineage=true` Response，在关闭送达后输入最新Prompt，再释放并完成刷新。Remix闭库另确认空child Prompt继承真实已保存draft Prompt；不声称多参考DNA字段验收。

下表日期均为2026-10-02、时间均为UTC。OS发送、renderer收到close事件、原Response释放、owned child exit0观察和retained-handle/端口检查完成分开记录。helper准备/请求排队及端口确认时间不算产品阻塞或响应延迟；本case不提供新的性能统计。

| case / launch | PID | OS WM_CLOSE发送 | renderer close-event观察 | 原Response实际释放 | owned child exit0观察 | retained-handle/端口检查完成 |
| --- | ---: | --- | --- | --- | --- | --- |
| derivative / initial | 21900 | 03:21:16.805 | 03:21:16.812 | 03:21:16.876 | 03:21:16.993 | 03:21:17.592 |
| derivative / fresh-profile restart | 23240 | 03:21:28.818 | — | — | 03:21:28.867 | 03:21:29.323 |
| Remix / initial | 2300 | 03:22:19.582 | 03:22:19.588 | 03:22:19.626 | 03:22:19.709 | 03:22:20.163 |
| Remix / fresh-profile restart | 23460 | 03:22:31.587 | — | — | 03:22:31.648 | 03:22:32.131 |

四次关闭均由已核验PID/start/binary/loopback port的retained native handle确认正常exit0、`portReleased=true`、`forceKill=false`，所有helper正常exit0。首次关闭仍持有Response时没有退出；重启使用同candidate/data与不同profile、launch ID、PID/start。重启真实native `get_asset/lineage/asset_session`回读最新parent Prompt、child身份、derived_from与继承Session后再正常关闭。

关闭回读从未SQLite打开源DB。原DB/WAL/SHM全保留，通过独占只读源handle生成唯一owned raw bundle和新验证clone；仅clone用SQLite读取已提交WAL、query_only和严格checkpoint `(0,0,0)`，typed业务前后不变后再immutable验证。两轮均`sourceBeforeAfterPreserved=true`，源三者bytes/size/mtime before==after。初次及重启闭库的完整typed business/schema SHA一致：

| case | 初次与重启闭库同一typed SHA256 | 最终结果 |
| --- | --- | --- |
| derivative | `1c1af3a036d1ba21f7260b2253f42231d3f11c159e63e77a2b62cc6f86c37966` | `PASS_REAL_UI_DIRECT_IMPORT_CLOSE_AND_FRESH_PROFILE_RESTART` |
| Remix | `cd53bd50e88b44d31436443648c9991f20e1d75addb9870f9131d04115e4cc0c` | 同上 |

本地证据以仓库外的`<private-evidence>`为根，目录分别为`<private-evidence>/native-model-v1/runs/2026-10-02T03-21-04-091Z-direct-derivative-e9f92077-fc2c-41c6-8aab-7826af82a39f`和`<private-evidence>/native-model-v1/runs/2026-10-02T03-22-05-912Z-direct-remix-a3cdd80a-7e01-4cc5-94b9-bb29159717f3`。各目录保留`orchestration.json`、`result.json`、两个独立`launch-*.json`、`setup-ui-steps.jsonl`、`direct-import-ui-steps.jsonl`、两次`direct-import-readback-*.json`、原始driver/readback stdout与stderr，以及保全的raw bundle/验证clone。两case无错误、恢复释放、重试、强制退出或产品源码修改。

尚未关闭的边界为真实物理文件选择框、Remix多参考DNA、任意外部IPC或直接`window.destroy`的全局关闭协调，以及复杂图片长期使用；late-picker独立case未在本轮执行。受控picker UI闭环不转记为这些结果，不改动已有3warm smoke或20warm模型性能口径。
