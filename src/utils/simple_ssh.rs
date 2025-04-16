use anyhow::{Context, Result};
use std::process::{Command, Stdio};

use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::expand_tilde;
use crate::utils::kitty_transfer::is_kitty_available;

// 使用基于子进程的方法
// 这个实现直接使用系统的ssh命令，绕过Rust的SSH库
pub fn connect_via_system_ssh(server: &ServerConfig, use_rzsz: bool, use_kitten: bool) -> Result<i32> {
    connect_via_system_ssh_with_command(server, None, use_rzsz, use_kitten)
}

// 支持命令的版本
pub fn connect_via_system_ssh_with_command(
    server: &ServerConfig, 
    command: Option<String>, 
    use_rzsz: bool, 
    use_kitten: bool
) -> Result<i32> {
    // 检查是否使用kitty的kitten ssh
    let use_kitty_kitten = use_kitten && is_kitty_available();
    
    // 获取系统ssh命令的完整路径
    let ssh_path = if use_kitty_kitten {
        std::path::PathBuf::from("kitty")
    } else {
        which::which("ssh")
            .unwrap_or_else(|_| std::path::PathBuf::from("/usr/bin/ssh"))
    };
    
    // 构建命令参数
    let mut args = Vec::new();
    
    // 如果是kitty kitten，添加kitten ssh命令
    if use_kitty_kitten {
        args.push("+kitten".to_string());
        args.push("ssh".to_string());
    }
    
    // 添加用户名和主机
    args.push(format!("{}@{}", server.username, server.host));
    
    // 添加端口
    if server.port != 22 {
        args.push("-p".to_string());
        args.push(server.port.to_string());
    }
    
    // 添加认证相关参数
    match &server.auth_type {
        AuthType::Key(key_path) => {
            println!("使用密钥认证，密钥路径: {}", key_path);
            let expanded_path = expand_tilde(key_path);
            println!("展开后的密钥路径: {}", expanded_path);
            args.push("-i".to_string());
            args.push(expanded_path.clone());
            
            // 如果同时提供了密码，在密钥认证后尝试密码认证
            if let Some(password) = &server.password {
                println!("检测到备用密码，准备使用expect处理密码输入");
                // 检查是否安装了expect
                if let Ok(expect_path) = which::which("expect") {
                    println!("找到expect程序: {}", expect_path.display());
                    
                    // 创建expect脚本
                    let expect_script = format!(
                        r#"#!/usr/bin/expect -f
set timeout 30
puts "开始SSH连接..."
spawn {} -i {} -p {} {}@{} -o StrictHostKeyChecking=no -o HashKnownHosts=no -o ServerAliveInterval=60 -o HostKeyAlgorithms=+ssh-rsa -o PubkeyAcceptedAlgorithms=+ssh-rsa
puts "等待密码提示..."
expect {{
    -re "password:" {{
        puts "检测到密码提示"
        puts "准备发送密码"
        send "{}\r"
        puts "密码已发送，等待Opt>提示"
        exp_continue
    }}
    -re "Opt>" {{
        puts "检测到Opt>提示，进入交互模式"
        interact
    }}
    timeout {{
        puts "超时，未检测到Opt>提示"
        exit 1
    }}
}}"#,
                        ssh_path.display(), 
                        expanded_path, 
                        server.port, 
                        server.username, 
                        server.host, 
                        password.replace("\"", "\\\"").replace("\\", "\\\\")
                    );
                    
                    println!("生成的expect脚本:\n{}", expect_script);
                    
                    // 创建临时脚本文件
                    let temp_dir = std::env::temp_dir();
                    let script_path = temp_dir.join(format!("rssh_expect_{}.sh", std::process::id()));
                    println!("创建临时脚本文件: {}", script_path.display());
                    std::fs::write(&script_path, expect_script)
                        .with_context(|| "无法创建expect脚本")?;
                    
                    // 设置脚本权限
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
                            .with_context(|| "无法设置脚本权限")?;
                        println!("设置脚本权限为700");
                    }
                    
                    println!("开始执行expect脚本...");
                    // 执行expect脚本
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::CommandExt;
                        let error = Command::new(expect_path)
                            .arg(&script_path)
                            .exec();
                        return Err(anyhow::anyhow!("执行expect脚本失败: {}", error));
                    }
                    
                    #[cfg(not(unix))]
                    {
                        let child = Command::new(expect_path)
                            .arg(&script_path)
                            .stdin(Stdio::inherit())
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .spawn()
                            .with_context(|| "无法启动expect进程")?;
                        
                        // 等待子进程结束
                        let status = child.wait()
                            .with_context(|| "等待expect进程失败")?;
                        
                        if !status.success() {
                            if let Some(code) = status.code() {
                                return Err(anyhow::anyhow!("expect进程退出，代码: {}", code));
                            } else {
                                return Err(anyhow::anyhow!("expect进程被信号中断"));
                            }
                        }
                    }
                } else {
                    println!("未找到expect程序，将使用普通SSH连接");
                }
            } else {
                println!("未设置备用密码，将只使用密钥认证");
            }
        },
        AuthType::Agent => {
            // 默认使用SSH代理，不需要额外参数
        },
        AuthType::Password(_password) => {
            // 检查是否安装了expect
            if let Ok(expect_path) = which::which("expect") {
                println!("使用expect自动处理密码输入...");
                
                // 创建expect脚本
                let expect_script = format!(
                    "#!/usr/bin/expect -f\n\
                     spawn {} -p {} {}@{} -o StrictHostKeyChecking=no -o HashKnownHosts=no -o ServerAliveInterval=60 -o HostKeyAlgorithms=+ssh-rsa -o PubkeyAcceptedAlgorithms=+ssh-rsa\n\
                     expect \"password:\"\n\
                     send \"{}\\\r\"\n\
                     interact",
                    ssh_path.display(), server.port, server.username, server.host, 
                    match &server.auth_type {
                        AuthType::Password(pwd) => pwd,
                        _ => "",
                    }.replace("\"", "\\\"").replace("\\", "\\\\")
                );
                
                // 创建临时脚本文件
                let temp_dir = std::env::temp_dir();
                let script_path = temp_dir.join(format!("rssh_expect_{}.sh", std::process::id()));
                std::fs::write(&script_path, expect_script)
                    .with_context(|| "无法创建expect脚本")?;
                
                // 设置脚本权限
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
                        .with_context(|| "无法设置脚本权限")?;
                }
                
                // 执行expect脚本
                let status = Command::new(expect_path)
                    .arg(&script_path)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .with_context(|| "无法启动expect进程")?
                    .wait()
                    .with_context(|| "等待expect进程失败")?;
                
                // 清理临时文件
                let _ = std::fs::remove_file(&script_path);
                
                if !status.success() {
                    if let Some(code) = status.code() {
                        return Err(anyhow::anyhow!("expect进程退出，代码: {}", code));
                    } else {
                        return Err(anyhow::anyhow!("expect进程被信号中断"));
                    }
                }
                
                return Ok(0);
            } else {
                println!("警告: 未安装expect，无法自动处理密码输入。");
                println!("请安装expect或使用密钥认证：");
                println!("  macOS: brew install expect");
                println!("  Ubuntu/Debian: sudo apt-get install expect");
                println!("  CentOS/RHEL: sudo yum install expect");
                return Err(anyhow::anyhow!("未安装expect"));
            }
        }
    }
    
    // 禁用严格主机密钥检查
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=no".to_string());
    
    // 禁用HashKnownHosts
    args.push("-o".to_string());
    args.push("HashKnownHosts=no".to_string());
    
    // 保持会话活跃
    args.push("-o".to_string());
    args.push("ServerAliveInterval=60".to_string());
    
    // 启用ssh-rsa算法支持
    args.push("-o".to_string());
    args.push("HostKeyAlgorithms=+ssh-rsa".to_string());
    args.push("-o".to_string());
    args.push("PubkeyAcceptedAlgorithms=+ssh-rsa".to_string());
    
    // 添加命令（如果有）
    if let Some(cmd) = command {
        args.push(cmd);
    }
    
    // 检查是否安装了lrzsz，如果是，则使用我们的rzsz代理
    let rzsz_enabled = is_lrzsz_installed();
    
    // 只有在用户通过命令行参数启用并且本地有lrzsz才使用代理
    let use_rzsz_proxy = use_rzsz && rzsz_enabled;
    
    println!("RZSZ文件传输{}", if rzsz_enabled { 
        if use_rzsz_proxy { "已启用" } else { "可用但未启用 (使用 --rzsz 参数启用)" }
    } else { 
        "未安装" 
    });
    
    // 如果用户已设置不使用代理，跳过代理流程
    if use_rzsz_proxy && rzsz_enabled {
        // 获取代理路径
        if let Ok(proxy_path) = get_rzsz_proxy_path() {
            println!("使用RZSZ代理: {}", proxy_path);
            
            // 使用更简单的方法调用代理
            // 直接在本地连接，将远程主机和端口等信息通过环境变量传递给代理
            
            // 创建命令并设置环境变量
            let mut cmd = Command::new(proxy_path);
            cmd.env("RSSH_HOST", &server.host)
               .env("RSSH_PORT", &server.port.to_string())
               .env("RSSH_USER", &server.username)
               .stdin(Stdio::inherit())
               .stdout(Stdio::inherit())
               .stderr(Stdio::inherit());
            
            // 添加密钥信息
            if let AuthType::Key(key_path) = &server.auth_type {
                let expanded_path = expand_tilde(key_path);
                cmd.env("RSSH_KEY", expanded_path);
            }
            
            println!("启动RZSZ代理...");
            
            // 运行代理程序
            let status = cmd.spawn()
                .with_context(|| "无法启动RZSZ代理程序")?
                .wait()
                .with_context(|| "等待RZSZ代理程序失败")?;
            
            println!("\n代理连接已关闭");
            
            if !status.success() {
                if let Some(code) = status.code() {
                    return Err(anyhow::anyhow!("RZSZ代理进程退出，代码: {}", code));
                } else {
                    return Err(anyhow::anyhow!("RZSZ代理进程被信号中断"));
                }
            }
            
            return Ok(0);
        } else {
            println!("未找到rzsz-proxy程序，使用普通SSH连接");
        }
    }
    
    // 输出一些调试信息
    println!("正在通过{}SSH连接到 {}@{}:{}", 
        if use_kitty_kitten { "kitty +kitten " } else { "" },
        server.username, 
        server.host, 
        server.port
    );
    
    if use_kitty_kitten {
        println!("已启用kitty kitten模式");
        println!("注意：如果新开窗口后退出，可能需要手动关闭kitty窗口");
        println!("如果遇到渲染问题或窗口问题，可以尝试不使用--kitten参数");
    }
    
    if rzsz_enabled {
        println!("提示: 如果需要rzsz文件传输功能，请确保远程服务器也安装了lrzsz软件包");
    }
    
    println!("命令: {} {}", ssh_path.display(), args.join(" "));
    
    // 创建一个新的进程
    let mut child = Command::new(ssh_path)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| "无法启动SSH进程")?;
    
    // 等待进程结束
    let status = child.wait()
        .with_context(|| "等待SSH进程失败")?;
    
    println!("\n连接已关闭");
    
    let exit_code = status.code().unwrap_or(1);
    if !status.success() {
        println!("SSH进程退出，代码: {}", exit_code);
    }
    
    Ok(exit_code)
}

/// 检查系统是否安装了lrzsz
fn is_lrzsz_installed() -> bool {
    let rz_installed = Command::new("which")
        .arg("rz")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    let sz_installed = Command::new("which")
        .arg("sz")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    if !rz_installed || !sz_installed {
        println!("提示: 本地未安装lrzsz软件包，rzsz文件传输功能不可用");
        println!("可以通过以下命令安装:");
        println!("  macOS: brew install lrzsz");
        println!("  Ubuntu/Debian: sudo apt-get install lrzsz");
        println!("  CentOS/RHEL: sudo yum install lrzsz");
        return false;
    }
    
    true
}

/// 获取rssh-rzsz-proxy二进制路径
fn get_rzsz_proxy_path() -> Result<String> {
    // 获取当前可执行文件路径
    let current_exe = std::env::current_exe()
        .with_context(|| "无法获取当前可执行文件路径")?;
    
    let current_dir = current_exe.parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取当前目录"))?;
    
    let proxy_path = current_dir.join("rzsz-proxy");
    
    // 直接转换为字符串路径，不添加任何引号
    if proxy_path.exists() {
        return Ok(proxy_path.to_string_lossy().to_string());
    }
    
    // 如果在当前目录没找到，使用绝对路径尝试
    let which_result = Command::new("which")
        .arg("rzsz-proxy")
        .output();
    
    if let Ok(output) = which_result {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }
    
    // 如果找不到，返回错误
    Err(anyhow::anyhow!("找不到rzsz-proxy可执行文件"))
}

// 直接调用系统的ssh命令
pub fn ssh_command_connect(server: &ServerConfig, use_kitten: bool) -> Result<()> {
    // 检查是否使用kitty的kitten ssh
    let use_kitty_kitten = use_kitten && is_kitty_available();
    
    let host_str = if server.port != 22 {
        format!("-p {} {}@{}", server.port, server.username, server.host)
    } else {
        format!("{}@{}", server.username, server.host)
    };
    
    let ssh_path = if use_kitty_kitten {
        "kitty".into()
    } else {
        which::which("ssh").unwrap_or_else(|_| "ssh".into())
    };
    
    // 提前声明变量以延长生命周期
    let expanded_path_storage;
    
    // 创建参数列表
    let mut all_args = Vec::new();
    
    // 如果使用kitty kitten，添加相应的命令和参数
    if use_kitty_kitten {
        all_args.push("+kitten");
        all_args.push("ssh");
    }
    
    // 添加ssh-rsa算法支持
    all_args.push("-o");
    all_args.push("HostKeyAlgorithms=+ssh-rsa");
    all_args.push("-o");
    all_args.push("PubkeyAcceptedAlgorithms=+ssh-rsa");
    
    // 添加认证相关参数
    match &server.auth_type {
        AuthType::Key(key_path) => {
            expanded_path_storage = expand_tilde(key_path);
            all_args.push("-i");
            all_args.push(&expanded_path_storage);
            all_args.push(&host_str);
        },
        AuthType::Agent => {
            all_args.push(&host_str);
        },
        AuthType::Password(password) => {
            println!("警告: 系统SSH命令不支持直接传递密码，请使用其他验证方式。");
            return Err(anyhow::anyhow!("不支持密码验证"));
        }
    };
    
    if use_kitty_kitten {
        println!("执行: kitty +kitten ssh {}", all_args[2..].join(" "));
        println!("已启用kitty kitten模式");
    } else {
        println!("执行: {} {}", ssh_path.display(), all_args.join(" "));
    }
    
    // 使用exec系统调用直接替换当前进程
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        
        // 创建命令但不启动
        let mut cmd = Command::new(ssh_path);
        cmd.args(&all_args);
        
        // 使用exec启动进程，替换当前进程
        let error = cmd.exec();
        
        // 如果exec返回，则表示出错
        return Err(anyhow::anyhow!("执行SSH命令失败: {}", error));
    }
    
    // 非Unix平台使用普通的spawn
    #[cfg(not(unix))]
    {
        let mut child = Command::new(ssh_path)
            .args(&all_args)
            .spawn()
            .with_context(|| "无法启动SSH进程")?;
        
        let status = child.wait()
            .with_context(|| "等待SSH进程失败")?;
        
        if !status.success() {
            if let Some(code) = status.code() {
                return Err(anyhow::anyhow!("SSH进程退出，代码: {}", code));
            } else {
                return Err(anyhow::anyhow!("SSH进程被信号中断"));
            }
        }
        
        Ok(())
    }
} 