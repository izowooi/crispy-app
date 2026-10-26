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
    Copy-Item ../LICENSE "$stage/LICENSE.txt" -Force
    $metadataText = cargo metadata --locked --offline --filter-platform x86_64-pc-windows-msvc --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'License metadata failed' }
    $metadata = $metadataText | ConvertFrom-Json
    $notices = [Text.StringBuilder]::new()
    foreach ($package in ($metadata.packages | Where-Object { $_.source } | Sort-Object name)) {
        [void]$notices.AppendLine("`n=== $($package.name) $($package.version) ($($package.license)) ===")
        $licenses = @(Get-ChildItem -LiteralPath (Split-Path $package.manifest_path -Parent) -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|UNLICENSE)' })
        if (!$licenses.Count) { $licenses = @(Get-ChildItem -LiteralPath "licenses/$($package.name)-$($package.version)" -File) }
        if (!$licenses.Count) { throw "Missing license text: $($package.name)" }
        foreach ($license in $licenses) { [void]$notices.AppendLine([IO.File]::ReadAllText($license.FullName)) }
        if ($package.name -eq 'epaint_default_fonts') {
            foreach ($license in (Get-ChildItem -LiteralPath (Join-Path (Split-Path $package.manifest_path -Parent) 'fonts') -File | Where-Object { $_.Extension -eq '.txt' })) {
                [void]$notices.AppendLine([IO.File]::ReadAllText($license.FullName))
            }
        }
    }
    [IO.File]::WriteAllText((Join-Path (Get-Location) "$stage/THIRD-PARTY-NOTICES.txt"), $notices.ToString(), [Text.Encoding]::UTF8)
    $archive = "dist/$name.zip"
    Compress-Archive -Path "$stage/*" -DestinationPath $archive -Force
    $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    "$hash  $name.zip" | Set-Content -Encoding ascii "dist/$name.sha256"
    Write-Output "Prepared $archive"
} finally { Pop-Location }
