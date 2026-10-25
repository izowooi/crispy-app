param([string]$Fixture = 'local\fixtures\sample.cbz')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Push-Location (Split-Path $PSScriptRoot -Parent)
$previousTesting = $env:SUA_VIEW_TESTING
$previousOutput = $env:SUA_VIEW_SMOKE_OUTPUT
try {
    $fixturePath = (Resolve-Path -LiteralPath $Fixture).Path
    $env:SUA_VIEW_TESTING = '1'
    $env:SUA_VIEW_SMOKE_OUTPUT = Join-Path (Get-Location) ('local\windows-smoke-' + [Guid]::NewGuid().ToString('N'))
    $unicodeFolder = [string]::Concat([char]0xD55C, [char]0xAE00, ' ', [char]0xBC30, [char]0xD3EC)
    $unpacked = Join-Path $env:SUA_VIEW_SMOKE_OUTPUT $unicodeFolder
    Expand-Archive -LiteralPath 'dist\Sua-View-0.1.0-windows-x64.zip' -DestinationPath $unpacked
    $process = Start-Process -FilePath (Join-Path $unpacked 'SuaView.exe') -ArgumentList @('--smoke-test', ('"' + $fixturePath + '"')) -WindowStyle Hidden -PassThru
    if (!$process.WaitForExit(30000)) { Stop-Process -Id $process.Id; throw 'Windows smoke test timed out.' }
    $process.Refresh()
    if ($process.ExitCode -ne 0) { throw "Windows smoke failed: $env:SUA_VIEW_SMOKE_OUTPUT" }
    Get-Content -LiteralPath (Join-Path $env:SUA_VIEW_SMOKE_OUTPUT 'result.txt')
    Write-Output "Evidence: $env:SUA_VIEW_SMOKE_OUTPUT"
}
finally { $env:SUA_VIEW_TESTING = $previousTesting; $env:SUA_VIEW_SMOKE_OUTPUT = $previousOutput; Pop-Location }
