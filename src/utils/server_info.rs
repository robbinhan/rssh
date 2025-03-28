use anyhow::Result;
use crate::models::ServerConfig;
use crate::utils::terminal_style::{Style, colors, Styled};

pub fn display_server_info(server: &ServerConfig) -> Result<()> {
    // 创建标签样式（青色加粗）
    let label_style = Style::new()
        .fg(colors::CYAN)
        .bold();
    
    // 创建值样式（白色加粗）
    let value_style = Style::new()
        .fg(colors::WHITE)
        .bold();

    // 创建分组样式（黄色加粗）
    let group_style = Style::new()
        .fg(colors::YELLOW)
        .bold();

    // 创建描述样式（灰色）
    let desc_style = Style::new()
        .fg(colors::BRIGHT_BLACK);

    // 创建命令样式（绿色加粗）
    let cmd_style = Style::new()
        .fg(colors::GREEN)
        .bold();

    // 显示基本信息
    println!("{}", "服务器基本信息".style(label_style));
    println!("{}: {}", "ID".style(label_style), server.id.clone().style(value_style));
    println!("{}: {}", "名称".style(label_style), server.name.clone().style(value_style));
    println!("{}: {}", "主机".style(label_style), server.host.clone().style(value_style));
    println!("{}: {}", "端口".style(label_style), server.port.to_string().style(value_style));
    println!("{}: {}", "用户名".style(label_style), server.username.clone().style(value_style));
    println!();

    // 显示认证信息
    println!("{}", "认证信息".style(label_style));
    println!("{}: {}", "认证类型".style(label_style), server.auth_type.clone().style(value_style));
    if let Some(key_path) = server.auth_type.get_key_path() {
        println!("{}: {}", "密钥路径".style(label_style), key_path.style(value_style));
    }
    println!();

    // 显示其他信息
    println!("{}", "其他信息".style(label_style));
    if let Some(group) = &server.group {
        println!("{}: {}", "分组".style(label_style), group.clone().style(group_style));
    }
    if let Some(desc) = &server.description {
        println!("{}: {}", "描述".style(label_style), desc.clone().style(desc_style));
    }
    println!();

    // 显示连接信息
    println!("{}", "连接信息".style(label_style));
    let ssh_cmd = format!("ssh {}@{} -p {} {}", 
        server.username,
        server.host,
        server.port,
        server.auth_type.get_ssh_args()
    );
    println!("{}: {}", "SSH命令".style(label_style), ssh_cmd.style(cmd_style));

    Ok(())
} 