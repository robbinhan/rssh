use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crate::models::{AuthType, ServerConfig, SessionConfig, SessionWindow};
use crate::config::{ConfigManager, get_db_path, get_session_dir, SessionManager};
use crate::utils::{SshClient, import_ssh_config, connect_via_system_ssh, connect_via_system_ssh_with_command, ssh_command_connect, russh_connect};
use crate::utils::rclone::RcloneConfig;
use uuid::Uuid;
use colored::*;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use crate::utils::server_info::display_server_info;
use shell_escape; // Import the new crate
use std::process::Command; // Import Command
use std::process::Stdio; // Import Stdio

#[derive(Parser)]
#[command(name = "rssh")]
#[command(author = "Rust SSH Manager")]
#[command(version = "0.1.0")]
#[command(about = "SSH连接管理工具", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// 连接方式枚举
#[derive(Copy, Clone, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum ConnectionMode {
    /// 使用内置的SSH库连接
    Library,
    /// 使用系统SSH命令连接（推荐）
    System,
    /// 使用exec替换当前进程连接
    Exec,
    /// 使用内置的SSH库但启用调试
    Debug,
    /// 使用Russh库连接（异步Rust SSH实现）
    Russh,
}

/// 文件传输模式
#[derive(Copy, Clone, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum TransferMode {
    /// 使用SCP传输文件（默认）
    Scp,
    /// 使用SFTP传输文件
    Sftp,
    // /// 使用Kitty传输协议（如果可用）
    // Kitty,
    /// 自动选择最佳传输方式
    Auto,
}

#[derive(Subcommand)]
enum Commands {
    /// 添加新的服务器
    Add {
        /// 服务器名称
        #[arg(short = 'n', long)]
        name: String,
        
        /// 主机名或IP地址
        #[arg(short = 'H', long)]
        host: String,
        
        /// 端口号
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 用户名
        #[arg(short, long)]
        username: String,
        
        /// 认证类型 (password, key, agent)
        #[arg(short = 't', long = "auth-type", default_value = "password")]
        auth_type: String,
        
        /// 密码或私钥路径
        #[arg(short = 'k', long = "auth-data")]
        auth_data: Option<String>,
        
        /// 备用密码（当密钥认证失败时使用）
        #[arg(short = 'p', long = "password")]
        password: Option<String>,
        
        /// 服务器分组
        #[arg(short, long)]
        group: Option<String>,
        
        /// 服务器描述
        #[arg(short, long)]
        description: Option<String>,
    },
    
    /// 列出所有服务器
    List {
        /// 按分组过滤
        #[arg(short, long)]
        group: Option<String>,
    },
    
    /// 连接到服务器
    Connect {
        /// 服务器ID或名称
        #[arg(index = 1)]
        server: String,
        
        /// 在服务器上执行的命令
        #[arg(short, long)]
        command: Option<String>,
        
        /// 连接方式
        #[arg(short, long, value_enum, default_value = "system")]
        mode: ConnectionMode,
        
        /// 启用rzsz文件传输功能（使用代理）
        #[arg(long)]
        rzsz: bool,
        
        /// 对于kitty终端，使用kitten ssh进行连接
        #[arg(long)]
        kitten: bool,
    },
    
    /// 删除服务器
    Remove {
        /// 服务器ID或名称
        server: String,
    },
    
    /// 编辑服务器
    Edit {
        /// 服务器ID或名称
        server: String,
    },
    
    /// 上传文件到远程服务器
    Upload {
        /// 服务器ID或名称
        #[arg(index = 1)]
        server: String,
        
        /// 本地文件路径
        #[arg(index = 2)]
        local_path: PathBuf,
        
        /// 远程目标路径（如果不指定，将使用与本地相同的文件名）
        #[arg(index = 3)]
        remote_path: Option<String>,
        
        /// 传输模式
        #[arg(short, long, value_enum, default_value = "auto")]
        mode: TransferMode,
    },
    
    /// 从远程服务器下载文件
    Download {
        /// 服务器ID或名称
        #[arg(index = 1)]
        server: String,
        
        /// 远程文件路径
        #[arg(index = 2)]
        remote_path: String,
        
        /// 本地目标路径（如果不指定，将使用与远程相同的文件名）
        #[arg(index = 3)]
        local_path: Option<PathBuf>,
        
        /// 传输模式
        #[arg(short, long, value_enum, default_value = "auto")]
        mode: TransferMode,
    },
    
    /// 从 SSH 配置文件导入服务器
    Import {
        /// SSH配置文件路径 (默认为 ~/.ssh/config)
        #[arg(short, long)]
        config: Option<PathBuf>,
        
        /// 服务器分组
        #[arg(short, long)]
        group: Option<String>,
        
        /// 跳过已存在的服务器
        #[arg(short, long)]
        skip_existing: bool,
    },
    
    /// 导出服务器配置到目录
    Export {
        /// 导出目录路径
        #[arg(index = 1)]
        path: PathBuf,
    },

    /// 从目录导入服务器配置
    ImportConfig {
        /// 导入目录路径
        #[arg(index = 1)]
        path: PathBuf,
    },

    /// 查看服务器详细信息
    Info {
        /// 服务器名称或ID
        server: String,
    },

    /// 在服务器之间复制文件或目录
    Copy {
        /// 源服务器名称
        #[arg(short, long)]
        from: String,
        
        /// 源服务器上的路径
        #[arg(short, long)]
        from_path: String,
        
        /// 目标服务器名称
        #[arg(short, long)]
        to: String,
        
        /// 目标服务器上的路径
        #[arg(short, long)]
        to_path: String,
    },

    /// 创建新的会话
    #[command(name = "session-create")]
    SessionCreate {
        /// 会话名称
        #[arg(short = 'n', long)]
        name: String,
        
        /// 会话描述
        #[arg(short, long)]
        description: Option<String>,
        
        /// 配置文件路径（可选，如果提供，将从该文件导入配置）
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    
    /// 列出所有会话
    #[command(name = "session-list")]
    SessionList,
    
    /// 编辑会话
    #[command(name = "session-edit")]
    SessionEdit {
        /// 会话名称或ID
        #[arg(index = 1)]
        session: String,
    },
    
    /// 删除会话
    #[command(name = "session-remove")]
    SessionRemove {
        /// 会话名称或ID
        #[arg(index = 1)]
        session: String,
    },
    
    /// 启动会话
    #[command(name = "session-start")]
    SessionStart {
        /// 会话名称或ID
        #[arg(index = 1)]
        session: String,
        
        /// 使用tmux（如果安装）
        #[arg(long)]
        tmux: bool,
        
        /// 使用kitty terminal支持的布局（如果在kitty中运行）
        #[arg(long)]
        kitty: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_manager = ConfigManager::new(get_db_path()?)?;
    
    match cli.command {
        Commands::Add { name, host, port, username, auth_type, auth_data, password, group, description } => {
            let auth = match auth_type.as_str() {
                "password" => {
                    let pwd = auth_data.ok_or_else(|| anyhow::anyhow!("使用密码认证时必须提供密码"))?;
                    AuthType::Password(pwd)
                },
                "key" => {
                    let key_path = auth_data.ok_or_else(|| anyhow::anyhow!("使用密钥认证时必须提供密钥路径"))?;
                    AuthType::Key(key_path)
                },
                "agent" => AuthType::Agent,
                _ => return Err(anyhow::anyhow!("未知的认证类型: {}", auth_type)),
            };
            
            let server = ServerConfig::new(
                Uuid::new_v4().to_string(),
                name,
                host,
                port,
                username,
                auth,
                group,
                description,
                password,
            );
            
            config_manager.add_server(server)?;
            println!("服务器添加成功");
        },
        
        Commands::List { group } => {
            let mut servers = config_manager.list_servers()?;
            
            // 按名称排序
            servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            
            let filtered_servers = if let Some(ref g) = group {  // 使用引用避免移动
                servers.into_iter()
                    .filter(|s| s.group.as_deref() == Some(g.as_str()))
                    .collect::<Vec<_>>()
            } else {
                servers
            };
            
            if filtered_servers.is_empty() {
                println!("没有找到服务器");
                return Ok(());
            }
            
            // 计算表格宽度
            let id_width = 8;  // 短ID
            let name_width = 15;
            let host_width = 20;
            let port_width = 6;
            let user_width = 10;
            let auth_width = 10;
            let group_width = 12;
            
            // 表格总宽度
            let total_width = id_width + name_width + host_width + port_width + user_width + auth_width + group_width + 15; // 15是分隔符的宽度
            
            // 打印表头 - 使用拼接字符串方式
            let top_border = format!("{}{}{}",
                "╭".bright_cyan(),
                "─".bright_cyan().to_string().repeat(total_width - 2),
                "╮".bright_cyan()
            );
            println!("{}", top_border);
            
            // 打印标题行
            println!("{} {:<id_width$} │ {:<name_width$} │ {:<host_width$} │ {:<port_width$} │ {:<user_width$} │ {:<auth_width$} │ {:<group_width$} {}",
                "│".bright_cyan(),
                "ID".bright_white().bold(),
                "名称".bright_white().bold(),
                "主机".bright_white().bold(),
                "端口".bright_white().bold(),
                "用户名".bright_white().bold(),
                "认证类型".bright_white().bold(),
                "分组".bright_white().bold(),
                "│".bright_cyan(),
            );
            
            // 打印分隔行 - 使用format!拼接字符串
            let separator = format!("{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
                "├".bright_cyan(),
                "─".bright_cyan().to_string().repeat(id_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(name_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(host_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(port_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(user_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(auth_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(group_width + 2),
                "┤".bright_cyan()
            );
            println!("{}", separator);
            
            // 打印数据行
            let mut row_idx = 0;
            for server in &filtered_servers {  // 使用引用避免移动所有权
                // 使用交替的行颜色
                let _row_color = if row_idx % 2 == 0 { "" } else { "" };
                row_idx += 1;
                
                // 截取ID的前8个字符
                let short_id = if server.id.len() > 8 {
                    &server.id[0..8]
                } else {
                    &server.id
                };
                
                let auth_type = match &server.auth_type {  // 使用引用
                    AuthType::Password(_) => "密码".bright_yellow(),
                    AuthType::Key(_) => "密钥".bright_blue(),
                    AuthType::Agent => "代理".bright_cyan(),
                };
                
                let group_str = server.group.as_deref().unwrap_or("--").bright_green();
                
                println!("{} {:<id_width$} │ {:<name_width$} │ {:<host_width$} │ {:<port_width$} │ {:<user_width$} │ {:<auth_width$} │ {:<group_width$} {}",
                    "│".bright_cyan(),
                    short_id.bright_yellow(),
                    server.name.bright_green(),
                    &server.host,
                    server.port.to_string().bright_blue(),
                    &server.username,
                    auth_type,
                    group_str,
                    "│".bright_cyan(),
                );
            }
            
            // 打印底部 - 使用format!拼接字符串
            let bottom_border = format!("{}{}{}",
                "╰".bright_cyan(),
                "─".bright_cyan().to_string().repeat(total_width - 2),
                "╯".bright_cyan()
            );
            println!("{}", bottom_border);
            
            // 打印服务器数量
            println!("\n共找到 {} 台服务器", filtered_servers.len().to_string().bright_green().bold());
            
            // 如果指定了分组，显示分组名
            if let Some(ref g) = group {  // 使用引用避免移动
                println!("分组: {}", g.bright_cyan().bold());
            }
            
            // 打印使用提示
            println!("\n提示: 使用 {} 连接到服务器", "rssh connect <ID或名称>".bright_yellow());
        },
        
        Commands::Connect { server, command, mode, rzsz, kitten } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let server_config = if server_config.is_none() {
                let servers = config_manager.list_servers()?;
                servers.into_iter().find(|s| s.name == server)
            } else {
                server_config
            };
            
            let server_config = match server_config {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
            };
            
            println!("正在连接到 {}@{}:{}...", 
                server_config.username.bright_yellow(), 
                server_config.host.bright_green(), 
                server_config.port.to_string().bright_blue()
            );
            
            if let Some(cmd) = command {
                // 执行命令总是使用SSH库
                let ssh_client = SshClient::connect(&server_config)?;
                let (stdout, stderr, exit_status) = ssh_client.execute_command(&cmd)?;
                
                if !stdout.is_empty() {
                    println!("{}", stdout);
                }
                
                if !stderr.is_empty() {
                    eprintln!("{}", stderr.bright_red());
                }
                
                std::process::exit(exit_status as i32);
            } else {
                // 根据连接模式选择不同的实现
                match mode {
                    ConnectionMode::Library => {
                        let ssh_client = SshClient::connect(&server_config)?;
                        println!("已连接，启动交互式shell...");
                        ssh_client.start_shell()?;
                    },
                    ConnectionMode::System => {
                        connect_via_system_ssh(&server_config, rzsz, kitten)?;
                    },
                    ConnectionMode::Exec => {
                        ssh_command_connect(&server_config, kitten)?;
                    },
                    ConnectionMode::Debug => {
                        let ssh_client = SshClient::connect(&server_config)?;
                        println!("已连接，启动交互式shell（调试模式）...");
                        println!("调试日志写入到: /tmp/rssh_debug.log");
                        println!("你可以按Alt+D切换调试模式，显示按键代码");
                        ssh_client.start_shell()?;
                    },
                    ConnectionMode::Russh => {
                        russh_connect(&server_config)?;
                    }
                }
            }
        },
        
        Commands::Remove { server } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let (server_id, server_name) = if let Some(s) = server_config {
                (s.id, s.name)
            } else {
                let servers = config_manager.list_servers()?;
                let found = servers.into_iter().find(|s| s.name == server);
                
                match found {
                    Some(s) => (s.id, s.name),
                    None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
                }
            };
            
            print!("确定要删除服务器 \"{}\" 吗? [y/N] ", server_name.bright_yellow());
            io::stdout().flush()?;
            
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;
            
            if confirm.trim().to_lowercase() == "y" {
                if config_manager.remove_server(&server_id)? {
                    println!("服务器已删除");
                } else {
                    println!("服务器删除失败");
                }
            } else {
                println!("取消删除");
            }
        },
        
        Commands::Edit { server } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let server_config = if server_config.is_none() {
                let servers = config_manager.list_servers()?;
                servers.into_iter().find(|s| s.name == server)
            } else {
                server_config
            };
            
            let mut server_config = match server_config {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
            };
            
            println!("编辑服务器 \"{}\"", server_config.name.bright_yellow());
            println!("按Enter跳过不修改");
            
            // 名称
            print!("名称 [{}]: ", server_config.name.bright_green());
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.name = input.trim().to_string();
            }
            
            // 主机
            print!("主机 [{}]: ", server_config.host.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.host = input.trim().to_string();
            }
            
            // 端口
            print!("端口 [{}]: ", server_config.port.to_string().bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                if let Ok(port) = input.trim().parse::<u16>() {
                    server_config.port = port;
                } else {
                    println!("端口无效，保持不变");
                }
            }
            
            // 用户名
            print!("用户名 [{}]: ", server_config.username.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.username = input.trim().to_string();
            }
            
            // 认证类型
            let auth_type = match &server_config.auth_type {
                AuthType::Password(_) => "password",
                AuthType::Key(_) => "key",
                AuthType::Agent => "agent",
            };
            
            print!("认证类型 [{}] (password/key/agent): ", auth_type.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            
            if !input.trim().is_empty() {
                match input.trim() {
                    "password" => {
                        print!("密码: ");
                        io::stdout().flush()?;
                        let password = rpassword::read_password()?;
                        server_config.auth_type = AuthType::Password(password);
                    },
                    "key" => {
                        print!("密钥路径: ");
                        io::stdout().flush()?;
                        input.clear();
                        io::stdin().read_line(&mut input)?;
                        let expanded_path = crate::utils::ssh_config::expand_tilde(input.trim());
                        server_config.auth_type = AuthType::Key(expanded_path);
                        
                        // 询问是否设置备用密码
                        print!("是否设置备用密码？[y/N] ");
                        io::stdout().flush()?;
                        input.clear();
                        io::stdin().read_line(&mut input)?;
                        if input.trim().to_lowercase() == "y" {
                            print!("备用密码: ");
                            io::stdout().flush()?;
                            let password = rpassword::read_password()?;
                            if !password.is_empty() {
                                server_config.password = Some(password);
                            }
                        } else {
                            server_config.password = None;
                        }
                    },
                    "agent" => {
                        server_config.auth_type = AuthType::Agent;
                        server_config.password = None;
                    },
                    _ => println!("未知认证类型，保持不变"),
                }
            }
            
            // 分组
            let group = server_config.group.as_deref().unwrap_or("无");
            print!("分组 [{}]: ", group.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if input.trim().is_empty() {
                // 保持不变
            } else if input.trim() == "无" || input.trim() == "none" {
                server_config.group = None;
            } else {
                server_config.group = Some(input.trim().to_string());
            }
            
            // 描述
            let description = server_config.description.as_deref().unwrap_or("无");
            print!("描述 [{}]: ", description.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if input.trim().is_empty() {
                // 保持不变
            } else if input.trim() == "无" || input.trim() == "none" {
                server_config.description = None;
            } else {
                server_config.description = Some(input.trim().to_string());
            }
            
            // 更新服务器配置
            if config_manager.update_server(server_config)? {
                println!("服务器更新成功");
            } else {
                println!("服务器更新失败");
            }
        },
        
        Commands::Upload { server, local_path, remote_path, mode } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let server_config = if server_config.is_none() {
                let servers = config_manager.list_servers()?;
                servers.into_iter().find(|s| s.name == server)
            } else {
                server_config
            };
            
            let server_config = match server_config {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
            };
            
            println!("准备上传文件到 {}@{}:{}...", 
                server_config.username.bright_yellow(), 
                server_config.host.bright_green(), 
                server_config.port.to_string().bright_blue()
            );
            
            // 根据指定的模式选择使用SCP还是SFTP
            match mode {
                TransferMode::Scp => {
                    // 使用SCP
                    crate::utils::upload_file(&server_config, &local_path, remote_path)?;
                },
                TransferMode::Sftp => {
                    // 使用SFTP作为备选方案
                    crate::utils::upload_file_sftp(&server_config, &local_path, remote_path)?;
                },
                // TransferMode::Kitty => {
                //     // 使用Kitty作为备选方案
                //     crate::utils::upload_file_kitty(&server_config, &local_path, remote_path)?;
                // },
                TransferMode::Auto => {
                    // 自动选择最佳传输方式
                    crate::utils::upload_file_auto(&server_config, &local_path, remote_path)?;
                }
            }
        },
        
        Commands::Download { server, remote_path, local_path, mode } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let server_config = if server_config.is_none() {
                let servers = config_manager.list_servers()?;
                servers.into_iter().find(|s| s.name == server)
            } else {
                server_config
            };
            
            let server_config = match server_config {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
            };
            
            println!("准备从 {}@{}:{} 下载文件...", 
                server_config.username.bright_yellow(), 
                server_config.host.bright_green(), 
                server_config.port.to_string().bright_blue()
            );
            
            // 根据指定的模式选择使用SCP还是SFTP
            match mode {
                TransferMode::Scp => {
                    // 使用SCP
                    crate::utils::download_file(&server_config, &remote_path, local_path)?;
                },
                TransferMode::Sftp => {
                    // 使用SFTP作为备选方案
                    crate::utils::download_file_sftp(&server_config, &remote_path, local_path)?;
                },
                // TransferMode::Kitty => {
                //     // 使用Kitty作为备选方案
                //     crate::utils::download_file_kitty(&server_config, &remote_path, local_path)?;
                // },
                TransferMode::Auto => {
                    // 自动选择最佳传输方式
                    crate::utils::download_file_auto(&server_config, &remote_path, local_path)?;
                }
            }
        },
        
        Commands::Import { config, group, skip_existing } => {
            // 确定 SSH 配置文件路径
            let config_path = match config {
                Some(path) => path,
                None => {
                    let mut home = dirs::home_dir()
                        .ok_or_else(|| anyhow::anyhow!("无法确定用户主目录"))?;
                    home.push(".ssh");
                    home.push("config");
                    home
                }
            };
            
            if !config_path.exists() {
                return Err(anyhow::anyhow!("找不到 SSH 配置文件: {}", config_path.display()));
            }
            
            println!("从 {} 导入服务器配置...", config_path.display());
            
            // 导入配置
            let mut configs = import_ssh_config(&config_path)?;
            
            // 如果指定了分组，则应用到所有服务器
            if let Some(ref g) = group {
                for config in &mut configs {
                    config.group = Some(g.clone());
                }
            }
            
            // 获取现有服务器列表（如果需要跳过已存在的）
            let existing_servers = if skip_existing {
                config_manager.list_servers()?
            } else {
                Vec::new()
            };
            
            let mut imported = 0;
            let mut skipped = 0;
            
            for server_config in configs {
                // 检查是否已存在同名或同主机的服务器
                if skip_existing && existing_servers.iter().any(|s| 
                    s.name == server_config.name || 
                    (s.host == server_config.host && 
                     s.port == server_config.port && 
                     s.username == server_config.username)) {
                    skipped += 1;
                    continue;
                }
                
                config_manager.add_server(server_config)?;
                imported += 1;
            }
            
            println!("导入完成! 已导入 {} 个服务器, 跳过 {} 个已存在的服务器。", 
                imported.to_string().bright_green(), 
                skipped.to_string().bright_yellow()
            );
        },
        
        Commands::Export { path } => {
            config_manager.export_config(&path)?;
            println!("配置已导出到: {}", path.display());
        },

        Commands::ImportConfig { path } => {
            config_manager.import_config(&path)?;
            println!("配置已从 {} 导入", path.display());
        },

        Commands::Info { server } => {
            // 首先尝试按ID查找
            let server_config = config_manager.get_server(&server)?;
            
            // 如果按ID找不到，尝试按名称查找
            let server_config = if server_config.is_none() {
                let servers = config_manager.list_servers()?;
                servers.into_iter().find(|s| s.name == server)
            } else {
                server_config
            };
            
            let server_config = match server_config {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("找不到指定的服务器: {}", server)),
            };
            
            display_server_info(&server_config)?;
        },

        Commands::Copy { from, from_path, to, to_path } => {
            println!("正在查找服务器配置...");
            let config = ConfigManager::new(get_db_path()?)?;
            
            println!("查找源服务器: {}", from);
            let from_server = if let Some(server) = config.get_server(&from)? {
                server
            } else {
                // 尝试按名称不区分大小写查找
                let servers = config.list_servers()?;
                servers.into_iter()
                    .find(|s| s.name.to_lowercase() == from.to_lowercase())
                    .ok_or_else(|| anyhow::anyhow!("源服务器 '{}' 不存在，请使用 'rssh list' 查看可用服务器", from))?
            };
            println!("找到源服务器: {} ({})", from_server.name, from_server.host);
            
            println!("查找目标服务器: {}", to);
            let to_server = if let Some(server) = config.get_server(&to)? {
                server
            } else {
                // 尝试按名称不区分大小写查找
                let servers = config.list_servers()?;
                servers.into_iter()
                    .find(|s| s.name.to_lowercase() == to.to_lowercase())
                    .ok_or_else(|| anyhow::anyhow!("目标服务器 '{}' 不存在，请使用 'rssh list' 查看可用服务器", to))?
            };
            println!("找到目标服务器: {} ({})", to_server.name, to_server.host);
            
            // 确保 rclone 已安装
            println!("检查 rclone 是否已安装...");
            RcloneConfig::ensure_rclone_installed()?;
            
            // 初始化 rclone 配置
            println!("初始化 rclone 配置...");
            let rclone_config = RcloneConfig::new()?;
            
            // 配置源服务器和目标服务器
            println!("配置源服务器...");
            rclone_config.configure_remote(&from_server)?;
            
            println!("配置目标服务器...");
            rclone_config.configure_remote(&to_server)?;
            
            // 执行复制
            println!("开始复制文件...");
            rclone_config.copy(&from_server, &from_path, &to_server, &to_path)?;
            println!("复制完成！");
        },

        Commands::SessionCreate { name, description, config } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            // 如果提供了配置文件，尝试从中导入
            if let Some(config_path) = config {
                // 检查文件是否存在
                if !config_path.exists() {
                    return Err(anyhow::anyhow!("配置文件不存在: {}", config_path.display()));
                }
                
                // 读取配置文件内容
                let content = std::fs::read_to_string(&config_path)
                    .context(format!("无法读取配置文件: {}", config_path.display()))?;
                
                // 解析TOML
                let parsed_config: toml::Value = toml::from_str(&content)
                    .context("无法解析TOML配置文件")?;
                
                // 创建窗口配置
                let mut windows = Vec::new();
                let empty_table = toml::value::Table::new();
                
                // 读取窗口配置
                if let Some(windows_table) = parsed_config.get("windows")
                    .and_then(|v| v.as_table()) 
                {
                    for (window_name, window_config) in windows_table {
                        let window_table = window_config.as_table().unwrap_or(&empty_table);
                        
                        // 检查必要的server字段
                        let server = match window_table.get("server").and_then(|v| v.as_str()) {
                            Some(s) => s.to_string(),
                            None => {
                                eprintln!("警告: 窗口 '{}' 未指定服务器，将被跳过", window_name);
                                continue;
                            }
                        };
                        
                        // 创建窗口配置
                        let window = SessionWindow {
                            title: Some(window_name.clone()),
                            server,
                            command: window_table.get("command").and_then(|v| v.as_str()).map(String::from),
                            position: window_table.get("position").and_then(|v| v.as_str()).map(String::from),
                            size: window_table.get("size").and_then(|v| v.as_str()).map(String::from),
                        };
                        
                        windows.push(window);
                    }
                } else {
                    return Err(anyhow::anyhow!("配置文件中未找到windows部分"));
                }
                
                // 读取选项
                let mut options = std::collections::HashMap::new();
                if let Some(opts_table) = parsed_config.get("options")
                    .and_then(|v| v.as_table()) 
                {
                    for (key, value) in opts_table {
                        if let Some(value_str) = value.as_str() {
                            options.insert(key.clone(), value_str.to_string());
                        }
                    }
                }
                
                // 创建会话
                let session = session_manager.create_session(
                    name, 
                    description, 
                    windows,
                    Some(options)
                )?;
                
                println!("成功创建会话: {}", session.name);
            } else {
                // 如果没有提供配置文件，创建一个空会话配置，用户稍后可以编辑它
                session_manager.create_session(name, description, Vec::new(), None)?;
                println!("已创建空会话配置，请使用 'rssh session-edit' 编辑它");
            }
        },
        
        Commands::SessionList => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            let sessions = session_manager.list_sessions()?;
            
            if sessions.is_empty() {
                println!("没有找到会话配置");
                return Ok(());
            }
            
            // 计算表格宽度
            let id_width = 8;  // 短ID
            let name_width = 20;
            let desc_width = 30;
            let windows_width = 10;
            
            // 表格总宽度
            let total_width = id_width + name_width + desc_width + windows_width + 10; // 10是分隔符的宽度
            
            // 打印表头
            let top_border = format!("{}{}{}",
                "╭".bright_cyan(),
                "─".bright_cyan().to_string().repeat(total_width - 2),
                "╮".bright_cyan()
            );
            println!("{}", top_border);
            
            // 打印标题行
            println!("{} {:<id_width$} │ {:<name_width$} │ {:<desc_width$} │ {:<windows_width$} {}",
                "│".bright_cyan(),
                "ID".bright_white().bold(),
                "名称".bright_white().bold(),
                "描述".bright_white().bold(),
                "窗口数".bright_white().bold(),
                "│".bright_cyan(),
            );
            
            // 打印分隔行
            let separator = format!("{}{}{}{}{}{}{}{}{}",
                "├".bright_cyan(),
                "─".bright_cyan().to_string().repeat(id_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(name_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(desc_width + 2),
                "┼".bright_cyan(),
                "─".bright_cyan().to_string().repeat(windows_width + 2),
                "┤".bright_cyan()
            );
            println!("{}", separator);
            
            // 打印数据行
            for session in &sessions {
                // 截取ID的前8个字符
                let short_id = if session.id.len() > 8 {
                    &session.id[0..8]
                } else {
                    &session.id
                };
                
                // 截取描述
                let desc = session.description.as_deref().unwrap_or("--");
                let short_desc = if desc.len() > desc_width {
                    desc[0..desc_width - 3].to_string() + "..."
                } else {
                    desc.to_string()
                };
                
                println!("{} {:<id_width$} │ {:<name_width$} │ {:<desc_width$} │ {:<windows_width$} {}",
                    "│".bright_cyan(),
                    short_id.bright_yellow(),
                    session.name.bright_green(),
                    short_desc,
                    session.windows.len().to_string().bright_blue(),
                    "│".bright_cyan(),
                );
            }
            
            // 打印底部
            let bottom_border = format!("{}{}{}",
                "╰".bright_cyan(),
                "─".bright_cyan().to_string().repeat(total_width - 2),
                "╯".bright_cyan()
            );
            println!("{}", bottom_border);
            
            // 打印会话数量
            println!("\n共找到 {} 个会话配置", sessions.len().to_string().bright_green().bold());
            
            // 打印使用提示
            println!("\n提示: 使用 {} 启动会话", "rssh session-start <ID或名称>".bright_yellow());
        },
        
        Commands::SessionEdit { session } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            // 首先尝试按ID查找
            let session_id = if session_manager.session_exists(&session) {
                session.clone()
            } else {
                // 尝试按名称查找
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s.id,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            // 获取会话配置的路径
            let session_path = session_manager.get_session_path(&session_id);
            
            // 使用系统默认编辑器打开配置文件
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            let status = std::process::Command::new(editor)
                .arg(&session_path)
                .status()
                .context("无法启动编辑器")?;
            
            if !status.success() {
                return Err(anyhow::anyhow!("编辑器返回非零状态码: {}", status));
            }
            
            println!("会话配置已更新");
        },
        
        Commands::SessionRemove { session } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            // 首先尝试按ID查找
            let session_id = if session_manager.session_exists(&session) {
                session.clone()
            } else {
                // 尝试按名称查找
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s.id,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            // 删除会话
            session_manager.remove_session(&session_id)?;
            println!("会话已删除");
        },
        
        Commands::SessionStart { session, tmux, kitty } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            // 首先尝试按ID查找
            let session_config = if session_manager.session_exists(&session) {
                session_manager.load_session(&session)?
            } else {
                // 尝试按名称查找
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            if session_config.windows.is_empty() {
                return Err(anyhow::anyhow!("会话 '{}' 没有配置窗口", session_config.name));
            }
            
            // 根据环境选择会话启动方式
            if kitty || (std::env::var("TERM").unwrap_or_default().contains("kitty") && !tmux) {
                // Restore calling the original function
                start_session_with_kitty(&config_manager, &session_config)?;
            } else if tmux || std::env::var("TMUX").is_ok() {
                start_session_with_tmux(&config_manager, &session_config)?;
            } else {
                // ... (Keep default sequential connection logic) ...
                 println!("警告: 未检测到支持多窗口的环境，将按顺序连接");
                
                for window in &session_config.windows {
                    // 查找服务器配置
                    let server_config = find_server(&config_manager, &window.server)?;
                    
                    println!("连接到 {}", server_config.name.bright_green());
                    
                    // 使用System模式连接
                    match connect_via_system_ssh_with_command(&server_config, window.command.clone(), false, false) {
                        Ok(exit_code) => {
                            if exit_code != 0 {
                                eprintln!("警告: 服务器 {} 返回非零状态码: {}", 
                                    server_config.name, exit_code);
                            }
                        },
                        Err(e) => {
                            eprintln!("连接到服务器 {} 时出错: {}", server_config.name, e);
                        }
                    }
                }
            }
        },
    }
    
    Ok(())
}

/// 查找服务器配置（按ID或名称）
fn find_server(config_manager: &ConfigManager, server_id_or_name: &str) -> Result<ServerConfig> {
    // 首先尝试按ID查找
    let server_config = config_manager.get_server(server_id_or_name)?;
    
    // 如果按ID找不到，尝试按名称查找
    let server_config = if server_config.is_none() {
        let servers = config_manager.list_servers()?;
        servers.into_iter().find(|s| s.name == server_id_or_name)
    } else {
        server_config
    };
    
    server_config.ok_or_else(|| anyhow::anyhow!("未找到服务器: {}", server_id_or_name))
}

/// 使用kitty终端的布局功能启动会话 (最终版本)
fn start_session_with_kitty(config_manager: &ConfigManager, session: &SessionConfig) -> Result<()> {
    if !std::env::var("TERM").unwrap_or_default().contains("kitty") {
        // Keep this check
        return Err(anyhow::anyhow!("当前终端不是kitty"));
    }
    
    println!("使用kitty启动会话: {}", session.name.bright_green());
    
    // --- Step 1: Generate the kitty session config file (.conf) --- 
    let mut tmp_session_file = std::env::temp_dir();
    tmp_session_file.push(format!("rssh_kitty_session_{}.conf", session.id));
    let mut session_conf_writer = std::io::BufWriter::new(std::fs::File::create(&tmp_session_file)?);
    
    writeln!(session_conf_writer, "# RSSH会话配置: {}", session.name)?;
    writeln!(session_conf_writer, "new_tab {}", session.name)?;
    writeln!(session_conf_writer, "layout splits")?;
    writeln!(session_conf_writer)?;
    
    let current_rssh_path = std::env::current_exe()
        .with_context(|| "无法获取当前rssh可执行文件路径")?;

    for (i, window) in session.windows.iter().enumerate() {
        let server_config = find_server(config_manager, &window.server)?;
        let title = window.title.as_deref().unwrap_or(&window.server);
        let window_var = format!("window={}", i);

        let mut base_ssh_args = format!("{}@{} -p {}", 
            server_config.username, server_config.host, server_config.port);
        if let Some(key_path) = server_config.auth_type.get_key_path() {
            let expanded_key_path = crate::utils::ssh_config::expand_tilde(key_path);
            base_ssh_args.push_str(&format!(" -i \"{}\"", expanded_key_path)); 
        }

        // --- Generate SSH payload (wait, chmod, exec, rm) --- 
        let final_ssh_payload = if let Some(cmd) = &window.command {
            println!("  处理窗口 '{}': 找到命令, 准备上传脚本...", title);
            let unique_id = format!("{}_{}", session.id.split('-').next().unwrap_or("session"), i);
            let local_script_path = std::env::temp_dir().join(format!("rssh_local_init_{}.sh", unique_id));
            let remote_script_path = format!("/tmp/rssh_remote_init_{}.sh", unique_id);

            // Generate script content, PREPENDING the TERM export
            let script_content = format!("#!/bin/sh\nset -e\nexport TERM=xterm-kitty\n{}\n", cmd);
            std::fs::write(&local_script_path, &script_content)
                 .with_context(|| format!("创建本地初始化脚本失败: {}", local_script_path.display()))?;
            println!("    本地脚本: {}", local_script_path.display());

            println!("    尝试上传到: {}@{}...", server_config.username, remote_script_path);
            let mut upload_command = Command::new(&current_rssh_path);
            upload_command
                .arg("upload")
                .arg(&window.server)
                .arg(&local_script_path)
                .arg(&remote_script_path);
            
            // Execute and capture output
            let upload_result = upload_command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            // --- Temporarily disable LOCAL script cleanup for debugging --- 
            // let _ = std::fs::remove_file(&local_script_path);
            // println!("    本地临时脚本已删除: {}", local_script_path.display());
            println!("    本地临时脚本保留用于调试: {}", local_script_path.display()); // Indicate script is kept

            match upload_result {
                Ok(upload_output) => {
                    if upload_output.status.success() {
                        println!("    上传成功 (退出码 0).");
                        // --- Final SSH command with wait, chmod, EXECUTION, and RM ---
                        let remote_script_escaped = shell_escape::escape(remote_script_path.into());
                        format!(
                            "'while [ ! -f {} ]; do sleep 0.1; done; chmod +x {} && {} && rm {} ; exec $SHELL'",
                            remote_script_escaped, // for while
                            remote_script_escaped, // for chmod
                            remote_script_escaped, // for execution
                            remote_script_escaped  // for rm
                        )
                    } else {
                        eprintln!("    [Error] 上传失败 (退出码: {:?}). 将只启动交互式 shell.", upload_output.status.code());
                        if !upload_output.stdout.is_empty() {
                            eprintln!("      Upload stdout: {}", String::from_utf8_lossy(&upload_output.stdout));
                        }
                        if !upload_output.stderr.is_empty() {
                            eprintln!("      Upload stderr: {}", String::from_utf8_lossy(&upload_output.stderr));
                        }
                        "''".to_string() // Fallback to interactive shell
                    }
                },
                Err(e) => {
                     eprintln!("    [Error] 执行 'rssh upload' 命令本身失败: {}. 将只启动交互式 shell.", e);
                     "''".to_string() // Fallback to interactive shell
                }
            }
        } else {
             println!("  处理窗口 '{}': 无初始命令，直接启动交互式 shell.", title);
            "''".to_string()
        };

        let final_ssh_cmd = format!("ssh -t {} {}", base_ssh_args, final_ssh_payload);
        println!("    最终 SSH 命令: {}", final_ssh_cmd); // Keep this for debugging temporarily?

        // --- Write the launch command to the .conf file --- 
        if i == 0 {
            writeln!(session_conf_writer, "# 第一个窗口 - {}", title)?;
            writeln!(session_conf_writer, "launch --var {} --title '{}' {}", window_var, title, final_ssh_cmd)?;
            writeln!(session_conf_writer)?;
        } else {
            let location = match window.position.as_deref() {
                Some("vsplit") => "vsplit",
                Some("hsplit") => "hsplit", 
                Some("split") => "vsplit",
                Some(custom) => custom,
                None => "vsplit",          
            };
            
            writeln!(session_conf_writer, "# 窗口 {} - {}", i+1, title)?;
            writeln!(session_conf_writer, "launch --location={} --var {} --title '{}' {}", 
                location, window_var, title, final_ssh_cmd)?;
            writeln!(session_conf_writer)?;
        }
    }
    session_conf_writer.flush()?;
    drop(session_conf_writer);
    println!("临时会话配置文件已生成: {}", tmp_session_file.display());

    // --- Step 2: Generate and execute the launch script (.sh) --- 
    let mut launch_script_path = std::env::temp_dir();
    launch_script_path.push(format!("rssh_kitty_launch_{}.sh", session.id));
    let mut script = std::fs::File::create(&launch_script_path)?;

    writeln!(script, "#!/bin/sh")?;
    writeln!(script, "export TERM=xterm-kitty")?;
    writeln!(script, "# 启动kitty新窗口并使用生成的会话配置")?;
    writeln!(script, "kitty --session '{}' --title 'RSSH Session: {}' & disown", 
             tmp_session_file.display(), session.name)?;
    writeln!(script, "exit 0")?;
    
    script.flush()?;
    drop(script);
    let mut perms = std::fs::metadata(&launch_script_path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&launch_script_path, perms)?;
    println!("临时启动脚本已生成: {}", launch_script_path.display());

    // --- Step 3: Execute the launch script --- 
    println!("执行启动脚本以打开 Kitty 窗口...");
    
    let _ = std::process::Command::new(&launch_script_path)
        .spawn()
        .context("无法执行启动脚本")?;

    std::thread::sleep(std::time::Duration::from_millis(500)); 

    // --- Temporarily disable launch script cleanup for debugging --- 
    // let _ = std::fs::remove_file(&launch_script_path);
    // println!("本地启动脚本已删除: {}", launch_script_path.display());
    println!("本地启动脚本保留用于调试: {}", launch_script_path.display()); // Indicate script is kept

    println!("会话已在新窗口启动。远程脚本执行后将被自动删除。");
    println!("会话配置文件保留在: {}", tmp_session_file.display());
    
    Ok(())
}

/// 使用tmux启动会话
fn start_session_with_tmux(config_manager: &ConfigManager, session: &SessionConfig) -> Result<()> {
    // 检查tmux是否安装
    let tmux_check = std::process::Command::new("which")
        .arg("tmux")
        .stdout(std::process::Stdio::null())
        .status();
    
    if tmux_check.is_err() || !tmux_check.unwrap().success() {
        return Err(anyhow::anyhow!("未找到tmux命令"));
    }
    
    println!("使用tmux启动会话: {}", session.name.bright_green());
    
    // 创建唯一的会话名称
    let tmux_session_name = format!("rssh_{}", session.id.split('-').next().unwrap_or("session"));
    
    // 创建tmux会话
    let create_status = std::process::Command::new("tmux")
        .args(["new-session", "-d", "-s", &tmux_session_name])
        .status()
        .context("无法创建tmux会话")?;
    
    if !create_status.success() {
        return Err(anyhow::anyhow!("无法创建tmux会话"));
    }
    
    // 对于每个窗口，创建相应的tmux窗口
    for (i, window) in session.windows.iter().enumerate() {
        // 查找服务器配置
        let server_config = find_server(config_manager, &window.server)?;
        
        // 创建SSH命令
        let mut ssh_cmd = format!("ssh {}@{} -p {}", 
            server_config.username, 
            server_config.host, 
            server_config.port);
        
        // 添加认证参数
        if let Some(key_path) = server_config.auth_type.get_key_path() {
            ssh_cmd.push_str(&format!(" -i {}", key_path));
        }
        
        // 添加命令（如果有）
        if let Some(cmd) = &window.command {
            ssh_cmd.push_str(&format!(" '{}'", cmd.replace("'", "'\''")));
        }
        
        // 窗口标题
        let title = window.title.as_deref().unwrap_or(&window.server);
        
        if i == 0 {
            // 重命名第一个窗口
            std::process::Command::new("tmux")
                .args(["rename-window", "-t", &format!("{}:0", tmux_session_name), title])
                .status()?;
            
            // 发送命令到第一个窗口
            std::process::Command::new("tmux")
                .args(["send-keys", "-t", &format!("{}:0", tmux_session_name), &ssh_cmd, "Enter"])
                .status()?;
        } else {
            // 创建新窗口
            std::process::Command::new("tmux")
                .args(["new-window", "-t", &tmux_session_name, "-n", title])
                .status()?;
            
            // 发送命令到新窗口
            std::process::Command::new("tmux")
                .args(["send-keys", "-t", &format!("{}:{}", tmux_session_name, i), &ssh_cmd, "Enter"])
                .status()?;
        }
    }
    
    // 附加到tmux会话
    std::process::Command::new("tmux")
        .args(["attach-session", "-t", &tmux_session_name])
        .status()
        .context("无法附加到tmux会话")?;
    
    Ok(())
} 