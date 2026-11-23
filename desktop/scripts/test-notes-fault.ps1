param([string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe")
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
$evidence=Join-Path $PSScriptRoot '../evidence/notes/n1'
$root=Join-Path $evidence ('notes-fault-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$root=(Resolve-Path -LiteralPath $root).Path
$priorMode=$env:QUICKGEN_DIAGNOSTIC
try{
 $env:QUICKGEN_DIAGNOSTIC='notes-fault'
 $process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($root+'.stdout.log') -RedirectStandardError ($root+'.stderr.log') -PassThru
}finally{$env:QUICKGEN_DIAGNOSTIC=$priorMode}
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;pid=$process.Id;startedUtc=[DateTime]::UtcNow.ToString('o');status='running'}
$identityPath=Join-Path $evidence 'notes-fault-latest.json'
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $identityPath
$clock=[Diagnostics.Stopwatch]::StartNew()
while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt 120){Start-Sleep -Seconds 2;$process.Refresh()}
if(-not $process.HasExited){$identity.status='timeout-still-running'}else{$process.WaitForExit();$identity.status='exited';$identity.exitCode=$process.ExitCode}
$resultPath=Join-Path $root 'board-smoke-result.json'
if(Test-Path -LiteralPath $resultPath){$identity.result=Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json}
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $identityPath
if($identity.status -ne 'exited' -or $identity.exitCode -ne 0 -or -not $identity.result.ok){throw ($identity | ConvertTo-Json -Depth 8)}
$identity | ConvertTo-Json -Depth 8
