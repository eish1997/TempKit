param([Parameter(Mandatory=$true)][string]$Comparison,[Parameter(Mandatory=$true)][ValidateSet('legacy','durable')][string]$Variant)
$ErrorActionPreference='Stop'
$comparisonRoot=(Resolve-Path -LiteralPath $Comparison).Path
$manifest=Get-Content -LiteralPath (Join-Path $comparisonRoot 'manifest.json') -Raw | ConvertFrom-Json
if($manifest.format -ne 'quickgen-performance-reference-v1'){throw 'Comparison manifest required'}
$exe=(Resolve-Path -LiteralPath (Join-Path $comparisonRoot ($Variant+'/quickgen-desktop.exe'))).Path
$root=Join-Path $comparisonRoot ($Variant+'-run-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$identityPath=$root+'.identity.json';$samplesPath=$root+'.samples.jsonl'
$prior=$env:QUICKGEN_DIAGNOSTIC
try{$env:QUICKGEN_DIAGNOSTIC='reliable-capacity';$process=Start-Process -FilePath $exe -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($root+'.stdout.log') -RedirectStandardError ($root+'.stderr.log') -PassThru}finally{$env:QUICKGEN_DIAGNOSTIC=$prior}
$identity=[ordered]@{variant=$Variant;exe=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;root=$root;pid=$process.Id;startTime=$process.StartTime.ToString('o');samples=$samplesPath;status='running';limitation=$manifest.limitation}
$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath
$clock=[Diagnostics.Stopwatch]::StartNew();$lastReport=-30
while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt 420){
 $all=Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId
 $ids=[Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
 do{$changed=$false;foreach($row in $all){if($ids.Contains([int]$row.ParentProcessId)-and $ids.Add([int]$row.ProcessId)){$changed=$true}}}while($changed)
 $rows=@();foreach($processId in $ids){$p=Get-Process -Id $processId -ErrorAction SilentlyContinue;if($p){$rows+=[ordered]@{pid=$p.Id;startTime=$p.StartTime.ToString('o');name=$p.ProcessName;cpuSeconds=$p.TotalProcessorTime.TotalSeconds;privateBytes=$p.PrivateMemorySize64;workingSetBytes=$p.WorkingSet64}}}
 $phase='startup-or-processing';$progress=Join-Path $root 'capacity-progress.json'
 if(Test-Path -LiteralPath $progress){try{$phase=(Get-Content -LiteralPath $progress -Raw|ConvertFrom-Json).phase}catch{}}
 [ordered]@{elapsedSeconds=$clock.Elapsed.TotalSeconds;phase=$phase;processes=$rows}|ConvertTo-Json -Depth 5 -Compress|Add-Content -LiteralPath $samplesPath
 if($clock.Elapsed.TotalSeconds-$lastReport -ge 30){Write-Output ($Variant+' pid='+$process.Id+' elapsed='+[int]$clock.Elapsed.TotalSeconds+'s phase='+$phase);$lastReport=$clock.Elapsed.TotalSeconds}
 Start-Sleep -Seconds 2;$process.Refresh()
}
if(-not $process.HasExited){$process.Kill();$process.WaitForExit();$identity.status='timeout'}else{$process.WaitForExit();$identity.status='exited';$identity.exitCode=$process.ExitCode}
$resultPath=Join-Path $root 'board-smoke-result.json'
if(Test-Path -LiteralPath $resultPath){$identity.result=Get-Content -LiteralPath $resultPath -Raw|ConvertFrom-Json}
$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath
if($identity.status -ne 'exited' -or $identity.exitCode -ne 0 -or -not $identity.result.ok){throw ('Comparison failed; preserve '+$identityPath)}
$identity|ConvertTo-Json -Depth 8
