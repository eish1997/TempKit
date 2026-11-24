$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../evidence/notes-rich-migration'))
Add-Type @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class RichExit {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$jobs=@();try{
 foreach($identityFile in Get-ChildItem -LiteralPath $root -Filter 'notes-rich-capacity-*.identity.json'){
  $identity=Get-Content -LiteralPath $identityFile.FullName -Raw -Encoding UTF8|ConvertFrom-Json
  $p=Get-Process -Id $identity.pid -ErrorAction SilentlyContinue
  if(-not $p){continue};if($p.Path -ne $identity.exe){throw 'PID identity changed'}
  $handle=[RichExit]::OpenProcess(0x1000,$false,$p.Id);if($handle -eq [IntPtr]::Zero){throw 'Cannot retain query handle'}
  $jobs+=@{handle=$handle;pid=$p.Id;exe=$p.Path;root=$identity.root;done=$false}
 }
 $clock=[Diagnostics.Stopwatch]::StartNew()
 while(@($jobs|Where-Object {-not $_.done}).Count -and $clock.Elapsed.TotalSeconds -lt 2000){
  foreach($job in $jobs){if($job.done){continue};[uint32]$code=0;if(-not [RichExit]::GetExitCodeProcess($job.handle,[ref]$code)){throw 'Exit query failed'};if($code -ne 259){$job.done=$true;@{pid=$job.pid;exe=$job.exe;exitCode=$code;observedUtc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath ($job.root+'.exit-observed.json') -Encoding UTF8;Write-Output ('Observed diagnostic exit pid='+$job.pid+' code='+$code)}}
  Start-Sleep -Seconds 2
 }
}finally{foreach($job in $jobs){$null=[RichExit]::CloseHandle($job.handle)}}
