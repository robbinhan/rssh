use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use std::io::Write;
use anyhow::{Result, Context};
use uuid::Uuid;
use toml;
use crate::models::{SessionConfig, SessionWindow};

/// Session配置管理器
pub struct SessionManager {
    /// Session配置文件目录
    config_dir: PathBuf,
}

impl SessionManager {
    /// 创建新的session管理器
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        // 确保配置目录存在
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }
        
        Ok(SessionManager { config_dir })
    }
    
    /// 保存session配置
    pub fn save_session(&self, session: &SessionConfig) -> Result<()> {
        let file_path = self.get_session_path(&session.id);
        let toml_str = toml::to_string_pretty(session)
            .context("无法序列化session配置")?;
            
        let mut file = fs::File::create(file_path)
            .context("无法创建session配置文件")?;
            
        file.write_all(toml_str.as_bytes())
            .context("无法写入session配置")?;
            
        Ok(())
    }
    
    /// 加载指定ID的session配置
    pub fn load_session(&self, id: &str) -> Result<SessionConfig> {
        let file_path = self.get_session_path(id);
        let content = fs::read_to_string(&file_path)
            .context(format!("无法读取session配置文件: {:?}", file_path))?;
            
        let session: SessionConfig = toml::from_str(&content)
            .context("无法解析session配置")?;
            
        Ok(session)
    }
    
    /// 加载所有session配置
    pub fn list_sessions(&self) -> Result<Vec<SessionConfig>> {
        let mut sessions = Vec::new();
        
        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "toml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(session) = toml::from_str::<SessionConfig>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }
        
        // 按名称排序
        sessions.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        
        Ok(sessions)
    }
    
    /// 删除session配置
    pub fn remove_session(&self, id: &str) -> Result<()> {
        let file_path = self.get_session_path(id);
        
        if file_path.exists() {
            fs::remove_file(file_path)
                .context("无法删除session配置文件")?;
        }
        
        Ok(())
    }
    
    /// 检查session是否存在
    pub fn session_exists(&self, id: &str) -> bool {
        self.get_session_path(id).exists()
    }
    
    /// 创建新的session配置
    pub fn create_session(
        &self, 
        name: String, 
        description: Option<String>,
        windows: Vec<SessionWindow>,
        options: Option<HashMap<String, String>>,
    ) -> Result<SessionConfig> {
        let id = Uuid::new_v4().to_string();
        let session = SessionConfig::new(id, name, description, windows, options);
        
        self.save_session(&session)?;
        
        Ok(session)
    }
    
    /// 编辑已有的session配置
    pub fn edit_session(
        &self,
        id: &str,
        name: Option<String>,
        description: Option<String>,
        windows: Option<Vec<SessionWindow>>,
        options: Option<HashMap<String, String>>,
    ) -> Result<SessionConfig> {
        let mut session = self.load_session(id)?;
        
        if let Some(name) = name {
            session.name = name;
        }
        
        if let Some(description) = description {
            session.description = Some(description);
        }
        
        if let Some(windows) = windows {
            session.windows = windows;
        }
        
        if let Some(options) = options {
            session.options = options;
        }
        
        self.save_session(&session)?;
        
        Ok(session)
    }
    
    /// 根据name查找session
    pub fn find_session_by_name(&self, name: &str) -> Result<Option<SessionConfig>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.into_iter().find(|s| s.name == name))
    }
    
    /// 获取session配置文件路径
    pub fn get_session_path(&self, id: &str) -> PathBuf {
        self.config_dir.join(format!("{}.toml", id))
    }
} 