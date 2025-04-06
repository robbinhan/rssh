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

# 使用russh库连接（基于异步Rust的SSH实现**实验中**）
rssh connect myserver --mode russh
```


### 在服务器上执行命令

```bash
rssh connect myserver --command "ls -la"
```


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

### 在服务器之间复制文件

```bash
# 从源服务器复制文件到目标服务器
rssh copy --from source_server --from-path /path/to/source/file.txt \
          --to target_server --to-path /path/to/target/file.txt

# 复制整个目录
rssh copy --from source_server --from-path /path/to/source/dir \
          --to target_server --to-path /path/to/target/dir
```

**注意事项：**
1. 首次使用时会自动安装和配置 rclone
2. 支持复制单个文件或整个目录
3. 使用 rclone 作为底层实现，支持断点续传
4. 路径可以是相对路径或绝对路径
5. 如果目标路径已存在同名文件，会被覆盖

## 配置文件

配置文件存储在以下位置：

- Linux/macOS: `~/.config/rssh/servers.db` 或者 `~/Library/Application\ Support/`
- Windows: `C:\Users\<用户名>\AppData\Roaming\rssh\servers.db`

## TODO
- [X] copy命令：从某个服务器的路径拷贝文件或目录到另一个服务器路径上
- [ ] connect group: 连接一组服务器，分屏展示（需要kitty终端支持）
- [ ] session: 可以支持根据配置以多个窗口连接服务器，同时执行命令（类似tmux的session）

## 许可证

MIT
