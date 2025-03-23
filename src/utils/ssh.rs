use anyhow::{Context, Result};
use ssh2::Session;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use std::fs::File;

use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::expand_tilde;

// 调试日志函数
fn debug_log(msg: &str) -> std::io::Result<()> {
    // 创建或追加到调试日志文件
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/rssh_debug.log")?;
    
    // 添加时间戳
    let now = chrono::Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    
    // 写入消息
    writeln!(file, "[{}] {}", timestamp, msg)?;
    file.flush()?;
    
    Ok(())
}

pub struct SshClient {
    session: Session,
    _stream: TcpStream,
}

impl SshClient {
    pub fn connect(server: &ServerConfig) -> Result<Self> {
        let addr = format!("{}:{}", server.host, server.port);
        
        let tcp = TcpStream::connect(&addr)
            .with_context(|| format!("无法连接到服务器 {}", addr))?;
        
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
        
        let mut sess = Session::new()
            .with_context(|| "无法创建SSH会话")?;
        
        sess.set_tcp_stream(tcp.try_clone()?);
        sess.handshake()
            .with_context(|| "SSH握手失败")?;
        
        match &server.auth_type {
            AuthType::Password(password) => {
                sess.userauth_password(&server.username, password)
                    .with_context(|| "密码认证失败")?;
            },
            AuthType::Key(key_path) => {
                let expanded_path = expand_tilde(key_path);
                let key_path = Path::new(&expanded_path);
                sess.userauth_pubkey_file(
                    &server.username,
                    None,
                    key_path,
                    None,
                )
                .with_context(|| format!("密钥认证失败，路径: {}", key_path.display()))?;
            },
            AuthType::Agent => {
                let mut agent = sess.agent()
                    .with_context(|| "无法连接到SSH代理")?;
                
                agent.connect()
                    .with_context(|| "连接SSH代理失败")?;
                
                agent.list_identities()
                    .with_context(|| "无法列出SSH代理身份")?;
                
                let identities = agent.identities()
                    .with_context(|| "读取SSH代理身份失败")?;
                
                if identities.is_empty() {
                    return Err(anyhow::anyhow!("SSH代理中没有可用的身份"));
                }
                
                let authenticated = identities.iter().any(|identity| {
                    agent.userauth(&server.username, identity).is_ok()
                });
                
                if !authenticated {
                    return Err(anyhow::anyhow!("SSH代理认证失败"));
                }
            }
        }
        
        if !sess.authenticated() {
            return Err(anyhow::anyhow!("SSH认证失败"));
        }
        
        Ok(SshClient {
            session: sess,
            _stream: tcp,
        })
    }
    
    pub fn execute_command(&self, command: &str) -> Result<(String, String, i32)> {
        let mut channel = self.session.channel_session()
            .with_context(|| "无法创建SSH通道")?;
        
        channel.exec(command)
            .with_context(|| format!("执行命令失败: {}", command))?;
        
        let mut stdout = String::new();
        channel.read_to_string(&mut stdout)
            .with_context(|| "读取标准输出失败")?;
        
        let mut stderr = String::new();
        channel.stderr().read_to_string(&mut stderr)
            .with_context(|| "读取标准错误失败")?;
        
        channel.wait_close()
            .with_context(|| "等待通道关闭失败")?;
        
        let exit_status = channel.exit_status()
            .with_context(|| "获取退出状态失败")?;
        
        Ok((stdout, stderr, exit_status))
    }
    
    pub fn start_shell(&self) -> Result<()> {
        debug_log("开始启动SSH交互式shell")?;
        
        let mut channel = self.session.channel_session()
            .with_context(|| "无法创建SSH通道")?;
        
        debug_log("SSH通道创建成功")?;
        
        // 获取终端大小
        let term_size = terminal_size();
        debug_log(&format!("终端大小: {}x{}", term_size.0, term_size.1))?;
        
        // 请求PTY，正确设置终端大小参数
        debug_log("请求PTY")?;
        channel.request_pty("xterm-256color", None, Some((
            term_size.0 as u32,   // 终端宽度
            term_size.1 as u32,   // 终端高度
            0,                   // 像素宽度（可选）
            0                    // 像素高度（可选）
        )))
        .with_context(|| "请求PTY失败")?;
        
        debug_log("正在启动shell")?;
        channel.shell()
            .with_context(|| "启动Shell失败")?;
        
        // 设置信号处理，优雅退出
        debug_log("设置信号处理程序")?;
        let running = Arc::new(AtomicBool::new(true));
        let r_clone = running.clone();
        
        #[cfg(unix)]
        let _ = ctrlc::set_handler(move || {
            r_clone.store(false, Ordering::SeqCst);
            debug_log("接收到Ctrl+C信号，准备关闭连接").unwrap_or(());
            eprintln!("\r\n正在关闭连接...");
        });
        
        // 主要的交互式shell实现
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            use libc::{self, fcntl, F_GETFL, F_SETFL, O_NONBLOCK};
            
            let stdin_fd = std::io::stdin().as_raw_fd();
            let stdout_fd = std::io::stdout().as_raw_fd();
            
            debug_log(&format!("stdin_fd={}, stdout_fd={}", stdin_fd, stdout_fd))?;
            
            // 保存原始状态
            let original_flags = unsafe { fcntl(stdin_fd, F_GETFL, 0) };
            debug_log(&format!("原始终端标志: {}", original_flags))?;
            
            let mut termios_org = termios::Termios::from_fd(stdin_fd)?;
            let termios_backup = termios_org.clone();
            
            // 设置终端为原始模式
            debug_log("设置终端为原始模式")?;
            termios::cfmakeraw(&mut termios_org);
            termios::tcsetattr(stdin_fd, termios::TCSANOW, &termios_org)?;
            
            // 设置非阻塞模式
            debug_log("设置终端为非阻塞模式")?;
            unsafe { fcntl(stdin_fd, F_SETFL, original_flags | O_NONBLOCK) };
            
            // 创建缓冲区
            let mut stdin_buf = [0u8; 1024];
            let mut channel_buf = [0u8; 4096];
            
            println!("连接成功，按Ctrl+C退出。");
            debug_log("进入主循环")?;
            
            // 尝试一种不同的方法 - 将Channel设置为非阻塞模式
            debug_log("通道模式：非阻塞I/O")?;
            // 注意：Channel没有set_blocking方法，我们只使用非阻塞的stdin
            
            // 支持键盘输入调试模式
            let mut debug_mode = false;
            
            // 主循环
            while running.load(Ordering::SeqCst) {
                // 检查stdin是否有数据可读（非阻塞模式）
                let read_result = unsafe { 
                    libc::read(stdin_fd, stdin_buf.as_mut_ptr() as *mut libc::c_void, stdin_buf.len()) 
                };
                
                if read_result > 0 {
                    debug_log(&format!("从stdin读取了{}字节数据", read_result))?;
                    
                    // 检查是否启用调试模式（按Alt+D）
                    if read_result >= 2 && stdin_buf[0] == 27 && stdin_buf[1] == 'd' as u8 {
                        debug_mode = !debug_mode;
                        debug_log(&format!("调试模式: {}", if debug_mode { "开启" } else { "关闭" }))?;
                        continue;
                    }
                    
                    // 在调试模式下显示按键代码
                    if debug_mode {
                        let mut key_codes = String::new();
                        for i in 0..read_result as usize {
                            key_codes.push_str(&format!("{} ", stdin_buf[i]));
                        }
                        debug_log(&format!("键盘输入: {}", key_codes))?;
                    }
                    
                    // 将数据发送到远程
                    match channel.write(&stdin_buf[0..read_result as usize]) {
                        Ok(n) => {
                            debug_log(&format!("向channel写入了{}字节数据", n))?;
                            // 确保所有数据都发送出去
                            if n < read_result as usize {
                                debug_log("未能发送所有数据！")?;
                            }
                            
                            // 尝试立即刷新通道
                            if let Err(e) = channel.flush() {
                                debug_log(&format!("刷新通道失败: {}", e))?;
                            } else {
                                debug_log("通道刷新成功")?;
                            }
                        },
                        Err(e) => {
                            debug_log(&format!("写入channel失败: {}", e))?;
                            break;
                        }
                    }
                } else if read_result < 0 {
                    let err = io::Error::last_os_error();
                    // EAGAIN和EWOULDBLOCK表示没有数据可读，不是真正的错误
                    if err.kind() != io::ErrorKind::WouldBlock {
                        debug_log(&format!("读取stdin错误: {:?}", err))?;
                        break;
                    }
                }
                
                // 检查channel是否有数据（非阻塞尝试读取）
                match channel.read(&mut channel_buf) {
                    Ok(n) if n > 0 => {
                        debug_log(&format!("从channel读取了{}字节数据", n))?;
                        
                        // 显示远程返回数据的十六进制表示（在调试模式下）
                        if debug_mode {
                            let mut hex_data = String::new();
                            for i in 0..std::cmp::min(n, 50) {
                                hex_data.push_str(&format!("{:02X} ", channel_buf[i]));
                            }
                            debug_log(&format!("从远程收到数据: {}", hex_data))?;
                        }
                        
                        let write_result = unsafe { 
                            libc::write(stdout_fd, channel_buf.as_ptr() as *const libc::c_void, n) 
                        };
                        
                        if write_result < 0 {
                            let err = io::Error::last_os_error();
                            debug_log(&format!("写入stdout错误: {:?}", err))?;
                            break;
                        } else {
                            debug_log(&format!("向stdout写入了{}字节数据", write_result))?;
                        }
                        
                        // 刷新stdout
                        unsafe { libc::fsync(stdout_fd) };
                    },
                    Ok(0) => {
                        debug_log("通道已关闭 (EOF)")?;
                        break; // 通道关闭
                    },
                    Ok(n) => {
                        debug_log(&format!("从channel读取了{}字节数据（意外情况）", n))?;
                    },
                    Err(e) => {
                        // 如果错误不是"WouldBlock"，则说明出现了实际错误
                        if e.kind() != io::ErrorKind::WouldBlock {
                            debug_log(&format!("读取channel错误: {:?}", e))?;
                            break;
                        }
                    }
                }
                
                // 短暂休眠以避免CPU使用率过高
                std::thread::sleep(Duration::from_millis(5));
            }
            
            // 恢复终端设置
            debug_log("恢复终端设置")?;
            termios::tcsetattr(stdin_fd, termios::TCSANOW, &termios_backup)?;
            unsafe { fcntl(stdin_fd, F_SETFL, original_flags) };
            
            // 确认通道关闭
            debug_log("关闭SSH通道")?;
            let _ = channel.close();
            let _ = channel.wait_close();
            
            println!("\r\n连接已关闭");
            debug_log("连接已关闭")?;
        }
        
        #[cfg(not(unix))]
        {
            // 非Unix系统使用简单的阻塞I/O
            let mut stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            
            // 在单独的线程中处理从远程到本地的数据流
            let mut channel_clone = channel.try_clone()?;
            let r_clone = running.clone();
            
            std::thread::spawn(move || {
                let mut buf = [0; 4096];
                while r_clone.load(Ordering::SeqCst) {
                    match channel_clone.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Err(_) = stdout.write_all(&buf[..n]) {
                                break;
                            }
                            if let Err(_) = stdout.flush() {
                                break;
                            }
                        },
                        Err(_) => break,
                    }
                }
            });
            
            // 在主线程中处理从本地到远程的数据流
            let mut buf = [0; 1024];
            while running.load(Ordering::SeqCst) {
                match stdin.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Err(_) = channel.write_all(&buf[..n]) {
                            break;
                        }
                    },
                    Err(_) => break,
                }
            }
            
            println!("\r\n连接已关闭");
        }
        
        Ok(())
    }
}

// 获取终端大小
pub fn terminal_size() -> (usize, usize) {
    #[cfg(unix)]
    {
        use libc::{ioctl, winsize, STDOUT_FILENO, TIOCGWINSZ};
        
        let mut ws = winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        
        // 获取终端大小
        if unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut ws) } == -1 {
            return (80, 24); // 默认值
        }
        
        (ws.ws_col as usize, ws.ws_row as usize)
    }
    
    #[cfg(not(unix))]
    {
        (80, 24) // 非Unix系统使用默认值
    }
} 