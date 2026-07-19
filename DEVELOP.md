# AI Session Web — 项目设计文档

## 概述

`ai-session-web` 是一个 Web UI 工具，用于管理本地 AI CLI 工具（Jcode、Codex、Continue）的会话文件。它扫描指定目录中的会话数据文件，提供列表浏览、JSON 查看、文件删除等功能。

技术栈：Rust（Axum + Askama 模板）+ 原生 JavaScript/CSS（无前端框架）。

---

## 架构分层

```
src/
├── main.rs          # 程序入口：配置加载、日志、启动 HTTP 服务
├── config.rs        # CLI 参数解析、配置合并
├── error.rs         # 统一错误类型 & Axum IntoResponse 适配
├── api/
│   ├── mod.rs       # Axum Router 构建、AppState 定义
│   ├── handlers.rs  # 三个 REST handler
│   └── response.rs  # ApiResponse<T> 统一 JSON 包裹
├── session/
│   ├── mod.rs       # SessionMeta、SessionSource、mtime 工具函数
│   ├── jcode.rs     # Jcode .json 解析器
│   ├── jcode_journal.rs  # Jcode .journal.jsonl 解析器
│   ├── codex.rs     # Codex .jsonl 解析器
│   ├── continue_.rs # Continue .json 解析器
│   └── registry.rs  # SessionRegistry：扫描、排序、删除
└── web/
    ├── mod.rs       # Askama 模板渲染 + 静态资源路由
    └── assets/
        ├── app.js   # 前端逻辑（渲染、事件、模态框）
        └── style.css
templates/
├── index.html       # 主页面骨架
└── config.html      # 配置信息页面
```

### 层间依赖

```
main.rs
  ├── config.rs      → session::SessionSource
  ├── error.rs
  ├── session/registry.rs → config::CliDir, session::mod
  └── api/mod.rs
        ├── web/mod.rs
        └── api/handlers.rs → session::registry (responses), session::SessionSource
```

---

## 数据流

```
磁盘目录
    │
    ▼
SessionRegistry::scan(cli_dirs)
    ├── 遍历每个 CliDir（按 --cli-dirs 顺序）
    │   └── WalkDir 扫描文件 → 匹配过滤器 → 调用对应解析器
    ├── jcode::parse_jcode(path)        # .json + .journal.jsonl 合并
    ├── codex::parse_codex(path)        # .jsonl
    └── continue_::parse_continue(path)  # .json
    │
    ▼
HashMap<"source:session_id", SessionMeta>
    │
    ▼
Registry::sorted_list() → Vec<SortedSessionGroup>
    ├── 按 cli_dirs 顺序分 CLI 类型
    │   ├── main_group: 主会话 + 关联临时会话
    │   ├── unlinked_temp: 其他临时会话
    │   └── broken: 残缺会话
    │
    ▼
JSON API → 前端渲染
```

---

## 核心数据结构

### `SessionMeta`（`session/mod.rs`）

每个会话的完整元数据，由各解析器填充：

| 字段 | 类型 | 说明 |
|---|---|---|
| `source` | `SessionSource` | Jcode / Codex / Continue |
| `session_id` | `String` | 文件名去掉扩展名 |
| `title` | `String` | JSON 中的 `title` 字段 |
| `name` | `String` | 显示名称（fallback: custom_title → short_name → name） |
| `has_custom_title` | `bool` | 仅当 JSON 存在非空 `custom_title` 时为 true |
| `total_messages` / `user_messages` / `ai_messages` | `usize` | 消息统计（Jcode 会合并 journal 消息） |
| `created_at` / `updated_at` | `String` | 从内容字段读取 |
| `effective_updated_at` | `String` | **排序用**：max(内容 updated_at, 所有关联文件 mtime)，ISO 8601 |
| `working_dir` | `String` | 工作目录路径 |
| `provider` | `String` | 模型提供商 |
| `file_path` | `PathBuf` | 主数据文件路径 |
| `associated_files` | `Vec<PathBuf>` | 删除时需要一并清除的关联文件 |

### `CliDir`（`config.rs`）

```rust
pub struct CliDir {
    pub path: PathBuf,
    pub cli_type: SessionSource,  // 从路径字符串中的关键字推断
}
```

---

## 解析器

### Jcode 解析器（`jcode.rs` + `jcode_journal.rs`）

Jcode 使用**双文件存储**：
- **`.json`**: 主文件，含 `messages[]`、`custom_title`、`created_at`、`updated_at`、`working_dir`、`provider_key`
- **`.journal.jsonl`**: 增量日志文件，每行一个 JSON，包含 `session_meta`（最新 cwd）、`response_item`（user/assistant 消息）、`env_snapshot`

解析流程：
1. 读取 `.json`，提取基本字段 + 消息计数
2. 如果 `.journal.jsonl` 存在，增量合并：消息数叠加，`updated_at` 取较大值，`working_dir` 覆盖
3. `has_custom_title` = `custom_title` 字段存在且非空
4. `effective_updated_at` = max(内容 updated_at, .json mtime, .journal.jsonl mtime)
5. 关联文件列表：`.json` + `.bak`（存在时）+ `.journal.jsonl`（存在时）

### Codex 解析器（`codex.rs`）

- 文件格式：`.jsonl`，每行一个 JSON 事件
- 解析 `session_meta` 事件获取 id/timestamp/cwd/provider
- 解析 `response_item`/`message` 事件计数 user/assistant
- `effective_updated_at` = max(内容 created_at, .jsonl mtime)
- 无 custom_title → `has_custom_title = false`

### Continue 解析器（`continue_.rs`）

- 文件格式：`.json`，单文件
- 解析 `history[]` 数组中的 `message.role` 进行计数
- 无 created_at/updated_at 内容字段 → 直接用 .json 的 mtime
- `effective_updated_at` = .json mtime

---

## 排序算法（`registry.rs` — `sorted_list()`）

```
输入：所有已解析的 SessionMeta
输出：Vec<SortedSessionGroup>，按 --cli-dirs 顺序排列

每个 CLI 类型分组内：
  1. 按 has_custom_title 分桶 → 主会话 / 临时会话
  2. 临时会话进一步分桶：
     - 关联临时：working_dir 匹配某个主会话的 working_dir
     - 其他临时：不匹配
  3. 主会话按 effective_updated_at 降序排列
  4. 每个主会话 → 其关联临时（降序）→ 下一主会话 → 其关联临时 → ...
  5. 其他临时 → 残缺会话
```

### API 响应结构

```json
[{
  "source": "jcode",
  "sections": [
    {
      "sectionType": "main_group",
      "title": "主会话名称",
      "mainSession": { "session_id": "...", "name": "..." },
      "sessions": [ /* 关联临时会话 */ ]
    },
    {
      "sectionType": "unlinked_temp",
      "title": "其他临时会话",
      "sessions": [ ... ]
    },
    {
      "sectionType": "broken",
      "title": "残缺会话",
      "broken": [ { "session_id": "...", "filePath": "..." } ]
    }
  ]
}]
```

---

## 删除逻辑

`registry.delete(source, session_id)`：

1. 从 `sessions` 和 `by_source` 中移除条目
2. 扫描 `file_path.parent()` 目录（max_depth=1）
3. 匹配条件：文件名包含 session_id 且扩展名在 `{json, jsonl, bak}` 中
4. 删除所有匹配文件

---

## 前端架构

纯原生 JavaScript + CSS，无框架依赖。

### 渲染流程

```
refreshList()
  ├── fetchSessions() → GET /api/sessions
  ├── 渲染 source-group（每个 CLI 类型）
  │   ├── main_group: 主会话行（蓝色高亮）+ 关联临时（缩进 40px）
  │   ├── unlinked_temp: 普通列表面
  │   └── broken: 残缺会话（删除线样式 + 灰色）
  └── attachEventListeners(container) — 事件委托
```

### 交互功能

| 功能 | 实现 |
|---|---|
| **查看 JSON** | 请求 `/api/sessions/{source}/{id}/json`，弹出模态框显示格式化 JSON |
| **删除会话** | 两步确认：主会话需二次确认（红字警告 "此操作不可恢复"） |
| **关闭模态框** | 点击 overlay / Escape 键 |
| **时间显示** | ISO 8601 → `toLocaleString('zh-CN')` 格式化 |

### 样式要点

- 主会话行：蓝色背景 `#f0f4ff` + 底部蓝色边框 `#4a6cf7`
- 关联临时：左缩进 40px，浅灰背景
- 残缺会话：`opacity: 0.7`，session_id 使用 `<s>` 删除线
- 响应式：768px 以下列变为垂直堆叠

---

## 删除确认流程

```
点击删除按钮
    │
    ▼
confirmDelete(source, id, isMain)
    │
    ├── isMain = false ──→ 普通确认框（⚠️ 确认删除）
    │                       └── 点击确认 → executeDelete()
    │
    └── isMain = true  ──→ 红色警告框（⚠️ 警告：删除主会话）
                             └── 点击确认
                                   │
                                   ▼
                              executeDelete() 进入二次确认
                                   │
                                   ├── 点击「返回」→ cancelSecondConfirm()
                                   └── 点击「确认删除」→ 真正执行删除
```

---

## 配置加载流程

```
Cli::parse()
    │
    ▼
AppConfig::from(&cli)
    ├── --cli-dirs 参数（空格或逗号分隔）
    │   └── infer_cli_type() 从路径关键字推断类型
    │       ├── 含 "jcode"    → SessionSource::Jcode
    │       ├── 含 "codex"    → SessionSource::Codex
    │       └── 含 "continue" → SessionSource::Continue
    │
    ▼
ConfigLoader::load(cli_config, defaults)
    └── cli_config 覆盖 defaults（仅非空值）
        └── 默认值：~/.jcode/sessions, ~/.codex/sessions, ~/.continue/sessions
```

---

## 文件格式速查

### Jcode `.json`
```json
{
  "messages": [{"role":"user|assistant","content":"..."}],
  "custom_title": "主会话名称",
  "title": "自动标题",
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-02T00:00:00Z",
  "working_dir": "/path/to/project",
  "provider_key": "anthropic"
}
```

### Jcode `.journal.jsonl`
```
{"type":"session_meta","payload":{"id":"...","timestamp":"...","cwd":"..."}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"..."}}
{"type":"env_snapshot","payload":{"captured_at":"..."}}
```

### Codex `.jsonl`
```
{"type":"session_meta","payload":{"id":"...","timestamp":"...","cwd":"...","model_provider":"..."}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"..."}}
```

### Continue `.json`
```json
{
  "title": "...",
  "workspaceDirectory": "/path",
  "history": [
    {"message": {"role": "user|assistant", "content": "..."}}
  ]
}
```

---

## 测试策略

16 个单元测试覆盖：

| 模块 | 测试 | 验证点 |
|---|---|---|
| `config` | CLI 解析（空格/逗号）、类型推断、默认值 | 3 tests |
| `jcode` | 纯 json、json+journal 合并、bak 文件关联 | 3 tests |
| `codex` | 标准解析、容忍无效行 | 2 tests |
| `continue_` | 标准解析 | 1 test |
| `jcode_journal` | 空文件、消息解析、无效行跳过 | 3 tests |
| `registry` | 多目录扫描、排序输出结构、目录扫描删除 | 3 tests |

---

## 本地开发

```bash
# 构建
cargo build

# 运行（指定目录）
cargo run -- --cli-dirs "C:\Users\me\.jcode\sessions" "C:\Users\me\.codex\sessions"

# 运行（默认端口 8100）
cargo run

# 测试
cargo test

# 发布构建
cargo build --release
```
