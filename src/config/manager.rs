use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use serde_json::{json, Value};
use std::fs;

use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::expand_tilde;

pub struct ConfigManager {
    conn: Arc<Mutex<Connection>>,
}

impl ConfigManager {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let create_db = !db_path.exists();
        
        let conn = Connection::open(&db_path)
            .with_context(|| format!("无法打开数据库 {}", db_path.display()))?;
        
        if create_db {
            Self::init_database(&conn)?;
        }
        
        Ok(ConfigManager {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    fn init_database(conn: &Connection) -> Result<()> {
        // 检查表是否存在
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='servers'")?;
        let table_exists = stmt.exists([])?;
        
        if !table_exists {
            // 如果表不存在，创建新表
            conn.execute(
                "CREATE TABLE servers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    username TEXT NOT NULL,
                    auth_type TEXT NOT NULL,
                    auth_data TEXT,
                    password TEXT,
                    group_name TEXT,
                    description TEXT
                )",
                [],
            )?;
        } else {
            // 如果表存在，检查是否需要添加 password 列
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('servers') WHERE name = 'password'")?;
            let has_password = stmt.exists([])?;
            
            if !has_password {
                conn.execute("ALTER TABLE servers ADD COLUMN password TEXT", [])?;
            }
        }
        
        Ok(())
    }
    
    pub fn add_server(&self, server: ServerConfig) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        let (auth_type, auth_data) = match &server.auth_type {
            AuthType::Password(pwd) => ("password", Some(pwd.clone())),
            AuthType::Key(key_path) => ("key", Some(key_path.clone())),
            AuthType::Agent => ("agent", None),
        };
        
        conn.execute(
            "INSERT INTO servers (id, name, host, port, username, auth_type, auth_data, password, group_name, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                server.id,
                server.name,
                server.host,
                server.port,
                server.username,
                auth_type,
                auth_data,
                server.password,
                server.group,
                server.description,
            ],
        )?;
        
        Ok(())
    }
    
    pub fn get_server(&self, id: &str) -> Result<Option<ServerConfig>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, name, host, port, username, auth_type, auth_data, password, group_name, description
             FROM servers WHERE id = ?1"
        )?;
        
        let server = stmt.query_row(params![id], |row| {
            let auth_type: String = row.get(5)?;
            let auth_data: Option<String> = row.get(6)?;
            let password: Option<String> = row.get(7)?;
            
            let auth = match (auth_type.as_str(), auth_data) {
                ("password", Some(pwd)) => AuthType::Password(pwd),
                ("key", Some(key_path)) => AuthType::Key(key_path),
                ("agent", _) => AuthType::Agent,
                _ => return Err(rusqlite::Error::InvalidColumnName("未知的认证类型".into())),
            };
            
            Ok(ServerConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                auth_type: auth,
                password,
                group: row.get(8)?,
                description: row.get(9)?,
            })
        });
        
        match server {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    pub fn list_servers(&self) -> Result<Vec<ServerConfig>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, name, host, port, username, auth_type, auth_data, password, group_name, description
             FROM servers ORDER BY name"
        )?;
        
        let servers_iter = stmt.query_map([], |row| {
            let auth_type: String = row.get(5)?;
            let auth_data: Option<String> = row.get(6)?;
            let password: Option<String> = row.get(7)?;
            
            let auth = match (auth_type.as_str(), auth_data) {
                ("password", Some(pwd)) => AuthType::Password(pwd),
                ("key", Some(key_path)) => AuthType::Key(key_path),
                ("agent", _) => AuthType::Agent,
                _ => return Err(rusqlite::Error::InvalidColumnName("未知的认证类型".into())),
            };
            
            Ok(ServerConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                auth_type: auth,
                password,
                group: row.get(8)?,
                description: row.get(9)?,
            })
        })?;
        
        let mut servers = Vec::new();
        for server in servers_iter {
            servers.push(server?);
        }
        
        Ok(servers)
    }
    
    pub fn remove_server(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let count = conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?;
        
        Ok(count > 0)
    }
    
    pub fn update_server(&self, server: ServerConfig) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let (auth_type, auth_data) = match &server.auth_type {
            AuthType::Password(pwd) => ("password", Some(pwd.clone())),
            AuthType::Key(key_path) => ("key", Some(key_path.clone())),
            AuthType::Agent => ("agent", None),
        };
        
        let count = conn.execute(
            "UPDATE servers 
             SET name = ?2, host = ?3, port = ?4, username = ?5, 
                 auth_type = ?6, auth_data = ?7, password = ?8, group_name = ?9, description = ?10
             WHERE id = ?1",
            params![
                server.id,
                server.name,
                server.host,
                server.port,
                server.username,
                auth_type,
                auth_data,
                server.password,
                server.group,
                server.description,
            ],
        )?;
        
        Ok(count > 0)
    }

    pub fn export_config(&self, export_path: &PathBuf) -> Result<()> {
        // 创建导出目录
        fs::create_dir_all(export_path)
            .with_context(|| format!("无法创建导出目录: {}", export_path.display()))?;

        // 创建keys子目录用于存储私钥文件
        let keys_dir = export_path.join("keys");
        fs::create_dir_all(&keys_dir)
            .with_context(|| format!("无法创建keys目录: {}", keys_dir.display()))?;

        let servers = self.list_servers()?;
        let mut processed_keys = std::collections::HashSet::new();

        // 处理每个服务器的私钥文件
        for server in &servers {
            if let AuthType::Key(key_path) = &server.auth_type {
                if !processed_keys.contains(key_path) {
                    processed_keys.insert(key_path.clone());
                    
                    // 展开路径中的 ~
                    let expanded_key_path = PathBuf::from(expand_tilde(key_path));
                    
                    // 检查私钥文件是否存在
                    if !expanded_key_path.exists() {
                        println!("警告: 私钥文件不存在，跳过: {}", key_path);
                        continue;
                    }
                    
                    // 获取私钥文件名
                    let key_filename = expanded_key_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown_key");
                    
                    // 复制私钥文件到keys目录
                    let target_path = keys_dir.join(key_filename);
                    fs::copy(&expanded_key_path, &target_path)
                        .with_context(|| format!("无法复制私钥文件: {} -> {}", expanded_key_path.display(), target_path.display()))?;
                }
            }
        }

        // 创建配置文件
        let config = json!({
            "version": "1.0",
            "servers": servers,
        });
        
        let json_string = serde_json::to_string_pretty(&config)?;
        let config_file = export_path.join("config.json");
        fs::write(&config_file, json_string)
            .with_context(|| format!("无法写入配置文件: {}", config_file.display()))?;

        // 创建README文件
        let readme_content = format!(
            "RSSH 配置备份\n\
             ============\n\n\
             导出时间: {}\n\n\
             目录结构:\n\
             - config.json: 服务器配置文件\n\
             - keys/: 私钥文件目录\n\n\
             导入说明:\n\
             1. 确保所有私钥文件已正确放置在 ~/.ssh/ 目录下\n\
             2. 使用命令 'rssh import-config <导出目录>' 导入配置\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        
        let readme_file = export_path.join("README.md");
        fs::write(&readme_file, readme_content)
            .with_context(|| format!("无法写入README文件: {}", readme_file.display()))?;
        
        Ok(())
    }

    pub fn import_config(&self, import_path: &PathBuf) -> Result<()> {
        // 检查是否是目录
        if !import_path.is_dir() {
            return Err(anyhow::anyhow!("导入路径必须是目录: {}", import_path.display()));
        }

        // 读取配置文件
        let config_file = import_path.join("config.json");
        let json_string = fs::read_to_string(&config_file)
            .with_context(|| format!("无法读取配置文件: {}", config_file.display()))?;
        
        let config: Value = serde_json::from_str(&json_string)?;
        
        if let Some(servers) = config.get("servers").and_then(|s| s.as_array()) {
            for server_json in servers {
                let server: ServerConfig = serde_json::from_value(server_json.clone())?;
                self.add_server(server)?;
            }
        }
        
        Ok(())
    }
} 