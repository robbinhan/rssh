use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::models::{AuthType, ServerConfig};

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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                auth_data TEXT,
                group_name TEXT,
                description TEXT
            )",
            [],
        )?;
        
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
            "INSERT INTO servers (id, name, host, port, username, auth_type, auth_data, group_name, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                server.id,
                server.name,
                server.host,
                server.port,
                server.username,
                auth_type,
                auth_data,
                server.group,
                server.description,
            ],
        )?;
        
        Ok(())
    }
    
    pub fn get_server(&self, id: &str) -> Result<Option<ServerConfig>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, name, host, port, username, auth_type, auth_data, group_name, description
             FROM servers WHERE id = ?1"
        )?;
        
        let server = stmt.query_row(params![id], |row| {
            let auth_type: String = row.get(5)?;
            let auth_data: Option<String> = row.get(6)?;
            
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
                group: row.get(7)?,
                description: row.get(8)?,
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
            "SELECT id, name, host, port, username, auth_type, auth_data, group_name, description
             FROM servers ORDER BY name"
        )?;
        
        let servers_iter = stmt.query_map([], |row| {
            let auth_type: String = row.get(5)?;
            let auth_data: Option<String> = row.get(6)?;
            
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
                group: row.get(7)?,
                description: row.get(8)?,
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
                 auth_type = ?6, auth_data = ?7, group_name = ?8, description = ?9
             WHERE id = ?1",
            params![
                server.id,
                server.name,
                server.host,
                server.port,
                server.username,
                auth_type,
                auth_data,
                server.group,
                server.description,
            ],
        )?;
        
        Ok(count > 0)
    }
} 