use serde_json::{json, Value};

/// 返回所有工具的 JSON Schema 定义（OpenAI function calling 格式）
pub fn get_tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "查询指定城市的当前天气",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "城市名称，如：北京、上海、Tokyo"
                        }
                    },
                    "required": ["city"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "calculator",
                "description": "计算数学表达式，支持加减乘除、幂、开方等",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "数学表达式，如：2+3*4、sqrt(16)、2^10"
                        }
                    },
                    "required": ["expression"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_time",
                "description": "获取当前日期和时间",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "读取指定路径的文件内容",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "文件路径"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "将内容写入指定路径的文件",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "文件路径"
                        },
                        "content": {
                            "type": "string",
                            "description": "要写入的内容"
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_shell",
                "description": "执行 shell 命令并返回输出",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "要执行的 shell 命令"
                        }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "fetch_webpage",
                "description": "抓取网页内容（返回 HTML 文本）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "要抓取的网页 URL"
                        }
                    },
                    "required": ["url"]
                }
            }
        }
    ])
}

/// 执行工具调用，返回结果字符串
pub async fn execute_tool(
    client: &reqwest::Client,
    name: &str,
    args: &Value,
) -> String {
    match name {
        "get_weather" => exec_weather(client, args).await,
        "calculator" => exec_calculator(args),
        "get_time" => exec_time(),
        "read_file" => exec_read_file(args),
        "write_file" => exec_write_file(args),
        "run_shell" => exec_shell(args),
        "fetch_webpage" => exec_fetch_webpage(client, args).await,
        _ => format!("未知工具: {}", name),
    }
}

// ==================== 各工具实现 ====================

async fn exec_weather(client: &reqwest::Client, args: &Value) -> String {
    let city = args["city"].as_str().unwrap_or("北京");
    let url = format!("https://wttr.in/{}?format=3&lang=zh", city);

    match client.get(&url).send().await {
        Ok(resp) => match resp.text().await {
            Ok(text) => text.trim().to_string(),
            Err(e) => format!("读取天气响应失败: {}", e),
        },
        Err(e) => format!("天气请求失败: {}", e),
    }
}

fn exec_calculator(args: &Value) -> String {
    let expr = args["expression"].as_str().unwrap_or("");
    match meval::eval_str(expr) {
        Ok(result) => format!("{} = {}", expr, result),
        Err(e) => format!("计算错误: {}", e),
    }
}

fn exec_time() -> String {
    let now = chrono::Local::now();
    format!(
        "当前时间: {}\nUnix 时间戳: {}",
        now.format("%Y-%m-%d %H:%M:%S %Z"),
        now.timestamp()
    )
}

fn exec_read_file(args: &Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if content.len() > 10000 {
                format!("{}...\n\n[文件过长，仅显示前 10000 字符]", &content[..10000])
            } else {
                content
            }
        }
        Err(e) => format!("读取文件失败: {}", e),
    }
}

fn exec_write_file(args: &Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");

    // 确保父目录存在
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(path, content) {
        Ok(_) => format!("成功写入 {} ({} 字节)", path, content.len()),
        Err(e) => format!("写入文件失败: {}", e),
    }
}

fn exec_shell(args: &Value) -> String {
    let command = args["command"].as_str().unwrap_or("");

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut result = String::new();

            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                result.push_str(&format!("\n[stderr]\n{}", stderr));
            }
            if result.is_empty() {
                result = format!("[命令执行完成，退出码: {}]", out.status.code().unwrap_or(-1));
            }

            // 限制输出长度
            if result.len() > 10000 {
                format!("{}...\n\n[输出过长，已截断]", &result[..10000])
            } else {
                result
            }
        }
        Err(e) => format!("执行命令失败: {}", e),
    }
}

async fn exec_fetch_webpage(client: &reqwest::Client, args: &Value) -> String {
    let url = args["url"].as_str().unwrap_or("");

    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(text) => {
                    // 简单截取，避免 token 爆炸
                    if text.len() > 8000 {
                        format!(
                            "[HTTP {}]\n{}...\n\n[网页内容过长，仅显示前 8000 字符]",
                            status,
                            &text[..8000]
                        )
                    } else {
                        format!("[HTTP {}]\n{}", status, text)
                    }
                }
                Err(e) => format!("读取网页内容失败: {}", e),
            }
        }
        Err(e) => format!("网页请求失败: {}", e),
    }
}
