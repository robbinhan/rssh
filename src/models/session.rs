use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

/// session窗口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWindow {
    /// 窗口标题
    pub title: Option<String>,
    /// 连接的服务器名称或ID
    pub server: String,
    /// 启动时执行的命令
    pub command: Option<String>,
    /// 窗口布局位置 (例如: "1,2" 表示行1列2)
    pub position: Option<String>,
    /// 窗口大小 (例如: "50%,60%" 表示宽度50%高度60%)
    pub size: Option<String>,
}

/// session配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// session唯一ID
    pub id: String,
    /// session名称
    pub name: String,
    /// session描述
    pub description: Option<String>,
    /// 窗口配置列表
    pub windows: Vec<SessionWindow>,
    /// 额外配置选项
    pub options: HashMap<String, String>,
}

impl SessionConfig {
    /// 创建新的session配置
    pub fn new(
        id: String,
        name: String,
        description: Option<String>,
        windows: Vec<SessionWindow>,
        options: Option<HashMap<String, String>>,
    ) -> Self {
        SessionConfig {
            id,
            name,
            description,
            windows,
            options: options.unwrap_or_default(),
        }
    }
}

/// 从TOML文件加载session配置
pub fn load_session_from_file(path: &PathBuf) -> anyhow::Result<SessionConfig> {
    let contents = std::fs::read_to_string(path)?;
    let config: SessionConfig = toml::from_str(&contents)?;
    Ok(config)
}

/// 将session配置保存到TOML文件
pub fn save_session_to_file(config: &SessionConfig, path: &PathBuf) -> anyhow::Result<()> {
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
} 