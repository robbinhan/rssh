use anyhow::{Context, Result};
use std::process::Command;
use crate::models::Server;

pub struct RcloneConfig {
    config_path: String,
}

impl RcloneConfig {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("rssh")
            .join("rclone");
        
        std::fs::create_dir_all(&config_dir)
            .context("无法创建 rclone 配置目录")?;
            
        Ok(Self {
            config_path: config_dir.join("rclone.conf").to_string_lossy().to_string(),
        })
    }

    pub fn ensure_rclone_installed() -> Result<()> {
        if which::which("rclone").is_err() {
            println!("正在安装 rclone...");
            let install_cmd = if cfg!(target_os = "macos") {
                "brew install rclone"
            } else if cfg!(target_os = "linux") {
                "curl https://rclone.org/install.sh | sudo bash"
            } else {
                return Err(anyhow::anyhow!("不支持的操作系统"));
            };
            
            Command::new("sh")
                .arg("-c")
                .arg(install_cmd)
                .status()
                .context("安装 rclone 失败")?;
        }
        Ok(())
    }

    pub fn configure_remote(&self, server: &Server) -> Result<()> {
        let remote_name = format!("rssh_{}", server.name);
        
        // 检查是否已配置
        let config_content = std::fs::read_to_string(&self.config_path)
            .unwrap_or_default();
            
        if config_content.contains(&format!("[{}]", remote_name)) {
            return Ok(());
        }

        // 构建 rclone 配置
        let mut config = toml::from_str::<toml::Value>(&config_content)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
            
        let mut remote = toml::map::Map::new();
        remote.insert("type".to_string(), toml::Value::String("sftp".to_string()));
        remote.insert("host".to_string(), toml::Value::String(server.host.clone()));
        remote.insert("user".to_string(), toml::Value::String(server.username.clone()));
        remote.insert("port".to_string(), toml::Value::Integer(server.port.unwrap_or(22) as i64));
        
        if let Some(auth_data) = &server.auth_data {
            remote.insert(
                "key_file".to_string(),
                toml::Value::String(auth_data.replace("~", &dirs::home_dir().unwrap().to_string_lossy()))
            );
        }
        
        if let Some(table) = config.as_table_mut() {
            table.insert(remote_name.clone(), toml::Value::Table(remote));
        }
        
        std::fs::write(
            &self.config_path,
            toml::to_string_pretty(&config).context("序列化配置失败")?
        ).context("写入 rclone 配置失败")?;
        
        Ok(())
    }

    pub fn copy(&self, from_server: &Server, from_path: &str, 
                to_server: &Server, to_path: &str) -> Result<()> {
        let from_remote = format!("rssh_{}:{}", from_server.name, from_path);
        let to_remote = format!("rssh_{}:{}", to_server.name, to_path);
        
        let status = Command::new("rclone")
            .arg("--config")
            .arg(&self.config_path)
            .arg("copy")
            .arg(&from_remote)
            .arg(&to_remote)
            .arg("-v")
            .status()
            .context("执行 rclone 命令失败")?;
            
        if !status.success() {
            return Err(anyhow::anyhow!("文件复制失败"));
        }
        
        Ok(())
    }
} 