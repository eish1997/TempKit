//! Durable drag copies are separate from blobs. No automatic deletion while consumers may read.
use std::{fs,io::{Write,Read},path::PathBuf};
use crate::resources::{ResourceService,guard_directory,open_blob};
use sha2::{Digest,Sha256};
pub const EXPORT_BUDGET:u64=512*1024*1024;
pub struct DragCopy { pub path:PathBuf, _file:fs::File, _directory:fs::File }
fn lease(path:PathBuf,directory:fs::File,expected:&[u8])->Result<DragCopy,String>{
 let mut file=open_blob(&path)?;
 if file.metadata().map_err(|e|e.to_string())?.len()!=expected.len() as u64{return Err("export copy changed".into())}
 let mut bytes=Vec::new();(&mut file).take(expected.len() as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes!=expected{return Err("export copy changed".into())}
 Ok(DragCopy{path,_file:file,_directory:directory})
}
impl ResourceService {
 pub fn drag_storage_status(&self)->Result<serde_json::Value,String>{
  let directory=self.root.join("drag-exports");if !directory.exists(){return Ok(serde_json::json!({"bytes":0,"files":0,"limit_bytes":EXPORT_BUDGET}))}
  let _guard=guard_directory(&directory)?;let mut bytes=0u64;let mut files=0usize;
  for entry in fs::read_dir(directory).map_err(|e|e.to_string())?{let entry=entry.map_err(|e|e.to_string())?;let metadata=fs::symlink_metadata(entry.path()).map_err(|e|e.to_string())?;files+=1;if files>4096{return Err("拖出副本数量过多，请检查目录".into())}bytes=bytes.checked_add(metadata.len()).ok_or("export size overflow")?;}
  Ok(serde_json::json!({"bytes":bytes,"files":files,"limit_bytes":EXPORT_BUDGET}))
 }
 /// Explicit user action after external consumers finish copying. Retain busy,
 /// modified, unknown and partial files. Delete the verified handle, not a path
 /// that could have changed between verification and removal.
 #[cfg(windows)]
 pub fn clean_drag_copies(&mut self)->Result<serde_json::Value,String>{
  self.clean_drag_batch(32*1024*1024,64)
 }
 #[cfg(windows)]
 fn clean_drag_batch(&mut self,byte_limit:u64,file_limit:usize)->Result<serde_json::Value,String>{
  use std::os::windows::{fs::{OpenOptionsExt,MetadataExt},io::AsRawHandle};
  use windows::Win32::{Foundation::HANDLE,Storage::FileSystem::{SetFileInformationByHandle,FILE_DISPOSITION_INFO,FileDispositionInfo}};
  let directory=self.root.join("drag-exports");let mut removed=0;let mut retained=0;let mut deferred=0;let mut checked_bytes=0u64;let mut checked_files=0usize;
  if directory.exists(){let _guard=guard_directory(&directory)?;
   let mut entries=fs::read_dir(&directory).map_err(|e|e.to_string())?.take(4097).collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
   if entries.len()>4096{return Err("拖出副本数量过多，请检查目录".into())}
   entries.sort_by_key(|entry|entry.file_name());let count=entries.len();let start=if count==0{0}else{self.drag_cleanup_cursor%count};let mut next=None;
   for offset in 0..count{
    let index=(start+offset)%count;let entry=&entries[index];
    let name=entry.file_name();let name=name.to_string_lossy();
    let digest=name.strip_prefix("QuickGen-").and_then(|s|s.rsplit_once('.')).filter(|(hash,ext)|hash.len()==64&&hash.bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase())&&matches!(*ext,"png"|"jpg"|"webp")).map(|(hash,_)|hash);
    let Some(digest)=digest else{retained+=1;continue};
    let cleaned=(||->Result<(),String>{
     let mut file=fs::OpenOptions::new().access_mode(0x80000000|0x00010000).share_mode(1).custom_flags(0x00200000).open(entry.path()).map_err(|e|e.to_string())?;
     let metadata=file.metadata().map_err(|e|e.to_string())?;if !metadata.is_file()||metadata.file_attributes()&0x400!=0||metadata.len()>crate::resources::MAX_BYTES{return Err("unexpected export copy".into())}
     if checked_files>=file_limit||checked_bytes.checked_add(metadata.len()).is_none_or(|n|n>byte_limit){deferred+=1;next.get_or_insert(index);return Err("next batch".into())}
     checked_files+=1;checked_bytes+=metadata.len();
     let mut hash=Sha256::new();let mut buf=[0u8;65536];let mut read=0u64;
     loop{let n=file.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}read+=n as u64;if read>metadata.len(){return Err("export changed".into())}hash.update(&buf[..n]);}
     if read!=metadata.len()||format!("{:x}",hash.finalize())!=digest{return Err("modified export retained".into())}
     let disposition=FILE_DISPOSITION_INFO{DeleteFile:true};
     unsafe{SetFileInformationByHandle(HANDLE(file.as_raw_handle()),FileDispositionInfo,(&disposition as *const FILE_DISPOSITION_INFO).cast(),std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32)}.map_err(|e|e.to_string())?;Ok(())
    })();if cleaned.is_ok(){removed+=1}else{retained+=1}
   }
   self.drag_cleanup_cursor=next.unwrap_or(0);
  }
  Ok(serde_json::json!({"removed":removed,"retained":retained,"deferred":deferred,"checked_bytes":checked_bytes,"status":self.drag_storage_status()?}))
 }
 pub fn prepare_drag_copy(&mut self,id:&str)->Result<DragCopy,String>{
  let (record,bytes)=self.read(id)?;
  let extension=match record.resource.mime.as_str(){"image/png"=>"png","image/jpeg"=>"jpg","image/webp"=>"webp",_=>return Err("unsupported export image".into())};
  let directory=self.root.join("drag-exports");
  fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
  let _guard=guard_directory(&directory)?;
  let digest=format!("{:x}",Sha256::digest(&bytes));
  let path=directory.join(format!("QuickGen-{digest}.{extension}"));
  // Never follow a pre-existing redirect or overwrite external modifications.
  if path.exists(){return lease(path,_guard,&bytes)}
  let mut used=0u64;
  for entry in fs::read_dir(&directory).map_err(|e|e.to_string())?{
   let entry=entry.map_err(|e|e.to_string())?;
   let metadata=fs::symlink_metadata(entry.path()).map_err(|e|e.to_string())?;
   if !metadata.is_file()||metadata.file_type().is_symlink(){return Err("unexpected export entry".into())}
   used=used.checked_add(metadata.len()).ok_or("export size overflow")?;
  }
  if used.checked_add(bytes.len() as u64).ok_or("export size overflow")?>EXPORT_BUDGET{return Err("drag export storage budget reached; existing copies retained".into())}
  // Partial writes remain visible to accounting and are never offered as completed copies.
  let pending=directory.join(format!("pending-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos()));
  let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&pending).map_err(|e|e.to_string())?;
  file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
  fs::hard_link(&pending,&path).map_err(|e|e.to_string())?;
  fs::remove_file(&pending).map_err(|e|e.to_string())?;
  lease(path,_guard,&bytes)
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 fn fixture()->(ResourceService,String,Vec<u8>){
  let root=std::env::temp_dir().join(format!("quickgen-drag-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();
  let mut buffer=std::io::Cursor::new(Vec::new());
  image::DynamicImage::new_rgba8(4,4).write_to(&mut buffer,image::ImageFormat::Png).unwrap();
  let bytes=buffer.into_inner();let record=service.import_bytes(bytes.clone(),"png","test".into(),crate::contracts::Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  (service,record.resource.id,bytes)
 }
 #[test]fn copy_reuses_and_survives_restart_without_touching_original(){
  let (mut service,id,bytes)=fixture();let root=service.root.clone();
  let copy=service.prepare_drag_copy(&id).unwrap();let path=copy.path.clone();
  assert_eq!(fs::read(&path).unwrap(),bytes);
  #[cfg(windows)]{assert!(fs::write(&path,b"overwrite").is_err());assert!(fs::remove_file(&path).is_err());}
  drop(copy);assert_eq!(service.prepare_drag_copy(&id).unwrap().path,path);
  assert_eq!(service.read(&id).unwrap().1,bytes);drop(service);
  let mut reopened=ResourceService::open(&root).unwrap();assert_eq!(reopened.prepare_drag_copy(&id).unwrap().path,path);
  fs::write(&path,b"tampered").unwrap();assert!(reopened.prepare_drag_copy(&id).is_err());assert_eq!(reopened.read(&id).unwrap().1,bytes);
 }
 #[test]fn budget_keeps_existing_copies_and_partial_files(){
  let(mut service,id,_)=fixture();let dir=service.root.join("drag-exports");fs::create_dir_all(&dir).unwrap();
  let pending=dir.join("pending-test");fs::File::create(&pending).unwrap().set_len(EXPORT_BUDGET).unwrap();
  assert!(service.prepare_drag_copy(&id).err().unwrap().contains("budget"));assert_eq!(fs::metadata(pending).unwrap().len(),EXPORT_BUDGET);assert!(service.read(&id).is_ok());
 }
 #[cfg(windows)]
 #[test]fn explicit_cleanup_preserves_leases_modified_files_and_originals(){
  let(mut service,id,bytes)=fixture();let copy=service.prepare_drag_copy(&id).unwrap();let path=copy.path.clone();
  let dir=service.root.join("drag-exports");fs::write(dir.join("pending-fixture"),b"unfinished").unwrap();
  let modified=dir.join(format!("QuickGen-{}.png","a".repeat(64)));fs::write(&modified,b"externally modified").unwrap();
  assert_eq!(service.drag_storage_status().unwrap()["files"],3);
  let result=service.clean_drag_copies().unwrap();assert_eq!(result["removed"],0);assert_eq!(result["retained"],3);assert!(path.exists());
  drop(copy);let external=open_blob(&path).unwrap();assert_eq!(service.clean_drag_copies().unwrap()["removed"],0);drop(external);
  let result=service.clean_drag_copies().unwrap();assert_eq!(result["removed"],1);assert_eq!(result["retained"],2);assert!(!path.exists());assert!(modified.exists());assert!(dir.join("pending-fixture").exists());
  assert_eq!(service.read(&id).unwrap().1,bytes);assert_eq!(service.clean_drag_copies().unwrap()["removed"],0);
  let recreated=service.prepare_drag_copy(&id).unwrap();assert_eq!(fs::read(&recreated.path).unwrap(),bytes);
 }
 #[cfg(windows)]
 #[test]fn cleanup_batches_bound_bytes_and_allow_explicit_continuation(){
  let(mut service,id,bytes)=fixture();drop(service.prepare_drag_copy(&id).unwrap());
  let mut other=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(5,4).write_to(&mut other,image::ImageFormat::Png).unwrap();let other=other.into_inner();
  let record=service.import_bytes(other.clone(),"png","other".into(),crate::contracts::Provenance{task_id:None,parent_ids:vec![]}).unwrap();drop(service.prepare_drag_copy(&record.resource.id).unwrap());
  let empty=service.clean_drag_batch(0,1).unwrap();assert_eq!(empty["removed"],0);assert_eq!(empty["deferred"],2);assert_eq!(empty["checked_bytes"],0);
  let limit=bytes.len().max(other.len()) as u64;
  let first=service.clean_drag_batch(limit,1).unwrap();assert_eq!(first["removed"],1);assert_eq!(first["deferred"],1);assert!(first["checked_bytes"].as_u64().unwrap()<=limit);
  let second=service.clean_drag_batch(limit,1).unwrap();assert_eq!(second["removed"],1);assert_eq!(second["deferred"],0);assert_eq!(second["status"]["files"],0);
  assert_eq!(service.read(&id).unwrap().1,bytes);assert_eq!(service.read(&record.resource.id).unwrap().1,other);
  drop(service.prepare_drag_copy(&id).unwrap());
  let modified=service.root.join("drag-exports").join(format!("QuickGen-{}.png","0".repeat(64)));fs::write(&modified,b"modified").unwrap();
  let first=service.clean_drag_batch(limit,1).unwrap();assert_eq!(first["removed"],0);assert_eq!(first["deferred"],1);
  let next=service.clean_drag_batch(limit,1).unwrap();assert_eq!(next["removed"],1);assert!(modified.exists(),"modified file must survive while later valid files still progress");
 }
}
