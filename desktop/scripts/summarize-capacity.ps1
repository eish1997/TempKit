param([string]$IdentityFile="$PSScriptRoot/../evidence/reliable-base/r4/capacity-latest.json")
$ErrorActionPreference='Stop'
$identity=Get-Content -LiteralPath $IdentityFile -Raw|ConvertFrom-Json
$samples=@(Get-Content -LiteralPath $identity.samples|ForEach-Object {$_|ConvertFrom-Json})
$intervals=@()
for($i=1;$i -lt $samples.Count;$i++){
 $prior=$samples[$i-1];$current=$samples[$i];$seconds=$current.elapsedSeconds-$prior.elapsedSeconds
 if($seconds -le 0){throw 'Non-increasing sample time'}
 $byPid=@{};foreach($entry in $prior.processes){$byPid[[int]$entry.pid]=[double]$entry.cpuSeconds}
 [double]$cpu=0;[long]$privateBytes=0
 foreach($entry in $current.processes){$privateBytes+=[long]$entry.privateBytes;if($byPid.ContainsKey([int]$entry.pid)){$cpu+=[Math]::Max([double]0,[double]($entry.cpuSeconds-$byPid[[int]$entry.pid]))}}
 $intervals+=[pscustomobject]@{endSeconds=[double]$current.elapsedSeconds;seconds=$seconds;cpuSeconds=$cpu;cpuOneCorePercent=100*$cpu/$seconds;privateMiB=$privateBytes/1MB;bucket=[int][Math]::Floor($current.elapsedSeconds/300)}
}
$windows=@($intervals|Group-Object bucket|ForEach-Object {
 $group=$_.Group;$memory=$group|Measure-Object privateMiB -Average -Minimum -Maximum
 [pscustomobject]@{fromSeconds=([int]$_.Name*300);toSeconds=([int]$_.Name*300+300);samples=$group.Count;cpuOneCorePercent=100*($group|Measure-Object cpuSeconds -Sum).Sum/($group|Measure-Object seconds -Sum).Sum;privateMiBAverage=$memory.Average;privateMiBMin=$memory.Minimum;privateMiBMax=$memory.Maximum}
})
$summary=[ordered]@{candidateSHA256=$identity.sha256;status=$identity.status;sampleCount=$samples.Count;lastElapsedSeconds=$samples[-1].elapsedSeconds;cpuUnits='100 percent = one fully used logical CPU; sums same-PID deltas across host and observed child processes';memoryUnits='sum of private bytes across observed live processes, MiB';fiveMinuteWindows=$windows;result=$identity.result;limitations=@('Short-lived decoder processes between samples may not be represented.','Other desktop workloads are uncontrolled; compare with a same-machine matched baseline.','No pass/fail performance threshold is inferred from these statistics.')}
$path=Join-Path $identity.root 'capacity-summary.json';$summary|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $path
$summary|ConvertTo-Json -Depth 8
