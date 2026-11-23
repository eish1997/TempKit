//! Explicit backup of unused drag files, including incomplete/unknown copies.
use crate::resources::{ResourceService,guard_directory,open_blob};
use std::{fs,io::{Read,Write},os::windows::{fs::{OpenOptionsExt,MetadataExt},io::AsRawHandle}};
use sha2::{Digest,Sha256};
use serde_json::json;
const BATCH_BYTES:u64=32*1024*1024;
fn digest(path:&std::path::Path)->Result<(u64,String),String>{
 let mut input=open_blob(path)?;let mut hash=Sha256::new();let mut size=0;let mut buf=[0u8;65536];
 loop{let n=input.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}size+=n as u64;if size>BATCH_BYTES{return Err("拖出恢复文件超过上限".into())}hash.update(&buf[..n]);}Ok((size,format!("{:x}",hash.finalize())))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn backup_verifies_manifest_before_removal_and_preserves_busy_or_oversized_files(){
  let root=std::env::temp_dir().join(format!("quickgen-drag-backup-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let original=png.into_inner();
  let resource=service.import_bytes(original.clone(),"png","fixture".into(),crate::contracts::Provenance{task_id:None,parent_ids:vec![]}).unwrap();let lease=service.prepare_drag_copy(&resource.resource.id).unwrap();
  let dir=root.join("drag-exports");fs::write(dir.join("pending-fixture"),b"unfinished image").unwrap();fs::write(dir.join("manifest.json"),b"unknown user bytes").unwrap();fs::File::create(dir.join("oversized-unknown")).unwrap().set_len(BATCH_BYTES+1).unwrap();
  fs::write(root.join("test-drag-manifest-failure"),b"isolated").unwrap();assert!(service.archive_drag_files().unwrap_err().contains("originals retained"));assert_eq!(fs::read(dir.join("pending-fixture")).unwrap(),b"unfinished image");assert!(lease.path.exists());
  fs::remove_file(root.join("test-drag-manifest-failure")).unwrap();let result=service.archive_drag_files().unwrap();assert_eq!(result["archived"],2);assert_eq!(result["retained"],2);
  let backup=std::path::Path::new(result["directory"].as_str().unwrap());let manifest:serde_json::Value=serde_json::from_slice(&fs::read(backup.join("manifest.json")).unwrap()).unwrap();
  for entry in manifest["files"].as_array().unwrap(){assert_eq!(digest(&backup.join(entry["path"].as_str().unwrap())).unwrap(),(entry["bytes"].as_u64().unwrap(),entry["sha256"].as_str().unwrap().into()));}
  assert_eq!(fs::read(backup.join("files/manifest.json")).unwrap(),b"unknown user bytes");assert!(!dir.join("pending-fixture").exists());assert!(lease.path.exists());assert!(dir.join("oversized-unknown").exists());
  drop(lease);assert_eq!(service.archive_drag_files().unwrap()["archived"],1);assert_eq!(service.archive_drag_files().unwrap()["archived"],0);assert_eq!(service.read(&resource.resource.id).unwrap().1,original);
 }
}
impl ResourceService{
 pub fn archive_drag_files(&self)->Result<serde_json::Value,String>{
  let directory=self.root.join("drag-exports");
  if !directory.exists(){return Ok(json!({"directory":null,"archived":0,"retained":0,"status":self.drag_storage_status()?}))}
  let _source_guard=guard_directory(&directory)?;
  let entries=fs::read_dir(&directory).map_err(|e|e.to_string())?.take(4097).collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
  if entries.len()>4096{return Err("拖出文件过多；文件全部保留".into())}
  let total=entries.len();let mut selected=Vec::new();let mut bytes=0u64;
  for entry in entries{
   // Keep the DELETE-capable handle through verification and removal. Existing
   // readers deny this open; new readers cannot acquire a lease during backup.
   let file=match fs::OpenOptions::new().access_mode(0x80000000|0x00010000).share_mode(1).custom_flags(0x00200000).open(entry.path()){Ok(file)=>file,Err(_)=>continue};
   let meta=file.metadata().map_err(|e|e.to_string())?;
   if !meta.is_file()||meta.file_attributes()&0x400!=0||selected.len()>=64||bytes.checked_add(meta.len()).is_none_or(|n|n>BATCH_BYTES){continue}
   let Some(name)=entry.file_name().to_str().map(str::to_owned)else{continue};
   bytes+=meta.len();selected.push((name,file,meta.len()));
  }
  if selected.is_empty(){return Ok(json!({"directory":null,"archived":0,"retained":total,"status":self.drag_storage_status()?}))}
  crate::image_execution::require_disk_room(crate::image_execution::available_disk_bytes(&self.root)?,bytes)?;
  let base=self.root.join("recovery-exports");fs::create_dir_all(&base).map_err(|e|e.to_string())?;let _base_guard=guard_directory(&base)?;
  let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();let folder=base.join(format!("drag-files-{}-{stamp}",std::process::id()));fs::create_dir(&folder).map_err(|e|e.to_string())?;let _folder_guard=guard_directory(&folder)?;
  let mut manifest=Vec::new();
  for (name,input,size) in &mut selected{
   // Recovery metadata lives at the top level; files are in a separate folder
   // so an unknown source called manifest.json cannot overwrite the manifest.
   let files=folder.join("files");fs::create_dir_all(&files).map_err(|e|e.to_string())?;let _files_guard=guard_directory(&files)?;
   let target=files.join(&name);let mut output=fs::OpenOptions::new().create_new(true).write(true).open(&target).map_err(|e|e.to_string())?;
   let mut hash=Sha256::new();let mut count=0u64;let mut buf=[0u8;65536];
   loop{let n=input.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}count+=n as u64;if count>*size{return Err("拖出文件变化；原文件保留".into())}output.write_all(&buf[..n]).map_err(|e|e.to_string())?;hash.update(&buf[..n]);}
   output.sync_all().map_err(|e|e.to_string())?;drop(output);let checksum=format!("{:x}",hash.finalize());
   if digest(&target)?!=(count,checksum.clone())||count!=*size{return Err("拖出恢复副本校验失败；原文件保留".into())}
   manifest.push(json!({"path":format!("files/{name}"),"bytes":count,"sha256":checksum}));
  }
  #[cfg(test)]
  if self.root.join("test-drag-manifest-failure").exists(){return Err("isolated manifest failure; originals retained".into())}
  let data=serde_json::to_vec_pretty(&json!({"format":"quickgen-drag-files-v1","files":manifest,"note":"Drag copies only, not a complete profile backup"})).map_err(|e|e.to_string())?;
  let path=folder.join("manifest.json");let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&path).map_err(|e|e.to_string())?;file.write_all(&data).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
  if fs::read(path).map_err(|e|e.to_string())?!=data{return Err("拖出恢复清单校验失败；原文件保留".into())}
  let mut removed=0;
  for(_,file,_)in selected{
   use windows::Win32::{Foundation::HANDLE,Storage::FileSystem::{SetFileInformationByHandle,FILE_DISPOSITION_INFO,FileDispositionInfo}};
   let info=FILE_DISPOSITION_INFO{DeleteFile:true};if unsafe{SetFileInformationByHandle(HANDLE(file.as_raw_handle()),FileDispositionInfo,(&info as *const FILE_DISPOSITION_INFO).cast(),std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32)}.is_ok(){removed+=1}
  }
  Ok(json!({"directory":folder,"archived":manifest.len(),"retained":total-removed,"status":self.drag_storage_status()?}))
 }
}
