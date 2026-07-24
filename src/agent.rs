use crate::config::AgentConfig;
use crate::tools;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::io::Write;

pub type Messages = Vec<Value>;

const MAX_TURNS: usize = 30;

/// 消息数上限，超过时自动裁剪
const MAX_MESSAGES: usize = 50;
/// 每次裁剪的目标条数
const TRIM_COUNT: usize = 20;

/// 裁剪对话历史：保留 system prompt + 最近消息，按轮次边界裁剪
fn trim_messages(messages: &mut Messages) {
    if messages.len() <= MAX_MESSAGES {
        return;
    }

    let total = messages.len();
    // 目标：从 messages[1] 开始裁掉约 TRIM_COUNT 条
    let mut cut_end = 1 + TRIM_COUNT.min(total - 2); // 至少保留最后 1 条

    // 按轮次边界调整：不要在 tool 消息中间切断
    // 确保 cut_end 落在 "user" 消息的开头（一轮对话的起点）
    while cut_end < total {
        let role = messages[cut_end]["role"].as_str().unwrap_or("");
        if role == "user" {
            break; // 找到轮次边界
        }
        cut_end += 1; // 跳过 assistant/tool 消息，继续往后找
    }

    let trimmed_count = cut_end - 1; // 不含 system prompt
    if trimmed_count == 0 {
        return;
    }

    println!("\x1b[2m✂️  对话历史过长({} 条)，已裁剪 {} 条旧消息\x1b[0m", total, trimmed_count);

    let system = messages[0].clone();
    let recent: Vec<Value> = messages[cut_end..].to_vec();
    *messages = vec![system];
    messages.extend(recent);
}

/// 危险工具列表，执行前需要用户确认
const DANGEROUS_TOOLS: &[&str] = &["write_file", "run_shell"];

/// 询问用户是否确认执行
fn confirm_execution(tool_name: &str, args_str: &str) -> bool {
    print!("\x1b[31m⚠️  危险操作: {}({})\x1b[0m", tool_name, args_str);
    print!("\n\x1b[31m   确认执行？[y/N]: \x1b[0m");
    std::io::stdout().flush().unwrap_or(());

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or(0);
    let answer = input.trim().to_lowercase();
    answer == "y" || answer == "yes"
}

/// SSE 流式请求的结果
enum StreamResult {
    /// 普通文本回复
    Content(String),
    /// 工具调用
    ToolCalls(Vec<Value>),
}

/// 发送流式请求，逐 token 打印，返回完整结果
async fn stream_request(
    client: &reqwest::Client,
    config: &AgentConfig,
    messages: &Messages,
    tool_defs: &Value,
) -> Result<StreamResult, String> {
    let url = format!("{}/chat/completions", config.base_url);
    let body = json!({
        "model": config.model,
        "messages": messages,
        "tools": tool_defs,
        "stream": true,
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回 {}: {}", status, text));
    }

    let mut content = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    let mut is_tool_call = false;

    // 逐行读取 SSE 流
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("流读取失败: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // 按行处理
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            // SSE 格式: "data: {...}" 或 "data: [DONE]"
            if !line.starts_with("data: ") {
                continue;
            }
            let data_str = &line[6..];
            if data_str == "[DONE]" {
                break;
            }

            let data: Value = match serde_json::from_str(data_str) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let delta = &data["choices"][0]["delta"];

            // 处理文本内容
            if let Some(text) = delta["content"].as_str() {
                if !is_tool_call {
                    print!("\x1b[34m{}\x1b[0m", text);
                    std::io::stdout().flush().unwrap_or(());
                }
                content.push_str(text);
            }

            // 处理 tool_calls（流式中分片到达）
            if let Some(calls) = delta["tool_calls"].as_array() {
                is_tool_call = true;
                for call in calls {
                    let idx = call["index"].as_u64().unwrap_or(0) as usize;

                    // 确保 tool_calls 向量足够长
                    while tool_calls.len() <= idx {
                        tool_calls.push(json!({
                            "id": "",
                            "type": "function",
                            "function": { "name": "", "arguments": "" }
                        }));
                    }

                    // 合并 id
                    if let Some(id) = call["id"].as_str() {
                        if !id.is_empty() {
                            tool_calls[idx]["id"] = json!(id);
                        }
                    }
                    // 合并 function name
                    if let Some(name) = call["function"]["name"].as_str() {
                        if !name.is_empty() {
                            let existing = tool_calls[idx]["function"]["name"]
                                .as_str().unwrap_or("").to_string();
                            tool_calls[idx]["function"]["name"] = json!(existing + name);
                        }
                    }
                    // 合并 arguments（分片拼接）
                    if let Some(args) = call["function"]["arguments"].as_str() {
                        let existing = tool_calls[idx]["function"]["arguments"]
                            .as_str().unwrap_or("").to_string();
                        tool_calls[idx]["function"]["arguments"] = json!(existing + args);
                    }
                }
            }
        }
    }

    if is_tool_call {
        Ok(StreamResult::ToolCalls(tool_calls))
    } else {
        println!(); // 流式输出后换行
        Ok(StreamResult::Content(content))
    }
}

pub async fn run_agent_loop(
    client: &reqwest::Client,
    config: &AgentConfig,
    messages: &mut Messages,
) -> Result<String, String> {
    let tool_defs = tools::get_tool_definitions();

    for turn in 0..MAX_TURNS {
        // 发送前检查消息数，超限则裁剪
        trim_messages(messages);

        println!("\x1b[2m\n--- 循环 第 {} 轮 ---\x1b[0m", turn + 1);
        println!("\x1b[36m📡 发送请求到 LLM...\x1b[0m");
        print!("\x1b[34m🤖 \x1b[0m");
        std::io::stdout().flush().unwrap_or(());

        let result = stream_request(client, config, messages, &tool_defs).await?;

        match result {
            StreamResult::ToolCalls(tool_calls) => {
                // 构建 assistant 消息（含 tool_calls）
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": tool_calls,
                }));

                // 逐个执行工具
                for tool_call in &tool_calls {
                    let call_id = tool_call["id"].as_str().unwrap_or("");
                    let func_name = tool_call["function"]["name"].as_str().unwrap_or("");
                    let args_str = tool_call["function"]["arguments"].as_str().unwrap_or("{}");
                    let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));

                    println!("\x1b[33m🔧 调用工具: {}({})\x1b[0m", func_name, args_str);

                    // 危险工具需要用户确认
                    let exec_result = if DANGEROUS_TOOLS.contains(&func_name) {
                        if confirm_execution(func_name, args_str) {
                            tools::execute_tool(client, func_name, &args).await
                        } else {
                            println!("\x1b[31m   ✖ 用户拒绝执行\x1b[0m");
                            format!("[用户拒绝了 {} 的执行]", func_name)
                        }
                    } else {
                        tools::execute_tool(client, func_name, &args).await
                    };

                    println!("\x1b[33m   ↳ 结果: {}\x1b[0m",
                        if exec_result.len() > 200 { &exec_result[..200] } else { &exec_result });

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": exec_result,
                    }));
                }

                // 继续循环，让 LLM 根据工具结果生成回复
                continue;
            }

            StreamResult::Content(content) => {
                if content.is_empty() {
                    return Err("LLM 未返回内容".to_string());
                }

                messages.push(json!({
                    "role": "assistant",
                    "content": content,
                }));

                println!("\x1b[32m\n✅ LLM 给出了最终回复\x1b[0m");
                return Ok(content);
            }
        }
    }

    Err("达到最大循环次数限制".to_string())
}
