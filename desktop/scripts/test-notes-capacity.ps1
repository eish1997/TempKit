param([string]$Exe="$PSScriptRoot/../src-tauri/target/diagnostics/release/quickgen-desktop.exe")
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
$evidence=Join-Path $PSScriptRoot '../evidence/notes/n4'
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
$root=Join-Path $evidence ('notes-capacity-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$root=(Resolve-Path -LiteralPath $root).Path
$prefix=Join-Path $evidence ([IO.Path]::GetFileName($root))
$priorMode=$env:QUICKGEN_DIAGNOSTIC
try{
 $env:QUICKGEN_DIAGNOSTIC='notes-capacity'
 $process=Start-Process -FilePath $candidate -ArgumentList ('--isolated-data "'+$root+'"') -WindowStyle Hidden -RedirectStandardOutput ($prefix+'.stdout.log') -RedirectStandardError ($prefix+'.stderr.log') -PassThru
}finally{$env:QUICKGEN_DIAGNOSTIC=$priorMode}
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;pid=$process.Id;startedUtc=[DateTime]::UtcNow.ToString('o');samples=($prefix+'.samples.jsonl');status='running';machine=@{os=[Environment]::OSVersion.VersionString;logicalProcessors=[Environment]::ProcessorCount;is64Bit=[Environment]::Is64BitOperatingSystem}}
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'notes-capacity-latest.json')
$clock=[Diagnostics.Stopwatch]::StartNew();$lastReport=-60;$previous=@{}
while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt 1920){
 $all=Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name
 $ids=[Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
 do{$changed=$false;foreach($row in $all){if($ids.Contains([int]$row.ParentProcessId)-and $ids.Add([int]$row.ProcessId)){$changed=$true}}}while($changed)
 $rows=@();foreach($processId in $ids){$p=Get-Process -Id $processId -ErrorAction SilentlyContinue;if($p){$rows+=[ordered]@{pid=$p.Id;name=$p.ProcessName;cpuSeconds=$p.TotalProcessorTime.TotalSeconds;privateBytes=$p.PrivateMemorySize64;workingSetBytes=$p.WorkingSet64}}}
 [ordered]@{elapsedSeconds=$clock.Elapsed.TotalSeconds;processes=$rows} | ConvertTo-Json -Depth 5 -Compress | Add-Content -LiteralPath ($prefix+'.samples.jsonl')
 if($clock.Elapsed.TotalSeconds-$lastReport -ge 60){Write-Output ('capacity live: pid='+$process.Id+' elapsed='+[int]$clock.Elapsed.TotalSeconds+'s root='+$root);$lastReport=$clock.Elapsed.TotalSeconds}
 Start-Sleep -Seconds 5;$process.Refresh()
}
if(-not $process.HasExited){$identity.status='timeout-still-running'}else{$process.WaitForExit();$identity.status='exited';$identity.exitCode=$process.ExitCode}
$path=Join-Path $root 'board-smoke-result.json'
if(Test-Path -LiteralPath $path){$identity.result=Get-Content -LiteralPath $path -Raw | ConvertFrom-Json}
$identity.elapsedSeconds=$clock.Elapsed.TotalSeconds
$identity | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'notes-capacity-latest.json')
if($identity.status -ne 'exited' -or $identity.exitCode -ne 0 -or -not $identity.result.ok){throw ($identity | ConvertTo-Json -Depth 8)}
$identity | ConvertTo-Json -Depth 8
