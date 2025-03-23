use anyhow::{Context, Result};
use std::process::{Command, Stdio};

use crate::models::{AuthType, ServerConfig};
use crate::utils::ssh_config::expand_tilde;

// 使用基于子进程的方法
// 这个实现直接使用系统的ssh命令，绕过Rust的SSH库
pub fn connect_via_system_ssh(server: &ServerConfig) -> Result<()> {
    // 构建命令参数
    let mut args = Vec::new();
    
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
            let expanded_path = expand_tilde(key_path);
            args.push("-i".to_string());
            args.push(expanded_path);
        },
        AuthType::Agent => {
            // 默认使用SSH代理，不需要额外参数
        },
        AuthType::Password(_) => {
            // 系统ssh命令不能直接传递密码，这只是作为备用方案
            println!("警告: 使用系统SSH命令时不支持密码验证，请使用密钥或代理方式。");
            return Err(anyhow::anyhow!("系统SSH不支持密码验证"));
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
    
    // 输出一些调试信息
    println!("正在通过系统SSH连接到 {}@{}:{}", server.username, server.host, server.port);
    println!("命令: ssh {}", args.join(" "));
    
    // 创建一个新的进程
    let mut child = Command::new("ssh")
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
    
    if !status.success() {
        if let Some(code) = status.code() {
            return Err(anyhow::anyhow!("SSH进程退出，代码: {}", code));
        } else {
            return Err(anyhow::anyhow!("SSH进程被信号中断"));
        }
    }
    
    Ok(())
}

// 直接调用系统的ssh命令
pub fn ssh_command_connect(server: &ServerConfig) -> Result<()> {
    let host_str = if server.port != 22 {
        format!("-p {} {}@{}", server.port, server.username, server.host)
    } else {
        format!("{}@{}", server.username, server.host)
    };
    
    let ssh_path = which::which("ssh").unwrap_or_else(|_| "ssh".into());
    
    // 提前声明变量以延长生命周期
    let expanded_path_storage;
    
    let args = match &server.auth_type {
        AuthType::Key(key_path) => {
            expanded_path_storage = expand_tilde(key_path);
            vec!["-i", &expanded_path_storage, &host_str]
        },
        AuthType::Agent => {
            vec![&host_str[..]]
        },
        AuthType::Password(_) => {
            println!("警告: 系统SSH命令不支持直接传递密码，请使用其他验证方式。");
            return Err(anyhow::anyhow!("不支持密码验证"));
        }
    };
    
    println!("执行: {} {}", ssh_path.display(), args.join(" "));
    
    // 使用exec系统调用直接替换当前进程
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        
        // 创建命令但不启动
        let mut cmd = Command::new(ssh_path);
        cmd.args(&args);
        
        // 使用exec启动进程，替换当前进程
        let error = cmd.exec();
        
        // 如果exec返回，则表示出错
        return Err(anyhow::anyhow!("执行SSH命令失败: {}", error));
    }
    
    // 非Unix平台使用普通的spawn
    #[cfg(not(unix))]
    {
        let mut child = Command::new(ssh_path)
            .args(&args)
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