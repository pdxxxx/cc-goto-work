# cc-goto-work

一个 Claude Code hook，使用 AI 模型智能判断会话是否需要继续工作。

## 功能说明

当 Claude Code 停止时，此 hook 会调用 AI 模型分析对话记录，智能判断是否需要让 Claude 继续工作。

### 工作原理

1. 当 Claude Code 触发 Stop 事件时，hook 通过 stdin 接收事件数据
2. 读取对话记录文件末尾（~10KB）的最近 20 条消息
3. 将对话内容发送给配置的 AI 模型进行分析
4. AI 判断任务是否完成：
   - **需要继续**：返回 JSON 响应阻止停止，让 Claude 继续工作
   - **已完成**：允许正常停止

### AI 判断标准

**判定为需要继续 (`should_continue: true`)：**
- Claude 的回复被截断（输出中断）
- 遇到 API 错误、速率限制或服务器问题
- Claude 说"我会做 X"但实际没有完成
- 回复明显不完整，还有更多工作要做
- 对话记录中出现 `[Error: ...]` 错误信息

**判定为已完成 (`should_continue: false`)：**
- Claude 完成了请求的任务
- Claude 提出问题等待用户回复
- Claude 明确表示完成或询问反馈
- 对话自然结束
- Claude 合理地拒绝了请求

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

安装脚本会：
1. 下载二进制文件到 `~/.claude/cc-goto-work/`
2. 引导您配置 AI API 设置
3. 自动配置 Claude Code 的 hook

### 手动安装

1. 从 [Releases](https://github.com/pdxxxx/cc-goto-work/releases) 下载对应平台的二进制文件：
   - `cc-goto-work-linux-amd64` - Linux x86_64
   - `cc-goto-work-linux-arm64` - Linux ARM64
   - `cc-goto-work-windows-amd64.exe` - Windows x86_64
   - `cc-goto-work-macos-amd64` - macOS x86_64 (Intel)
   - `cc-goto-work-macos-arm64` - macOS ARM64 (Apple Silicon)

2. 将二进制文件放置到 `~/.claude/cc-goto-work/` 目录

3. 创建配置文件 `~/.claude/cc-goto-work/config.yaml`：
   ```yaml
   # OpenAI 兼容的 API 地址
   api_base: https://api.openai.com/v1

   # API 密钥
   api_key: your-api-key-here

   # 模型名称
   model: gpt-4o-mini

   # 请求超时（秒），可选
   timeout: 30

   # 自定义系统提示词，可选
   # system_prompt: |
   #   Your custom prompt here...
   ```

4. 配置 Claude Code hook，编辑 `~/.claude/settings.json`：
   ```json
   {
     "hooks": {
       "Stop": [
         {
           "hooks": [
             {
               "type": "command",
               "command": "~/.claude/cc-goto-work/cc-goto-work",
               "timeout": 120
             }
           ]
         }
       ]
     }
   }
   ```

## 配置说明

### 配置文件

默认路径：`~/.claude/cc-goto-work/config.yaml`

| 字段 | 必填 | 说明 |
|------|------|------|
| `api_base` | 是 | OpenAI 兼容的 API 地址 |
| `api_key` | 是 | API 密钥 |
| `model` | 是 | 模型名称 |
| `timeout` | 否 | 请求超时秒数，默认 30 |
| `system_prompt` | 否 | 自定义系统提示词 |

### 支持的 API 服务

任何 OpenAI 兼容的 API 都可以使用：

**OpenAI:**
```yaml
api_base: https://api.openai.com/v1
model: gpt-4o-mini
```

**DeepSeek:**
```yaml
api_base: https://api.deepseek.com/v1
model: deepseek-chat
```

**本地 Ollama:**
```yaml
api_base: http://localhost:11434/v1
model: llama3
```

### 命令行选项

```
cc-goto-work [选项]

选项：
    -c, --config <路径>  配置文件路径（默认：~/.claude/cc-goto-work/config.yaml）
    -h, --help           显示帮助信息
    -V, --version        显示版本信息
```

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
