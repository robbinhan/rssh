use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use serde_json::{json, Value};
use std::fs;

use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::{expand_tilde, sanitize_host_alias};

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

    /// 导出为 OpenSSH config 语法的单个文件，可被 ~/.ssh/config 通过 Include 引入。
    ///
    /// 与 `export_config` 不同：这里生成的是标准 ssh_config 文本（Host/HostName/...），
    /// 而不是 rssh 自己的 JSON 备份格式。
    pub fn export_ssh_config(&self, export_file: &PathBuf) -> Result<()> {
        // 确保父目录存在
        if let Some(parent) = export_file.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("无法创建导出目录: {}", parent.display()))?;
            }
        }

        let servers = self.list_servers()?;

        let mut content = String::new();
        content.push_str("# RSSH 导出的 SSH config\n");
        content.push_str(&format!(
            "# 导出时间: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        content.push_str("#\n");
        content.push_str("# 用法: 在 ~/.ssh/config 顶部加入一行(使用绝对路径):\n");
        content.push_str(&format!("#   Include {}\n", export_file.display()));
        content.push_str("# 之后即可使用 `ssh <别名>` 连接。\n\n");

        // 别名去重，避免重复 Host 块
        let mut used_aliases: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut password_count = 0;

        for server in &servers {
            let alias = unique_host_alias(&sanitize_host_alias(&server.name), &mut used_aliases);

            // 描述 / 分组写成注释
            let mut comment_parts = Vec::new();
            if let Some(desc) = &server.description {
                if !desc.trim().is_empty() {
                    comment_parts.push(desc.trim().to_string());
                }
            }
            if let Some(group) = &server.group {
                if !group.trim().is_empty() {
                    comment_parts.push(format!("分组: {}", group.trim()));
                }
            }
            if !comment_parts.is_empty() {
                content.push_str(&format!("# {}\n", comment_parts.join(" | ")));
            }

            content.push_str(&format!("Host {}\n", alias));
            content.push_str(&format!("    HostName {}\n", server.host));
            content.push_str(&format!("    Port {}\n", server.port));
            content.push_str(&format!("    User {}\n", server.username));

            match &server.auth_type {
                AuthType::Key(key_path) => {
                    // ssh 自身支持 ~，保留原始路径即可
                    content.push_str(&format!("    IdentityFile {}\n", key_path));
                    content.push_str("    IdentitiesOnly yes\n");
                }
                AuthType::Agent => {
                    // 走 ssh-agent，无需额外指令
                }
                AuthType::Password(_) => {
                    // ssh config 无法保存明文密码，连接时交互式输入
                    content.push_str("    # 密码认证: ssh config 无法保存密码，连接时需手动输入\n");
                    password_count += 1;
                }
            }

            content.push('\n');
        }

        fs::write(export_file, content)
            .with_context(|| format!("无法写入 ssh config 文件: {}", export_file.display()))?;

        if password_count > 0 {
            println!(
                "注意: 有 {} 个服务器为密码认证，ssh config 无法保存密码，连接时需手动输入。",
                password_count
            );
        }

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

/// 保证别名唯一，冲突时追加 `-2`、`-3` 等后缀。
fn unique_host_alias(
    alias: &str,
    used: &mut std::collections::HashSet<String>,
) -> String {
    if used.insert(alias.to_string()) {
        return alias.to_string();
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{}-{}", alias, suffix);
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_colliding_aliases() {
        let mut used = std::collections::HashSet::new();
        assert_eq!(unique_host_alias("web", &mut used), "web");
        assert_eq!(unique_host_alias("web", &mut used), "web-2");
        assert_eq!(unique_host_alias("web", &mut used), "web-3");
    }

    #[test]
    fn export_ssh_config_emits_valid_blocks() {
        let base = std::env::temp_dir().join(format!("rssh-test-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let db_path = base.join("test.db");
        let out_path = base.join("exported_config");

        let mgr = ConfigManager::new(db_path).unwrap();

        // key 认证，名称含空格 -> 别名应被清洗
        mgr.add_server(ServerConfig::new(
            "1".into(), "prod web".into(), "10.0.0.1".into(), 2222, "deploy".into(),
            AuthType::Key("~/.ssh/id_ed25519".into()), None, Some("生产机".into()), None,
        )).unwrap();
        // agent 认证
        mgr.add_server(ServerConfig::new(
            "2".into(), "bastion".into(), "10.0.0.2".into(), 22, "root".into(),
            AuthType::Agent, Some("infra".into()), None, None,
        )).unwrap();
        // 密码认证 -> 不应出现 IdentityFile，应出现密码注释
        mgr.add_server(ServerConfig::new(
            "3".into(), "db".into(), "10.0.0.3".into(), 22, "admin".into(),
            AuthType::Password("secret".into()), None, None, Some("secret".into()),
        )).unwrap();
        // 同名 -> 别名去重
        mgr.add_server(ServerConfig::new(
            "4".into(), "bastion".into(), "10.0.0.4".into(), 22, "root".into(),
            AuthType::Agent, None, None, None,
        )).unwrap();

        mgr.export_ssh_config(&out_path).unwrap();
        let content = fs::read_to_string(&out_path).unwrap();

        assert!(content.contains("Host prod-web"));
        assert!(content.contains("    HostName 10.0.0.1"));
        assert!(content.contains("    Port 2222"));
        assert!(content.contains("    IdentityFile ~/.ssh/id_ed25519"));
        assert!(content.contains("    IdentitiesOnly yes"));
        assert!(content.contains("Host bastion\n"));
        assert!(content.contains("Host bastion-2\n"));
        // 密码认证不写明文密码
        assert!(!content.contains("secret"));
        assert!(content.contains("密码认证"));
        // Include 用法提示存在
        assert!(content.contains("Include"));

        fs::remove_dir_all(&base).ok();
    }
} 