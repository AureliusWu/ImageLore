# v0.26 直接导入的后台执行与关闭保护

2026-10-02状态：源码门禁通过，新EXE原生复验待执行。

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

未增加依赖、变更schema或命令数量。修复不提供任意外部IPC或直接`window.destroy`的全局原生关闭协调。新产物仍须实际验证导入期间窗口响应、工作流正常WM_CLOSE退出0及重启读回；旧候选的模型与关闭证据不转记到新构建。
