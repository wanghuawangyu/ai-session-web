<div align="center">

# AI Session Web

**浏览、管理本地 AI CLI 会话文件** · Jcode · Codex · Continue

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-dea584?logo=rust)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/wanghuawangyu/ai-session-web?logo=github)](https://github.com/wanghuawangyu/ai-session-web/releases)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](#license)

单文件 Web UI，零数据库依赖，纯原生前端。

</div>

---

## 概述

AI Session Web 是一个轻量级 Web 工具，用于管理本地 AI 编程助手的会话文件。它自动扫描指定目录中的会话数据，提供友好的 Web 界面进行浏览、查看和删除操作。

**支持的 CLI 工具**：

| 来源 | 文件格式 | 特性 |
|---|---|---|
| **Jcode** | `.json` + `.journal.jsonl` | 双文件合并、custom_title、增量日志 |
| **Codex** | `.jsonl` | 单文件、容忍无效行 |
| **Continue** | `.json` | 单文件、history 解析 |

---

## 截图预览

```
┌─ Jcode Sessions ────────────────────────── 42 sessions ─┐
│  [蓝色高亮] 项目A开发 · 👤 304 / 🤖 249 / 💬 553       │
│    ┌─ 关联临时会话 ──────────────────────────────────┐  │
│    │  临时会话1      · 👤 1 / 🤖 0 / 💬 1           │  │
│    │  临时会话2      · 👤 2 / 🤖 1 / 💬 3           │  │
│    └─────────────────────────────────────────────────┘  │
│  [蓝色高亮] 项目B开发 · 👤 56 / 🤖 47 / 💬 103        │
│    ┌─ 关联临时会话 ──────────────────────────────────┐  │
│    │  临时会话3      · 👤 1 / 🤖 0 / 💬 1           │  │
│    └─────────────────────────────────────────────────┘  │
│  其他临时会话 · 👤 3 / 🤖 1 / 💬 4                     │
│  ~~损坏数据~~ (残缺会话，无法解析)                       │
└─────────────────────────────────────────────────────────┘
```

---

## 快速开始

### 下载

从 [Releases](https://github.com/wanghuawangyu/ai-session-web/releases) 页面下载对应平台的压缩包。

### 安装

#### Windows (x86_64)

```powershell
# 1. 下载（自动获取最新版本）
curl -sSL -o ai-session-web.zip https://github.com/wanghuawangyu/ai-session-web/releases/latest/download/ai-session-web-x86_64-pc-windows-gnu.zip

# 2. 解压
tar -xf ai-session-web.zip

# 3. （可选）将目录加入系统 PATH
#    或直接移动到常用位置：
#    move ai-session-web.exe C:\Tools\
```

或手动下载：

1. 前往 [Releases 页面](https://github.com/wanghuawangyu/ai-session-web/releases)
2. 下载最新版本的 `ai-session-web-x86_64-pc-windows-gnu.zip`
3. 解压到任意目录（如 `C:\Tools\ai-session-web\`）
4. 将该目录添加到系统环境变量 `PATH` 中，或直接运行

#### Linux (x86_64 / ARM64)

```bash
# 下载并解压到 /usr/local/bin（自动获取最新版本）
curl -sSL https://github.com/wanghuawangyu/ai-session-web/releases/latest/download/ai-session-web-x86_64-unknown-linux-musl.tar.gz \
  | sudo tar xz -C /usr/local/bin

# ARM64 用户请替换为：
# ai-session-web-aarch64-unknown-linux-musl.tar.gz

# 验证安装
ai-session-web --version
```

> 上述链接自动跳转到最新版本，无需手动指定版本号。

### 运行

```bash
# 使用默认目录（自动检测 ~/.jcode/sessions 等）
ai-session-web.exe

# 指定自定义目录
ai-session-web.exe --cli-dirs "D:\data\.jcode\sessions" "D:\data\.codex\sessions"

# 逗号分隔
ai-session-web.exe --cli-dirs "D:\data\.jcode\sessions,D:\data\.codex\sessions"

# 自定义端口和日志
ai-session-web.exe --port 8080 --log ./app.log --log-level debug
```

打开浏览器访问 **http://127.0.0.1:8100**

### 使用默认参数启动

如果不传任何参数，程序会自动查找以下目录：

| 平台 | 路径 |
|---|---|
| Windows | `%USERPROFILE%\.jcode\sessions` |
| Windows | `%USERPROFILE%\.codex\sessions` |
| Windows | `%USERPROFILE%\.continue\sessions` |
| Linux/macOS | `$HOME/.jcode/sessions`（等） |

---

## 核心功能

### 📋 会话管理

- **自动扫描** — 递归扫描指定目录，自动识别来源类型
- **消息统计** — 显示 user / assistant / total 消息数
- **提供商标签** — 紫色标签显示模型提供商（如 anthropic、openai）
- **时间轴** — 创建时间和最后修改时间
- **工作目录** — 显示每个会话对应的项目路径

### 🧠 智能排序

会话按以下层次排列：

1. **主会话**（有 `custom_title`）— 按最后修改时间降序
2. **关联临时会话** — 跟随主会话，缩进显示（同一工作目录）
3. **其他临时会话** — 独立会话
4. **残缺会话** — 文件存在但无法解析，用 ~~删除线~~ 标记

> 排序顺序严格遵循 `--cli-dirs` 参数的传入顺序。

### 🗑️ 删除操作

- 每个会话均有独立删除按钮
- 删除时扫描目录，清除所有关联文件（`.json`、`.jsonl`、`.bak`、`.journal.jsonl`）
- **主会话二次确认**：删除有 custom_title 的会话需两次确认，防止误操作

### 🔍 JSON 查看

- 点击"查看"弹出模态框，显示格式化的会话原始 JSON
- Jcode 会话自动合并 `.journal.jsonl` 中的消息

### ⚙️ 配置页面

访问 `http://127.0.0.1:8100/config` 查看当前运行时配置。

---

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/api/sessions` | 获取排序后的会话列表 |
| `DELETE` | `/api/sessions/{source}/{session_id}` | 删除会话 |
| `GET` | `/api/sessions/{source}/{session_id}/json` | 获取会话原始 JSON |

---

## 本地开发

```bash
# 构建
cargo build

# 测试（16 个测试）
cargo test

# 发布构建（LTO + strip）
cargo build --release

# 启动开发
cargo run -- --cli-dirs "路径1" "路径2"
```

### 技术栈

| 层 | 技术 |
|---|---|
| 后端框架 | Axum 0.8 |
| 模板引擎 | Askama 0.12 |
| 异步运行时 | Tokio |
| CLI 解析 | Clap 4 |
| 日志 | Tracing + Tracing Subscriber |
| 序列化 | Serde / Serde JSON |
| 文件扫描 | Walkdir |
| 时间处理 | Chrono |
| 前端 | 原生 JavaScript + CSS3（零框架） |

---

## 项目结构

```
src/
├── main.rs          # 入口：配置、日志、启动
├── config.rs        # CLI 参数解析、配置合并
├── error.rs         # 统一错误处理
├── api/
│   ├── mod.rs       # Router + AppState
│   ├── handlers.rs  # REST handler
│   └── response.rs  # 统一 JSON 响应
├── session/
│   ├── mod.rs       # 核心数据结构、mtime 工具
│   ├── jcode.rs     # Jcode 解析器
│   ├── jcode_journal.rs  # Jcode 增量日志解析
│   ├── codex.rs     # Codex 解析器
│   ├── continue_.rs # Continue 解析器
│   └── registry.rs  # 注册表、排序、删除
└── web/
    ├── mod.rs       # 模板渲染 + 静态资源
    └── assets/
        ├── app.js
        └── style.css
templates/
├── index.html
└── config.html
```

---

## 许可证

本项目采用 MIT OR Apache-2.0 双许可证。

---

<div align="center">
  <sub>Built with Rust · Axum · 原生 JS</sub>
</div>
