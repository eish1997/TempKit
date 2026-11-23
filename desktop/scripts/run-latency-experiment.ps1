param([Parameter(Mandatory=$true)][string]$IsolatedData,[ValidateSet('baseline','no-hover','fixed-pass','static-webview')][string]$Mode='baseline',[int]$Samples=6)
$ErrorActionPreference='Stop'
$desktopRoot=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path (Join-Path $desktopRoot 'src-tauri/target/diagnostics/release/quickgen-desktop.exe')).Path
$running=@(Get-Process quickgen-desktop -ErrorAction SilentlyContinue)
if($running.Count){throw 'QuickGen is already running; preserve it and finish that session first.'}
if(-not [IO.Path]::IsPathRooted($IsolatedData)){throw 'An explicit absolute isolated directory is required.'}
$folder=Join-Path $desktopRoot ('test-output/latency-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+$Mode)
New-Item -ItemType Directory -Path $folder | Out-Null
$oldMode=$env:QUICKGEN_DIAGNOSTIC
$oldStatus=$env:QUICKGEN_DIAGNOSTIC_STATUS
$oldArgs=$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
try {
 $env:QUICKGEN_DIAGNOSTIC=$Mode
 $env:QUICKGEN_DIAGNOSTIC_STATUS=Join-Path $folder 'view.json'
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$null
 $child=Start-Process -FilePath $exe -ArgumentList ('--isolated-data "'+$IsolatedData+'"') -WindowStyle Hidden -PassThru
} finally {
 $env:QUICKGEN_DIAGNOSTIC=$oldMode
 $env:QUICKGEN_DIAGNOSTIC_STATUS=$oldStatus
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$oldArgs
}
try {
 $ready=$false
 for($attempt=0;$attempt -lt 20;$attempt++){
  Start-Sleep -Milliseconds 500
  if($child.HasExited){throw 'QuickGen exited before the experiment.'}
  $statusPath=Join-Path $folder 'view.json'
  if(Test-Path $statusPath){try{$view=Get-Content $statusPath -Raw | ConvertFrom-Json;if($view.open -and $view.passthrough -and $view.visualCount -gt 0){$ready=$true;break}}catch{}}
 }
 if(-not $ready){throw 'No verified open passthrough canvas with images; experiment not sampled.'}
 & (Join-Path $PSScriptRoot 'measure-desktop-resources.ps1') -RootProcessId $child.Id -Samples $Samples | Export-Csv (Join-Path $folder 'resources.csv') -NoTypeInformation
 $finalView=Get-Content (Join-Path $folder 'view.json') -Raw | ConvertFrom-Json
 if(-not ($finalView.open -and $finalView.passthrough)){throw 'Canvas mode changed during sampling; comparison invalid.'}
 $summary=@(Import-Csv (Join-Path $folder 'resources.csv') | Group-Object Id | ForEach-Object {$a=$_.Group[0];$b=$_.Group[-1];[pscustomobject]@{Id=$_.Name;CPUSeconds=([double]$b.CPU-[double]$a.CPU);GDIStart=$a.GDI;GDIEnd=$b.GDI;USERStart=$a.USER;USEREnd=$b.USER}})
 [pscustomobject]@{Mode=$Mode;SHA256=(Get-FileHash $exe).Hash;View=$view;Samples=$Samples;Resources=$summary;Limitation='Resource samples do not measure desktop input latency.'} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $folder 'report.json') -Encoding utf8
 Write-Output $folder
} finally {
 $child.Refresh()
 if(-not $child.HasExited){Stop-Process -Id $child.Id}
}
