param([Parameter(Mandatory=$true)][string]$Backup,[string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe")
$ErrorActionPreference='Stop'
$backupRoot=(Resolve-Path -LiteralPath $Backup).Path
$manifest=Get-Content -LiteralPath (Join-Path $backupRoot 'manifest.json') -Raw | ConvertFrom-Json
$verification=Get-Content -LiteralPath (Join-Path $backupRoot 'verification.json') -Raw | ConvertFrom-Json
if($manifest.format -ne 'quickgen-pre-migration-backup-v1' -or -not $verification.allSourceAndCopyHashesMatch){throw 'Verified production backup required.'}
$candidate=(Resolve-Path -LiteralPath $Exe).Path
$evidence=Join-Path $PSScriptRoot '../evidence/reliable-base/r4'
$root=Join-Path $evidence ('rehearsal-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $root | Out-Null
$root=(Resolve-Path -LiteralPath $root).Path
# This marker belongs to this newly created test root; it is never put in production.
[IO.File]::WriteAllBytes((Join-Path $root 'quickgen-isolated-v1'),[Text.Encoding]::UTF8.GetBytes('QuickGen isolated test data v1'))
foreach($file in $manifest.files){
 $relative=if($file.root -eq 'roaming'){$file.relative}elseif($file.root -eq 'local' -and $file.relative.StartsWith('EBWebView\',[StringComparison]::OrdinalIgnoreCase)){Join-Path 'webview' $file.relative}else{continue}
 $source=Join-Path (Join-Path $backupRoot $file.root) $file.relative
 $target=[IO.Path]::GetFullPath((Join-Path $root $relative))
 if(-not $target.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Backup path escaped rehearsal root.'}
 if((Get-FileHash -LiteralPath $source).Hash -ne $file.sha256){throw 'Backup hash mismatch; original backup is preserved.'}
 New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null;Copy-Item -LiteralPath $source -Destination $target
 if((Get-FileHash -LiteralPath $target).Hash -ne $file.sha256){throw 'Rehearsal copy verification failed.'}
}
$prefix=Join-Path $evidence ([IO.Path]::GetFileName($root));$priorMode=$env:QUICKGEN_DIAGNOSTIC
try{$env:QUICKGEN_DIAGNOSTIC='reliable-rehearsal';$process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($prefix+'.stdout.log') -RedirectStandardError ($prefix+'.stderr.log') -PassThru}finally{$env:QUICKGEN_DIAGNOSTIC=$priorMode}
$clock=[Diagnostics.Stopwatch]::StartNew()
while(-not $process.WaitForExit(5000)-and $clock.Elapsed.TotalSeconds -lt 120){Write-Output ('rehearsal running pid='+$process.Id+' elapsed='+[int]$clock.Elapsed.TotalSeconds+'s')}
if(-not $process.HasExited){$process.Kill();$process.WaitForExit();throw "Rehearsal timed out; evidence $root"}
$path=Join-Path $root 'board-smoke-result.json';if(-not(Test-Path -LiteralPath $path)){throw "No rehearsal result; exit=$($process.ExitCode), root=$root"}
$result=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;sourceBackup=$backupRoot;exitCode=$process.ExitCode;result=$result}
$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $evidence 'rehearsal-latest.json')
if(-not $result.ok -or $process.ExitCode -ne 0){throw ($result|ConvertTo-Json -Depth 8)}
$identity|ConvertTo-Json -Depth 8
