# cc-goto-work

A Claude Code hook that detects `RESOURCE_EXHAUSTED` errors and automatically instructs Claude to continue working.

## What it does

When Claude Code stops due to a `RESOURCE_EXHAUSTED` error (typically caused by API rate limits), this hook:

1. Detects the error in the conversation transcript
2. Waits for a configurable amount of time (default: 30 seconds)
3. Instructs Claude to continue working on the task

This prevents you from having to manually restart Claude when hitting rate limits during long-running tasks.

## Installation

### Quick Install (Recommended)

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/pdxxxx/cc-goto-work/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/pdxxxx/cc-goto-work/main/install.ps1 | iex
```

### Manual Installation

1. Download the appropriate binary from [Releases](https://github.com/pdxxxx/cc-goto-work/releases):
   - `cc-goto-work-linux-amd64` - Linux x86_64
   - `cc-goto-work-linux-arm64` - Linux ARM64
   - `cc-goto-work-windows-amd64.exe` - Windows x86_64
   - `cc-goto-work-macos-amd64` - macOS x86_64 (Intel)
   - `cc-goto-work-macos-arm64` - macOS ARM64 (Apple Silicon)

2. Place the binary in a directory of your choice (e.g., `~/.local/bin/` or `C:\Users\<username>\bin\`)

3. Add the hook configuration to your Claude Code settings file:

   **Location:**
   - User settings: `~/.claude/settings.json`
   - Project settings: `.claude/settings.json`

   **Configuration:**
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

## Usage

### Command Line Options

```
cc-goto-work [OPTIONS]

OPTIONS:
    -w, --wait <SECONDS>    Wait time before continuing (default: 30)
    -h, --help              Print help information
    -V, --version           Print version information
```

### Examples

**Use default wait time (30 seconds):**
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

**Custom wait time (60 seconds):**
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

**No wait time:**
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

## How it works

1. When Claude Code triggers a Stop event, this hook receives the event data via stdin
2. The hook reads the conversation transcript file to check for `RESOURCE_EXHAUSTED` errors
3. If detected, it waits for the configured time and returns a JSON response instructing Claude to continue
4. If not detected, it allows the stop to proceed normally

The hook also includes protection against infinite loops via the `stop_hook_active` flag.

## Building from source

```bash
# Clone the repository
git clone https://github.com/pdxxxx/cc-goto-work.git
cd cc-goto-work

# Build release binary
cargo build --release

# Binary will be at target/release/cc-goto-work
```

## License

MIT License - see [LICENSE](LICENSE) for details.
