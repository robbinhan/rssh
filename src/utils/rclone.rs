use anyhow::{Context, Result};
use std::process::Command;
use crate::models::{ServerConfig, AuthType};
use shellexpand;

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

    pub fn configure_remote(&self, server: &ServerConfig) -> Result<()> {
        println!("正在配置服务器: {} ({})", server.name, server.host);
        
        // 检查是否已经配置过
        let output = Command::new("rclone")
            .arg("config")
            .arg("show")
            .output()?;
        
        let config = String::from_utf8_lossy(&output.stdout);
        let remote_name = format!("rssh_{}", server.name);
        
        if config.contains(&remote_name) {
            println!("服务器 {} 已配置", server.name);
            return Ok(());
        }
        
        println!("正在为服务器 {} 创建新的 rclone 配置...", server.name);
        
        // 创建新的远程配置
        let mut cmd = Command::new("rclone");
        cmd.arg("config")
           .arg("create")
           .arg(&remote_name)
           .arg("sftp")
           .arg(format!("host={}", server.host))
           .arg(format!("user={}", server.username))
           .arg(format!("port={}", server.port));
        
        match &server.auth_type {
            AuthType::Password(pass) => {
                cmd.arg(format!("pass={}", pass));
            },
            AuthType::Key(key_path) => {
                let expanded_path = shellexpand::tilde(key_path);
                println!("使用密钥认证，密钥文件: {}", expanded_path);
                cmd.arg(format!("key_file={}", expanded_path));
            },
            AuthType::Agent => {
                println!("使用 SSH 代理认证");
                cmd.arg("use_insecure_cipher=false");
            }
        }
        
        // 显示配置内容
        println!("[{}]", remote_name);
        println!("type = sftp");
        println!("host = {}", server.host);
        println!("user = {}", server.username);
        println!("port = {}", server.port);
        
        if let AuthType::Key(key_path) = &server.auth_type {
            println!("key_file = {}", shellexpand::tilde(key_path));
        }
        
        // 创建配置
        let status = cmd.status()?;
        
        if status.success() {
            println!("服务器 {} 配置完成", server.name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("服务器 {} 配置失败", server.name))
        }
    }

    pub fn copy(&self, from_server: &ServerConfig, from_path: &str, to_server: &ServerConfig, to_path: &str) -> Result<()> {
        println!("准备从 {} ({}) 复制到 {} ({})", from_server.name, from_server.host, to_server.name, to_server.host);
        
        // 使用 rclone 复制文件
        let from_remote = format!("rssh_{}:{}", from_server.name, from_path);
        let to_remote = format!("rssh_{}:{}", to_server.name, to_path);
        
        println!("执行命令: rclone copy {} {}", from_remote, to_remote);
        
        let mut cmd = Command::new("rclone");
        cmd.arg("copy")
           .arg(&from_remote)
           .arg(&to_remote)
           .arg("-v");
        
        // 显示完整命令
        let command_str = format!("{:?}", cmd);
        println!("完整命令: {}", command_str);
        
        let status = cmd.status()?;
            
        if status.success() {
            println!("文件复制成功");
            Ok(())
        } else {
            Err(anyhow::anyhow!("文件复制失败"))
        }
    }
} 