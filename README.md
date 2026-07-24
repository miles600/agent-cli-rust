# Agent CLI Rust

一个用 Rust 编写的命令行 AI Agent，支持工具调用、流式输出、子 Agent 委派等能力。

## 功能特性

- **多轮对话** — 会话内共享对话历史，支持 `reset` 清空记忆
- **SSE 流式输出** — LLM 回复逐 token 实时显示
- **工具调用（Function Calling）** — 8 个内置工具：
  | 工具 | 功能 | 安全等级 |
  |------|------|----------|
  | `get_weather` | 查询城市天气 | 安全 |
  | `calculator` | 数学表达式计算 | 安全 |
  | `get_time` | 获取当前时间 | 安全 |
  | `read_file` | 读取文件内容 | 安全 |
  | `fetch_webpage` | 抓取网页内容 | 安全 |
  | `write_file` | 写入文件 | ⚠️ 需确认 |
  | `run_shell` | 执行 Shell 命令 | ⚠️ 需确认 |
  | `delegate` | 委派子 Agent 执行独立任务 | 安全 |
- **危险工具确认** — `write_file`、`run_shell` 执行前需用户手动确认
- **失败重试与熔断** — 工具失败后 LLM 自动分析换策略，连续 3 次失败强制停止
- **对话历史裁剪** — 超过 50 条消息时按轮次边界自动裁剪，保留 system prompt + 最近消息
- **跨会话记忆** — 通过 `AGENTS.md` 文件持久化信息，启动时自动注入 system prompt
- **子 Agent 委派** — `delegate` 工具创建独立上下文的子 Agent，不污染主对话记忆

## 项目结构

```
src/
├── main.rs      # 入口：REPL 循环、命令处理、AGENTS.md 加载
├── agent.rs     # 核心：SSE 流式请求、tool_calls 循环、裁剪、熔断
├── config.rs    # 配置：读取 api_keys.yaml，解析 Provider
└── tools.rs     # 工具：定义 + 执行逻辑（8 个工具）
```

## 快速开始

### 1. 配置 API Key

在项目上级目录创建 `api_keys.yaml`：

```yaml
default: openai

providers:
  openai:
    url: https://api.openai.com/v1
    api_key: sk-xxx
    model: gpt-4o

  deepseek:
    url: https://api.deepseek.com/v1
    api_key: sk-xxx
    model: deepseek-chat
```

### 2. 运行

```bash
cargo run
```

### 3. 使用

```
👤 你: 北京今天天气怎么样？
🔧 调用工具: get_weather({"city": "北京"})
🤖 北京今天晴，气温 28°C...

👤 你: 帮我算 2^10 + sqrt(144)
🔧 调用工具: calculator({"expression": "2^10 + sqrt(144)"})
🤖 2^10 + sqrt(144) = 1036

👤 你: reset        ← 清空对话记忆
👤 你: quit         ← 退出
```

## 技术栈

| 依赖 | 用途 |
|------|------|
| `tokio` | 异步运行时 |
| `reqwest` | HTTP 客户端（支持 SSE 流） |
| `serde` / `serde_json` / `serde_yaml` | 序列化 |
| `rustyline` | 命令行输入增强（历史记录、行编辑） |
| `futures-util` | 异步流处理 |
| `meval` | 数学表达式求值 |
| `chrono` | 时间处理 |

## 构建

```bash
# 开发版
cargo build

# 发布版（优化后更快更小）
cargo build --release

# 运行
./target/debug/agent-cli-rust
./target/release/agent-cli-rust
```

## 环境变量

| 变量 | 作用 |
|------|------|
| `AGENT_PROVIDER` | 指定使用哪个 Provider（优先级高于 yaml 的 default） |
