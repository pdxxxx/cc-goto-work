# cc-goto-work

让 Claude Code 自动继续未完成的工作。

当 Claude 意外停止（如输出截断、API 错误）时，此工具会自动检测并让 Claude 继续工作。

## 安装

### 使用 npx（推荐）

```bash
npx cc-goto-work
```

运行后会显示交互式菜单：

```
╔════════════════════════════════════════════════════════════╗
║              cc-goto-work 安装程序                         ║
╚════════════════════════════════════════════════════════════╝

请选择操作：

  1 - 完整安装 (下载 + 配置 API + 配置 Hook)
  2 - 仅下载二进制文件
  3 - 仅配置 API 设置
  4 - 仅配置 Claude Code Hook
  0 - 退出
```

### 手动安装

1. 从 [Releases](https://github.com/pdxxxx/cc-goto-work/releases) 下载对应平台的二进制文件

2. 放置到 `~/.claude/cc-goto-work/` 目录

3. 创建配置文件 `~/.claude/cc-goto-work/config.yaml`：
   ```yaml
   api_base: https://api.openai.com/v1
   api_key: your-api-key-here
   model: gpt-4o-mini
   ```

4. 编辑 `~/.claude/settings.json`：
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

## 配置

配置文件：`~/.claude/cc-goto-work/config.yaml`

| 字段 | 必填 | 说明 |
|------|------|------|
| `api_base` | 是 | OpenAI 兼容的 API 地址 |
| `api_key` | 是 | API 密钥 |
| `model` | 是 | 模型名称 |
| `timeout` | 否 | 请求超时秒数，默认 30 |
| `debug` | 否 | 开启调试日志 |

### API 服务示例

```yaml
# OpenAI
api_base: https://api.openai.com/v1
model: gpt-4o-mini

# DeepSeek
api_base: https://api.deepseek.com/v1
model: deepseek-chat

# 本地 Ollama
api_base: http://localhost:11434/v1
model: llama3
```

## 许可证

MIT License
