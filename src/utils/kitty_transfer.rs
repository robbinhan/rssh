use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 使用Kitty的transfer协议上传文件到远程服务器
pub fn upload_via_kitty<P: AsRef<Path>>(
    local_path: P,
    remote_path: Option<String>,
) -> Result<()> {
    let local_path = local_path.as_ref();
    
    // 确保本地文件存在
    if !local_path.exists() {
        return Err(anyhow::anyhow!("本地文件不存在: {}", local_path.display()));
    }
    
    // 确定远程目标路径
    let remote_dest = match remote_path {
        Some(path) => path,
        // 如果没有指定远程路径，使用本地文件名
        None => local_path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow::anyhow!("无法从本地路径获取文件名: {}", local_path.display()))?
    };

    // 构建Kitty传输命令 (修正后的语法)
    let mut args = vec!["transfer", "-d", "upload"];
    
    // 添加本地文件路径
    args.push(local_path.to_str().ok_or_else(|| anyhow::anyhow!("本地路径包含无效UTF-8"))?);

    // 添加远程路径
    args.push(&remote_dest);
    
    // 输出信息
    println!("使用Kitty传输文件...");
    println!("命令: kitten {}", args.join(" "));
    
    // 执行命令
    let status = Command::new("kitten")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| "无法启动kitty传输命令")?
        .wait()
        .with_context(|| "等待kitty传输命令失败")?;
    
    if status.success() {
        println!("文件传输成功!");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件传输失败，退出码: {:?}", status.code()))
    }
}

/// 从远程服务器下载文件
pub fn download_via_kitty(
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
    
    // 构建Kitty传输命令
    let mut args = vec!["kitten", "transfer", "--direction=receive"];
    
    // 如果指定了本地路径，添加--dest参数
    args.push("--dest");
    args.push(local_dest.to_str().unwrap_or(""));
    
    // 添加远程文件路径
    args.push(remote_path);
    
    // 输出信息
    println!("使用Kitty传输文件...");
    println!("命令: kitty {}", args.join(" "));
    
    // 执行命令
    let status = Command::new("kitty")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| "无法启动kitty传输命令")?
        .wait()
        .with_context(|| "等待kitty传输命令失败")?;
    
    if status.success() {
        println!("文件传输成功!");
        Ok(())
    } else {
        Err(anyhow::anyhow!("文件传输失败，退出码: {:?}", status.code()))
    }
}

/// 检测当前环境是否支持Kitty的传输协议
pub fn is_kitty_available() -> bool {
    // 检查TERM环境变量
    let is_kitty_term = std::env::var("TERM").map(|val| val == "xterm-kitty").unwrap_or(false);
    
    // 检查kitty命令是否可用
    let has_kitty_command = Command::new("kitty")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    is_kitty_term && has_kitty_command
} 