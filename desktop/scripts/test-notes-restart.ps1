param([Parameter(Mandatory=$true)][string]$SourceIdentity,[string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe")
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
$prepared=python "$PSScriptRoot/verify-notes-restart.py" prepare $SourceIdentity
if($LASTEXITCODE -ne 0){throw 'Recovery copy verification failed'}
$plan=$prepared | ConvertFrom-Json
$root=$plan.root
$priorMode=$env:QUICKGEN_DIAGNOSTIC
try{
 $env:QUICKGEN_DIAGNOSTIC='notes-restart'
 $process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($root+'.stdout.log') -RedirectStandardError ($root+'.stderr.log') -PassThru
}finally{$env:QUICKGEN_DIAGNOSTIC=$priorMode}
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;pid=$process.Id;startedUtc=[DateTime]::UtcNow.ToString('o');status='running'}
$identityPath=Join-Path $root 'restart-identity.json'
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $identityPath
Write-Output ('restart root='+$root+' pid='+$process.Id)
$clock=[Diagnostics.Stopwatch]::StartNew()
while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt 120){Start-Sleep -Seconds 2;$process.Refresh()}
if(-not $process.HasExited){$identity.status='timeout-still-running'}else{$process.WaitForExit();$identity.status='exited';$identity.exitCode=$process.ExitCode}
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $identityPath
if($identity.status -ne 'exited' -or $identity.exitCode -ne 0){throw ($identity | ConvertTo-Json -Depth 8)}
python "$PSScriptRoot/verify-notes-restart.py" audit $root
if($LASTEXITCODE -ne 0){throw 'Native restart audit failed'}
