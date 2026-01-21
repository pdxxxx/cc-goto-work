#!/bin/bash
#
# cc-goto-work installer for Linux and macOS
# https://github.com/pdxxxx/cc-goto-work
#

set -e

REPO="pdxxxx/cc-goto-work"
INSTALL_DIR="$HOME/.claude/cc-goto-work"
CONFIG_FILE="$INSTALL_DIR/config.yaml"
CLAUDE_SETTINGS_DIR="$HOME/.claude"
CLAUDE_SETTINGS_FILE="$CLAUDE_SETTINGS_DIR/settings.json"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_banner() {
    echo -e "${BLUE}"
    echo "============================================================"
    echo "           cc-goto-work Installer                           "
    echo "   Claude Code AI-based Session Detector Hook               "
    echo "============================================================"
    echo -e "${NC}"
}

print_step() {
    echo -e "${GREEN}==>${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}Warning:${NC} $1"
}

print_error() {
    echo -e "${RED}Error:${NC} $1"
}

prompt_yes_no() {
    local prompt="$1"
    local default="$2"
    local response

    if [ "$default" = "y" ]; then
        prompt="$prompt [Y/n]: "
    else
        prompt="$prompt [y/N]: "
    fi

    read -p "$prompt" response
    response=${response:-$default}

    case "$response" in
        [yY][eE][sS]|[yY]) return 0 ;;
        *) return 1 ;;
    esac
}

prompt_input() {
    local prompt="$1"
    local default="$2"
    local response

    read -p "$prompt [$default]: " response
    echo "${response:-$default}"
}

detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux)
            case "$arch" in
                x86_64) echo "linux-amd64" ;;
                aarch64|arm64) echo "linux-arm64" ;;
                *) echo ""; return 1 ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64) echo "macos-amd64" ;;
                arm64) echo "macos-arm64" ;;
                *) echo ""; return 1 ;;
            esac
            ;;
        *)
            echo ""
            return 1
            ;;
    esac
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
}

download_binary() {
    local version="$1"
    local platform="$2"
    local dest="$3"

    local url="https://github.com/$REPO/releases/download/$version/cc-goto-work-$platform"

    print_step "Downloading cc-goto-work $version for $platform..."
    curl -fsSL "$url" -o "$dest"
    chmod +x "$dest"
}

create_config() {
    local api_base="$1"
    local api_key="$2"
    local model="$3"

    print_step "Creating config file..."

    cat > "$CONFIG_FILE" << EOF
# cc-goto-work configuration
# https://github.com/pdxxxx/cc-goto-work

# OpenAI compatible API base URL
api_base: $api_base

# API key for authentication
api_key: $api_key

# Model name to use
model: $model

# Request timeout in seconds (optional)
timeout: 30

# Enable debug logging (optional). Writes cc-goto-work.log next to the executable.
debug: false

# Custom system prompt (optional)
# Uncomment and modify to customize AI behavior
# system_prompt: |
#   You are a supervisor for an AI coding agent...
EOF

    chmod 600 "$CONFIG_FILE"
    print_step "Config file created at $CONFIG_FILE"
}

configure_settings() {
    local binary_path="$1"

    print_step "Configuring Claude Code settings..."

    # Create settings directory if not exists
    mkdir -p "$CLAUDE_SETTINGS_DIR"

    local hook_command="$binary_path"

    if [ -f "$CLAUDE_SETTINGS_FILE" ]; then
        # Check if file has content
        if [ -s "$CLAUDE_SETTINGS_FILE" ]; then
            # Backup existing settings
            cp "$CLAUDE_SETTINGS_FILE" "$CLAUDE_SETTINGS_FILE.backup"
            print_warning "Existing settings backed up to $CLAUDE_SETTINGS_FILE.backup"

            # Check if hooks.Stop already exists
            if grep -q '"Stop"' "$CLAUDE_SETTINGS_FILE" 2>/dev/null; then
                print_warning "Stop hook already configured in settings.json"
                echo ""
                echo "Please manually add or update the hook configuration:"
                echo ""
                echo -e "${YELLOW}Command to use:${NC} $hook_command"
                echo ""
                return
            fi

            # Try to add hooks to existing settings
            if command -v jq &> /dev/null; then
                local new_settings=$(jq --arg cmd "$hook_command" '.hooks.Stop = [{"hooks": [{"type": "command", "command": $cmd, "timeout": 120}]}]' "$CLAUDE_SETTINGS_FILE")
                echo "$new_settings" > "$CLAUDE_SETTINGS_FILE"
            else
                print_warning "jq not found. Please manually add the hook configuration."
                echo ""
                echo "Add the following to your $CLAUDE_SETTINGS_FILE:"
                echo ""
                cat << EOF
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$hook_command",
            "timeout": 120
          }
        ]
      }
    ]
  }
}
EOF
                return
            fi
        else
            # Empty file, write new settings
            write_new_settings "$hook_command"
        fi
    else
        # No existing file, create new
        write_new_settings "$hook_command"
    fi

    print_step "Settings configured successfully!"
}

write_new_settings() {
    local hook_command="$1"

    cat > "$CLAUDE_SETTINGS_FILE" << EOF
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$hook_command",
            "timeout": 120
          }
        ]
      }
    ]
  }
}
EOF
}

main() {
    print_banner

    # Detect platform
    print_step "Detecting platform..."
    local platform=$(detect_platform)
    if [ -z "$platform" ]; then
        print_error "Unsupported platform: $(uname -s) $(uname -m)"
        exit 1
    fi
    echo "    Platform: $platform"

    # Get latest version
    print_step "Fetching latest version..."
    local version=$(get_latest_version)
    if [ -z "$version" ]; then
        print_error "Failed to get latest version"
        exit 1
    fi
    echo "    Version: $version"

    echo ""

    # Confirm installation
    if ! prompt_yes_no "Install cc-goto-work $version?" "y"; then
        echo "Installation cancelled."
        exit 0
    fi

    echo ""

    # Create installation directory
    mkdir -p "$INSTALL_DIR"

    # Download binary
    local binary_path="$INSTALL_DIR/cc-goto-work"
    download_binary "$version" "$platform" "$binary_path"

    echo ""

    # Configure API settings
    print_step "Configuring AI settings..."
    echo ""
    echo "This hook uses an AI model to detect incomplete sessions."
    echo "You need to provide an OpenAI-compatible API endpoint."
    echo ""

    local api_base=$(prompt_input "API base URL" "https://api.openai.com/v1")
    local api_key=$(prompt_input "API key" "")
    local model=$(prompt_input "Model name" "gpt-4o-mini")

    if [ -z "$api_key" ]; then
        print_warning "No API key provided. You'll need to edit $CONFIG_FILE before using."
    fi

    echo ""

    # Create config file
    create_config "$api_base" "$api_key" "$model"

    echo ""

    # Configure Claude settings
    if prompt_yes_no "Configure Claude Code settings automatically?" "y"; then
        configure_settings "$binary_path"
    else
        echo ""
        echo "To configure manually, add this to $CLAUDE_SETTINGS_FILE:"
        echo ""
        cat << EOF
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$binary_path",
            "timeout": 120
          }
        ]
      }
    ]
  }
}
EOF
    fi

    echo ""
    echo -e "${GREEN}Installation complete!${NC}"
    echo ""
    echo "Binary installed to: $binary_path"
    echo "Config file: $CONFIG_FILE"
    echo "Settings file: $CLAUDE_SETTINGS_FILE"
    echo ""
    echo "Restart Claude Code for the hook to take effect."
}

main "$@"
