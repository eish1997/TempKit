param([string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe",[ValidateRange(1,5)][int]$Repeats=3,[string]$DataParent='',[string]$EvidenceDirectory='')
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
if(-not $EvidenceDirectory){$EvidenceDirectory=Join-Path $PSScriptRoot '../evidence/generation/g2'}
$evidence=(Resolve-Path -LiteralPath $EvidenceDirectory).Path
if(Test-Path -LiteralPath (Join-Path $evidence 'generation-performance-latest.json')){throw 'Preserve prior samples; use a fresh EvidenceDirectory'}
if(-not $DataParent){$DataParent=$evidence}
$DataParent=(Resolve-Path -LiteralPath $DataParent).Path
$runs=@()
foreach($repeat in 1..$Repeats){foreach($mode in @('small','large')){
 $root=Join-Path $DataParent ('generation-performance-'+$mode+'-'+[guid]::NewGuid().ToString('N'))
 New-Item -ItemType Directory -Path $root | Out-Null
 $priorMode=$env:QUICKGEN_DIAGNOSTIC;$priorPerformance=$env:QUICKGEN_PERFORMANCE
 try{
  $env:QUICKGEN_DIAGNOSTIC='generation-recovery';$env:QUICKGEN_PERFORMANCE=$mode
  $process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($root+'.stdout.log') -RedirectStandardError ($root+'.stderr.log') -PassThru
 }finally{$env:QUICKGEN_DIAGNOSTIC=$priorMode;$env:QUICKGEN_PERFORMANCE=$priorPerformance}
 $clock=[Diagnostics.Stopwatch]::StartNew();$memory=@()
 @{root=$root;pid=$process.Id;mode=$mode;repeat=$repeat;status='running';sha256=(Get-FileHash -LiteralPath $candidate).Hash} | ConvertTo-Json | Set-Content (Join-Path $evidence 'generation-performance-current.json')
 while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt 90){
  $sampleStart=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  $all=Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId
  $ids=[Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
  do{$changed=$false;foreach($row in $all){if($ids.Contains([int]$row.ParentProcessId)-and $ids.Add([int]$row.ProcessId)){$changed=$true}}}while($changed)
  $rows=@();foreach($processId in $ids){$child=Get-Process -Id $processId -ErrorAction SilentlyContinue;if($child){$rows+=@{pid=$child.Id;name=$child.ProcessName;privateBytes=$child.PrivateMemorySize64;workingSetBytes=$child.WorkingSet64;cpuSeconds=$child.TotalProcessorTime.TotalSeconds}}}
  $sample=@{elapsedSeconds=$clock.Elapsed.TotalSeconds;startEpochMs=$sampleStart;endEpochMs=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();processes=$rows};$memory+=$sample
  Start-Sleep -Milliseconds 500;$process.Refresh()
 }
 $memory | ForEach-Object {$_ | ConvertTo-Json -Depth 4 -Compress} | Set-Content ($root+'.memory.jsonl')
 if(-not $process.HasExited){throw "Performance timeout; isolated process $($process.Id) still running; root $root"}
 $process.WaitForExit()
 @{root=$root;pid=$process.Id;mode=$mode;repeat=$repeat;status='exited';exitCode=$process.ExitCode} | ConvertTo-Json | Set-Content (Join-Path $evidence 'generation-performance-current.json')
 $result=Get-Content (Join-Path $root 'board-smoke-result.json') -Raw | ConvertFrom-Json
 $run=@{root=$root;mode=$mode;repeat=$repeat;exitCode=$process.ExitCode;result=$result;memory=$memory}
 $run | ConvertTo-Json -Depth 10 | Set-Content ($root+'.result.json')
 if($process.ExitCode -ne 0 -or -not $result.ok){throw "Performance fixture failed; evidence $root"}
 $runs+=$run
 Write-Output "Finished $mode repeat $repeat; evidence $root"
}}
@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;runs=$runs;limitations=@('Sampled process-tree memory may miss short-lived decoders and peaks between samples.','CIM sampling adds observer overhead.','Current-source reference only; not the missing pre-G0 baseline.')} | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $evidence 'generation-performance-latest.json')
@{status='complete';runs=$runs.Count} | ConvertTo-Json | Set-Content (Join-Path $evidence 'generation-performance-current.json')
