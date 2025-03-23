#!/bin/bash

# RSSH安装脚本

set -e

echo "开始安装RSSH - Rust SSH连接管理工具..."

# 检查是否安装了Rust
if ! command -v cargo &> /dev/null; then
    echo "未检测到Rust工具链，是否安装? (y/n)"
    read -r install_rust
    if [ "$install_rust" = "y" ] || [ "$install_rust" = "Y" ]; then
        echo "安装Rust工具链..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        source "$HOME/.cargo/env"
    else
        echo "未安装Rust工具链，无法继续安装。"
        exit 1
    fi
fi

# 编译项目
echo "编译RSSH..."
cargo build --release

# 确定安装路径
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
    echo "无权限写入 $INSTALL_DIR，将安装到用户目录。"
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

# 复制二进制文件
echo "安装RSSH到 $INSTALL_DIR..."
cp "target/release/rssh" "$INSTALL_DIR/"

# 检查PATH中是否有安装目录
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "警告: $INSTALL_DIR 不在PATH环境变量中。"
    echo "建议添加以下行到您的shell配置文件:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi

echo "RSSH安装成功! 可以使用 'rssh --help' 命令查看使用方法。" 