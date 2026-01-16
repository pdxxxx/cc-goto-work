#
# cc-goto-work installer for Windows
# https://github.com/pdxxxx/cc-goto-work
#

$ErrorActionPreference = "Stop"

$REPO = "pdxxxx/cc-goto-work"
$INSTALL_DIR = "$env:USERPROFILE\.local\bin"
$CLAUDE_SETTINGS_DIR = "$env:USERPROFILE\.claude"
$CLAUDE_SETTINGS_FILE = "$CLAUDE_SETTINGS_DIR\settings.json"

function Write-Banner {
    Write-Host ""
    Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║           cc-goto-work Installer                          ║" -ForegroundColor Cyan
    Write-Host "║   Claude Code RESOURCE_EXHAUSTED Auto-Continue Hook       ║" -ForegroundColor Cyan
    Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Step {
    param([string]$Message)
    Write-Host "==> " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warning {
    param([string]$Message)
    Write-Host "Warning: " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Error {
    param([string]$Message)
    Write-Host "Error: " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Read-YesNo {
    param(
        [string]$Prompt,
        [string]$Default = "y"
    )

    if ($Default -eq "y") {
        $promptText = "$Prompt [Y/n]: "
    } else {
        $promptText = "$Prompt [y/N]: "
    }

    $response = Read-Host $promptText
    if ([string]::IsNullOrWhiteSpace($response)) {
        $response = $Default
    }

    return $response -match "^[yY]"
}

function Read-Input {
    param(
        [string]$Prompt,
        [string]$Default
    )

    $response = Read-Host "$Prompt [$Default]"
    if ([string]::IsNullOrWhiteSpace($response)) {
        return $Default
    }
    return $response
}

function Get-LatestVersion {
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest" -UseBasicParsing
        return $release.tag_name
    } catch {
        return $null
    }
}

function Get-DownloadUrl {
    param([string]$Version)
    return "https://github.com/$REPO/releases/download/$Version/cc-goto-work-windows-amd64.exe"
}

function Install-Binary {
    param(
        [string]$Version,
        [string]$DestDir
    )

    $url = Get-DownloadUrl -Version $Version
    $destPath = Join-Path $DestDir "cc-goto-work.exe"

    Write-Step "Downloading cc-goto-work $Version..."
    Write-Host "    URL: $url"

    # Create directory if not exists
    if (-not (Test-Path $DestDir)) {
        New-Item -ItemType Directory -Path $DestDir -Force | Out-Null
    }

    # Download file
    try {
        Invoke-WebRequest -Uri $url -OutFile $destPath -UseBasicParsing
    } catch {
        Write-Error "Failed to download: $_"
        exit 1
    }

    Write-Host "    Downloaded to: $destPath"
    return $destPath
}

function Set-ClaudeSettings {
    param(
        [string]$BinaryPath,
        [int]$WaitTime
    )

    Write-Step "Configuring Claude Code settings..."

    # Create settings directory if not exists
    if (-not (Test-Path $CLAUDE_SETTINGS_DIR)) {
        New-Item -ItemType Directory -Path $CLAUDE_SETTINGS_DIR -Force | Out-Null
    }

    # Escape backslashes for JSON
    $escapedPath = $BinaryPath -replace '\\', '\\\\'
    $hookCommand = "$escapedPath --wait $WaitTime"

    $newSettings = @{
        hooks = @{
            Stop = @(
                @{
                    hooks = @(
                        @{
                            type = "command"
                            command = "$BinaryPath --wait $WaitTime"
                            timeout = 120
                        }
                    )
                }
            )
        }
    }

    if (Test-Path $CLAUDE_SETTINGS_FILE) {
        # Backup existing settings
        $backupPath = "$CLAUDE_SETTINGS_FILE.backup"
        Copy-Item $CLAUDE_SETTINGS_FILE $backupPath -Force
        Write-Warning "Existing settings backed up to $backupPath"

        try {
            $existingSettings = Get-Content $CLAUDE_SETTINGS_FILE -Raw | ConvertFrom-Json

            # Check if Stop hook already exists
            if ($existingSettings.hooks -and $existingSettings.hooks.Stop) {
                Write-Warning "Stop hook already configured in settings.json"
                Write-Host ""
                Write-Host "Please manually update the hook configuration:"
                Write-Host ""
                Write-Host "Command to use: " -NoNewline
                Write-Host "$BinaryPath --wait $WaitTime" -ForegroundColor Yellow
                Write-Host ""
                return
            }

            # Add hooks to existing settings
            if (-not $existingSettings.hooks) {
                $existingSettings | Add-Member -NotePropertyName "hooks" -NotePropertyValue @{} -Force
            }
            $existingSettings.hooks | Add-Member -NotePropertyName "Stop" -NotePropertyValue $newSettings.hooks.Stop -Force

            $existingSettings | ConvertTo-Json -Depth 10 | Set-Content $CLAUDE_SETTINGS_FILE -Encoding UTF8
        } catch {
            # If parsing fails, ask user what to do
            Write-Warning "Could not parse existing settings.json"
            if (Read-YesNo "Overwrite with new settings?" "n") {
                $newSettings | ConvertTo-Json -Depth 10 | Set-Content $CLAUDE_SETTINGS_FILE -Encoding UTF8
            } else {
                Write-Host ""
                Write-Host "Please manually add the hook configuration to $CLAUDE_SETTINGS_FILE"
                Show-ManualConfig -BinaryPath $BinaryPath -WaitTime $WaitTime
                return
            }
        }
    } else {
        # Create new settings file
        $newSettings | ConvertTo-Json -Depth 10 | Set-Content $CLAUDE_SETTINGS_FILE -Encoding UTF8
    }

    Write-Step "Settings configured successfully!"
}

function Show-ManualConfig {
    param(
        [string]$BinaryPath,
        [int]$WaitTime
    )

    $escapedPath = $BinaryPath -replace '\\', '\\\\'

    Write-Host ""
    Write-Host "Add the following to your settings.json:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host @"
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$escapedPath --wait $WaitTime",
            "timeout": 120
          }
        ]
      }
    ]
  }
}
"@
}

function Add-ToPath {
    param([string]$Directory)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")

    if ($currentPath -notlike "*$Directory*") {
        Write-Warning "$Directory is not in your PATH"
        Write-Host ""

        if (Read-YesNo "Add to user PATH?" "y") {
            $newPath = "$currentPath;$Directory"
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Step "Added to PATH. Please restart your terminal for changes to take effect."
        } else {
            Write-Host ""
            Write-Host "To add manually, run:" -ForegroundColor Yellow
            Write-Host ""
            Write-Host "    `$env:Path += `";$Directory`"" -ForegroundColor Cyan
            Write-Host ""
        }
    }
}

function Main {
    Write-Banner

    # Detect platform
    Write-Step "Detecting platform..."
    Write-Host "    Platform: windows-amd64"

    # Get latest version
    Write-Step "Fetching latest version..."
    $version = Get-LatestVersion
    if (-not $version) {
        Write-Error "Failed to get latest version"
        exit 1
    }
    Write-Host "    Version: $version"

    Write-Host ""

    # Confirm installation
    if (-not (Read-YesNo "Install cc-goto-work $version?" "y")) {
        Write-Host "Installation cancelled."
        exit 0
    }

    Write-Host ""

    # Ask for installation directory
    $INSTALL_DIR = Read-Input "Installation directory" $INSTALL_DIR

    # Ask for wait time
    $waitTime = Read-Input "Wait time in seconds before retry" "30"
    $waitTimeInt = [int]$waitTime

    Write-Host ""

    # Download binary
    $binaryPath = Install-Binary -Version $version -DestDir $INSTALL_DIR

    Write-Host ""

    # Configure settings
    if (Read-YesNo "Configure Claude Code settings automatically?" "y") {
        Set-ClaudeSettings -BinaryPath $binaryPath -WaitTime $waitTimeInt
    } else {
        Show-ManualConfig -BinaryPath $binaryPath -WaitTime $waitTimeInt
    }

    Write-Host ""

    # Check PATH
    Add-ToPath -Directory $INSTALL_DIR

    Write-Host ""
    Write-Host "Installation complete!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Binary installed to: $binaryPath"
    Write-Host "Settings file: $CLAUDE_SETTINGS_FILE"
    Write-Host ""
    Write-Host "Restart Claude Code for the hook to take effect."
}

# Run main
Main
