use anyhow::Result;
use clap::{Parser, Subcommand};
use crate::models::{AuthType, ServerConfig};
use crate::config::{ConfigManager, get_db_path};
use crate::utils::{SshClient, import_ssh_config, connect_via_system_ssh, ssh_command_connect, russh_connect};
use crate::utils::rclone::RcloneConfig;
use uuid::Uuid;
use colored::*;
use std::io::{self, Write};
use std::path::PathBuf;
use crate::utils::server_info::display_server_info;

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
    /// 使用Kitty传输协议（如果可用）
    Kitty,
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
                TransferMode::Kitty => {
                    // 使用Kitty作为备选方案
                    crate::utils::upload_file_kitty(&server_config, &local_path, remote_path)?;
                },
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
                TransferMode::Kitty => {
                    // 使用Kitty作为备选方案
                    crate::utils::download_file_kitty(&server_config, &remote_path, local_path)?;
                },
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
        }
    }
    
    Ok(())
} 