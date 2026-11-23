param([string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe")
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
$evidence=Join-Path $PSScriptRoot '../evidence/reliable-base/review'
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
$root=Join-Path $evidence ('review-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$root=(Resolve-Path -LiteralPath $root).Path
$runs=@()
foreach($phase in 1..3){
$logPrefix=Join-Path $evidence ([IO.Path]::GetFileName($root)+'-phase'+$phase)
$priorMode=$env:QUICKGEN_DIAGNOSTIC
try {
 $env:QUICKGEN_DIAGNOSTIC='reliable-review'
 $process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($logPrefix+'.stdout.log') -RedirectStandardError ($logPrefix+'.stderr.log') -PassThru
} finally {$env:QUICKGEN_DIAGNOSTIC=$priorMode}
if(-not $process.WaitForExit(60000)){$process.Kill();$process.WaitForExit();throw "Board verification timeout; evidence $root"}
$path=Join-Path $root 'board-smoke-result.json'
if(-not (Test-Path -LiteralPath $path)){throw "No board result; exit=$($process.ExitCode), evidence=$root"}
$result=Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
Copy-Item -LiteralPath $path -Destination ($logPrefix+'.result.json')
if(-not $result.ok -or $process.ExitCode -ne 0){throw ($result | ConvertTo-Json -Depth 8)}
if($phase -eq 3 -and -not $result.restart){throw 'Missing board restart verification'}
$runs+=[ordered]@{phase=$phase;exitCode=$process.ExitCode;result=$result}
}
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;exitCode=$process.ExitCode;result=$result;runs=$runs}
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'review-latest.json')
if(-not $result.ok -or $process.ExitCode -ne 0){throw ($result | ConvertTo-Json -Depth 8)}
$identity | ConvertTo-Json -Depth 8
