use anyhow::{Context, Result};
use async_trait::async_trait;
use russh::{client, ChannelId};
use russh_keys::key;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::expand_tilde;

// SSH客户端处理程序
struct Handler {
    connection_success: bool,
}

impl Handler {
    fn new() -> Self {
        Handler {
            connection_success: false,
        }
    }
}

// 使用async_trait和最新版API实现Handler
#[async_trait]
impl client::Handler for Handler {
    type Error = anyhow::Error;
    
    async fn check_server_key(
        self,
        _server_public_key: &key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        // 简化起见，接受所有服务器密钥
        Ok((self, true))
    }

    async fn channel_open_confirmation(
        self,
        _channel: ChannelId,
        _max_packet_size: u32,
        _window_size: u32,
        session: client::Session,
    ) -> Result<(Self, client::Session), Self::Error> {
        // 通道打开成功
        let mut this = self;
        this.connection_success = true;
        Ok((this, session))
    }

    async fn data(
        self,
        _channel: ChannelId,
        data: &[u8],
        session: client::Session,
    ) -> Result<(Self, client::Session), Self::Error> {
        // 接收到数据，打印到标准输出
        let data_vec = data.to_vec();
        tokio::spawn(async move {
            if let Err(e) = tokio::io::stdout().write_all(&data_vec).await {
                eprintln!("写入stdout失败: {}", e);
            }
            if let Err(e) = tokio::io::stdout().flush().await {
                eprintln!("刷新stdout失败: {}", e);
            }
        });
        Ok((self, session))
    }

    async fn extended_data(
        self,
        _channel: ChannelId,
        _data_type: u32,
        data: &[u8],
        session: client::Session,
    ) -> Result<(Self, client::Session), Self::Error> {
        // 接收到扩展数据（通常是stderr），打印到stderr
        let data_vec = data.to_vec();
        tokio::spawn(async move {
            if let Err(e) = tokio::io::stderr().write_all(&data_vec).await {
                eprintln!("写入stderr失败: {}", e);
            }
            if let Err(e) = tokio::io::stderr().flush().await {
                eprintln!("刷新stderr失败: {}", e);
            }
        });
        Ok((self, session))
    }
}

// 使用russh库连接远程服务器
pub async fn connect_with_russh(server: &ServerConfig) -> Result<()> {
    // 配置客户端
    let config = client::Config {
        // 配置客户端参数
        // 注意：根据russh 0.40.1版本的API，没有connection_timeout字段
        ..Default::default()
    };

    let config = Arc::new(config);
    let handler = Handler::new();

    // 解析服务器地址
    let socket_addr = format!("{}:{}", server.host, server.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("无法解析服务器地址"))?;

    // 连接到服务器
    println!("正在使用russh连接到 {}@{}:{}...", 
        server.username, 
        server.host, 
        server.port
    );

    let mut session = client::connect(config, socket_addr, handler).await
        .with_context(|| "无法连接到服务器")?;

    // 进行认证
    match &server.auth_type {
        AuthType::Password(password) => {
            let auth_success = session.authenticate_password(&server.username, password).await
                .with_context(|| "密码认证失败")?;
            if !auth_success {
                return Err(anyhow::anyhow!("认证失败：服务器拒绝了密码"));
            }
        },
        AuthType::Key(key_path) => {
            let expanded_path = expand_tilde(key_path);
            
            match russh_keys::load_secret_key(&expanded_path, None) {
                Ok(key_pair) => {
                    let auth_success = session.authenticate_publickey(&server.username, Arc::new(key_pair)).await
                        .with_context(|| "密钥认证失败")?;
                    
                    if !auth_success {
                        return Err(anyhow::anyhow!("认证失败：服务器拒绝了密钥"));
                    }
                },
                Err(e) => {
                    if e.to_string().contains("ssh-rsa") {
                        return Err(anyhow::anyhow!(
                            "无法加载SSH-RSA类型的密钥: {}\n\
                             原因: 当前使用的russh库不支持ssh-rsa密钥格式\n\
                             解决方案: 请使用--mode system或--mode exec连接模式，\n\
                             或者生成更新的密钥类型如ED25519: ssh-keygen -t ed25519", 
                             expanded_path));
                    } else {
                        return Err(anyhow::anyhow!("无法加载私钥: {}\n原因: {}", expanded_path, e));
                    }
                }
            }
        },
        AuthType::Agent => {
            return Err(anyhow::anyhow!("Russh模式暂不支持SSH Agent认证"));
        }
    }

    // 打开通道
    let mut channel = session.channel_open_session().await
        .with_context(|| "无法打开会话通道")?;

    // 设置终端大小
    let terminal_size = crate::utils::ssh::terminal_size();
    let (width, height) = (terminal_size.0 as u32, terminal_size.1 as u32);

    // 请求PTY
    channel.request_pty(
        true, 
        "xterm-256color", 
        width, height, 
        0, 0, 
        &[]
    ).await
        .with_context(|| "无法请求PTY")?;

    // 请求shell
    channel.request_shell(true).await
        .with_context(|| "无法请求shell")?;

    println!("已连接，启动交互式shell...");

    // 设置标准输入
    let stdin = tokio::io::stdin();
    let mut stdin_reader = tokio::io::BufReader::new(stdin);

    // 处理用户输入并发送到远程
    let mut buffer = [0u8; 1024];
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    // 处理Ctrl+C
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })?;

    // 主循环
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        // 使用tokio的select在多个异步任务之间选择
        tokio::select! {
            // 读取标准输入
            result = async {
                stdin_reader.read(&mut buffer).await
            } => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        // 发送数据到远程
                        if let Err(e) = channel.data(&buffer[0..n]).await {
                            eprintln!("发送数据失败: {}", e);
                            break;
                        }
                    },
                    Err(e) => {
                        eprintln!("读取标准输入失败: {}", e);
                        break;
                    }
                }
            },
            // 定期检查退出条件
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                continue;
            }
        }
    }

    // 关闭连接
    let _ = channel.eof().await;
    let _ = channel.close().await;
    session.disconnect(russh::Disconnect::ByApplication, "会话结束", "").await?;

    println!("\n连接已关闭");
    Ok(())
}

// 使用russh库进行连接的入口函数
pub fn russh_connect(server: &ServerConfig) -> Result<()> {
    // 创建tokio运行时
    let runtime = tokio::runtime::Runtime::new()
        .with_context(|| "无法创建tokio运行时")?;
    
    // 在tokio运行时中执行异步连接函数
    let result = runtime.block_on(connect_with_russh(server));
    
    // 处理错误，提供使用system模式的建议
    if let Err(err) = &result {
        if err.to_string().contains("Unsupported key type") || 
           err.to_string().contains("无法加载私钥") {
            eprintln!("\n注意: Russh模式不支持某些类型的SSH密钥。");
            eprintln!("推荐使用system模式，它具有最佳兼容性：");
            eprintln!("  rssh connect {} --mode system\n", server.name);
        }
    }
    
    result
} 