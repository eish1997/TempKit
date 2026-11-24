param([ValidateSet('build','test','start')][string]$Action='test',[string]$IsolatedData,[string]$TargetDirectory)
$ErrorActionPreference='Stop'
$desktopRoot=Split-Path -Parent $PSScriptRoot
$target=if($TargetDirectory){if(-not [IO.Path]::IsPathRooted($TargetDirectory)){throw 'Absolute target directory required.'};$TargetDirectory}else{Join-Path $desktopRoot 'src-tauri/target/notes'}
if($Action -eq 'start'){
 if(-not $IsolatedData -or -not [IO.Path]::IsPathRooted($IsolatedData)){throw 'Explicit absolute isolated directory required.'}
 $exe=Join-Path $target 'release/quickgen-desktop.exe'
 if(-not(Test-Path -LiteralPath $exe)){throw 'Build the notes candidate first.'}
 Start-Process -FilePath $exe -ArgumentList ('--isolated-data "'+$IsolatedData+'"') -WindowStyle Hidden -PassThru
}else{
 $previous=$env:CARGO_TARGET_DIR
 try{
  $env:CARGO_TARGET_DIR=$target
  if($Action -eq 'test'){& "$PSScriptRoot/desktop.cmd" test -- --offline}else{& "$PSScriptRoot/desktop.cmd" build}
  if($LASTEXITCODE -ne 0){throw "Notes $Action failed: $LASTEXITCODE"}
 }finally{$env:CARGO_TARGET_DIR=$previous}
}
