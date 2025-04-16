# RSSH Session 功能使用指南

RSSH的Session功能允许你配置和管理多窗口SSH连接，类似于tmux的会话管理，但专注于SSH连接。

## 主要优势

1. **一键连接多个服务器** - 使用一个命令同时连接到多个服务器
2. **终端复用** - 在一个终端窗口中查看多个服务器
3. **统一布局** - 为常用的服务器组合保存布局配置
4. **自动命令执行** - 连接后自动执行特定命令
5. **支持多种终端环境** - 兼容Kitty终端和tmux

## 基本使用

### 创建会话

有两种方式创建会话：

1. 创建空会话，然后编辑配置：

```bash
# 创建一个新的空会话
rssh session-create --name dev_servers --description "开发环境服务器"

# 编辑会话配置
rssh session-edit dev_servers
```

2. 从现有的TOML配置文件创建会话：

```bash
# 从配置文件创建会话
rssh session-create --name dev_servers --config ~/my_session.toml
```

### 列出会话

```bash
rssh session-list
```

### 启动会话

```bash
# 自动选择最佳环境启动会话
rssh session-start dev_servers

# 指定使用kitty终端布局启动
rssh session-start dev_servers --kitty

# 指定使用tmux启动
rssh session-start dev_servers --tmux
```

### 删除会话

```bash
rssh session-remove dev_servers
```

## 配置文件格式

会话配置使用TOML格式，包含两个主要部分：全局选项和窗口配置。

```toml
# 会话选项配置
[options]
# layout选项可以是"tiled"（平铺）或"tabs"（标签页），用于tmux会话
layout = "tiled"
# 是否在启动后自动调整窗口大小以适应终端大小
auto_resize = "true"

# 窗口配置
# 每个窗口对应一个服务器连接
[windows.web1]
# 服务器名称或ID (必需字段)
server = "web-server-1"
# 连接后执行的命令 (可选)
command = "cd /var/www && ls -la"
# 窗口位置 (可选，kitty终端布局使用)
position = "vsplit"
# 窗口大小 (可选，kitty终端布局使用)
size = "50%"

[windows.db]
server = "database-server"
command = "mysql -u root"
position = "hsplit" 
size = "50%"
```

### 窗口配置说明

每个窗口部分以 `[windows.NAME]` 形式定义，其中 `NAME` 是窗口的唯一标识符：

- `server`: 要连接的服务器名称或ID（必需）
- `command`: 连接后要执行的命令（可选）
- `position`: 窗口位置，用于kitty布局（可选）
  - 可以是 `vsplit`（垂直分割）
  - 可以是 `hsplit`（水平分割）
  - 可以是 `split`（自动选择分割方式）
  - 也可以是 `before`、`after`、`first`、`last`、`neighbor`（kitty layout位置）

### 全局选项

全局选项在 `[options]` 部分定义：

- `layout`: 布局类型，可以是 `tiled`（平铺）或 `tabs`（标签页）
- `auto_resize`: 是否自动调整窗口大小

## 使用场景示例

### 开发环境

```toml
[options]
layout = "tiled"

[windows.web]
server = "web-dev"
command = "cd /var/www/project && tail -f logs/access.log"
position = "vsplit"
size = "50%"

[windows.api]
server = "api-dev"
command = "cd /opt/api && ./run_debug.sh"
position = "hsplit"
size = "50%"

[windows.db]
server = "db-dev"
command = "mysql -u dev"
position = "vsplit"
size = "30%"
```

### 监控环境

```toml
[options]
layout = "tiled"

[windows.cpu]
server = "monitoring"
command = "htop"
position = "vsplit"
size = "50%"

[windows.disk]
server = "storage"
command = "df -h; watch -n 5 'df -h'"
position = "hsplit"
size = "50%"

[windows.logs]
server = "app-server"
command = "cd /var/log && tail -f application.log"
position = "vsplit"
size = "40%"
```

## 注意事项

1. **会话配置存储**：所有会话配置存储在 `~/.config/rssh/sessions/` 目录下
2. **终端兼容性**：
   - Kitty布局功能仅在Kitty终端中可用，且需要启用远程控制功能
     - 在kitty.conf中添加 `allow_remote_control yes`
     - 更多信息请参考 [Kitty远程控制文档](https://sw.kovidgoyal.net/kitty/remote-control/)
   - Tmux功能需要安装tmux
3. **服务器必须存在**：窗口配置中引用的服务器必须已添加到RSSH中
4. **编辑器设置**：编辑会话配置使用系统 `$EDITOR` 环境变量指定的编辑器 