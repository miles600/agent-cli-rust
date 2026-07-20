use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Deserialize)]
struct ProviderEntry {
    url: String,
    api_key: String,
    model: String,
}

#[derive(Deserialize)]
struct YamlConfig {
    default: Option<String>,
    providers: HashMap<String, ProviderEntry>,
}

pub fn load_config() -> Result<AgentConfig, String> {
    let exe_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let config_path: PathBuf = exe_dir.join("../api_keys.yaml");

    let raw = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("无法读取配置文件 {:?}: {}", config_path, e))?;

    let yaml: YamlConfig =
        serde_yaml::from_str(&raw).map_err(|e| format!("解析 YAML 失败: {}", e))?;

    if yaml.providers.is_empty() {
        return Err("api_keys.yaml 中没有配置任何 Provider".to_string());
    }

    let target = std::env::var("AGENT_PROVIDER")
        .ok()
        .or(yaml.default.clone())
        .ok_or_else(|| {
            let available: Vec<_> = yaml.providers.keys().cloned().collect();
            format!(
                "未指定 Provider。请设置 AGENT_PROVIDER 环境变量或在 yaml 中配置 default\n可用: {}",
                available.join(", ")
            )
        })?;

    let provider = yaml.providers.get(&target).ok_or_else(|| {
        let available: Vec<_> = yaml.providers.keys().cloned().collect();
        format!(
            "未找到 Provider \"{}\"，可用: {}",
            target,
            available.join(", ")
        )
    })?;

    Ok(AgentConfig {
        provider: target,
        base_url: provider.url.clone(),
        api_key: provider.api_key.clone(),
        model: provider.model.clone(),
    })
}

pub fn list_providers() -> Vec<String> {
    let exe_dir = std::env::current_dir().unwrap_or_default();
    let config_path: PathBuf = exe_dir.join("../api_keys.yaml");
    let raw = std::fs::read_to_string(&config_path).unwrap_or_default();
    let yaml: Result<YamlConfig, _> = serde_yaml::from_str(&raw);
    match yaml {
        Ok(y) => y.providers.keys().cloned().collect(),
        Err(_) => vec![],
    }
}
