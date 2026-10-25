$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    $versionLine = Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    $version = $versionLine.Matches[0].Groups[1].Value
    $name = "Disk-Lens-$version-Windows-x64"
    $stage = Join-Path 'dist' $name
    New-Item -ItemType Directory -Force $stage | Out-Null
    cargo build --locked --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'GUI build failed' }
    Copy-Item target/x86_64-pc-windows-msvc/release/disk-lens.exe "$stage/Disk Lens.exe" -Force
    cargo build --locked --release --target x86_64-pc-windows-msvc --features console
    if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
    Copy-Item target/x86_64-pc-windows-msvc/release/disk-lens.exe "$stage/disk-lens-cli.exe" -Force
    Copy-Item WINDOWS.md "$stage/README.md" -Force
    $archive = "dist/$name.zip"
    Compress-Archive -Path "$stage/*" -DestinationPath $archive -Force
    $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    "$hash  $name.zip" | Set-Content -Encoding ascii "dist/$name.sha256"
    Write-Output "Prepared $archive"
} finally { Pop-Location }
