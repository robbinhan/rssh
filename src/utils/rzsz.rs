use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::{self, Write, Read};
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use colored::*;

/// RZSZ特征标识
const ZMODEM_DETECT: &[u8] = b"**\x18B00000000000000\r\n";
const RZ_COMMAND: &[u8] = b"rz\r";
const SZ_COMMAND: &[u8] = b"sz";

/// 检测是否是rz或sz命令
pub fn is_rzsz_command(data: &[u8]) -> Option<&'static str> {
    // 检测rz命令
    if data.len() >= 3 && &data[0..3] == RZ_COMMAND {
        return Some("rz");
    }
    
    // 检测sz命令（可能带文件名）
    if data.len() >= 2 && &data[0..2] == SZ_COMMAND {
        return Some("sz");
    }
    
    None
}

/// 监听rzsz命令并处理
pub fn handle_rzsz(data: &[u8], channel: &mut ssh2::Channel) -> Result<bool> {
    match is_rzsz_command(data) {
        Some("rz") => {
            // 处理接收文件(rz)
            handle_receive_file(channel)?;
            return Ok(true);
        },
        Some("sz") => {
            // 处理发送文件(sz)
            let args = String::from_utf8_lossy(&data[2..]).trim().to_string();
            handle_send_file(channel, &args)?;
            return Ok(true);
        },
        _ => {}
    }
    
    Ok(false)
}

/// 处理rz命令（从本地上传文件到远程）
fn handle_receive_file(_channel: &mut ssh2::Channel) -> Result<()> {
    println!("\n检测到rz命令，准备上传文件到远程服务器...");
    
    // 使用文件选择器让用户选择文件
    let file_path = select_file_dialog()?;
    if file_path.is_empty() {
        println!("未选择文件，取消上传");
        return Ok(());
    }
    
    println!("准备上传文件: {}", file_path);
    
    // 创建临时脚本来执行rz命令
    let script = format!(r#"
        #!/bin/bash
        # 自动rz上传脚本
        sleep 1
        echo -e "开始上传..."
        sz "{}" > /dev/null 2>&1
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
            echo "文件上传成功！"
        else
            echo "文件上传失败，错误码: $exit_code"
        fi
    "#, file_path);
    
    let mut temp_script = std::env::temp_dir();
    temp_script.push("rssh_rz_helper.sh");
    std::fs::write(&temp_script, script)?;
    
    // 设置执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_script, perms)?;
    }
    
    // 启动进程执行rz操作
    let mut cmd = Command::new(&temp_script)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("无法启动rz上传助手")?;
    
    // 等待完成
    let status = cmd.wait()?;
    
    // 删除临时脚本
    let _ = std::fs::remove_file(temp_script);
    
    if !status.success() {
        eprintln!("rz上传过程中发生错误");
    }
    
    Ok(())
}

/// 处理sz命令（从远程下载文件到本地）
fn handle_send_file(channel: &mut ssh2::Channel, args: &str) -> Result<()> {
    println!("\n检测到sz命令，准备从远程服务器下载文件: {}", args);
    
    // 询问用户保存位置
    let save_path = select_save_location(args)?;
    if save_path.is_empty() {
        println!("未指定保存位置，取消下载");
        return Ok(());
    }
    
    println!("文件将保存到: {}", save_path);
    
    // 创建临时脚本来执行sz命令
    let script = format!(r#"
        #!/bin/bash
        # 自动sz下载脚本
        sleep 1
        echo -e "开始下载..."
        rz -y > /dev/null 2>&1
        exit_code=$?
        if [ $exit_code -eq 0 ]; then
            echo "文件下载成功！保存到 {}"
        else
            echo "文件下载失败，错误码: $exit_code"
        fi
    "#, save_path);
    
    let mut temp_script = std::env::temp_dir();
    temp_script.push("rssh_sz_helper.sh");
    std::fs::write(&temp_script, script)?;
    
    // 设置执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_script, perms)?;
    }
    
    // 启动进程执行sz操作
    let mut cmd = Command::new(&temp_script)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("无法启动sz下载助手")?;
    
    // 向远程发送sz命令
    let write_data = format!("sz {}\r\n", args);
    channel.write_all(write_data.as_bytes())?;
    channel.flush()?;
    
    // 等待完成
    let status = cmd.wait()?;
    
    // 删除临时脚本
    let _ = std::fs::remove_file(temp_script);
    
    if !status.success() {
        eprintln!("sz下载过程中发生错误");
    }
    
    Ok(())
}

/// 弹出文件选择对话框
fn select_file_dialog() -> Result<String> {
    // 首先尝试使用GUI文件选择器
    if let Ok(path) = try_gui_file_selection() {
        return Ok(path);
    }
    
    // 回退到命令行输入
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

/// 选择保存位置对话框
fn select_save_location(default_filename: &str) -> Result<String> {
    // 首先尝试使用GUI保存对话框
    if let Ok(path) = try_gui_save_selection(default_filename) {
        return Ok(path);
    }
    
    // 回退到命令行输入
    println!("请输入保存文件的完整路径 (默认: {}):", default_filename);
    let mut path = String::new();
    io::stdin().read_line(&mut path)?;
    
    let path = path.trim();
    if path.is_empty() {
        // 使用当前目录和默认文件名
        let mut current_dir = std::env::current_dir()?;
        current_dir.push(default_filename);
        return Ok(current_dir.to_string_lossy().to_string());
    }
    
    Ok(path.to_string())
}

/// 尝试使用GUI文件选择器
fn try_gui_file_selection() -> Result<String> {
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
    Err(anyhow::anyhow!("无法使用GUI文件选择器"))
}

/// 尝试使用GUI保存对话框
fn try_gui_save_selection(default_filename: &str) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose file name default name \"{}\" with prompt \"保存文件\")",
            default_filename
        );
        
        let output = Command::new("osascript")
            .args(["-e", &script])
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
            .args(["--file-selection", "--save", "--filename", default_filename])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(path);
            }
        }
    }
    
    // 如果无法使用GUI，返回错误
    Err(anyhow::anyhow!("无法使用GUI文件选择器"))
} 