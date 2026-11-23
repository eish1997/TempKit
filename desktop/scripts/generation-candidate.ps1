param([ValidateSet('build','test')][string]$Action='build')
$ErrorActionPreference='Stop'
$desktopRoot=Split-Path -Parent $PSScriptRoot
$target=Join-Path $desktopRoot 'src-tauri/target/generation'
$evidence=Join-Path $desktopRoot 'evidence/generation/g3'
New-Item -ItemType Directory -Path $evidence -Force | Out-Null
$previous=$env:CARGO_TARGET_DIR
try {
 $env:CARGO_TARGET_DIR=$target
 & "$PSScriptRoot/desktop.cmd" $Action *> (Join-Path $evidence ($Action+'-clipboard.log'))
 $result=$LASTEXITCODE
 [ordered]@{action=$Action;exitCode=$result;target=$target;finishedUtc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content (Join-Path $evidence ($Action+'-clipboard-result.json')) -Encoding utf8
 if($result -ne 0){throw "Generation $Action failed: $result"}
}finally{$env:CARGO_TARGET_DIR=$previous}
