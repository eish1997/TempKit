param([string]$DestinationRoot="$PSScriptRoot/../evidence/reliable-base/r4")
$ErrorActionPreference='Stop'
$sources=[ordered]@{roaming=(Join-Path $env:APPDATA 'com.quickgen.desktop');local=(Join-Path $env:LOCALAPPDATA 'com.quickgen.desktop')}
function Assert-ProductionClosed {
$processes=Get-CimInstance Win32_Process | Where-Object {$_.Name -match '^(QuickGen|quickgen-desktop|msedgewebview2)\.exe$'}
foreach($process in $processes){
 foreach($source in $sources.Values){if($process.CommandLine -and $process.CommandLine.IndexOf($source,[StringComparison]::OrdinalIgnoreCase)-ge 0){throw 'Production WebView data is still open; close that instance before backup.'}}
 if($process.Name -match '^(QuickGen|quickgen-desktop)\.exe$' -and $process.CommandLine -notmatch '--isolated-data'){throw 'A production QuickGen process is running; close it before backup.'}
}
}
Assert-ProductionClosed
New-Item -ItemType Directory -Path $DestinationRoot -Force | Out-Null
$base=(Resolve-Path -LiteralPath $DestinationRoot).Path
foreach($source in $sources.Values){if($base.StartsWith([IO.Path]::GetFullPath($source),[StringComparison]::OrdinalIgnoreCase)){throw 'Backup destination must be outside production data.'}}
$destination=Join-Path $base ('production-before-migration-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $destination | Out-Null
$manifest=@()
foreach($entry in $sources.GetEnumerator()){
 $source=$entry.Value;if(-not (Test-Path -LiteralPath $source)){continue}
 if((Get-Item -LiteralPath $source).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Redirected production root rejected.'}
 $children=@(Get-ChildItem -LiteralPath $source -Recurse -Force)
 if($children|Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}){throw 'Reparse point encountered; preserve source and inspect before backup.'}
 $files=@($children|Where-Object {-not $_.PSIsContainer})
 foreach($file in $files){
  $relative=$file.FullName.Substring($source.Length).TrimStart('\','/')
  $target=Join-Path (Join-Path $destination $entry.Key) $relative
  New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
  $before=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
  Copy-Item -LiteralPath $file.FullName -Destination $target
  $copy=(Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash
  $after=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
  if($before -ne $copy -or $before -ne $after){throw "Source changed or copy differs: $relative"}
  $manifest+=[pscustomobject][ordered]@{root=$entry.Key;relative=$relative;length=$file.Length;sha256=$before}
 }
 $afterFiles=@(Get-ChildItem -LiteralPath $source -Recurse -File -Force)
 if($afterFiles.Count -ne $files.Count){throw 'Production file inventory changed during backup.'}
 foreach($record in @($manifest|Where-Object root -eq $entry.Key)){
  if((Get-FileHash -LiteralPath (Join-Path $source $record.relative) -Algorithm SHA256).Hash -ne $record.sha256){throw 'Production contents changed during final verification.'}
 }
}
$report=[ordered]@{format='quickgen-pre-migration-backup-v1';createdUtc=[DateTime]::UtcNow.ToString('o');sources=$sources;files=$manifest;productionModified=$false}
$report|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $destination 'manifest.json') -Encoding UTF8
# Re-read both complete trees after all copies, not only one file at a time.
# The rehearsal requires this completion record; partial backups have none.
Assert-ProductionClosed
foreach($entry in $sources.GetEnumerator()){
 $expected=@($manifest|Where-Object root -eq $entry.Key)
 $actual=if(Test-Path -LiteralPath $entry.Value){@(Get-ChildItem -LiteralPath $entry.Value -Recurse -File -Force)}else{@()}
 if(@($actual).Count -ne $expected.Count){throw 'Production inventory changed before backup completion.'}
 foreach($record in $expected){
  $original=Join-Path $entry.Value $record.relative
  $copied=Join-Path (Join-Path $destination $entry.Key) $record.relative
  if((Get-FileHash -LiteralPath $original).Hash -ne $record.sha256 -or (Get-FileHash -LiteralPath $copied).Hash -ne $record.sha256){throw 'Final source/copy verification differs; preserve the incomplete backup.'}
 }
}
Assert-ProductionClosed
[ordered]@{fileCount=$manifest.Count;bytes=($manifest|Measure-Object length -Sum).Sum;allSourceAndCopyHashesMatch=$true;verifiedUtc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $destination 'verification.json') -Encoding UTF8
[pscustomobject]@{directory=$destination;fileCount=$manifest.Count;bytes=($manifest|Measure-Object length -Sum).Sum;verified=$true}|ConvertTo-Json
