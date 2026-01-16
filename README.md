# cc-goto-work

一个 Claude Code hook，用于检测 `RESOURCE_EXHAUSTED` 错误并自动让 Claude 继续工作。

## 功能说明

当 Claude Code 因 `RESOURCE_EXHAUSTED` 错误（通常由 API 速率限制引起）停止时，此 hook 会：

1. 检测对话记录中的错误
2. 等待可配置的时间（默认：30 秒）
3. 指示 Claude 继续处理任务

这样您就不必在长时间运行的任务中遇到速率限制时手动重启 Claude。

## 安装

### 快速安装（推荐）

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/pdxxxx/cc-goto-work/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/pdxxxx/cc-goto-work/main/install.ps1 | iex
```

### 手动安装

1. 从 [Releases](https://github.com/pdxxxx/cc-goto-work/releases) 下载对应平台的二进制文件：
   - `cc-goto-work-linux-amd64` - Linux x86_64
   - `cc-goto-work-linux-arm64` - Linux ARM64
   - `cc-goto-work-windows-amd64.exe` - Windows x86_64
   - `cc-goto-work-macos-amd64` - macOS x86_64 (Intel)
   - `cc-goto-work-macos-arm64` - macOS ARM64 (Apple Silicon)

2. 将二进制文件放置在您选择的目录中（例如 `~/.local/bin/` 或 `C:\Users\<用户名>\bin\`）

3. 将 hook 配置添加到 Claude Code 设置文件中：

   **文件位置：**
   - 用户设置：`~/.claude/settings.json`
   - 项目设置：`.claude/settings.json`

   **配置内容：**
   ```json
   {
     "hooks": {
       "Stop": [
         {
           "hooks": [
             {
               "type": "command",
               "command": "/path/to/cc-goto-work --wait 30",
               "timeout": 60
             }
           ]
         }
       ]
     }
   }
   ```

## 使用方法

### 命令行选项

```
cc-goto-work [选项]

选项：
    -w, --wait <秒数>    继续之前的等待时间（默认：30）
    -h, --help           显示帮助信息
    -V, --version        显示版本信息
```

### 配置示例

**使用默认等待时间（30 秒）：**
```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "cc-goto-work"
          }
        ]
      }
    ]
  }
}
```

**自定义等待时间（60 秒）：**
```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "cc-goto-work --wait 60"
          }
        ]
      }
    ]
  }
}
```

**不等待：**
```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "cc-goto-work --wait 0"
          }
        ]
      }
    ]
  }
}
```

## 工作原理

1. 当 Claude Code 触发 Stop 事件时，此 hook 通过 stdin 接收事件数据
2. Hook 读取对话记录文件以检查 `RESOURCE_EXHAUSTED` 错误
3. 如果检测到错误，等待配置的时间后返回 JSON 响应，指示 Claude 继续工作
4. 如果未检测到错误，允许正常停止

Hook 支持多次重试 —— 只要检测到 `RESOURCE_EXHAUSTED` 错误，就会等待并重试，等待时间机制可防止快速循环。

## 从源码构建

```bash
# 克隆仓库
git clone https://github.com/pdxxxx/cc-goto-work.git
cd cc-goto-work

# 构建 release 版本
cargo build --release

# 二进制文件位于 target/release/cc-goto-work
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)。
