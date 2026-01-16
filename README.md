# cc-goto-work

一个 Claude Code hook，用于检测多种 API 错误并自动让 Claude 继续工作。

## 功能说明

当 Claude Code 因临时性 API 错误停止时，此 hook 会自动检测并让 Claude 继续工作。

### 支持检测的错误类型

**可重试错误（自动继续）：**
- `RESOURCE_EXHAUSTED` - API 配额/过载
- `Rate Limit` - HTTP 429 速率限制
- `Server Overload` - HTTP 503/529 服务器过载
- `max_tokens` - 输出被截断（立即继续，无需等待）
- `Unavailable` - 网络/服务不可达

**不可重试错误（允许停止，防止死循环）：**
- `context_length_exceeded` - 上下文长度超限
- `cost_limit` / `spending_limit` - 费用超限

### 检测架构

Hook 使用分层检测机制，按优先级处理：

1. **Fatal 错误检测** - 上下文/费用超限直接放行，防止死循环
2. **stop_reason 边界检测** - `end_turn` 正常停止时不会误触发历史错误
3. **结构化 JSON 错误检测** - 解析 `error.type` 字段精确匹配（仅检查最近 5 条消息）
4. **HTTP 状态码检测** - 429/503/529 状态码（仅检查最近 5 条消息）
5. **Raw 文本兜底** - 仅在 JSON 解析失败时使用（仅检查最近 8 条消息）

### 性能优化

- 使用 `Seek` 只读取 transcript 文件末尾 ~10KB，无需全量读取
- 分层检测，匹配即返回，避免不必要的处理
- 仅检查最近几条消息，避免历史错误误触发

## 安装

### 快速安装（推荐）

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/pddx/cc-goto-work/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/pddx/cc-goto-work/main/install.ps1 | iex
```

### 手动安装

1. 从 [Releases](https://github.com/pddx/cc-goto-work/releases) 下载对应平台的二进制文件：
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

**不等待（适用于 max_tokens 等场景）：**
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
2. Hook 读取对话记录文件末尾（~10KB）进行分层检测
3. 根据检测结果决定是否阻止停止：
   - **Fatal 错误**：允许停止（防止死循环）
   - **正常停止**（`end_turn`）：允许停止
   - **可重试错误**：等待配置时间后返回 JSON 响应，指示 Claude 继续工作
4. Hook 支持多次重试，等待时间机制可防止快速循环

## 从源码构建

```bash
# 克隆仓库
git clone https://github.com/pddx/cc-goto-work.git
cd cc-goto-work

# 构建 release 版本
cargo build --release

# 二进制文件位于 target/release/cc-goto-work
```

## 测试

```bash
# 运行单元测试
cargo test

# 手动测试
echo '{"transcript_path":"/tmp/test.jsonl"}' | (echo '{"type":"error","error":{"type":"RESOURCE_EXHAUSTED"}}' > /tmp/test.jsonl && ./target/release/cc-goto-work --wait 0)
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)。
