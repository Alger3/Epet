param(
  [switch]$SkipInstall,
  [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

function Assert-Command([string]$Name, [string]$Hint) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "缺少命令 '$Name'。$Hint"
  }
}

Assert-Command "node" "请安装 README 指定的 Node.js 版本。"
Assert-Command "npm" "Node.js 安装应包含 npm。"
Assert-Command "cargo" "请通过 rustup 安装 Rust stable 与 MSVC 工具链。"
Assert-Command "rustc" "请通过 rustup 安装 Rust stable 与 MSVC 工具链。"

Push-Location $repoRoot
try {
  Write-Host "[Epet] Windows verified build" -ForegroundColor Cyan
  Write-Host "Node: $(node --version)"
  Write-Host "npm:  $(npm --version)"
  Write-Host "Rust: $(rustc --version)"

  if (-not $SkipInstall) {
    npm ci
  }

  if (-not $SkipTests) {
    npm test
    npm run test:e2e
    npm run lint
  }

  npm run build:desktop

  $bundleRoot = Join-Path $repoRoot "apps/desktop/src-tauri/target/release/bundle/nsis"
  $installers = @(Get-ChildItem -Path $bundleRoot -Filter "*.exe" -File -ErrorAction SilentlyContinue)
  if ($installers.Count -eq 0) {
    throw "构建完成但未在 $bundleRoot 找到 NSIS 安装包。"
  }

  Write-Host "`n构建成功：" -ForegroundColor Green
  foreach ($installer in $installers) {
    $hash = (Get-FileHash -Algorithm SHA256 $installer.FullName).Hash.ToLowerInvariant()
    Write-Host "  $($installer.FullName)"
    Write-Host "  SHA256 $hash"
  }
} finally {
  Pop-Location
}
