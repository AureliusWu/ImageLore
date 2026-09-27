# ImageLore Roadmap

## v0.10.0 — Clean Core ✅
全新 SQLite 数据基线、规范化标签、FTS、Collection、Lineage。

## v0.11.0 — Chinese Desktop ✅
中文界面、Windows GUI subsystem、正式 NSIS 安装包。

## v0.12.0 — Native Drag & Drop ✅
原生图片/文件夹拖拽导入。

## v0.13.0 — Reliability & Flow ✅
无损自动保存、轻量图库记录、事务写入、导入解锁、CJK 搜索、模块拆分、CI 验证。

## v0.13.1 — Build Hygiene ✅
依赖锁定、快速 CI / Release 分离、并发取消、单次 NSIS 构建与可复现发布。

## v0.14.0 — Library Continuity ✅
- 自动 / 手动 Backup 与安全 Restore
- SHA-256 重复内容检测
- 后台导入进度与取消
- Tag / Collection 管理
- 全库可搜索 Parent Picker
- 更完整的 ComfyUI Metadata Adapter
- 本地 asset protocol 预览与缓存治理
- Migration v2

## v0.15.0 — Generation Intelligence
- 更多生成器 Metadata Adapter
- Sidecar v3 与基于 fingerprint/UUID 的跨库谱系恢复
- 图片并排 Compare
- Generation Session / Branch Notes
- Model alias / normalization
- 保存筛选与 Library Health 深化

## v1.0.0 — Stable Local Library
- Windows 代码签名与自动更新
- 启动故障恢复与 rotating logs
- 大图库 50k+ 性能验收
- CJK n-gram / trigram 搜索索引
- 正式 Backup / Restore 灾难恢复验收
- 稳定迁移兼容性测试矩阵
