use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthType {
    Password(String),
    Key(String),
    Agent,
}

impl ServerConfig {
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