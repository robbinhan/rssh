pub mod manager;
pub mod session_manager;

pub use manager::ConfigManager;
pub use session_manager::SessionManager;

use anyhow::{Context, Result};
use std::path::PathBuf;
use dirs;

pub fn get_config_dir() -> Result<PathBuf> {
    let mut config_dir = dirs::config_dir()
        .with_context(|| "无法确定配置目录")?;
    
    config_dir.push("rssh");
    
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .with_context(|| format!("无法创建配置目录: {}", config_dir.display()))?;
    }
    
    Ok(config_dir)
}

pub fn get_db_path() -> Result<PathBuf> {
    let mut db_path = get_config_dir()?;
    db_path.push("servers.db");
    
    Ok(db_path)
}

pub fn get_session_dir() -> Result<PathBuf> {
    let mut session_dir = get_config_dir()?;
    session_dir.push("sessions");
    
    if !session_dir.exists() {
        std::fs::create_dir_all(&session_dir)
            .with_context(|| format!("无法创建会话目录: {}", session_dir.display()))?;
    }
    
    Ok(session_dir)
} 