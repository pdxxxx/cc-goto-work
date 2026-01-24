#!/usr/bin/env node

const readline = require('readline');
const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { spawn } = require('child_process');

// ============================================================================
// Constants
// ============================================================================

const REPO = 'pdxxxx/cc-goto-work';
const INSTALL_DIR = path.join(os.homedir(), '.claude', 'cc-goto-work');
const CONFIG_FILE = path.join(INSTALL_DIR, 'config.yaml');
const CLAUDE_SETTINGS_DIR = path.join(os.homedir(), '.claude');
const CLAUDE_SETTINGS_FILE = path.join(CLAUDE_SETTINGS_DIR, 'settings.json');

// Colors (ANSI escape codes)
const colors = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  bold: '\x1b[1m',
};

// ============================================================================
// Utilities
// ============================================================================

function print(msg) {
  console.log(msg);
}

function printBanner() {
  print(`${colors.cyan}${colors.bold}`);
  print('╔════════════════════════════════════════════════════════════╗');
  print('║              cc-goto-work 安装程序                         ║');
  print('║      让 Claude Code 自动继续未完成的工作                   ║');
  print('╚════════════════════════════════════════════════════════════╝');
  print(`${colors.reset}`);
}

function printStep(msg) {
  print(`${colors.green}▶${colors.reset} ${msg}`);
}

function printSuccess(msg) {
  print(`${colors.green}✔${colors.reset} ${msg}`);
}

function printWarning(msg) {
  print(`${colors.yellow}⚠${colors.reset} ${msg}`);
}

function printError(msg) {
  print(`${colors.red}✖${colors.reset} ${msg}`);
}

function printMenu() {
  print('');
  print(`${colors.bold}请选择操作：${colors.reset}`);
  print('');
  print(`  ${colors.cyan}1${colors.reset} - 完整安装 (下载 + 配置 API + 配置 Hook)`);
  print(`  ${colors.cyan}2${colors.reset} - 仅下载二进制文件`);
  print(`  ${colors.cyan}3${colors.reset} - 仅配置 API 设置`);
  print(`  ${colors.cyan}4${colors.reset} - 仅配置 Claude Code Hook`);
  print(`  ${colors.cyan}5${colors.reset} - ${colors.blue}检查更新${colors.reset}`);
  print(`  ${colors.cyan}6${colors.reset} - ${colors.red}卸载 cc-goto-work${colors.reset}`);
  print(`  ${colors.cyan}0${colors.reset} - 退出`);
  print('');
}

// ============================================================================
// Readline Interface
// ============================================================================

function createRL() {
  return readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
}

function question(rl, prompt) {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      resolve(answer.trim());
    });
  });
}

async function promptInput(rl, prompt, defaultValue = '') {
  const displayDefault = defaultValue ? ` [${defaultValue}]` : '';
  const answer = await question(rl, `${prompt}${displayDefault}: `);
  return answer || defaultValue;
}

async function promptConfirm(rl, prompt, defaultYes = true) {
  const hint = defaultYes ? '[Y/n]' : '[y/N]';
  const answer = await question(rl, `${prompt} ${hint}: `);
  if (!answer) return defaultYes;
  return answer.toLowerCase().startsWith('y');
}

// ============================================================================
// Platform Detection
// ============================================================================

function detectPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === 'linux') {
    if (arch === 'x64') return 'linux-amd64';
    if (arch === 'arm64') return 'linux-arm64';
  } else if (platform === 'darwin') {
    if (arch === 'x64') return 'macos-amd64';
    if (arch === 'arm64') return 'macos-arm64';
  } else if (platform === 'win32') {
    if (arch === 'x64') return 'windows-amd64';
  }

  return null;
}

function getBinaryName(platformStr) {
  if (platformStr.startsWith('windows')) {
    return 'cc-goto-work.exe';
  }
  return 'cc-goto-work';
}

// ============================================================================
// GitHub API
// ============================================================================

function httpsGet(url) {
  return new Promise((resolve, reject) => {
    const options = {
      headers: { 'User-Agent': 'cc-goto-work-installer' },
    };

    https.get(url, options, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        // Follow redirect
        httpsGet(res.headers.location).then(resolve).catch(reject);
        return;
      }

      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode}`));
        return;
      }

      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    }).on('error', reject);
  });
}

async function getLatestVersion() {
  const url = `https://api.github.com/repos/${REPO}/releases/latest`;
  const data = await httpsGet(url);
  const json = JSON.parse(data.toString());
  return json.tag_name;
}

// ============================================================================
// Download Binary
// ============================================================================

async function downloadBinary(version, platformStr) {
  const binaryName = getBinaryName(platformStr);
  const fileName = platformStr.startsWith('windows')
    ? `cc-goto-work-${platformStr}.exe`
    : `cc-goto-work-${platformStr}`;

  const url = `https://github.com/${REPO}/releases/download/${version}/${fileName}`;
  const destPath = path.join(INSTALL_DIR, binaryName);

  printStep(`正在下载 ${version} (${platformStr})...`);

  // Ensure directory exists
  fs.mkdirSync(INSTALL_DIR, { recursive: true });

  const data = await httpsGet(url);
  fs.writeFileSync(destPath, data);

  // Make executable on Unix
  if (!platformStr.startsWith('windows')) {
    fs.chmodSync(destPath, 0o755);
  }

  // Save version info
  const versionFile = path.join(INSTALL_DIR, '.version');
  fs.writeFileSync(versionFile, version);

  printSuccess(`二进制文件已下载到: ${destPath}`);
  return destPath;
}

// ============================================================================
// Configuration
// ============================================================================

function createConfig(apiBase, apiKey, model) {
  const configContent = `# cc-goto-work configuration
# https://github.com/${REPO}

providers:
  - api_base: ${apiBase}
    api_key: ${apiKey}
    models:
      - ${model}

# Request timeout in seconds (optional)
timeout: 30

# Enable debug logging (optional)
debug: false
`;

  fs.mkdirSync(INSTALL_DIR, { recursive: true });
  fs.writeFileSync(CONFIG_FILE, configContent, { mode: 0o600 });
  printSuccess(`配置文件已创建: ${CONFIG_FILE}`);
}

async function configureAPI(rl) {
  print('');
  print('此工具使用 AI 模型来检测未完成的会话。');
  print('请提供 OpenAI 兼容的 API 端点信息。');
  print('');

  const apiBase = await promptInput(rl, 'API 地址', 'https://api.openai.com/v1');
  const apiKey = await promptInput(rl, 'API 密钥', '');
  const model = await promptInput(rl, '模型名称', 'gpt-4o-mini');

  if (!apiKey) {
    printWarning('未提供 API 密钥，请在使用前编辑配置文件');
  }

  createConfig(apiBase, apiKey, model);
}

// ============================================================================
// Claude Settings
// ============================================================================

function configureClaudeSettings(binaryPath) {
  fs.mkdirSync(CLAUDE_SETTINGS_DIR, { recursive: true });

  const hookConfig = {
    hooks: {
      Stop: [
        {
          hooks: [
            {
              type: 'command',
              command: binaryPath,
              timeout: 120,
            },
          ],
        },
      ],
    },
  };

  let settings = {};

  if (fs.existsSync(CLAUDE_SETTINGS_FILE)) {
    try {
      const content = fs.readFileSync(CLAUDE_SETTINGS_FILE, 'utf8');
      if (content.trim()) {
        settings = JSON.parse(content);

        // Backup existing
        fs.writeFileSync(`${CLAUDE_SETTINGS_FILE}.backup`, content);
        printWarning(`已备份现有配置到: ${CLAUDE_SETTINGS_FILE}.backup`);

        // Check if Stop hook exists
        if (settings.hooks && settings.hooks.Stop) {
          printWarning('Stop hook 已存在，正在更新...');
        }
      }
    } catch (e) {
      printWarning('无法解析现有配置，将创建新配置');
    }
  }

  // Merge hooks
  settings.hooks = settings.hooks || {};
  settings.hooks.Stop = hookConfig.hooks.Stop;

  fs.writeFileSync(CLAUDE_SETTINGS_FILE, JSON.stringify(settings, null, 2));
  printSuccess(`Claude Code 配置已更新: ${CLAUDE_SETTINGS_FILE}`);
}

// ============================================================================
// Installation Actions
// ============================================================================

async function fullInstall(rl) {
  printStep('开始完整安装...');
  print('');

  // Detect platform
  const platformStr = detectPlatform();
  if (!platformStr) {
    printError(`不支持的平台: ${os.platform()} ${os.arch()}`);
    return false;
  }
  print(`  平台: ${platformStr}`);

  // Get latest version
  printStep('获取最新版本...');
  let version;
  try {
    version = await getLatestVersion();
    print(`  版本: ${version}`);
  } catch (e) {
    printError(`获取版本失败: ${e.message}`);
    return false;
  }

  print('');
  if (!(await promptConfirm(rl, `确认安装 cc-goto-work ${version}?`))) {
    print('安装已取消');
    return false;
  }

  // Download binary
  print('');
  let binaryPath;
  try {
    binaryPath = await downloadBinary(version, platformStr);
  } catch (e) {
    printError(`下载失败: ${e.message}`);
    return false;
  }

  // Configure API
  print('');
  await configureAPI(rl);

  // Configure Claude settings
  print('');
  if (await promptConfirm(rl, '自动配置 Claude Code?')) {
    configureClaudeSettings(binaryPath);
  } else {
    print('');
    print('请手动添加以下配置到 Claude Code:');
    print(`  命令: ${binaryPath}`);
  }

  return true;
}

async function downloadOnly(rl) {
  printStep('仅下载二进制文件...');
  print('');

  const platformStr = detectPlatform();
  if (!platformStr) {
    printError(`不支持的平台: ${os.platform()} ${os.arch()}`);
    return false;
  }
  print(`  平台: ${platformStr}`);

  printStep('获取最新版本...');
  let version;
  try {
    version = await getLatestVersion();
    print(`  版本: ${version}`);
  } catch (e) {
    printError(`获取版本失败: ${e.message}`);
    return false;
  }

  print('');
  try {
    await downloadBinary(version, platformStr);
  } catch (e) {
    printError(`下载失败: ${e.message}`);
    return false;
  }

  return true;
}

async function configureAPIOnly(rl) {
  printStep('配置 API 设置...');
  await configureAPI(rl);
  return true;
}

async function configureHookOnly(rl) {
  printStep('配置 Claude Code Hook...');
  print('');

  const platformStr = detectPlatform();
  const binaryName = getBinaryName(platformStr || 'linux-amd64');
  const binaryPath = path.join(INSTALL_DIR, binaryName);

  if (!fs.existsSync(binaryPath)) {
    printWarning(`二进制文件不存在: ${binaryPath}`);
    if (!(await promptConfirm(rl, '是否继续配置?', false))) {
      return false;
    }
  }

  configureClaudeSettings(binaryPath);
  return true;
}

// ============================================================================
// Update
// ============================================================================

const VERSION_FILE = path.join(INSTALL_DIR, '.version');

function getInstalledVersion() {
  try {
    if (fs.existsSync(VERSION_FILE)) {
      return fs.readFileSync(VERSION_FILE, 'utf8').trim();
    }
  } catch (e) {
    // Ignore
  }
  return null;
}

function saveInstalledVersion(version) {
  try {
    fs.mkdirSync(INSTALL_DIR, { recursive: true });
    fs.writeFileSync(VERSION_FILE, version);
  } catch (e) {
    // Ignore
  }
}

function compareVersions(v1, v2) {
  // Remove 'v' prefix if present
  const normalize = (v) => v.replace(/^v/, '').split('.').map(Number);
  const parts1 = normalize(v1);
  const parts2 = normalize(v2);

  for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
    const p1 = parts1[i] || 0;
    const p2 = parts2[i] || 0;
    if (p1 > p2) return 1;
    if (p1 < p2) return -1;
  }
  return 0;
}

async function checkUpdate(rl) {
  printStep('检查更新...');
  print('');

  const platformStr = detectPlatform();
  if (!platformStr) {
    printError(`不支持的平台: ${os.platform()} ${os.arch()}`);
    return false;
  }

  const binaryName = getBinaryName(platformStr);
  const binaryPath = path.join(INSTALL_DIR, binaryName);

  if (!fs.existsSync(binaryPath)) {
    printWarning('未检测到安装，请先安装');
    return false;
  }

  // Get installed version
  const installedVersion = getInstalledVersion();
  if (installedVersion) {
    print(`  当前版本: ${installedVersion}`);
  } else {
    print(`  当前版本: ${colors.yellow}未知${colors.reset}`);
  }

  // Get latest version
  printStep('获取最新版本...');
  let latestVersion;
  try {
    latestVersion = await getLatestVersion();
    print(`  最新版本: ${latestVersion}`);
  } catch (e) {
    printError(`获取版本失败: ${e.message}`);
    return false;
  }

  print('');

  // Compare versions
  if (installedVersion && compareVersions(installedVersion, latestVersion) >= 0) {
    printSuccess('已是最新版本，无需更新');
    return true;
  }

  // Ask to update
  const updateMsg = installedVersion
    ? `发现新版本！是否从 ${installedVersion} 更新到 ${latestVersion}?`
    : `是否更新到 ${latestVersion}?`;

  if (!(await promptConfirm(rl, updateMsg))) {
    print('更新已取消');
    return false;
  }

  // Download new version
  print('');
  try {
    await downloadBinary(latestVersion, platformStr);
    saveInstalledVersion(latestVersion);
    printSuccess(`已更新到 ${latestVersion}`);
    return true;
  } catch (e) {
    printError(`更新失败: ${e.message}`);
    return false;
  }
}

// ============================================================================
// Uninstall
// ============================================================================

function removeStopHook() {
  if (!fs.existsSync(CLAUDE_SETTINGS_FILE)) {
    printWarning('Claude Code 配置文件不存在');
    return false;
  }

  try {
    const content = fs.readFileSync(CLAUDE_SETTINGS_FILE, 'utf8');
    if (!content.trim()) {
      printWarning('Claude Code 配置文件为空');
      return false;
    }

    const settings = JSON.parse(content);

    if (!settings.hooks || !settings.hooks.Stop) {
      printWarning('Stop hook 不存在，无需移除');
      return false;
    }

    // Backup before modifying
    fs.writeFileSync(`${CLAUDE_SETTINGS_FILE}.backup`, content);
    printWarning(`已备份现有配置到: ${CLAUDE_SETTINGS_FILE}.backup`);

    // Remove Stop hook
    delete settings.hooks.Stop;

    // Remove hooks object if empty
    if (Object.keys(settings.hooks).length === 0) {
      delete settings.hooks;
    }

    fs.writeFileSync(CLAUDE_SETTINGS_FILE, JSON.stringify(settings, null, 2));
    printSuccess('已从 Claude Code 配置中移除 Stop hook');
    return true;
  } catch (e) {
    printError(`移除 Stop hook 失败: ${e.message}`);
    return false;
  }
}

function removeDirectory(dirPath) {
  if (!fs.existsSync(dirPath)) {
    return false;
  }

  try {
    fs.rmSync(dirPath, { recursive: true, force: true });
    return true;
  } catch (e) {
    printError(`删除目录失败: ${e.message}`);
    return false;
  }
}

async function uninstall(rl) {
  printStep('卸载 cc-goto-work...');
  print('');

  // Check what exists
  const platformStr = detectPlatform();
  const binaryName = getBinaryName(platformStr || 'linux-amd64');
  const binaryPath = path.join(INSTALL_DIR, binaryName);

  const binaryExists = fs.existsSync(binaryPath);
  const configExists = fs.existsSync(CONFIG_FILE);
  const dirExists = fs.existsSync(INSTALL_DIR);

  if (!binaryExists && !configExists && !dirExists) {
    printWarning('未检测到安装，无需卸载');
    return false;
  }

  print('检测到以下已安装内容:');
  if (binaryExists) print(`  - 二进制文件: ${binaryPath}`);
  if (configExists) print(`  - 配置文件: ${CONFIG_FILE}`);
  if (dirExists) print(`  - 安装目录: ${INSTALL_DIR}`);
  print('');

  if (!(await promptConfirm(rl, `${colors.red}确认卸载 cc-goto-work?${colors.reset}`, false))) {
    print('卸载已取消');
    return false;
  }

  print('');

  // Remove Stop hook from Claude settings
  printStep('移除 Claude Code Hook...');
  removeStopHook();

  // Remove installation directory
  printStep('删除安装目录...');
  if (dirExists) {
    if (removeDirectory(INSTALL_DIR)) {
      printSuccess(`已删除: ${INSTALL_DIR}`);
    }
  } else {
    printWarning('安装目录不存在');
  }

  return true;
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  printBanner();

  const rl = createRL();

  try {
    while (true) {
      printMenu();

      const choice = await question(rl, '请输入选项 (0-6): ');

      let success = false;

      switch (choice) {
        case '1':
          success = await fullInstall(rl);
          break;
        case '2':
          success = await downloadOnly(rl);
          break;
        case '3':
          success = await configureAPIOnly(rl);
          break;
        case '4':
          success = await configureHookOnly(rl);
          break;
        case '5':
          success = await checkUpdate(rl);
          break;
        case '6':
          success = await uninstall(rl);
          break;
        case '0':
        case 'q':
        case 'exit':
          print('');
          print('再见！');
          rl.close();
          return;
        default:
          printError('无效选项，请输入 0-6');
          continue;
      }

      if (success) {
        print('');
        printSuccess('操作完成！');
        print('');
        print('请重启 Claude Code 以使配置生效。');
      }

      print('');
      if (!(await promptConfirm(rl, '继续其他操作?', false))) {
        print('');
        print('再见！');
        break;
      }
    }
  } finally {
    rl.close();
  }
}

main().catch((err) => {
  printError(`发生错误: ${err.message}`);
  process.exit(1);
});
