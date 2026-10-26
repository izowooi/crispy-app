$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe' }
    if (-not (Test-Path -LiteralPath $cargoPath)) { throw 'Rust cargo was not found' }
    & $cargoPath build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $packageDir = Join-Path $projectRoot 'dist/obsidian-disk-windows-x64'
    New-Item -ItemType Directory -Path $packageDir -Force | Out-Null
    Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/obsidian-disk.exe' -Destination $packageDir -Force
    Copy-Item -LiteralPath 'README.md' -Destination $packageDir -Force
    Copy-Item -LiteralPath '../LICENSE' -Destination $packageDir -Force
    $zipPath = Join-Path $projectRoot 'dist/obsidian-disk-windows-x64.zip'
    Compress-Archive -Path "$packageDir/*" -DestinationPath $zipPath -Force
    Get-FileHash -LiteralPath $zipPath -Algorithm SHA256
} finally {
    Pop-Location
}
