# RSSH - Rust SSH 连接管理工具

这是一个用Rust编写的SSH连接管理工具，可以方便地管理和连接到多个远程服务器。

## 功能

- 添加、编辑和删除服务器配置
- 支持密码、密钥和SSH代理认证
- 服务器分组管理
- 交互式SSH连接
- 远程命令执行
- 从 ~/.ssh/config 导入服务器配置
- 多种连接模式，包括内置库和系统SSH命令

## 安装

### 从源码构建

确保你已经安装了Rust工具链：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

然后克隆仓库并构建：

```bash
git clone https://github.com/yourusername/rssh.git
cd rssh
cargo build --release
```

构建完成后，二进制文件将位于 `target/release/rssh`。你可以将其复制到系统路径中：

```bash
sudo cp target/release/rssh /usr/local/bin/
```

## 使用方法

### 添加服务器

```bash
rssh add --name myserver --host example.com --username user
```

使用密钥认证：

```bash
rssh add --name myserver --host example.com --username user --auth-type key --auth-data ~/.ssh/id_rsa
```

注意：密钥路径支持波浪号(`~`)表示用户主目录。

### 从 ~/.ssh/config 导入服务器

```bash
rssh import
```

或者指定配置文件路径和分组：

```bash
rssh import --config /path/to/ssh/config --group work
```

若要跳过已存在的服务器：

```bash
rssh import --skip-existing
```

### 列出所有服务器

```bash
rssh list
```

### 按分组列出服务器

```bash
rssh list --group prod
```

### 连接到服务器

```bash
rssh connect myserver
```

#### 连接模式

RSSH支持多种连接模式，以适应不同环境和需求：

```bash
# 默认使用内置SSH库
rssh connect myserver

# 使用系统SSH命令（推荐方式，最稳定可靠）
rssh connect myserver --mode system

# 使用exec直接替换当前进程（也很稳定）
rssh connect myserver --mode exec

# 使用调试模式（将记录详细日志到/tmp/rssh_debug.log）
rssh connect myserver --mode debug

# 使用russh库连接（基于异步Rust的SSH实现）
rssh connect myserver --mode russh
```

**推荐的连接模式**:
1. `system` - 使用系统的SSH命令，提供最佳兼容性和最稳定的交互体验
2. `exec` - 也使用系统的SSH命令，但直接替换当前进程
3. `russh` - 使用异步Rust的SSH库，可能在某些环境下有更好的性能
4. `library` - 使用内置的SSH2库（默认模式）
5. `debug` - 用于调试的内置库模式

调试模式下，你可以按`Alt+D`开启/关闭键盘输入的调试信息。

### 在服务器上执行命令

```bash
rssh connect myserver --command "ls -la"
```

### 编辑服务器

```bash
rssh edit myserver
```

### 删除服务器

```bash
rssh remove myserver
```

## 配置文件

配置文件存储在以下位置：

- Linux/macOS: `~/.config/rssh/servers.db`
- Windows: `C:\Users\<用户名>\AppData\Roaming\rssh\servers.db`

## 问题排查

如果你遇到交互式Shell问题，请尝试不同的连接模式：

```bash
# 最可靠的方式
rssh connect myserver --mode system

# 如果你想尝试现代的异步Rust实现
rssh connect myserver --mode russh
```

其他排查提示：
1. 确认你的终端环境支持PTY和原始模式
2. 如果使用密钥认证，确保密钥文件权限正确（600或400）
3. 检查`/tmp/rssh_debug.log`调试日志（如果使用`--mode debug`）

## 许可证

MIT 