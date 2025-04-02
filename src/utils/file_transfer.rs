use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use colored::*;

use crate::models::ServerConfig;
use crate::utils::ssh_config::expand_tilde;

/// 使用SCP上传文件到远程服务器
pub fn upload_file<P: AsRef<Path>>(
    server: &ServerConfig,
    local_path: P,
    remote_path: Option<String>,
) -> Result<()> {
    let local_path = local_path.as_ref();
    
    // 确保本地文件存在
    if !local_path.exists() {
        return Err(anyhow::anyhow!("本地文件不存在: {}", local_path.display()));
    }
    
    // 确定远程路径
    let remote_dest = match remote_path {
        Some(path) => path,
        None => {
            // 如果没有指定远程路径，使用本地文件名
            let file_name = local_path.file_name()
                .ok_or_else(|| anyhow::anyhow!("无法确定文件名"))?
                .to_string_lossy();
            format!("./{}", file_name)
        }
    };
    
    // 构建SCP命令
    let mut cmd = Command::new("scp");
    
    // 设置端口
    if server.port != 22 {
        cmd.args(["-P", &server.port.to_string()]);
    }
    
    // 添加认证相关参数
    match &server.auth_type {
        crate::models::AuthType::Key(key_path) => {
            let expanded_path = expand_tilde(key_path);
            cmd.args(["-i", &expanded_path]);
        },
        crate::models::AuthType::Agent => {
            // 使用SSH代理，不需要额外参数
        },
        crate::models::AuthType::Password(_) => {
            return Err(anyhow::anyhow!("SCP不支持直接传递密码，请使用密钥或代理认证"));
        }
    }
    
    // 禁用主机密钥检查
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    
    // 添加本地和远程路径
    cmd.arg(local_path.as_os_str())
        .arg(format!("{}@{}:{}", server.username, server.host, remote_dest));
    
    // 显示命令
    let cmd_str = format!("{:?}", cmd);
    println!("执行: {}", cmd_str.bright_blue());
    
    // 执行命令
    let status = cmd.status()
        .with_context(|| "无法执行SCP命令")?;
    
    if status.success() {
        println!("文件上传成功！");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件上传失败，SCP退出代码: {:?}", status.code()))
    }
}

/// 从远程服务器下载文件
pub fn download_file(
    server: &ServerConfig,
    remote_path: &str,
    local_path: Option<PathBuf>,
) -> Result<()> {
    // 确定本地路径
    let local_dest = match local_path {
        Some(path) => path,
        None => {
            // 如果没有指定本地路径，使用远程文件的基本名称
            let file_name = Path::new(remote_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new(remote_path))
                .to_string_lossy();
            PathBuf::from(file_name.to_string())
        }
    };
    
    // 构建SCP命令
    let mut cmd = Command::new("scp");
    
    // 设置端口
    if server.port != 22 {
        cmd.args(["-P", &server.port.to_string()]);
    }
    
    // 添加认证相关参数
    match &server.auth_type {
        crate::models::AuthType::Key(key_path) => {
            let expanded_path = expand_tilde(key_path);
            cmd.args(["-i", &expanded_path]);
        },
        crate::models::AuthType::Agent => {
            // 使用SSH代理，不需要额外参数
        },
        crate::models::AuthType::Password(_) => {
            return Err(anyhow::anyhow!("SCP不支持直接传递密码，请使用密钥或代理认证"));
        }
    }
    
    // 禁用主机密钥检查
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    
    // 添加远程和本地路径
    cmd.arg(format!("{}@{}:{}", server.username, server.host, remote_path))
        .arg(local_dest.as_os_str());
    
    // 显示命令
    let cmd_str = format!("{:?}", cmd);
    println!("执行: {}", cmd_str.bright_blue());
    
    // 执行命令
    let status = cmd.status()
        .with_context(|| "无法执行SCP命令")?;
    
    if status.success() {
        println!("文件下载成功！");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件下载失败，SCP退出代码: {:?}", status.code()))
    }
}

/// 使用SFTP上传文件到远程服务器（作为备选方案）
pub fn upload_file_sftp<P: AsRef<Path>>(
    server: &ServerConfig,
    local_path: P,
    remote_path: Option<String>,
) -> Result<()> {
    let local_path = local_path.as_ref();
    
    // 确保本地文件存在
    if !local_path.exists() {
        return Err(anyhow::anyhow!("本地文件不存在: {}", local_path.display()));
    }
    
    // 确定远程路径
    let remote_dest = match remote_path {
        Some(path) => path,
        None => {
            // 如果没有指定远程路径，使用本地文件名
            let file_name = local_path.file_name()
                .ok_or_else(|| anyhow::anyhow!("无法确定文件名"))?
                .to_string_lossy();
            format!("./{}", file_name)
        }
    };
    
    // 构建SFTP批处理命令
    let sftp_command = format!("put {} {}", 
        local_path.display(), 
        remote_dest
    );
    
    // 创建临时批处理文件
    let mut sftp_batch = std::env::temp_dir();
    sftp_batch.push("rssh_sftp_batch.txt");
    std::fs::write(&sftp_batch, sftp_command)
        .with_context(|| "无法创建SFTP批处理文件")?;
    
    // 构建SFTP命令
    let mut cmd = Command::new("sftp");
    
    // 设置端口
    if server.port != 22 {
        cmd.args(["-P", &server.port.to_string()]);
    }
    
    // 添加认证相关参数
    match &server.auth_type {
        crate::models::AuthType::Key(key_path) => {
            let expanded_path = expand_tilde(key_path);
            cmd.args(["-i", &expanded_path]);
        },
        crate::models::AuthType::Agent => {
            // 使用SSH代理，不需要额外参数
        },
        crate::models::AuthType::Password(_) => {
            return Err(anyhow::anyhow!("SFTP不支持直接传递密码，请使用密钥或代理认证"));
        }
    }
    
    // 禁用主机密钥检查
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    
    // 使用批处理文件
    cmd.args(["-b", sftp_batch.to_str().unwrap()]);
    
    // 添加远程主机
    cmd.arg(format!("{}@{}", server.username, server.host));
    
    // 显示命令
    let cmd_str = format!("{:?}", cmd);
    println!("执行: {}", cmd_str.bright_blue());
    
    // 执行命令
    let status = cmd.status()
        .with_context(|| "无法执行SFTP命令")?;
    
    // 删除临时批处理文件
    let _ = std::fs::remove_file(sftp_batch);
    
    if status.success() {
        println!("文件上传成功！");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件上传失败，SFTP退出代码: {:?}", status.code()))
    }
}

/// 从远程服务器使用SFTP下载文件（作为备选方案）
pub fn download_file_sftp(
    server: &ServerConfig,
    remote_path: &str,
    local_path: Option<PathBuf>,
) -> Result<()> {
    // 确定本地路径
    let local_dest = match local_path {
        Some(path) => path,
        None => {
            // 如果没有指定本地路径，使用远程文件的基本名称
            let file_name = Path::new(remote_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new(remote_path))
                .to_string_lossy();
            PathBuf::from(file_name.to_string())
        }
    };
    
    // 构建SFTP批处理命令
    let sftp_command = format!("get {} {}", 
        remote_path, 
        local_dest.display()
    );
    
    // 创建临时批处理文件
    let mut sftp_batch = std::env::temp_dir();
    sftp_batch.push("rssh_sftp_batch.txt");
    std::fs::write(&sftp_batch, sftp_command)
        .with_context(|| "无法创建SFTP批处理文件")?;
    
    // 构建SFTP命令
    let mut cmd = Command::new("sftp");
    
    // 设置端口
    if server.port != 22 {
        cmd.args(["-P", &server.port.to_string()]);
    }
    
    // 添加认证相关参数
    match &server.auth_type {
        crate::models::AuthType::Key(key_path) => {
            let expanded_path = expand_tilde(key_path);
            cmd.args(["-i", &expanded_path]);
        },
        crate::models::AuthType::Agent => {
            // 使用SSH代理，不需要额外参数
        },
        crate::models::AuthType::Password(_) => {
            return Err(anyhow::anyhow!("SFTP不支持直接传递密码，请使用密钥或代理认证"));
        }
    }
    
    // 禁用主机密钥检查
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    
    // 使用批处理文件
    cmd.args(["-b", sftp_batch.to_str().unwrap()]);
    
    // 添加远程主机
    cmd.arg(format!("{}@{}", server.username, server.host));
    
    // 显示命令
    let cmd_str = format!("{:?}", cmd);
    println!("执行: {}", cmd_str.bright_blue());
    
    // 执行命令
    let status = cmd.status()
        .with_context(|| "无法执行SFTP命令")?;
    
    // 删除临时批处理文件
    let _ = std::fs::remove_file(sftp_batch);
    
    if status.success() {
        println!("文件下载成功！");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件下载失败，SFTP退出代码: {:?}", status.code()))
    }
}

/// 使用Kitty传输协议上传文件到远程服务器
pub fn upload_file_kitty<P: AsRef<Path>>(
    server: &ServerConfig,
    local_path: P,
    remote_path: Option<String>,
) -> Result<()> {
    let local_path = local_path.as_ref();
    
    // 确保本地文件存在
    if !local_path.exists() {
        return Err(anyhow::anyhow!("本地文件不存在: {}", local_path.display()));
    }
    
    // 检查是否在Kitty终端
    if !crate::utils::kitty_transfer::is_kitty_available() {
        return Err(anyhow::anyhow!("当前终端不是Kitty或Kitty命令不可用，无法使用Kitty传输协议"));
    }
    
    // 构建远程路径（使用用户名@主机:路径格式）
    let remote_dest = match &remote_path {
        Some(path) => {
            format!("{}@{}:{}", server.username, server.host, path)
        },
        None => {
            // 使用服务器上的当前目录和本地文件名
            let file_name = local_path.file_name()
                .ok_or_else(|| anyhow::anyhow!("无法确定文件名"))?
                .to_string_lossy();
            format!("{}@{}:./{}",  server.username, server.host, file_name)
        }
    };
    
    // 使用Kitty的传输协议
    crate::utils::kitty_transfer::upload_via_kitty(local_path, Some(remote_dest))
}

/// 使用Kitty传输协议从远程服务器下载文件
pub fn download_file_kitty(
    server: &ServerConfig,
    remote_path: &str,
    local_path: Option<PathBuf>,
) -> Result<()> {
    // 检查是否在Kitty终端
    if !crate::utils::kitty_transfer::is_kitty_available() {
        return Err(anyhow::anyhow!("当前终端不是Kitty或Kitty命令不可用，无法使用Kitty传输协议"));
    }
    
    // 构建远程路径（使用用户名@主机:路径格式）
    let remote_full_path = format!("{}@{}:{}", server.username, server.host, remote_path);
    
    // 使用Kitty的传输协议
    crate::utils::kitty_transfer::download_via_kitty(&remote_full_path, local_path)
}

/// 自动选择最佳传输方式上传文件
pub fn upload_file_auto<P: AsRef<Path>>(
    server: &ServerConfig,
    local_path: P,
    remote_path: Option<String>,
) -> Result<()> {
    // 如果是Kitty终端，优先使用Kitty传输
    if crate::utils::kitty_transfer::is_kitty_available() {
        println!("检测到Kitty终端，使用Kitty传输协议");
        upload_file_kitty(server, local_path, remote_path)
    } 
    // 否则使用SCP（通常是最可靠的方式）
    else {
        println!("使用SCP传输文件");
        upload_file(server, local_path, remote_path)
    }
}

/// 自动选择最佳传输方式下载文件
pub fn download_file_auto(
    server: &ServerConfig,
    remote_path: &str,
    local_path: Option<PathBuf>,
) -> Result<()> {
    // 如果是Kitty终端，优先使用Kitty传输
    if crate::utils::kitty_transfer::is_kitty_available() {
        println!("检测到Kitty终端，使用Kitty传输协议");
        download_file_kitty(server, remote_path, local_path)
    } 
    // 否则使用SCP（通常是最可靠的方式）
    else {
        println!("使用SCP传输文件");
        download_file(server, remote_path, local_path)
    }
} 