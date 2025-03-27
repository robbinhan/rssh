use std::io::{self, Write};
use crate::utils::terminal_style::{Style, colors, Styled};

/// 处理rzsz命令
pub fn handle_rzsz(data: &[u8], _channel: &mut impl Write) -> io::Result<bool> {
    // 检查是否是rz命令
    if data == b"rz\r" {
        let style = Style::new()
            .fg(colors::YELLOW)
            .bold();
        println!("{}", "检测到rz命令，暂不支持文件上传".style(style));
        return Ok(true);
    }

    // 检查是否是sz命令
    if data.starts_with(b"sz ") {
        let style = Style::new()
            .fg(colors::YELLOW)
            .bold();
        println!("{}", "检测到sz命令，暂不支持文件下载".style(style));
        return Ok(true);
    }

    Ok(false)
} 