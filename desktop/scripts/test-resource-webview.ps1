param(
    [string]$Exe = "$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe"
)
$ErrorActionPreference = 'Stop'
$candidate = (Resolve-Path -LiteralPath $Exe).Path
$evidence = Join-Path $PSScriptRoot '../evidence/reliable-base/r1'
$root = Join-Path $evidence ('webview-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$root = (Resolve-Path -LiteralPath $root).Path
$arguments = '--resource-smoke --isolated-data "' + $root + '"'
$runs = @()
foreach ($phase in 1..2) {
$logPrefix = Join-Path $evidence ([IO.Path]::GetFileName($root) + '-phase' + $phase)
$process = Start-Process -FilePath $candidate -ArgumentList $arguments -WindowStyle Hidden -RedirectStandardOutput ($logPrefix + '.stdout.log') -RedirectStandardError ($logPrefix + '.stderr.log') -PassThru
if (-not $process.WaitForExit(45000)) {
    $process.Kill()
    $process.WaitForExit()
    throw "Resource WebView timed out; evidence retained at $root"
}
$resultPath = Join-Path $root 'resource-smoke-result.json'
if (-not (Test-Path -LiteralPath $resultPath)) { throw "No WebView result; exit=$($process.ExitCode), root=$root" }
$result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
Copy-Item -LiteralPath $resultPath -Destination ($logPrefix + '.result.json')
if ($phase -eq 2 -and -not $result.restart) { throw 'Second process did not report restart verification' }
if (-not $result.ok -or $process.ExitCode -ne 0) { throw ($result | ConvertTo-Json -Depth 5) }
$runs += [ordered]@{phase=$phase;exitCode=$process.ExitCode;result=$result}
}
$identity = [ordered]@{exe=$candidate; sha256=(Get-FileHash -LiteralPath $candidate).Hash; root=$root; exitCode=$process.ExitCode; result=$result; runs=$runs}
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'resource-webview-latest.json')
if (-not $result.ok) { throw ($result | ConvertTo-Json -Depth 5) }
$identity | ConvertTo-Json -Depth 8
