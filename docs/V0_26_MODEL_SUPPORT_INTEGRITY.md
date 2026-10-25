# 固定模型配套文件的完整性校验

记录日期：2026-10-25；应用版本仍为 `0.26.0`，schema 仍为 11。本次修复不改变模型、权重、固定 revision 或既有向量索引的身份。

## 缺口与修复

原实现完整核对 ONNX 权重的大小和 SHA-256，但配套 JSON 只检查大小与 JSON 语法。固定权重与不属于该 revision 的有效 JSON 可以通过缓存检查，避免触发修复；初始化时也会再次读取未经摘要绑定的配套文件。这可能导致初始化失败或兼容但不正确的预处理/分词行为。本次没有发现或改写实际用户的模型缓存，不据此断言既有检索结果已经受到影响。

修复将五个配套文件的 SHA-256 固定在 `semantic.rs`，缓存检查、暂存下载验收和模型初始化均使用同一读取函数。传给预处理器与 tokenizer 的就是经过检查的内存字节，避免校验与使用之间再次读取配套文件。大小上限、JSON 语法、取消边界及原有原子替换策略继续生效。

新安装的审计清单使用 `manifest_version: 2`，补充配套文件路径和摘要；清单本身不作为信任依据。原有 v1 清单的缓存只要实际文件匹配，仍走离线分支，不下载、不改写文件或 mtime。损坏缓存离线修复失败时保留旧文件；重新联网后只有完整核验通过的暂存内容才能替换。不会因为清单版本变化清空或重建已有索引。

## 固定来源与实际字节

视觉来源为 [Qdrant/clip-ViT-B-32-vision 的固定 revision](https://huggingface.co/Qdrant/clip-ViT-B-32-vision/tree/e0c24ed0fa57fa3e4f97f30de74c51d944036ace)，文本来源为 [Qdrant/clip-ViT-B-32-text 的固定 revision](https://huggingface.co/Qdrant/clip-ViT-B-32-text/tree/48ca1db27cb4063eb311ec2aa7f087a808112876)。本轮匿名取得五个配套文件，并将响应 revision、Git blob SHA-1、大小及保存字节的 SHA-256 相互核对。

| 组件 / 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| vision / `preprocessor_config.json` | 780 | `ce945ef831c9972c135b5b198a03d8eeb70478cd69c0238f24caf1903a9965e6` |
| text / `tokenizer.json` | 2,224,147 | `b68d571997a1f81bf521fb73806740ddb91e4ed6666cb6e996c066bb289cf55b` |
| text / `config.json` | 536 | `4d5923d94bbc4e29864de837df14c138c12b93a0b738c64f3ca41f0c539e17b7` |
| text / `special_tokens_map.json` | 588 | `2cdb3b8331a60c92fc1e55a13e9fd61fd2293c5a51275fdcccd62b780052530e` |
| text / `tokenizer_config.json` | 705 | `6bdcee9ccce2a16ca2b4c0c5ed00b42c50ea225f4472a8c4c1e963a2902c2881` |

两个固定 model card 均声明 MIT；本轮还保存了 OpenAI CLIP 的许可原文与模型说明作为上游背景。这不证明 Qdrant ONNX 转换所用的完整上游 revision 或重建过程，完整分发归属仍单独审查。此次没有下载 ONNX 权重，权重 LFS 元数据与应用已固定的大小/摘要一致不算实际权重下载验证。

## 实际回归与范围

先只加入回归，再使用原生产检查器编译运行：三字节合成 ONNX `abc` 与匹配的测试权重摘要，加上不属于视觉固定 revision 的有效 `{}` JSON。原检查器接受它，新增拒绝断言实际失败。第一次构建曾因 ort-sys 离线链接 stub 缺少 `OrtGetApiBase` 而失败，该失败单独保留，不作为缺口复现；修正原生缓存选择后才得到有效 RED。

修复后的归档原生库实际通过：

- 同一缺口回归 **1 passed / 0 failed**。
- 完整、未过滤库测试 **239 passed / 0 failed / 3 ignored / 0 filtered**，87.67 秒；三个既有忽略项未改动。
- 健康 v1 缓存不访问网络，权重/配套/清单字节及 mtime 保持；有效 JSON 被替换后离线拒绝并保全，后续验证下载修复成功。
- 现有取消/错误权重/坏 JSON/网络故障保全测试补充配套摘要不符的有效 JSON 路径，其他暂存目录保持。
- `cargo fmt --check` 与 optimized、locked、offline 的库 Clippy `-D warnings` 通过，编译和测试期间全部已跟踪源码身份保持。

上述原生测试使用独立临时目录和合成小文件，不构造真实模型推理结论。实际五份公开文件的字节核对与合成故障回归分别记录。该结果不关闭新源码的真实模型推理、GUI 导入/重启、完整启动与检索性能、长期内存或 v1.0.0 发布门槛；UI 中持久化的“已就绪”也不等于每次展示时重新完成文件摘要检查。

## 同提交远端验证

修复提交 `152d2a4f84fb6a039750b59f010b202e6ae22cde` 的 [Native CI](https://github.com/AureliusWu/ImageLore/actions/runs/40180556173) 实际成功，原始日志确认 **239 passed / 0 failed / 3 ignored / 0 filtered**，1114.32 秒；同一运行的 fmt/check/库 Clippy 均通过。[快速 CI](https://github.com/AureliusWu/ImageLore/actions/runs/40180558767)与[敏感信息门禁](https://github.com/AureliusWu/ImageLore/actions/runs/40180555361)也成功。本地源码与提交 blob 的 LF 归一后身份一致，原始哈希分别保存；测试后仅补充文档，没有把观察时 HEAD 代替编译源码。

后续前端许可材料与防护提交 `7b79d14` 没有更改应用的 `src`、`src-tauri`、扩展、版本或产品锁文件；它自身的快速 CI/敏感门禁另行通过，不把上述 Native 运行改记为其他提交的运行。版本仍为 `0.26.0`，桌面 current 未推广。

原始日志、失败现场、公开输入副本及测试二进制全部保存在仓库外；本机没有启动应用或操作用户资料库。真实模型、GUI、性能及完整发布门槛仍按前述范围分别验收。
