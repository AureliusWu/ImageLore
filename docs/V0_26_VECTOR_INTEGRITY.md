# v0.26.0 语义向量完整性修复

日期：2026-10-02（Asia/Shanghai）。修复提交 `06469c209fb0f8bd9abbdde6232b2b219a60edd0`；版本仍为 `0.26.0`，schema 11、85 条命令及返回字段未变。

## 问题与实际复现

旧索引只核对 model/fingerprint，没有验证向量内容。空或短向量会成为有限的 -1 匹配；多余尾字节被丢弃；零、非单位向量可能参与排名；NaN/Inf 或 TEXT 类型会被漏掉但仍不进入重建。候选资产转换错误还被 `filter_map(Result::ok)` 静默忽略。

独立实验将 `de8367a` 的 semantic/search 两份源码投影到其余当前 crate，加入一个真实 Rust exact 测试。九种坏索引和一处资产 path 类型错误均跑完，修复及合法记录保留的正控制通过后，严格拒绝/重建断言按预期失败。所有九种旧索引的 SQL stale 都为 0、未被安排重建；资产转换错误也未传播。实验只使用合成内存库，没有模型、网络或正常用户库。

这是旧两文件实现的实际反例，不是完整旧提交 EXE 的原生结果。实验保留原字节备份、实际 Cargo exit 101、十项记录及唯一预期断言；随后两份修复稿均按原 SHA256 恢复，HEAD 未改变。没有将编译失败当作反例。

## 修复后的边界

- 向量必须为 2048 字节、512 个有限 float32、非零，单位范数误差严格小于 0.001。真实旧三图向量最大误差约 4.08e-7；容差容纳舍入，不容纳错误数据。
- 模型归一化先转 f64 再累加/相除，避免大但有限的 float32 输出溢出后变成零。无效输出不覆写旧索引，也不发布 text_ready=1。
- 文本 query、参考图及排名读取严格校验；当前允许候选中的坏索引明确提示重新建立索引，SQL/类型转换错误传播，不静默少结果。过滤或排除范围以外的坏行不阻塞当前合法候选。
- status stale 与增量 index_rows 使用同一逐行检查。内容损坏即使 fingerprint 未变，也进入重建；合法行保持。双方空 fingerprint 为 stale，missing/旧 fingerprint/model 原有规则保留。
- 借用当前 SQLite row 的 BLOB 并按行评分，不额外复制整库向量或构造无界坏 ID 参数列表。每行索引准备仍检查取消。

`indexed` 仍表示已存模型行数；可用性由准确 stale 与严格读取决定，没有偷偷改变既有字段语义。50k 的完整 freshness 检查最坏要逻辑读取约 97.7 MiB 向量，且持有 DB mutex；校验和评分的锁时间、原生响应及规模成本仍需新候选实际测量，不能声称没有性能代价。

## 验证与剩余门槛

原有六项门禁在旧稿反例恢复之后重跑通过：137 个 Rust 测试通过、3 个显式重型测试未默认执行，49 项 Node 测试通过，85 条契约一致，前端 build、fmt、locked check、lib Clippy `-D warnings` 和 diff check 通过。新增十项回归覆盖坏内容/类型、重建、合法行保留、坏输出不覆写、错误传播、范数容差、空 fingerprint 及排除范围。

同批修复还包括 `ab3a731` 的 Windows loopback 取消回归准备：明确 accepted socket 的阻塞模式，先完成 HTTP client 构造再开始测试服务器的有限等待；原 700ms 取消断言未放宽。此前远端 `de8367a` Native CI 实际失败。修复提交 `06469c2` 的 [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/36888966407) 与[前端 CI](https://github.com/AureliusWu/ImageLore/actions/runs/36888966553) 已实际完成并成功；不能据此把旧提交或新 EXE 的原生验收改为通过。

本机反例：`<private-evidence>\semantic-baseline-repro-20261001T155300-b84736e286684da083e92428e48c77ee`。恢复后的完整门禁：`<private-evidence>/stage-a-after-baseline-repro-gates-20261001.{json,txt}`。

上述门禁证明源码回归；尚不证明新 EXE 的真实模型、300→5k→50k 编码、取消/离线、自然图召回或 GUI 内存通过。新候选须绑定实际源码与 EXE SHA，按各项原生门槛继续执行。
