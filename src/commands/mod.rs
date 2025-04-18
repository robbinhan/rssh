use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use crate::models::{AuthType, ServerConfig, SessionConfig, SessionWindow};
use crate::config::{ConfigManager, get_db_path, get_session_dir, SessionManager};
use crate::utils::{SshClient, import_ssh_config, connect_via_system_ssh, connect_via_system_ssh_with_command, ssh_command_connect, russh_connect};
use crate::utils::rclone::RcloneConfig;
use uuid::Uuid;
use std::io::{self, Write, stdout};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use crate::utils::server_info::display_server_info;
use shell_escape;
use std::process::Command;
use std::process::Stdio;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::*,
};

#[derive(Parser)]
#[command(name = "rssh")]
#[command(author = "Rust SSH Manager")]
#[command(version = "0.1.0")]
#[command(about = "SSH连接管理工具", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum ConnectionMode {
    Library,
    System,
    Exec,
    Debug,
    Russh,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum TransferMode {
    Scp,
    Sftp,
    Auto,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        #[arg(short = 'n', long)]
        name: String,
        
        #[arg(short = 'H', long)]
        host: String,
        
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        #[arg(short, long)]
        username: String,
        
        #[arg(short = 't', long = "auth-type", default_value = "password")]
        auth_type: String,
        
        #[arg(short = 'k', long = "auth-data")]
        auth_data: Option<String>,
        
        #[arg(short = 'p', long = "password")]
        password: Option<String>,
        
        #[arg(short, long)]
        group: Option<String>,
        
        #[arg(short, long)]
        description: Option<String>,
    },
    
    List {
        #[arg(short, long)]
        group: Option<String>,
    },
    
    Connect {
        #[arg(index = 1)]
        server: String,
        
        #[arg(short, long)]
        command: Option<String>,
        
        #[arg(short, long, value_enum, default_value = "system")]
        mode: ConnectionMode,
        
        #[arg(long)]
        rzsz: bool,
        
        #[arg(long)]
        kitten: bool,
    },
    
    Remove {
        server: String,
    },
    
    Edit {
        server: String,
    },
    
    Upload {
        #[arg(index = 1)]
        server: String,
        
        #[arg(index = 2)]
        local_path: PathBuf,
        
        #[arg(index = 3)]
        remote_path: Option<String>,
        
        #[arg(short, long, value_enum, default_value = "auto")]
        mode: TransferMode,
    },
    
    Download {
        #[arg(index = 1)]
        server: String,
        
        #[arg(index = 2)]
        remote_path: String,
        
        #[arg(index = 3)]
        local_path: Option<PathBuf>,
        
        #[arg(short, long, value_enum, default_value = "auto")]
        mode: TransferMode,
    },
    
    Import {
        #[arg(short, long)]
        config: Option<PathBuf>,
        
        #[arg(short, long)]
        group: Option<String>,
        
        #[arg(short, long)]
        skip_existing: bool,
    },
    
    Export {
        #[arg(index = 1)]
        path: PathBuf,
    },

    ImportConfig {
        #[arg(index = 1)]
        path: PathBuf,
    },

    Info {
        server: String,
    },

    Copy {
        #[arg(short, long)]
        from: String,
        
        #[arg(short, long)]
        from_path: String,
        
        #[arg(short, long)]
        to: String,
        
        #[arg(short, long)]
        to_path: String,
    },

    #[command(name = "session-create")]
    SessionCreate {
        #[arg(short = 'n', long)]
        name: String,
        
        #[arg(short, long)]
        description: Option<String>,
        
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    
    #[command(name = "session-list")]
    SessionList,
    
    #[command(name = "session-edit")]
    SessionEdit {
        #[arg(index = 1)]
        session: String,
    },
    
    #[command(name = "session-remove")]
    SessionRemove {
        #[arg(index = 1)]
        session: String,
    },
    
    #[command(name = "session-start")]
    SessionStart {
        #[arg(index = 1)]
        session: String,
        
        #[arg(long)]
        tmux: bool,
        
        #[arg(long)]
        kitty: bool,
    },
}

fn run_list_tui<B: Backend>(
    terminal: &mut Terminal<B>,
    servers: Vec<ServerConfig>,
    group_filter: Option<String>,
) -> Result<Option<ServerConfig>> {
    let mut table_state = TableState::default();
    if !servers.is_empty() {
        table_state.select(Some(0));
    }

    loop {
        terminal.draw(|f| ui(f, &servers, group_filter.as_deref(), &mut table_state))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(None),
                        KeyCode::Down | KeyCode::Char('j') => {
                            if !servers.is_empty() {
                                let i = match table_state.selected() {
                                    Some(i) => {
                                        if i >= servers.len() - 1 {
                                            0
                                        } else {
                                            i + 1
                                        }
                                    }
                                    None => 0,
                                };
                                table_state.select(Some(i));
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if !servers.is_empty() {
                                let i = match table_state.selected() {
                                    Some(i) => {
                                        if i == 0 {
                                            servers.len() - 1
                                        } else {
                                            i - 1
                                        }
                                    }
                                    None => servers.len() - 1,
                                };
                                table_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(selected_index) = table_state.selected() {
                                if let Some(selected_server) = servers.get(selected_index).cloned() {
                                    return Ok(Some(selected_server));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, servers: &[ServerConfig], group_filter: Option<&str>, state: &mut TableState) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    let title_text = match group_filter {
        Some(g) => format!(" RSSH 服务器列表 (分组: {}) ", g),
        None => " RSSH 服务器列表 ".to_string(),
    };
    let title = Block::default()
        .title(title_text.bold())
        .title_alignment(Alignment::Center)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);
    f.render_widget(title, main_layout[0]);

    if servers.is_empty() {
        let msg = Paragraph::new(Text::styled("没有找到服务器", Style::default().fg(Color::Yellow)))
            .block(Block::default().borders(Borders::all()))
            .alignment(Alignment::Center);
        f.render_widget(msg, main_layout[1]);
    } else {
        let header_cells = [
            "ID (8)", "名称", "主机", "端口", "用户", "认证", "分组"
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White).bold()));
        let header = Row::new(header_cells)
            .style(Style::default().bg(Color::Blue))
            .height(1)
            .bottom_margin(1);

        let rows = servers.iter().map(|server| {
            let short_id = if server.id.len() > 8 {
                &server.id[0..8]
            } else {
                &server.id
            };
            let auth_str = match &server.auth_type {
                AuthType::Password(_) => "密码",
                AuthType::Key(_) => "密钥",
                AuthType::Agent => "代理",
            };
            let group_str = server.group.as_deref().unwrap_or("--");

            let cells = vec![
                Cell::from(short_id).style(Style::default().fg(Color::Yellow)),
                Cell::from(server.name.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(server.host.clone()),
                Cell::from(server.port.to_string()).style(Style::default().fg(Color::Cyan)),
                Cell::from(server.username.clone()),
                Cell::from(auth_str).style(match &server.auth_type {
                     AuthType::Password(_) => Style::default().fg(Color::Yellow),
                     AuthType::Key(_) => Style::default().fg(Color::Blue),
                     AuthType::Agent => Style::default().fg(Color::Cyan),
                }),
                Cell::from(group_str).style(Style::default().fg(Color::Magenta)),
            ];
            Row::new(cells).height(1)
        });

        let widths = [
            Constraint::Length(10),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Length(8),
            Constraint::Percentage(15),
            Constraint::Length(8),
            Constraint::Percentage(15),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("服务器"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");

        f.render_stateful_widget(table, main_layout[1], state);
    }

    let footer_text = Text::styled("↑/k: 上 | ↓/j: 下 | Enter: 连接 | q: 退出", Style::default().fg(Color::DarkGray));
    let footer = Paragraph::new(footer_text)
        .alignment(Alignment::Center);
    f.render_widget(footer, main_layout[2]);
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
            
            servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            let filtered_servers = if let Some(ref g) = group {
                servers.into_iter()
                    .filter(|s| s.group.as_deref() == Some(g.as_str()))
                    .collect::<Vec<_>>()
            } else {
                servers
            };

            enable_raw_mode()?;
            let mut stdout = stdout();
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend)?;

            let selected_server_option = run_list_tui(&mut terminal, filtered_servers, group)?;

            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            if let Some(server_to_connect) = selected_server_option {
                println!("准备连接到选中的服务器: {}", server_to_connect.name.clone().green());
                connect_via_system_ssh(&server_to_connect, false, false)?;
            } else {
                println!("已退出列表视图。");
            }
        },
        
        Commands::Connect { server, command, mode, rzsz, kitten } => {
            let server_config = config_manager.get_server(&server)?;
            
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
                server_config.username.on_bright_yellow(), 
                server_config.host.on_bright_green(), 
                server_config.port.to_string().on_bright_blue()
            );
            
            if let Some(cmd) = command {
                let ssh_client = SshClient::connect(&server_config)?;
                let (stdout, stderr, exit_status) = ssh_client.execute_command(&cmd)?;
                
                if !stdout.is_empty() {
                    println!("{}", stdout);
                }
                
                if !stderr.is_empty() {
                    eprintln!("{}", stderr.on_red());
                }
                
                std::process::exit(exit_status as i32);
            } else {
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
            let server_config = config_manager.get_server(&server)?;
            
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
            
            print!("确定要删除服务器 \"{}\" 吗? [y/N] ", server_name.on_bright_yellow());
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
            let server_config = config_manager.get_server(&server)?;
            
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
            
            print!("名称 [{}]: ", server_config.name.bright_green());
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.name = input.trim().to_string();
            }
            
            print!("主机 [{}]: ", server_config.host.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.host = input.trim().to_string();
            }
            
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
            
            print!("用户名 [{}]: ", server_config.username.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                server_config.username = input.trim().to_string();
            }
            
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
            
            let group = server_config.group.as_deref().unwrap_or("无");
            print!("分组 [{}]: ", group.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if input.trim().is_empty() {
            } else if input.trim() == "无" || input.trim() == "none" {
                server_config.group = None;
            } else {
                server_config.group = Some(input.trim().to_string());
            }
            
            let description = server_config.description.as_deref().unwrap_or("无");
            print!("描述 [{}]: ", description.bright_green());
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;
            if input.trim().is_empty() {
            } else if input.trim() == "无" || input.trim() == "none" {
                server_config.description = None;
            } else {
                server_config.description = Some(input.trim().to_string());
            }
            
            if config_manager.update_server(server_config)? {
                println!("服务器更新成功");
            } else {
                println!("服务器更新失败");
            }
        },
        
        Commands::Upload { server, local_path, remote_path, mode } => {
            let server_config = config_manager.get_server(&server)?;
            
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
            
            match mode {
                TransferMode::Scp => {
                    crate::utils::upload_file(&server_config, &local_path, remote_path)?;
                },
                TransferMode::Sftp => {
                    crate::utils::upload_file_sftp(&server_config, &local_path, remote_path)?;
                },
                TransferMode::Auto => {
                    crate::utils::upload_file_auto(&server_config, &local_path, remote_path)?;
                }
            }
        },
        
        Commands::Download { server, remote_path, local_path, mode } => {
            let server_config = config_manager.get_server(&server)?;
            
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
            
            match mode {
                TransferMode::Scp => {
                    crate::utils::download_file(&server_config, &remote_path, local_path)?;
                },
                TransferMode::Sftp => {
                    crate::utils::download_file_sftp(&server_config, &remote_path, local_path)?;
                },
                TransferMode::Auto => {
                    crate::utils::download_file_auto(&server_config, &remote_path, local_path)?;
                }
            }
        },
        
        Commands::Import { config, group, skip_existing } => {
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
            
            let mut configs = import_ssh_config(&config_path)?;
            
            if let Some(ref g) = group {
                for config in &mut configs {
                    config.group = Some(g.clone());
                }
            }
            
            let existing_servers = if skip_existing {
                config_manager.list_servers()?
            } else {
                Vec::new()
            };
            
            let mut imported = 0;
            let mut skipped = 0;
            
            for server_config in configs {
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
            let server_config = config_manager.get_server(&server)?;
            
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
                let servers = config.list_servers()?;
                servers.into_iter()
                    .find(|s| s.name.to_lowercase() == to.to_lowercase())
                    .ok_or_else(|| anyhow::anyhow!("目标服务器 '{}' 不存在，请使用 'rssh list' 查看可用服务器", to))?
            };
            println!("找到目标服务器: {} ({})", to_server.name, to_server.host);
            
            println!("检查 rclone 是否已安装...");
            RcloneConfig::ensure_rclone_installed()?;
            
            println!("初始化 rclone 配置...");
            let rclone_config = RcloneConfig::new()?;
            
            println!("配置源服务器...");
            rclone_config.configure_remote(&from_server)?;
            
            println!("配置目标服务器...");
            rclone_config.configure_remote(&to_server)?;
            
            println!("开始复制文件...");
            rclone_config.copy(&from_server, &from_path, &to_server, &to_path)?;
            println!("复制完成！");
        },

        Commands::SessionCreate { name, description, config } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            if let Some(config_path) = config {
                if !config_path.exists() {
                    return Err(anyhow::anyhow!("配置文件不存在: {}", config_path.display()));
                }
                
                let content = std::fs::read_to_string(&config_path)
                    .context(format!("无法读取配置文件: {}", config_path.display()))?;
                
                let parsed_config: toml::Value = toml::from_str(&content)
                    .context("无法解析TOML配置文件")?;
                
                let mut windows = Vec::new();
                let empty_table = toml::value::Table::new();
                
                if let Some(windows_table) = parsed_config.get("windows")
                    .and_then(|v| v.as_table()) 
                {
                    for (window_name, window_config) in windows_table {
                        let window_table = window_config.as_table().unwrap_or(&empty_table);
                        
                        let server = match window_table.get("server").and_then(|v| v.as_str()) {
                            Some(s) => s.to_string(),
                            None => {
                                eprintln!("警告: 窗口 '{}' 未指定服务器，将被跳过", window_name);
                                continue;
                            }
                        };
                        
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
                
                let session = session_manager.create_session(
                    name, 
                    description, 
                    windows,
                    Some(options)
                )?;
                
                println!("成功创建会话: {}", session.name);
            } else {
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
            
            println!("共找到 {} 个会话配置", sessions.len().to_string().bright_green().bold());
            
            println!("\n提示: 使用 {} 启动会话", "rssh session-start <ID或名称>".bright_yellow());
        },
        
        Commands::SessionEdit { session } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            let session_id = if session_manager.session_exists(&session) {
                session.clone()
            } else {
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s.id,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            let session_path = session_manager.get_session_path(&session_id);
            
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
            
            let session_id = if session_manager.session_exists(&session) {
                session.clone()
            } else {
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s.id,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            session_manager.remove_session(&session_id)?;
            println!("会话已删除");
        },
        
        Commands::SessionStart { session, tmux, kitty } => {
            let session_manager = SessionManager::new(get_session_dir()?)?;
            
            let session_config = if session_manager.session_exists(&session) {
                session_manager.load_session(&session)?
            } else {
                match session_manager.find_session_by_name(&session)? {
                    Some(s) => s,
                    None => return Err(anyhow::anyhow!("未找到会话: {}", session)),
                }
            };
            
            if session_config.windows.is_empty() {
                return Err(anyhow::anyhow!("会话 '{}' 没有配置窗口", session_config.name));
            }
            
            if kitty || (std::env::var("TERM").unwrap_or_default().contains("kitty") && !tmux) {
                start_session_with_kitty(&config_manager, &session_config)?;
            } else if tmux || std::env::var("TMUX").is_ok() {
                start_session_with_tmux(&config_manager, &session_config)?;
            } else {
                println!("警告: 未检测到支持多窗口的环境，将按顺序连接");
                
                for window in &session_config.windows {
                    let server_config = find_server(&config_manager, &window.server)?;
                    
                    println!("连接到 {}", server_config.name.bright_green());
                    
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

fn find_server(config_manager: &ConfigManager, server_id_or_name: &str) -> Result<ServerConfig> {
    let server_config = config_manager.get_server(server_id_or_name)?;
    
    let server_config = if server_config.is_none() {
        let servers = config_manager.list_servers()?;
        servers.into_iter().find(|s| s.name == server_id_or_name)
    } else {
        server_config
    };
    
    server_config.ok_or_else(|| anyhow::anyhow!("未找到服务器: {}", server_id_or_name))
}

fn start_session_with_kitty(config_manager: &ConfigManager, session: &SessionConfig) -> Result<()> {
    if !std::env::var("TERM").unwrap_or_default().contains("kitty") {
        return Err(anyhow::anyhow!("当前终端不是kitty"));
    }
    
    println!("使用kitty启动会话: {}", session.name.bright_green());
    
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

        let final_ssh_payload = if let Some(cmd) = &window.command {
            println!("  处理窗口 '{}': 找到命令, 准备上传脚本...", title);
            let unique_id = format!("{}_{}", session.id.split('-').next().unwrap_or("session"), i);
            let local_script_path = std::env::temp_dir().join(format!("rssh_local_init_{}.sh", unique_id));
            let remote_script_path = format!("/tmp/rssh_remote_init_{}.sh", unique_id);

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
            
            let upload_result = upload_command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            let _ = std::fs::remove_file(&local_script_path);
            println!("    本地临时脚本已删除: {}", local_script_path.display());

            match upload_result {
                Ok(upload_output) => {
                    if upload_output.status.success() {
                        println!("    上传成功 (退出码 0).");
                        let remote_script_escaped = shell_escape::escape(remote_script_path.into());
                        format!(
                            "'while [ ! -f {} ]; do sleep 0.1; done; chmod +x {} && {} && rm {} ; exec $SHELL'",
                            remote_script_escaped,
                            remote_script_escaped,
                            remote_script_escaped,
                            remote_script_escaped
                        )
                    } else {
                        eprintln!("    [Error] 上传失败 (退出码: {:?}). 将只启动交互式 shell.", upload_output.status.code());
                        if !upload_output.stdout.is_empty() {
                            eprintln!("      Upload stdout: {}", String::from_utf8_lossy(&upload_output.stdout));
                        }
                        if !upload_output.stderr.is_empty() {
                            eprintln!("      Upload stderr: {}", String::from_utf8_lossy(&upload_output.stderr));
                        }
                        "''".to_string()
                    }
                },
                Err(e) => {
                     eprintln!("    [Error] 执行 'rssh upload' 命令本身失败: {}. 将只启动交互式 shell.", e);
                     "''".to_string()
                }
            }
        } else {
             println!("  处理窗口 '{}': 无初始命令，直接启动交互式 shell.", title);
            "''".to_string()
        };

        let final_ssh_cmd = format!("ssh -t {} {}", base_ssh_args, final_ssh_payload);
        println!("    最终 SSH 命令: {}", final_ssh_cmd);

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

    println!("执行启动脚本以打开 Kitty 窗口...");
    
    let _ = std::process::Command::new(&launch_script_path)
        .spawn()
        .context("无法执行启动脚本")?;

    std::thread::sleep(std::time::Duration::from_millis(500)); 

    let _ = std::fs::remove_file(&launch_script_path);
    println!("本地启动脚本已删除: {}", launch_script_path.display());

    println!("会话已在新窗口启动。远程脚本执行后将被自动删除。");
    println!("会话配置文件保留在: {}", tmp_session_file.display());
    
    Ok(())
}

fn start_session_with_tmux(config_manager: &ConfigManager, session: &SessionConfig) -> Result<()> {
    let tmux_check = std::process::Command::new("which")
        .arg("tmux")
        .stdout(std::process::Stdio::null())
        .status();
    
    if tmux_check.is_err() || !tmux_check.unwrap().success() {
        return Err(anyhow::anyhow!("未找到tmux命令"));
    }
    
    println!("使用tmux启动会话: {}", session.name.bright_green());
    
    let tmux_session_name = format!("rssh_{}", session.id.split('-').next().unwrap_or("session"));
    
    let create_status = std::process::Command::new("tmux")
        .args(["new-session", "-d", "-s", &tmux_session_name])
        .status()
        .context("无法创建tmux会话")?;
    
    if !create_status.success() {
        return Err(anyhow::anyhow!("无法创建tmux会话"));
    }
    
    for (i, window) in session.windows.iter().enumerate() {
        let server_config = find_server(config_manager, &window.server)?;
        
        let mut ssh_cmd = format!("ssh {}@{} -p {}", 
            server_config.username, 
            server_config.host, 
            server_config.port);
        
        if let Some(key_path) = server_config.auth_type.get_key_path() {
            ssh_cmd.push_str(&format!(" -i {}", key_path));
        }
        
        if let Some(cmd) = &window.command {
            ssh_cmd.push_str(&format!(" '{}'", cmd.replace("'", "'\''")));
        }
        
        let title = window.title.as_deref().unwrap_or(&window.server);
        
        if i == 0 {
            std::process::Command::new("tmux")
                .args(["rename-window", "-t", &format!("{}:0", tmux_session_name), title])
                .status()?;
            
            std::process::Command::new("tmux")
                .args(["send-keys", "-t", &format!("{}:0", tmux_session_name), &ssh_cmd, "Enter"])
                .status()?;
        } else {
            std::process::Command::new("tmux")
                .args(["new-window", "-t", &tmux_session_name, "-n", title])
                .status()?;
            
            std::process::Command::new("tmux")
                .args(["send-keys", "-t", &format!("{}:{}", tmux_session_name, i), &ssh_cmd, "Enter"])
                .status()?;
        }
    }
    
    std::process::Command::new("tmux")
        .args(["attach-session", "-t", &tmux_session_name])
        .status()
        .context("无法附加到tmux会话")?;
    
    Ok(())
} 