param([int]$RootProcessId,[int]$Samples=6,[int]$IntervalSeconds=2)
# Read-only diagnostics. No window messages, input injection or system settings.
if (-not ("QuickGenGuiCounters" -as [type])) { Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class QuickGenGuiCounters {
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
}
"@
}
$processRows=Get-CimInstance Win32_Process
$ids=[System.Collections.Generic.HashSet[int]]::new()
[void]$ids.Add($RootProcessId)
do { $count=$ids.Count; foreach($row in $processRows){if($ids.Contains([int]$row.ParentProcessId)){[void]$ids.Add([int]$row.ProcessId)}} } while($count -ne $ids.Count)
for($i=0;$i -lt $Samples;$i++){
 foreach($idValue in $ids){
  $proc=Get-Process -Id $idValue -ErrorAction SilentlyContinue
  if($proc){[pscustomobject]@{Time=(Get-Date).ToString('o');Id=$proc.Id;Name=$proc.ProcessName;CPU=$proc.CPU;GDI=[QuickGenGuiCounters]::GetGuiResources($proc.Handle,0);USER=[QuickGenGuiCounters]::GetGuiResources($proc.Handle,1);Handles=$proc.HandleCount;Memory=$proc.WorkingSet64}}
 }
 if($i+1 -lt $Samples){Start-Sleep -Seconds $IntervalSeconds}
}
