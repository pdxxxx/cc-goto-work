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

3. 创建配置文件 `~/.claude/cc-goto-work/config.yaml`（参见下方配置说明）

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

支持配置多个 API Provider，每个 Provider 可以配置多个模型。工具会并发请求所有模型，然后采用多数投票机制决定最终结果。

```yaml
providers:
  - api_base: https://api.openai.com/v1
    api_key: your-openai-key
    models:
      - gpt-4o-mini
      - gpt-4o

  - api_base: https://api.deepseek.com/v1
    api_key: your-deepseek-key
    models:
      - deepseek-chat

timeout: 30  # 可选，默认 30 秒
debug: false  # 可选，开启调试日志
```

### 投票机制

- 所有配置的模型会并发接收相同的 transcript 进行判断
- 每个模型返回 `should_continue: true`（继续工作）或 `should_continue: false`（允许停止）
- 最终决定采用多数投票结果
- **平票时倾向于继续工作**（保守策略，宁可多跑一轮也不意外停止）

### 配置项说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `providers` | 是 | Provider 列表 |
| `providers[].api_base` | 是 | OpenAI 兼容的 API 地址 |
| `providers[].api_key` | 是 | API 密钥 |
| `providers[].models` | 是 | 该 Provider 下使用的模型列表 |
| `timeout` | 否 | 请求超时秒数，默认 30 |
| `debug` | 否 | 开启调试日志 |
| `system_prompt` | 否 | 自定义系统提示词 |

### API 服务示例

```yaml
# 单 Provider 单模型
providers:
  - api_base: https://api.openai.com/v1
    api_key: sk-xxx
    models:
      - gpt-4o-mini

# 单 Provider 多模型
providers:
  - api_base: https://api.openai.com/v1
    api_key: sk-xxx
    models:
      - gpt-4o-mini
      - gpt-4o

# 多 Provider 组合
providers:
  - api_base: https://api.openai.com/v1
    api_key: sk-openai-xxx
    models:
      - gpt-4o-mini
      - gpt-4o
  - api_base: https://api.deepseek.com/v1
    api_key: sk-deepseek-xxx
    models:
      - deepseek-chat
  - api_base: http://localhost:11434/v1
    api_key: ollama
    models:
      - llama3
```

## 许可证

MIT License
