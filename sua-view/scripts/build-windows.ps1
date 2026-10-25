param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Push-Location (Split-Path $PSScriptRoot -Parent)
$previousRustflags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = "$previousRustflags -C target-feature=+crt-static".Trim()
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) { $cargoPath = $cargo.Source } else { $cargoPath = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    if (!(Test-Path -LiteralPath $cargoPath)) { throw 'Install Rust stable and Visual Studio C++ Build Tools first.' }
    if (!$SkipChecks) {
        & $cargoPath test --locked
        if ($LASTEXITCODE) { throw 'Rust tests failed.' }
        & $cargoPath clippy --locked --all-targets -- -D warnings
        if ($LASTEXITCODE) { throw 'Clippy failed.' }
    }
    & $cargoPath build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE) { throw 'Rust release build failed.' }
    $framework = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
    $wpf = Join-Path $framework 'WPF'
    $out = Join-Path (Get-Location) 'dist\Sua-View-windows-x64'
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    & "$framework\csc.exe" /nologo /target:winexe /platform:x64 /optimize+ /utf8output "/out:$out\SuaView.exe" '/win32manifest:native\windows\app.manifest' "/reference:$wpf\PresentationCore.dll" "/reference:$wpf\PresentationFramework.dll" "/reference:$wpf\WindowsBase.dll" /reference:System.Xaml.dll /reference:System.Windows.Forms.dll /reference:System.Web.Extensions.dll '/resource:assets\ReaderTheme.json,SuaView.ReaderTheme.json' '/resource:native\windows\ReaderStyles.xaml,SuaView.ReaderStyles.xaml' 'native\windows\SuaView.cs' 'native\windows\Appearance.cs' 'native\windows\ReaderChrome.cs'
    if ($LASTEXITCODE) { throw 'Windows UI build failed.' }
    Copy-Item -LiteralPath 'target\x86_64-pc-windows-msvc\release\sua_view_core.dll' -Destination $out
    Copy-Item -LiteralPath 'target\x86_64-pc-windows-msvc\release\sua-view-engine.exe' -Destination $out
    Copy-Item -LiteralPath 'native\windows\SuaView.exe.config' -Destination $out
    Copy-Item -LiteralPath 'docs\WINDOWS.md' -Destination "$out\README.txt"
    $metadataText = & $cargoPath metadata --locked --offline --filter-platform x86_64-pc-windows-msvc --format-version 1
    if ($LASTEXITCODE) { throw 'Cannot read dependency license metadata.' }
    $metadata = $metadataText | ConvertFrom-Json
    $notices = [Text.StringBuilder]::new()
    [void]$notices.AppendLine('Sua View: third-party Rust dependency licenses')
    foreach ($package in ($metadata.packages | Where-Object { $_.source } | Sort-Object name)) {
        [void]$notices.AppendLine("`n=== $($package.name) $($package.version) ($($package.license)) ===")
        $packageRoot = Split-Path $package.manifest_path -Parent
        $licenses = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|COPYING)' })
        if ($licenses.Count -eq 0) { throw "Missing license text: $($package.name)" }
        foreach ($license in $licenses) {
            [void]$notices.AppendLine("--- $($license.Name) ---")
            [void]$notices.AppendLine([IO.File]::ReadAllText($license.FullName))
        }
    }
    [IO.File]::WriteAllText("$out\THIRD-PARTY-NOTICES.txt", $notices.ToString(), [Text.Encoding]::UTF8)
    $zip = Join-Path (Get-Location) 'dist\Sua-View-0.1.0-windows-x64.zip'
    $assets = @('SuaView.exe', 'SuaView.exe.config', 'sua_view_core.dll', 'sua-view-engine.exe', 'README.txt', 'THIRD-PARTY-NOTICES.txt') | ForEach-Object { Join-Path $out $_ }
    Compress-Archive -LiteralPath $assets -DestinationPath $zip -Force
    # Use .NET directly: a Windows PowerShell child can inherit PowerShell 7's
    # module search path in CI, where Get-FileHash may not be available.
    $sha256 = [Security.Cryptography.SHA256]::Create()
    $zipStream = [IO.File]::OpenRead($zip)
    try { $hash = [BitConverter]::ToString($sha256.ComputeHash($zipStream)).Replace('-', '').ToLowerInvariant() }
    finally { $zipStream.Dispose(); $sha256.Dispose() }
    [IO.File]::WriteAllText("$zip.sha256", "$hash  $([IO.Path]::GetFileName($zip))`n", [Text.Encoding]::ASCII)
    Write-Output "Built $zip"
}
finally { $env:RUSTFLAGS = $previousRustflags; Pop-Location }
