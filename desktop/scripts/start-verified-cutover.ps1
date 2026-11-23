param(
 [Parameter(Mandatory=$true)][string]$Backup,
 [Parameter(Mandatory=$true)][string]$ExpectedSHA256,
 [string]$Exe="$PSScriptRoot/../src-tauri/target/release/quickgen-desktop.exe"
)
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
if((Get-FileHash -LiteralPath $candidate).Hash -ne $ExpectedSHA256){throw 'Cutover executable differs from accepted candidate.'}
$base=(Resolve-Path -LiteralPath "$PSScriptRoot/../evidence/reliable-base/r4").Path
$capacity=Get-Content -LiteralPath (Join-Path $base 'capacity-latest.json') -Raw|ConvertFrom-Json
if($capacity.status -ne 'exited' -or $capacity.exitCode -ne 0 -or -not $capacity.result.ok -or $capacity.result.elapsedMs -lt 1800000 -or $capacity.result.switches -ne 20){throw 'Final capacity gate has not passed.'}
if($capacity.result.sample.images -ne 100 -or $capacity.result.jobs -ne 50 -or $capacity.result.items -ne 150 -or (Get-FileHash -LiteralPath $capacity.exe).Hash -ne $capacity.sha256){throw 'Capacity sample or candidate identity differs.'}
if(Test-Path -LiteralPath (Join-Path $base 'final/production-session.json')){throw 'A cutover session was already recorded; inspect and resume it instead of relaunching.'}
$backupRoot=(Resolve-Path -LiteralPath $Backup).Path
$manifest=Get-Content -LiteralPath (Join-Path $backupRoot 'manifest.json') -Raw|ConvertFrom-Json
$verified=Get-Content -LiteralPath (Join-Path $backupRoot 'verification.json') -Raw|ConvertFrom-Json
if($manifest.format -ne 'quickgen-pre-migration-backup-v1' -or -not $verified.allSourceAndCopyHashesMatch){throw 'Complete verified backup required.'}
$sources=[ordered]@{roaming=(Join-Path $env:APPDATA 'com.quickgen.desktop');local=(Join-Path $env:LOCALAPPDATA 'com.quickgen.desktop')}
function Assert-NoQuickGen {
 $processes=Get-CimInstance Win32_Process | Where-Object {$_.Name -match '^(QuickGen|quickgen-desktop|msedgewebview2)\.exe$'}
 foreach($process in $processes){
  if($process.Name -match '^(QuickGen|quickgen-desktop)\.exe$'){throw 'A QuickGen instance is still running; preserve it.'}
  foreach($source in $sources.Values){if($process.CommandLine -and $process.CommandLine.IndexOf($source,[StringComparison]::OrdinalIgnoreCase)-ge 0){throw 'Production WebView profile is still in use.'}}
 }
}
Assert-NoQuickGen
foreach($entry in $sources.GetEnumerator()){
 if([IO.Path]::GetFullPath($manifest.sources.($entry.Key)) -ine [IO.Path]::GetFullPath($entry.Value)){throw 'Backup belongs to another production root.'}
 $records=@($manifest.files|Where-Object root -eq $entry.Key)
 $files=if(Test-Path -LiteralPath $entry.Value){@(Get-ChildItem -LiteralPath $entry.Value -Recurse -File -Force)}else{@()}
 if(@($files).Count -ne $records.Count){throw 'Production inventory differs from backup; back up and rehearse the new snapshot.'}
 foreach($record in $records){
  $original=[IO.Path]::GetFullPath((Join-Path $entry.Value $record.relative))
  $copyBase=Join-Path $backupRoot $entry.Key
  $copy=[IO.Path]::GetFullPath((Join-Path $copyBase $record.relative))
  if(-not $original.StartsWith($entry.Value+'\',[StringComparison]::OrdinalIgnoreCase) -or -not $copy.StartsWith($copyBase+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid backup relative path.'}
  if((Get-FileHash -LiteralPath $original).Hash -ne $record.sha256 -or (Get-FileHash -LiteralPath $copy).Hash -ne $record.sha256){throw 'Production or backup changed; preserve both and rehearse the new snapshot.'}
 }
}
$sourceHashes=Get-Content -LiteralPath (Join-Path $base 'final/source-hashes.json') -Raw|ConvertFrom-Json
$projectRoot=(Resolve-Path -LiteralPath "$PSScriptRoot/../..").Path
foreach($entry in $sourceHashes.PSObject.Properties){if((Get-FileHash -LiteralPath (Join-Path $projectRoot $entry.Name)).Hash -ne $entry.Value){throw 'Product source changed after final verification.'}}
Assert-NoQuickGen
$prior=$env:QUICKGEN_DIAGNOSTIC
try{$env:QUICKGEN_DIAGNOSTIC='';$process=Start-Process -FilePath $candidate -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $base 'final/production.stdout.log') -RedirectStandardError (Join-Path $base 'final/production.stderr.log')}finally{$env:QUICKGEN_DIAGNOSTIC=$prior}
[ordered]@{exe=$candidate;sha256=$ExpectedSHA256;pid=$process.Id;startTime=$process.StartTime.ToString('o');backup=$backupRoot;verifiedFileCount=$manifest.files.Count;status='started-for-cutover';profile=$sources.roaming}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $base 'final/production-session.json')
Get-Content -LiteralPath (Join-Path $base 'final/production-session.json')
