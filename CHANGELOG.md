# Changelog

## v0.1.0 (2026-07-19)

首个可用版本。

### 功能

- 支持 Jcode（`.json` + `.journal.jsonl` 双文件合并）、Codex（`.jsonl`）、Continue（`.json`）三种会话
- CLI 目录自动识别，`--cli-dirs` 参数（空格/逗号分隔）
- 智能排序：主会话（custom_title）→ 关联临时会话 → 其他临时 → 残缺
- 消息统计（👤 🤖 💬）+ 提供商标签
- 主会话二次确认删除保护
- 删除时扫描目录清除所有关联文件
- JSON 查看（含 journal 合并）
- 配置页面 `/config`
- 文件日志 + 控制台日志

### 技术细节

- Rust + Axum 0.8 + Askama 0.12
- 纯原生 JavaScript/CSS 前端
- 16 个单元测试
- Release 构建 2.4 MB（LTO + strip）
