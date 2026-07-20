use crate::config::AgentConfig;
use serde_json::{json, Value};

pub type Messages = Vec<Value>;

const MAX_TURNS: usize = 30;

pub async fn run_agent_loop(
    client: &reqwest::Client,
    config: &AgentConfig,
    messages: &mut Messages,
) -> Result<String, String> {
    for turn in 0..MAX_TURNS {
        println!("\x1b[2m\n--- 循环 第 {} 轮 ---\x1b[0m", turn + 1);
        println!("\x1b[36m📡 发送请求到 LLM...\x1b[0m");

        let url = format!("{}/chat/completions", config.base_url);
        let body = json!({
            "model": config.model,
            "messages": messages,
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

        let data: Value = resp.json().await.map_err(|e| format!("解析响应失败: {}", e))?;

        let choice = &data["choices"][0]["message"];
        let content = choice["content"].as_str().unwrap_or("").to_string();

        if content.is_empty() {
            return Err("LLM 未返回内容".to_string());
        }

        println!("\x1b[34m🤖 {}\x1b[0m", content);

        messages.push(json!({
            "role": "assistant",
            "content": content,
        }));

        println!("\x1b[32m\n✅ LLM 给出了最终回复\x1b[0m");
        return Ok(content);
    }

    Err("达到最大循环次数限制".to_string())
}
