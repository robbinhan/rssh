use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::models::{AuthType, ServerConfig};

/// 将包含波浪号的路径扩展为完整路径
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if path.len() == 1 {
                return home.display().to_string();
            }
            if path.starts_with("~/") {
                let path_without_tilde = &path[2..];
                let mut new_path = PathBuf::from(home);
                new_path.push(path_without_tilde);
                return new_path.display().to_string();
            }
        }
    }
    path.to_string()
}

/// 将服务器名转换为合法的 ssh Host 别名。
///
/// ssh 的 Host 模式以空白分隔，且 `*`/`?` 是通配符，因此将这些字符替换为 `-`，
/// 避免一个名称被解析成多个别名或被当作通配匹配。
///
/// 这是 `export-ssh-config` 写入 `Host` 行、以及 wezterm `connect` 推导
/// `SSHMUX:<别名>` 域名时**共用**的规则，必须保持一致。
pub fn sanitize_host_alias(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '*' || c == '?' {
                '-'
            } else {
                c
            }
        })
        .collect();

    if sanitized.is_empty() {
        "unnamed".to_string()
    } else {
        sanitized
    }
}

pub struct SshConfigEntry {
    pub host: String,
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub identity_file: Option<String>,
}

impl SshConfigEntry {
    pub fn new(host: &str) -> Self {
        SshConfigEntry {
            host: host.to_string(),
            hostname: None,
            port: None,
            user: None,
            identity_file: None,
        }
    }

    pub fn to_server_config(&self) -> Option<ServerConfig> {
        // 如果没有主机名，则无法创建服务器配置
        let hostname = self.hostname.clone()?;
        
        // 默认用户名为当前用户
        let username = self.user.clone().unwrap_or_else(|| {
            std::env::var("USER").unwrap_or_else(|_| "root".to_string())
        });
        
        // 默认端口为22
        let port = self.port.unwrap_or(22);
        
        // 认证类型
        let auth_type = if let Some(identity_file) = &self.identity_file {
            // 处理路径中的波浪号
            let expanded_path = expand_tilde(identity_file);
            AuthType::Key(expanded_path)
        } else {
            AuthType::Agent
        };
        
        Some(ServerConfig::new(
            Uuid::new_v4().to_string(),
            self.host.clone(),
            hostname,
            port,
            username,
            auth_type,
            None,
            None,
            None,
        ))
    }
}

pub fn parse_ssh_config<P: AsRef<Path>>(path: P) -> Result<Vec<SshConfigEntry>> {
    let file = File::open(path.as_ref())
        .with_context(|| format!("无法打开文件: {}", path.as_ref().display()))?;
    
    let reader = BufReader::new(file);
    
    let mut entries = Vec::new();
    let mut current_entry: Option<SshConfigEntry> = None;
    
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        
        // 跳过空行和注释
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // 将行分割为键和值
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        
        if parts.len() < 2 {
            continue;
        }
        
        let key = parts[0].trim().to_lowercase();
        let value = parts[1].trim();
        
        if key == "host" && !value.contains('*') {
            // 如果有当前条目，则将其添加到结果中
            if let Some(entry) = current_entry {
                entries.push(entry);
            }
            
            // 创建新条目
            current_entry = Some(SshConfigEntry::new(value));
        } else if let Some(ref mut entry) = current_entry {
            // 更新当前条目
            match key.as_str() {
                "hostname" => entry.hostname = Some(value.to_string()),
                "port" => {
                    if let Ok(port) = value.parse::<u16>() {
                        entry.port = Some(port);
                    }
                },
                "user" => entry.user = Some(value.to_string()),
                "identityfile" => entry.identity_file = Some(value.to_string()),
                _ => {},
            }
        }
    }
    
    // 添加最后一个条目
    if let Some(entry) = current_entry {
        entries.push(entry);
    }
    
    Ok(entries)
}

pub fn import_ssh_config<P: AsRef<Path>>(path: P) -> Result<Vec<ServerConfig>> {
    let entries = parse_ssh_config(path)?;

    let configs: Vec<ServerConfig> = entries
        .iter()
        .filter_map(|entry| entry.to_server_config())
        .collect();

    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_whitespace_and_glob_chars() {
        assert_eq!(sanitize_host_alias("my server"), "my-server");
        assert_eq!(sanitize_host_alias("prod*?"), "prod--");
        assert_eq!(sanitize_host_alias("  web-1  "), "web-1");
    }

    #[test]
    fn falls_back_for_empty_name() {
        assert_eq!(sanitize_host_alias("   "), "unnamed");
    }
} 