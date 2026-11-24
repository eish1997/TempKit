param([Parameter(Mandatory=$true)][string]$Identity)
$ErrorActionPreference='Stop'
$evidence=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../evidence/notes-rich-migration'))
$path=(Resolve-Path -LiteralPath $Identity).Path
if(-not $path.StartsWith($evidence+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Not a rich test identity'}
$row=Get-Content -LiteralPath $path -Raw -Encoding UTF8|ConvertFrom-Json
$process=Get-Process -Id $row.pid
if($process.Path -ne $row.exe -or -not $row.desktop.StartsWith('QuickGenTest-')){throw 'Test identity mismatch'}
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;using System.ComponentModel;
public static class StopRichTest {
 delegate bool Callback(IntPtr hwnd,IntPtr data);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenDesktopW(string name,uint flags,bool inherit,uint access);
 [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("user32.dll")] static extern bool EnumDesktopWindows(IntPtr desktop,Callback cb,IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd,StringBuilder text,int size);
 [DllImport("user32.dll",SetLastError=true)] static extern bool PostMessageW(IntPtr hwnd,uint msg,IntPtr wp,IntPtr lp);
 public static void Close(string name,uint pid){var desktop=OpenDesktopW(name,0,false,0x81);if(desktop==IntPtr.Zero)throw new Win32Exception();try{int count=0;Callback cb=(hwnd,data)=>{uint owner;GetWindowThreadProcessId(hwnd,out owner);var text=new StringBuilder(256);GetWindowTextW(hwnd,text,256);if(owner==pid&&text.ToString()=="QuickGen"){if(!PostMessageW(hwnd,0x10,IntPtr.Zero,IntPtr.Zero))throw new Win32Exception();count++;}return true;};EnumDesktopWindows(desktop,cb,IntPtr.Zero);if(count!=1)throw new Exception("Owned test main window not found");}finally{CloseDesktop(desktop);}}
}
'@
@{pid=$row.pid;exe=$row.exe;reason='Capacity failed: renderer private memory reached multiple GiB; preserve samples and stop via normal save barrier';utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath ($row.root+'.aborted-memory.json') -Encoding UTF8
[StopRichTest]::Close($row.desktop,$row.pid)
Write-Output ('Requested normal close of own diagnostic '+$row.pid)
