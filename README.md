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
git clone https://github.com/robbinhan/rssh.git
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

### 在连接状态下使用rzsz传输文件

rssh支持在连接到服务器后直接使用rz和sz命令传输文件，支持两种模式：

1. **使用内置的rzsz代理服务** (推荐模式)：
   ```bash
   # 使用--rzsz参数启用rzsz代理功能
   rssh connect myserver --mode system --rzsz
   ```

2. **使用默认的SSH库模式**：
   ```bash
   # 使用library模式，不需要额外参数
   rssh connect myserver --mode library
   ```

连接后，在远程服务器上：
```bash
# 下载文件：在远程服务器上执行，将弹出本地保存对话框
sz filename.txt  # 将远程文件下载到本地

# 上传文件：在远程服务器上执行，将弹出本地文件选择对话框
rz  # 将本地文件上传到远程服务器
```

**注意事项:**
1. 使用rzsz功能需要在本地和远程服务器上都安装`lrzsz`包
2. 本地安装方法：
   - MacOS: `brew install lrzsz`
   - Ubuntu/Debian: `sudo apt-get install lrzsz`
   - CentOS/RHEL: `sudo yum install lrzsz`
3. 远程服务器安装方法：
   - Ubuntu/Debian: `sudo apt-get install lrzsz`
   - CentOS/RHEL: `sudo yum install lrzsz`
4. 上传/下载时可能会出现乱码，这是正常现象，代表文件正在传输
5. 文件传输完成会显示成功或失败信息

**兼容性问题：**
由于ZMODEM协议需要特定的终端环境支持，在以下情况下可能不工作：
1. 某些终端模拟器不完全支持ZMODEM协议
2. 文件较大时可能会超时
3. 本地与远程的lrzsz版本不兼容

如果遇到rzsz传输问题，建议使用rssh的专用上传下载命令：
```bash
# 上传本地文件到远程服务器
rssh upload myserver local_file.txt /path/on/remote/server/

# 从远程服务器下载文件到本地
rssh download myserver /path/on/remote/server/file.txt local_file.txt
```

**高级调试:**
如果遇到传输问题，可以查看以下日志文件：
- `/tmp/rz_debug.log` - 下载日志
- `/tmp/sz_debug.log` - 上传日志

支持的平台:
- MacOS (使用AppleScript弹出文件选择对话框)
- Linux (使用zenity弹出文件选择对话框，需要安装zenity)
- 其他平台将使用命令行方式进行文件选择

### 上传文件到服务器

```bash
# 上传文件（将在远程使用相同的文件名）
rssh upload myserver local_file.txt

# 指定远程路径
rssh upload myserver local_file.txt /path/to/remote_file.txt

# 使用SFTP传输
rssh upload myserver local_file.txt --mode sftp

# 使用Kitty传输协议（如果您使用的是Kitty终端）
rssh upload myserver local_file.txt --mode kitty

# 自动选择最佳传输方式（默认）
rssh upload myserver local_file.txt --mode auto
```

### 从服务器下载文件

```bash
# 下载文件（将在本地使用相同的文件名）
rssh download myserver /path/to/remote_file.txt

# 指定本地路径
rssh download myserver /path/to/remote_file.txt local_file.txt

# 使用SFTP传输
rssh download myserver /path/to/remote_file.txt --mode sftp

# 使用Kitty传输协议（如果您使用的是Kitty终端）
rssh download myserver /path/to/remote_file.txt --mode kitty

# 自动选择最佳传输方式（默认）
rssh download myserver /path/to/remote_file.txt --mode auto
```

#### 传输模式

RSSH支持多种文件传输模式：

1. `auto` - 自动选择最佳传输方式（默认）:
   - 如果检测到Kitty终端，会使用Kitty传输协议
   - 否则会使用SCP
   
2. `scp` - 使用SCP传输（最广泛支持的方式）

3. `sftp` - 使用SFTP传输（更安全，支持断点续传）

4. `kitty` - 使用Kitty终端内置的传输协议：
   - 只有在使用Kitty终端时才可用
   - 需要安装Kitty终端 (https://sw.kovidgoyal.net/kitty/)
   - 比rzsz更现代、更可靠
   - 支持更大的文件和进度显示
   
**提示：** 在Kitty终端中，优先使用Kitty传输协议或auto模式，它比传统的rzsz更现代、更可靠，且不会在传输过程中显示乱码。

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

## TODO
[ ] copy命令：从某个服务器的路径拷贝文件或目录到另一个服务器路径上

## 许可证

MIT
