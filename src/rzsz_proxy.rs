use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::env;
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// rzsz命令的特征码
const RZ_COMMAND: &[u8] = b"rz\r";
const SZ_COMMAND_PREFIX: &[u8] = b"sz ";
const ZMODEM_START: &[u8] = b"**\x18B00000000000000\r\n";

fn main() -> io::Result<()> {
    // 创建一个共享的原子布尔值，用于控制程序退出
    let running = Arc::new(AtomicBool::new(true));
    
    // 克隆一份用于信号处理
    let r = running.clone();
    
    // 设置Ctrl+C信号处理
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("无法设置Ctrl+C处理程序");
    
    println!("RSSH-RZSZ代理启动...");
    
    // 检查是否从环境变量提供SSH连接信息
    let host = env::var("RSSH_HOST").ok();
    let port = env::var("RSSH_PORT").ok();
    let user = env::var("RSSH_USER").ok();
    let key = env::var("RSSH_KEY").ok();
    
    let args: Vec<String>;
    
    if host.is_some() && user.is_some() {
        // 从环境变量构建SSH命令
        println!("从环境变量获取SSH连接信息");
        println!("主机: {} 用户: {} 端口: {}", 
            host.as_ref().unwrap(), 
            user.as_ref().unwrap(), 
            port.as_ref().unwrap_or(&"22".to_string()));
        
        let mut ssh_args = Vec::new();
        
        // 获取ssh命令的完整路径
        let ssh_cmd = "ssh"; // 使用系统默认的SSH
        
        ssh_args.push(ssh_cmd.to_string());
        
        // 添加端口
        if let Some(port_str) = port {
            ssh_args.push("-p".to_string());
            ssh_args.push(port_str);
        }
        
        // 添加密钥
        if let Some(key_path) = key {
            ssh_args.push("-i".to_string());
            ssh_args.push(key_path);
        }
        
        // 添加常用SSH选项
        ssh_args.push("-o".to_string());
        ssh_args.push("StrictHostKeyChecking=no".to_string());
        
        ssh_args.push("-o".to_string());
        ssh_args.push("HashKnownHosts=no".to_string());
        
        ssh_args.push("-o".to_string());
        ssh_args.push("ServerAliveInterval=60".to_string());
        
        // 添加用户和主机
        ssh_args.push(format!("{}@{}", user.unwrap(), host.unwrap()));
        
        args = ssh_args;
    } else {
        // 使用命令行参数
        args = env::args().skip(1).collect();
        
        if args.is_empty() {
            eprintln!("错误: 没有提供SSH命令，也没有设置环境变量");
            return Ok(());
        }
    }
    
    println!("启动SSH命令: {}", args.join(" "));
    
    // 创建子进程
    println!("准备启动SSH命令：{}", &args[0]);
    println!("参数: {}", args[1..].join(" "));
    
    let cmd_result = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn();
    
    let mut cmd = match cmd_result {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("启动SSH命令失败：{}", e);
            if e.kind() == io::ErrorKind::NotFound {
                eprintln!("找不到命令: {}", &args[0]);
                eprintln!("请确保SSH已正确安装并在PATH中");
            } else if e.kind() == io::ErrorKind::PermissionDenied {
                eprintln!("权限被拒绝: {}", &args[0]);
                eprintln!("请确保命令有执行权限");
            }
            return Err(e);
        }
    };
    
    // 获取子进程的标准输入和输出
    let mut child_stdin = cmd.stdin.take().expect("无法获取子进程stdin");
    let mut child_stdout = cmd.stdout.take().expect("无法获取子进程stdout");
    
    // 创建缓冲区
    let mut stdin_buf = [0u8; 1024];
    let mut stdout_buf = [0u8; 4096];
    
    // 将标准输入设置为非阻塞模式 (仅Linux/MacOS)
    #[cfg(unix)]
    set_stdin_nonblocking();
    
    // 创建输出线程
    let r_clone = running.clone();
    let stdout_thread = thread::spawn(move || {
        let mut zmodem_mode = false;
        
        while r_clone.load(Ordering::SeqCst) {
            // 从SSH读取数据
            match child_stdout.read(&mut stdout_buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    // 检查是否是ZMODEM数据开始
                    if !zmodem_mode && contains_zmodem_data(&stdout_buf[0..n]) {
                        println!("检测到ZMODEM数据传输开始");
                        zmodem_mode = true;
                        
                        // 启动rz下载助手 (在单独的线程中)
                        thread::spawn(|| {
                            if let Err(e) = start_zmodem_download() {
                                eprintln!("启动下载助手失败: {}", e);
                            }
                        });
                        
                        // 等待rz程序启动
                        thread::sleep(Duration::from_millis(500));
                    }
                    
                    // 将数据写入标准输出
                    io::stdout().write_all(&stdout_buf[0..n]).ok();
                    io::stdout().flush().ok();
                },
                Err(e) => {
                    // 只报告非阻塞错误
                    if e.kind() != io::ErrorKind::WouldBlock {
                        eprintln!("从SSH读取错误: {}", e);
                        break;
                    }
                }
            }
            
            // 短暂休眠
            thread::sleep(Duration::from_millis(5));
        }
    });
    
    // 主线程处理标准输入
    while running.load(Ordering::SeqCst) {
        // 从标准输入读取
        let read_result = io::stdin().read(&mut stdin_buf);
        
        match read_result {
            Ok(0) => break, // EOF
            Ok(n) => {
                // 检测rz/sz命令
                if is_rz_command(&stdin_buf[0..n]) {
                    println!("检测到rz命令");
                    
                    // 检查是否在Kitty终端
                    let is_kitty = std::env::var("TERM").map(|val| val == "xterm-kitty").unwrap_or(false);
                    if is_kitty {
                        println!("\n提示: 您正在使用Kitty终端，建议使用Kitty的文件传输协议代替rz/sz。");
                        println!("命令示例:");
                        println!("  上传: kitty +kitten transfer 本地文件路径");
                        println!("  下载: kitty +kitten transfer --direction=receive 远程文件路径");
                        println!("或者直接使用rssh命令:");
                        println!("  上传: rssh upload 服务器名 本地文件路径 [远程路径] --mode kitty");
                        println!("  下载: rssh download 服务器名 远程文件路径 [本地路径] --mode kitty\n");
                    }
                    
                    // 处理rz命令 (上传文件到远程)
                    if let Ok(file_path) = select_file_dialog() {
                        if !file_path.is_empty() {
                            println!("上传文件: {}", file_path);
                            
                            // 将rz命令发送给SSH
                            child_stdin.write_all(&stdin_buf[0..n]).ok();
                            child_stdin.flush().ok();
                            
                            // 启动sz上传助手 (在单独的线程中)
                            thread::spawn(move || {
                                thread::sleep(Duration::from_millis(300));
                                if let Err(e) = start_zmodem_upload(&file_path) {
                                    eprintln!("启动上传助手失败: {}", e);
                                }
                            });
                            
                            continue;
                        }
                    }
                }
                else if is_sz_command(&stdin_buf[0..n]) {
                    println!("检测到sz命令");
                    
                    // 检查是否在Kitty终端
                    let is_kitty = std::env::var("TERM").map(|val| val == "xterm-kitty").unwrap_or(false);
                    if is_kitty {
                        println!("\n提示: 您正在使用Kitty终端，建议使用Kitty的文件传输协议代替rz/sz。");
                        println!("命令示例:");
                        println!("  上传: kitty +kitten transfer 本地文件路径");
                        println!("  下载: kitty +kitten transfer --direction=receive 远程文件路径");
                        println!("或者直接使用rssh命令:");
                        println!("  上传: rssh upload 服务器名 本地文件路径 [远程路径] --mode kitty");
                        println!("  下载: rssh download 服务器名 远程文件路径 [本地路径] --mode kitty\n");
                    }
                    
                    // 将sz命令发送给SSH (下载文件会在stdout线程处理)
                }
                
                // 将数据发送给SSH
                if let Err(e) = child_stdin.write_all(&stdin_buf[0..n]) {
                    eprintln!("发送数据到SSH失败: {}", e);
                    break;
                }
                
                if let Err(e) = child_stdin.flush() {
                    eprintln!("刷新SSH输入失败: {}", e);
                    break;
                }
            },
            Err(e) => {
                // 只报告非阻塞错误
                if e.kind() != io::ErrorKind::WouldBlock {
                    eprintln!("读取标准输入错误: {}", e);
                    break;
                }
            }
        }
        
        // 短暂休眠
        thread::sleep(Duration::from_millis(5));
    }
    
    // 等待子进程结束
    let _ = cmd.wait();
    
    // 等待输出线程结束
    let _ = stdout_thread.join();
    
    println!("代理程序退出");
    Ok(())
}

/// 设置标准输入为非阻塞模式 (仅Unix系统)
#[cfg(unix)]
fn set_stdin_nonblocking() {
    use std::os::unix::io::AsRawFd;
    use libc::{fcntl, F_GETFL, F_SETFL, O_NONBLOCK};
    
    let stdin_fd = io::stdin().as_raw_fd();
    unsafe {
        let flags = fcntl(stdin_fd, F_GETFL, 0);
        fcntl(stdin_fd, F_SETFL, flags | O_NONBLOCK);
    }
}

/// 检测输入是否是rz命令
fn is_rz_command(data: &[u8]) -> bool {
    data.len() >= 3 && &data[0..3] == RZ_COMMAND
}

/// 检测输入是否是sz命令
fn is_sz_command(data: &[u8]) -> bool {
    data.len() >= 3 && &data[0..3] == SZ_COMMAND_PREFIX
}

/// 检测输出是否包含ZMODEM数据
fn contains_zmodem_data(data: &[u8]) -> bool {
    // 更精确地检测ZMODEM的启动序列
    if data.len() < 4 {
        return false;
    }
    
    // 查找ZMODEM头部标识: "**\x18B"
    for i in 0..data.len()-4 {
        if data[i] == b'*' && data[i+1] == b'*' && data[i+2] == 0x18 && data[i+3] == b'B' {
            return true;
        }
    }
    
    false
}

/// 启动文件上传助手 (使用sz命令)
fn start_zmodem_upload(file_path: &str) -> io::Result<()> {
    println!("准备上传文件: {}", file_path);
    
    // 创建一个临时脚本来执行sz命令
    let script = format!(r#"
        #!/bin/bash
        sleep 0.5
        echo "开始上传文件: {}"
        # 使用更多选项提高兼容性
        # -e: 转义控制字符
        # -y: 覆盖已存在的文件
        # -v: 详细模式
        # -b: 二进制模式
        # -O: 使用16比特CRC
        # 增加超时时间到30秒
        sz -veybO --timeout=30 "{}" > /tmp/sz_debug.log 2>&1
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
            echo "文件上传成功！"
        else
            echo "文件上传失败，错误码: $exit_code (详情请查看 /tmp/sz_debug.log)"
            tail -n 20 /tmp/sz_debug.log
            
            echo ""
            echo "可能的问题:"
            echo "1. 终端不支持完整的ZMODEM协议"
            echo "2. 传输超时 - 文件太大或网络延迟高"
            echo "3. 本地lrzsz版本与远程不兼容"
            echo ""
            echo "尝试使用直接的命令进行文件传输:"
            echo "rssh upload remote_host {} /path/to/remote/dir/"
        fi
    "#, file_path, file_path, file_path);
    
    let temp_script = create_temp_script("rssh_rzsz_upload.sh", &script)?;
    
    // 启动脚本
    let mut cmd = Command::new(&temp_script)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    
    // 等待完成
    let _ = cmd.wait();
    
    // 删除临时脚本
    let _ = std::fs::remove_file(temp_script);
    
    Ok(())
}

/// 启动文件下载助手 (使用rz命令)
fn start_zmodem_download() -> io::Result<()> {
    println!("检测到ZMODEM下载请求，准备接收文件");
    
    // 创建一个临时脚本来执行rz命令，使用更多选项以提高兼容性
    let script = r#"
        #!/bin/bash
        sleep 0.5
        echo "开始下载文件..."
        cd "$HOME"  # 切换到用户主目录
        
        # 使用更多的rz选项来提高兼容性
        # -e: 转义控制字符
        # -y: 覆盖已存在的文件
        # -v: 详细模式
        # -b: 二进制模式
        # -O: 使用16比特CRC
        # 增加超时时间到30秒
        rz -veyb -O --timeout=30 > /tmp/rz_debug.log 2>&1
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
            echo "文件下载成功！保存在 $(pwd)"
            ls -l | grep -E "^-" | tail -n 1  # 显示最新下载的文件
        else
            echo "文件下载失败，错误码: $exit_code (详情请查看 /tmp/rz_debug.log)"
            tail -n 20 /tmp/rz_debug.log
            
            echo ""
            echo "可能的问题:"
            echo "1. 终端不支持完整的ZMODEM协议"
            echo "2. 传输超时 - 文件太大或网络延迟高"
            echo "3. 本地lrzsz版本与远程不兼容"
            echo ""
            echo "尝试使用直接的命令进行文件传输:"
            echo "rssh download remote_host /path/to/remote/file local_file"
        fi
    "#;
    
    let temp_script = create_temp_script("rssh_rzsz_download.sh", script)?;
    
    // 启动脚本
    let mut cmd = Command::new(&temp_script)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    
    // 等待完成
    let _ = cmd.wait();
    
    // 删除临时脚本
    let _ = std::fs::remove_file(temp_script);
    
    Ok(())
}

/// 创建临时脚本文件
fn create_temp_script(name: &str, content: &str) -> io::Result<PathBuf> {
    let mut temp_path = std::env::temp_dir();
    temp_path.push(name);
    
    let mut file = File::create(&temp_path)?;
    file.write_all(content.as_bytes())?;
    
    // 在Unix系统上设置执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_path, perms)?;
    }
    
    Ok(temp_path)
}

/// 弹出文件选择对话框
fn select_file_dialog() -> io::Result<String> {
    // 首先尝试使用GUI选择器
    if let Ok(path) = try_gui_file_selection() {
        return Ok(path);
    }
    
    // 回退到命令行
    println!("请输入要上传的文件完整路径:");
    let mut path = String::new();
    io::stdin().read_line(&mut path)?;
    
    let path = path.trim().to_string();
    if path.is_empty() || !Path::new(&path).exists() {
        println!("文件不存在或路径为空");
        return Ok(String::new());
    }
    
    Ok(path)
}

/// 尝试使用GUI文件选择器
fn try_gui_file_selection() -> io::Result<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args(["-e", "POSIX path of (choose file)"])
            .output()?;
        
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(path);
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // 尝试使用 zenity
        let output = Command::new("zenity")
            .args(["--file-selection"])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(path);
            }
        }
    }
    
    // 如果无法使用GUI，返回错误
    Err(io::Error::new(io::ErrorKind::NotFound, "无法使用GUI文件选择器"))
} 