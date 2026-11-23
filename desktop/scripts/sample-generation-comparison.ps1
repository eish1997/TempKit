param([Parameter(Mandatory=$true)][string]$Protocol,[Parameter(Mandatory=$true)][ValidateSet('old','current')][string]$Variant)
$ErrorActionPreference='Stop'
$protocolPath=(Resolve-Path -LiteralPath $Protocol).Path
$spec=Get-Content -LiteralPath $protocolPath -Raw | ConvertFrom-Json
if($spec.format -ne 'quickgen-retrospective-comparison-v1'){throw 'Invalid protocol'}
$run=$spec.variants.$Variant
$exe=(Resolve-Path -LiteralPath $run.exe).Path
if((Get-FileHash -LiteralPath $exe).Hash -ne $run.sha256){throw 'Candidate identity mismatch'}
$folder=Split-Path $protocolPath -Parent
$identity=Join-Path $folder ($Variant+'.identity.json')
if(Test-Path -LiteralPath $identity){throw 'Existing run is preserved; do not overwrite'}
$process=Start-Process -FilePath $exe -ArgumentList ('--isolated-data "'+$run.data+'"') -WindowStyle Hidden -RedirectStandardOutput (Join-Path $folder ($Variant+'.stdout.log')) -RedirectStandardError (Join-Path $folder ($Variant+'.stderr.log')) -PassThru
$record=[ordered]@{pid=$process.Id;startTime=$process.StartTime.ToString('o');exe=$exe;sha256=$run.sha256;data=$run.data;protocolSHA256=(Get-FileHash -LiteralPath $protocolPath).Hash;status='sampling';variant=$Variant}
$record|ConvertTo-Json|Set-Content -LiteralPath $identity
$clock=[Diagnostics.Stopwatch]::StartNew()
$samples=Join-Path $folder ($Variant+'.samples.jsonl')
while($clock.Elapsed.TotalSeconds -lt ($spec.warmupSeconds+$spec.sampleSeconds)){
 $process.Refresh();if($process.HasExited){throw 'Candidate exited during sampling'}
 $all=Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId
 $ids=[Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
 do{$changed=$false;foreach($row in $all){if($ids.Contains([int]$row.ParentProcessId)-and $ids.Add([int]$row.ProcessId)){$changed=$true}}}while($changed)
 $rows=@();foreach($processId in $ids){$p=Get-Process -Id $processId -ErrorAction SilentlyContinue;if($p){$rows+=@{pid=$p.Id;start=$p.StartTime.ToString('o');name=$p.ProcessName;cpu=$p.TotalProcessorTime.TotalSeconds;private=$p.PrivateMemorySize64;workingSet=$p.WorkingSet64}}}
 @{elapsed=$clock.Elapsed.TotalSeconds;processes=$rows}|ConvertTo-Json -Depth 5 -Compress|Add-Content -LiteralPath $samples
 Start-Sleep -Seconds $spec.intervalSeconds
}
$record.status='sampled-awaiting-normal-exit';$record|ConvertTo-Json|Set-Content -LiteralPath $identity
Write-Output ($Variant+' sampled; PID '+$process.Id+' remains open for normal UI exit')
