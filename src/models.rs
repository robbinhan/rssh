use serde::{Deserialize, Serialize};
use crate::utils::terminal_style::{Style, colors, Styled, StyledText};

/// 认证类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthType {
    Password(String),
    Key(String),
    Agent,
}

impl AuthType {
    /// 获取密钥路径（如果是密钥认证）
    pub fn get_key_path(&self) -> Option<&str> {
        match self {
            AuthType::Key(path) => Some(path),
            _ => None,
        }
    }

    /// 获取 SSH 命令参数
    pub fn get_ssh_args(&self) -> String {
        match self {
            AuthType::Key(path) => format!("-i {}", path),
            _ => String::new(),
        }
    }
}

impl Styled for AuthType {
    fn style(self, style: Style) -> StyledText {
        let text = match self {
            AuthType::Password(_) => "密码认证",
            AuthType::Key(_) => "密钥认证",
            AuthType::Agent => "SSH Agent",
        };
        text.style(style)
    }
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub group: Option<String>,
    pub description: Option<String>,
}

impl ServerConfig {
    /// 创建新的服务器配置
    pub fn new(
        id: String,
        name: String,
        host: String,
        port: u16,
        username: String,
        auth_type: AuthType,
        group: Option<String>,
        description: Option<String>,
    ) -> Self {
        ServerConfig {
            id,
            name,
            host,
            port,
            username,
            auth_type,
            group,
            description,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    pub host: String,
    pub username: String,
    pub port: Option<u16>,
    pub auth_type: AuthType,
    pub auth_data: Option<String>,
    pub group: Option<String>,
} 