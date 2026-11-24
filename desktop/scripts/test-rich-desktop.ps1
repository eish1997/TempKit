param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [ValidateSet('notes-rich-smoke','notes-rich-capacity','notes-restart','notes-fault','notes-capacity','default-launch')][string]$Mode='notes-rich-smoke',
 [string]$DataRoot,
 [int]$TimeoutSeconds=240,
 [switch]$CrashAfterProgress,
 [ValidateRange(0,65535)][int]$DiagnosticPort=0,
 [switch]$CaptureWindows
)
$ErrorActionPreference='Stop'
$candidate=(Resolve-Path -LiteralPath $Exe).Path
if($DiagnosticPort -and ($DiagnosticPort -lt 1024 -or $Mode -eq 'default-launch')){throw 'Debug port is only for an isolated diagnostic candidate'}
if($CrashAfterProgress -and $Mode -ne 'notes-rich-capacity'){throw 'Crash injection requires rich capacity mode'}
$evidence=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../evidence/notes-rich-migration'))
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
if(-not $DataRoot){$DataRoot=Join-Path $evidence ($Mode+'-'+[guid]::NewGuid().ToString('N'))}
$root=[IO.Path]::GetFullPath($DataRoot)
if(-not $root.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Diagnostic root must stay inside notes-rich-migration evidence'}
New-Item -ItemType Directory -Path $root -Force | Out-Null
if(Test-Path -LiteralPath (Join-Path $root 'board-smoke-result.json')){throw 'Use a fresh diagnostic directory or preserve old result before reusing'}
# Creates an unselected Windows desktop. Never calls SwitchDesktop or sends input.
# APIs: https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-createdesktopw
# https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.ComponentModel;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class RichTestDesktop {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Startup {
  public int cb; public string reserved,desktop,title; public uint x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reservedSize; public IntPtr reservedBytes,input,output,error;
 }
 [StructLayout(LayoutKind.Sequential)] public struct Child {public IntPtr process,thread;public uint processId,threadId;}
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int left,top,right,bottom;}
 [DllImport("user32.dll",SetLastError=true)] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll",SetLastError=true)] public static extern bool PrintWindow(IntPtr hwnd,IntPtr dc,uint flags);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateDesktopW(string name,IntPtr device,IntPtr mode,uint flags,uint access,IntPtr security);
 [DllImport("user32.dll",SetLastError=true)] public static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessW(string app,StringBuilder command,IntPtr pa,IntPtr ta,bool inherit,uint flags,IntPtr env,string cwd,ref Startup startup,out Child child);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool TerminateProcess(IntPtr process,uint code);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 public static uint ExitCode(IntPtr process){uint code;if(!GetExitCodeProcess(process,out code))throw new Win32Exception();return code;}
 public class WindowInfo{public long handle;public string title;public bool visible;}
 delegate bool EnumWindow(IntPtr window,IntPtr data);
 [DllImport("user32.dll",SetLastError=true)] static extern bool EnumDesktopWindows(IntPtr desktop,EnumWindow callback,IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll",SetLastError=true)] static extern bool PostMessageW(IntPtr hwnd,uint message,IntPtr wp,IntPtr lp);
 public static WindowInfo[] Windows(IntPtr desktop,uint pid){var rows=new List<WindowInfo>();EnumWindow callback=(hwnd,data)=>{uint owner;GetWindowThreadProcessId(hwnd,out owner);if(owner==pid){var title=new StringBuilder(256);GetWindowTextW(hwnd,title,title.Capacity);rows.Add(new WindowInfo{handle=hwnd.ToInt64(),title=title.ToString(),visible=IsWindowVisible(hwnd)});}return true;};if(!EnumDesktopWindows(desktop,callback,IntPtr.Zero))throw new Win32Exception();return rows.ToArray();}
 public static void CloseMain(long hwnd,uint pid){uint owner;GetWindowThreadProcessId(new IntPtr(hwnd),out owner);if(owner!=pid)throw new Exception("Window owner changed");if(!PostMessageW(new IntPtr(hwnd),0x0010,IntPtr.Zero,IntPtr.Zero))throw new Win32Exception();}
 public static IntPtr Create(string name){var result=CreateDesktopW(name,IntPtr.Zero,IntPtr.Zero,0,0x1ff,IntPtr.Zero);if(result==IntPtr.Zero)throw new Win32Exception();return result;}
 public static Child Launch(string name,string exe,string root){var startup=new Startup{cb=Marshal.SizeOf(typeof(Startup)),desktop=name,flags=1,show=0};Child child;
  if(!CreateProcessW(exe,new StringBuilder("\""+exe+"\" --isolated-data \""+root+"\""),IntPtr.Zero,IntPtr.Zero,false,0x08000000,IntPtr.Zero,System.IO.Path.GetDirectoryName(exe),ref startup,out child))throw new Win32Exception();
  CloseHandle(child.thread);return child;
 }
}
'@
$desktopName='QuickGenTest-'+[guid]::NewGuid().ToString('N')
$desktop=[RichTestDesktop]::Create($desktopName)
$priorMode=$env:QUICKGEN_DIAGNOSTIC;$priorStatus=$env:QUICKGEN_DIAGNOSTIC_STATUS
$priorBrowserArguments=$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$identity=[ordered]@{exe=$candidate;sha256=(Get-FileHash -LiteralPath $candidate).Hash;root=$root;mode=$Mode;desktop=$desktopName;physicalInputTest=$false;startedUtc=[DateTime]::UtcNow.ToString('o');status='starting'}
$identityPath=$root+'.identity.json';$samples=$root+'.samples.jsonl';$child=$null
try{
 $env:QUICKGEN_DIAGNOSTIC=$Mode;$env:QUICKGEN_DIAGNOSTIC_STATUS=Join-Path $root 'native-status.json'
 if($DiagnosticPort){$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port='+$DiagnosticPort;$identity.diagnosticPort=$DiagnosticPort}
 $child=[RichTestDesktop]::Launch($desktopName,$candidate,$root)
 $env:QUICKGEN_DIAGNOSTIC=$priorMode;$env:QUICKGEN_DIAGNOSTIC_STATUS=$priorStatus
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$priorBrowserArguments
 $process=Get-Process -Id $child.processId
 $identity.pid=$process.Id;$identity.status='running';$identity.samples=$samples
 $identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath
 Write-Output ('Started isolated desktop pid='+$process.Id+' root='+$root)
 $clock=[Diagnostics.Stopwatch]::StartNew();$lastReport=-60
 $closedDefault=$false
 while(-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt $TimeoutSeconds){
  $all=Get-CimInstance Win32_Process|Select-Object ProcessId,ParentProcessId,Name
  $ids=[Collections.Generic.HashSet[int]]::new();$null=$ids.Add($process.Id)
  do{$changed=$false;foreach($row in $all){if($ids.Contains([int]$row.ParentProcessId)-and $ids.Add([int]$row.ProcessId)){$changed=$true}}}while($changed)
  $rows=@();foreach($processId in $ids){$p=Get-Process -Id $processId -ErrorAction SilentlyContinue;if($p){$rows+=[ordered]@{pid=$p.Id;name=$p.ProcessName;cpuSeconds=$p.TotalProcessorTime.TotalSeconds;privateBytes=$p.PrivateMemorySize64;workingSetBytes=$p.WorkingSet64}}}
  [ordered]@{elapsedSeconds=$clock.Elapsed.TotalSeconds;processes=$rows}|ConvertTo-Json -Depth 5 -Compress|Add-Content -LiteralPath $samples
  if($clock.Elapsed.TotalSeconds-$lastReport -ge 60){Write-Output ('Isolated test elapsed='+[int]$clock.Elapsed.TotalSeconds+'s');$lastReport=$clock.Elapsed.TotalSeconds}
  if($Mode -eq 'default-launch' -and -not $closedDefault -and $clock.Elapsed.TotalSeconds -ge 15){
   $windows=@([RichTestDesktop]::Windows($desktop,$child.processId));$main=@($windows|Where-Object {$_.title -eq 'QuickGen' -and $_.visible})
   if($main.Count -ne 1){throw 'Default build did not expose one ready main window on its isolated desktop'}
   if($CaptureWindows){
    Add-Type -AssemblyName System.Drawing
    $captures=@();$index=0
    foreach($owned in @($windows|Where-Object {$_.visible -and ($_.title -eq '便签' -or $_.title -eq 'QuickGen')})){
     $rect=[RichTestDesktop+Rect]::new();$handle=[IntPtr]::new($owned.handle)
     if(-not [RichTestDesktop]::GetWindowRect($handle,[ref]$rect)){throw 'Cannot read owned window bounds'}
     $bitmap=[Drawing.Bitmap]::new(($rect.right-$rect.left),($rect.bottom-$rect.top));$graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
     try{$copied=[RichTestDesktop]::PrintWindow($handle,$dc,0)}finally{$graphics.ReleaseHdc($dc);$graphics.Dispose()}
     $capture=Join-Path $root ('native-window-'+$index+'.png');$bitmap.Save($capture,[Drawing.Imaging.ImageFormat]::Png);$bitmap.Dispose()
     $captures+=@{title=$owned.title;handle=$owned.handle;path=$capture;apiReturnedSuccess=$copied;visualReview='required; successful API return does not prove WebView content rendered'};$index++
    };$identity.nativeCaptures=$captures
   }
   $identity.nativeWindows=$windows;$identity.closeRequest='WM_CLOSE to owned main; normal save barrier';[RichTestDesktop]::CloseMain($main[0].handle,$child.processId);$closedDefault=$true
  }
  if($CrashAfterProgress){$progressPath=Join-Path $root 'capacity-progress.json';if(Test-Path -LiteralPath $progressPath){$progress=$null;try{$progress=Get-Content -LiteralPath $progressPath -Raw -Encoding UTF8|ConvertFrom-Json}catch{};if($progress.edits -ge 1){
   python (Join-Path $PSScriptRoot 'verify-rich-recovery.py') snapshot $root
   if($LASTEXITCODE -ne 0){throw 'Cannot preserve confirmed pre-crash snapshot'}
   $identity.confirmedEdits=$progress.edits;$identity.status='expected-crash';$identity.elapsedSeconds=$clock.Elapsed.TotalSeconds
   if(-not [RichTestDesktop]::TerminateProcess($child.process,124)){throw 'Cannot inject crash into own child'}
   $process.WaitForExit();$identity.exitCode=[RichTestDesktop]::ExitCode($child.process);$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath ($root+'.crash-identity.json') -Encoding UTF8
   if($identity.exitCode -ne 124){throw 'Unexpected crash exit code'};Write-Output ('Injected crash after confirmed edits: '+$root);return
  }}}
  Start-Sleep -Seconds 2;$process.Refresh()
 }
 $identity.elapsedSeconds=$clock.Elapsed.TotalSeconds
 if(-not $process.HasExited){$identity.status='timeout';$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath;throw 'Isolated diagnostic timed out; failure evidence retained'}
 $process.WaitForExit();$identity.status='exited';$identity.exitCode=[RichTestDesktop]::ExitCode($child.process)
 if($Mode -eq 'default-launch'){$identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath -Encoding UTF8;if($identity.exitCode -ne 0 -or -not $closedDefault){throw 'Default launch / normal close failed'};Write-Output ('PASS default launch '+$identityPath);return}
 $identity.resultPath=Join-Path $root 'board-smoke-result.json'
 $identity|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $identityPath -Encoding UTF8
 $result=$null;if(Test-Path -LiteralPath $identity.resultPath){$result=Get-Content -LiteralPath $identity.resultPath -Raw -Encoding UTF8|ConvertFrom-Json}
 if($identity.exitCode -ne 0 -or -not $result.ok){throw ('Diagnostic failed: '+$identityPath)}
 Write-Output ('PASS '+$identityPath)
}finally{
 $env:QUICKGEN_DIAGNOSTIC=$priorMode;$env:QUICKGEN_DIAGNOSTIC_STATUS=$priorStatus
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$priorBrowserArguments
 if($child){if($process -and -not $process.HasExited){$null=[RichTestDesktop]::TerminateProcess($child.process,124)};$null=[RichTestDesktop]::CloseHandle($child.process)}
 $null=[RichTestDesktop]::CloseDesktop($desktop)
}
