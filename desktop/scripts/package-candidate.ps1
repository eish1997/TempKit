param([Parameter(Mandatory=$true)][string]$ExpectedSHA256,[Parameter(Mandatory=$true)][string]$SourceManifest,[string]$ExecutablePath,[ValidateSet('reliable-base','notes-integration','notes-rich','generation')][string]$Stage='reliable-base',[string]$OutputDirectory,[string]$BuildSourceManifest)
$ErrorActionPreference = 'Stop'
$desktopRoot = Split-Path -Parent $PSScriptRoot
$binary = if($ExecutablePath){(Resolve-Path -LiteralPath $ExecutablePath).Path}else{Join-Path $desktopRoot 'src-tauri\target\release\quickgen-desktop.exe'}
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'Build the release executable first.' }
$digest = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
if($digest -ne $ExpectedSHA256){throw 'Release executable differs from the accepted candidate.'}
$sourceHashes=Get-Content -LiteralPath $SourceManifest -Raw|ConvertFrom-Json
$projectRoot=Split-Path -Parent $desktopRoot
foreach($entry in $sourceHashes.PSObject.Properties){
 $path=[IO.Path]::GetFullPath((Join-Path $projectRoot $entry.Name))
 if(-not $path.StartsWith($projectRoot+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Source manifest escaped project.'}
 if((Get-FileHash -LiteralPath $path).Hash -ne $entry.Value){throw ('Source changed after acceptance: '+$entry.Name)}
}
$buildSourceHashes=$null
if($Stage -in @('generation','notes-rich')){
 if(-not $BuildSourceManifest){throw 'Packaging this stage requires the frozen executable build source manifest.'}
 $buildSourceHashes=Get-Content -LiteralPath $BuildSourceManifest -Raw|ConvertFrom-Json
 foreach($entry in $buildSourceHashes.PSObject.Properties){
  $relative=$entry.Name.Replace('\','/')
  # Packaging support is not compiled into the executable; all other frozen build inputs must match.
  if($relative -eq 'desktop/scripts/package-candidate.ps1'){continue}
  $path=[IO.Path]::GetFullPath((Join-Path $projectRoot $entry.Name))
  if(-not $path.StartsWith($projectRoot+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Build manifest escaped project.'}
  if((Get-FileHash -LiteralPath $path).Hash -ne $entry.Value){throw ('Executable build input changed: '+$entry.Name)}
 }
}
$name = 'QuickGen-Windows-candidate-' + $digest.Substring(0,12).ToLowerInvariant()
$dist = if($OutputDirectory){[IO.Path]::GetFullPath($OutputDirectory)}else{Join-Path $desktopRoot 'dist'}
$folder = Join-Path $dist $name
$zip = $folder + '.zip'
if ((Test-Path -LiteralPath $folder) -or (Test-Path -LiteralPath $zip)) { throw "Candidate already exists: $folder. Preserve it instead of overwriting." }
New-Item -ItemType Directory -Path $folder -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $folder 'QuickGen.exe')
[ordered]@{stage=$Stage;executableSHA256=$digest;buildFeatures='default (desktop-diagnostics disabled)';sourceHashes=$sourceHashes;executableBuildSourceHashes=$buildSourceHashes}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $folder '构建信息.json') -Encoding UTF8
$notes = @'
QuickGen Windows 可靠底座版

解压整个压缩包后双击 QuickGen.exe。运行前请先退出已有 QuickGen 实例。
需要 Windows x64 与 Microsoft Edge WebView2 Runtime。
本候选在 Windows 10 Education 19045.6456 上构建；未承诺其他系统配置已通过验收。

本版沿用原QuickGen入口、能力配置、封面与主题；图片、场景和模拟任务由Rust持久管理。
拖动入口或标题行空白处可贴顶部/左右边；四边与四角可调整尺寸，最窄单列。
上下拖动箭头图标调整卡片高度（32/56/80/112），也支持方向键和Home/End；布局自动保存。
点击添加图片或从资源管理器拖入图片，顶部堆叠打开画板。
支持大纲、框选、多图拖动；多图拖到能力卡后逐张模拟执行，结果回到原图旁。
Alt＋·（数字1左侧键）切换画板操作和桌面穿透；图钉控制面板常驻。
自定义添加和预设库仍在独立小面板中，X只关闭小面板。
QuickGen位于系统托盘，不占任务栏；左键打开面板，右键可退出。
堆叠及画板图片使用与面板一致的细描边。
聚焦主窗口后按Alt+F4退出；程序会先保存最后的画板操作。保存失败时会提示并保留窗口。
本版仅使用模拟执行器，不调用付费模型或真实Harness；输出为带“模拟结果”标记的合成图，不代表真实AI生成。
归档失败显示错误及“重试保存”；只保存已有输出，不重新执行任务。
“取消”只用于排队任务；“隐藏”只隐藏卡片，可在任务摘要中重新显示。
结果由宿主任务恢复，明确移除的结果不会因重载或重启再次插入。
多屏、混合DPI现场验收留待正式发布前完成。

此压缩包只包含程序及说明，不含用户图片、配置、日志或开发工具链。
程序沿用当前 Windows 用户的 QuickGen 配置与封面副本；不是隔离数据的便携版。
配置目录：%APPDATA%\com.quickgen.desktop
日志目录：%LOCALAPPDATA%\com.quickgen.desktop\logs
资源、场景与任务目录：%APPDATA%\com.quickgen.desktop\resource-store。
首次切换会备份并核对旧场景，再原子激活新库；旧WebView快照保留，但不再持续写入。
迁移失败会明确提示，不清空原数据。请保留数据目录并反馈具体错误。
托盘“保存恢复副本”会保存当前场景并导出完整资源/数据库/迁移备份及哈希清单。
回退旧exe只能看到切换时的旧快照，不能读取新版本产生的变化。不要删除新库强行回退；详见随包“备份与恢复.md”。

外部图片在面板展开完成后等待1秒才可直接投放能力，保护期内仅导入。
当前单屏交互基线已收口：用户确认桌面流畅、悬浮变淡、托盘打开面板及退出正常。
此包为日常构建，不启用临时诊断模式。容量证据仅覆盖100张受控图片、50项模拟任务和30分钟运行，不代表无限容量。
多屏、混合DPI与真实模型/Harness能力仍未交付。
此包不是安装器，不添加开机启动。无需从源码构建即可启动。
'@
if($Stage -in @('notes-integration','notes-rich')){$notes=Get-Content -LiteralPath (Join-Path $desktopRoot '../docs/quickgen/notes-usage.md') -Raw -Encoding UTF8}
if($Stage -eq 'notes-rich'){
 Copy-Item -LiteralPath (Join-Path $desktopRoot 'ui/vendor/licenses') -Destination (Join-Path $folder '第三方许可') -Recurse
 $launcher=@'
@echo off
setlocal
set "QUICKGEN_CANDIDATE_DATA=%~dp0isolated-data"
start "" "%~dp0QuickGen.exe" --isolated-data "%QUICKGEN_CANDIDATE_DATA%"
'@
 Set-Content -LiteralPath (Join-Path $folder '启动隔离候选.cmd') -Value $launcher -Encoding ASCII
}
if($Stage -eq 'generation'){
 $notes=Get-Content -LiteralPath (Join-Path $desktopRoot '../docs/quickgen/generation-usage.md') -Raw
 $notes=$notes.Replace('(generation-plan.md)','(验收状态.md)').Replace('(recovery.md)','(备份与恢复.md)')
 Copy-Item -LiteralPath (Join-Path $desktopRoot '../docs/quickgen/generation-acceptance.md') -Destination (Join-Path $folder '验收状态.md')
 Copy-Item -LiteralPath (Join-Path $desktopRoot '../docs/quickgen/generation-compatibility.json') -Destination (Join-Path $folder '兼容配置参考.json')
}
Set-Content -LiteralPath (Join-Path $folder '使用说明.txt') -Value $notes -Encoding UTF8
$recoverySource = if($Stage -eq 'generation'){Join-Path $desktopRoot '../docs/quickgen/generation-recovery.md'}elseif($Stage -in @('notes-integration','notes-rich')){Join-Path $desktopRoot '../docs/quickgen/notes-recovery.md'}else{Join-Path $desktopRoot '../docs/quickgen/recovery.md'}
Copy-Item -LiteralPath $recoverySource -Destination (Join-Path $folder '备份与恢复.md')
Set-Content -LiteralPath (Join-Path $folder 'SHA256SUMS.txt') -Value "$digest  QuickGen.exe" -Encoding ASCII
Compress-Archive -LiteralPath $folder -DestinationPath $zip
# Check the packaged bytes, not just the source executable.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
 $entry = @($archive.Entries | Where-Object { $_.Name -eq 'QuickGen.exe' })
 if($Stage -eq 'generation'){
  $expected=@('QuickGen.exe','构建信息.json','使用说明.txt','备份与恢复.md','SHA256SUMS.txt','验收状态.md','兼容配置参考.json')|Sort-Object
  $actual=@($archive.Entries|Where-Object {$_.Name}|ForEach-Object {$_.Name})|Sort-Object
  if(Compare-Object $expected $actual){throw 'Unexpected generation package contents.'}
 }
 if($Stage -eq 'notes-rich'){
  $expected=@('QuickGen.exe','构建信息.json','使用说明.txt','备份与恢复.md','SHA256SUMS.txt','启动隔离候选.cmd')
  $expected+=@(Get-ChildItem -LiteralPath (Join-Path $desktopRoot 'ui/vendor/licenses') -File | ForEach-Object {'第三方许可/'+$_.Name})
  $actual=@($archive.Entries|Where-Object {$_.Name}|ForEach-Object {($_.FullName.Replace('\','/') -split '/',2)[1]})
  if(Compare-Object ($expected|Sort-Object) ($actual|Sort-Object)){throw 'Unexpected notes-rich package contents.'}
 }
 if ($entry.Count -ne 1) { throw 'Archive must contain exactly one executable.' }
 $stream = $entry[0].Open()
 $sha = [System.Security.Cryptography.SHA256]::Create()
 try { $packagedDigest = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','') }
 finally { $stream.Dispose(); $sha.Dispose() }
 if ($packagedDigest -ne $digest) { throw 'Packaged executable hash mismatch.' }
} finally { $archive.Dispose() }
[pscustomobject]@{Archive=$zip;ExecutableSHA256=$digest;ArchiveSHA256=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash}
