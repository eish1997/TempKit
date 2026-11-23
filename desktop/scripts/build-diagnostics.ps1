$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$previousTarget=$env:CARGO_TARGET_DIR
try {
 $env:CARGO_TARGET_DIR=Join-Path $root 'src-tauri/target/diagnostics'
 & (Join-Path $PSScriptRoot 'desktop.cmd') build -- --features desktop-diagnostics
 if($LASTEXITCODE -ne 0){throw 'Diagnostic build failed'}
} finally {$env:CARGO_TARGET_DIR=$previousTarget}
