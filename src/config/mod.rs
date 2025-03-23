pub mod manager;

pub use manager::ConfigManager;

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