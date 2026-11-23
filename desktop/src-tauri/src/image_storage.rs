//! User-requested staging maintenance. No active output is removed.
use crate::{resources::{ResourceService,guard_directory,open_blob},contracts::TaskState};
use std::{fs,io::{Read,Write},path::Path};
use sha2::{Digest,Sha256};
use serde_json::{json,Value};
struct Entry{name:String,size:u64,protected:bool}
fn digest_file(path:&Path)->Result<(u64,String),String>{
 let mut file=open_blob(path)?;let mut hash=Sha256::new();let mut count=0u64;let mut buf=[0u8;65536];
 loop{let n=file.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}count+=n as u64;if count>crate::image_execution::IMAGE_STAGING_BUDGET{return Err("遗留文件超过复制上限；原文件已保留".into())}hash.update(&buf[..n]);}
 Ok((count,format!("{:x}",hash.finalize())))
}
impl ResourceService{
 fn image_storage_entries(&self)->Result<Vec<Entry>,String>{
  let protected:Vec<String>=self.task_snapshot()?.into_iter().filter(|t|t.state==TaskState::Running&&t.request.capability.connection_id!="internal-mock").map(|t|crate::image_execution::received_path(&self.root,&t).file_stem().unwrap().to_string_lossy().into_owned()).collect();
  let _guard=guard_directory(&self.root.join("staging"))?;let mut rows=Vec::new();
  for entry in fs::read_dir(self.root.join("staging")).map_err(|e|e.to_string())?{
   let entry=entry.map_err(|e|e.to_string())?;let name=entry.file_name().to_str().ok_or("invalid staging name")?.to_string();
   if !name.starts_with("image-received-"){continue}if rows.len()>=1024{return Err("暂存文件过多，已保留文件，请检查恢复目录".into())}
   let file=open_blob(&entry.path())?;let size=file.metadata().map_err(|e|e.to_string())?.len();
   let is_protected=protected.iter().any(|stem|name.starts_with(&format!("{stem}.")));
   rows.push(Entry{name,size,protected:is_protected});
  }Ok(rows)
 }
 pub fn image_storage_status(&self)->Result<Value,String>{
  let rows=self.image_storage_entries()?;let pending=self.pending_image_bytes()?;
  let staged=rows.iter().try_fold(0u64,|n,e|n.checked_add(e.size).ok_or("staging size overflow"))?;
  Ok(json!({"limit_bytes":crate::image_execution::IMAGE_STAGING_BUDGET,"staged_bytes":staged,"pending_bytes":pending,"protected_files":rows.iter().filter(|e|e.protected).count(),"residue_files":rows.iter().filter(|e|!e.protected).count()}))
 }
 pub fn clean_received_copies(&self)->Result<Value,String>{let removed=self.reclaim_received_copies()?;Ok(json!({"removed":removed,"status":self.image_storage_status()?}))}
 /// Copy, flush and verify every file and manifest before removing any source.
 /// Unknown bytes remain in the recovery export, not silently discarded.
 pub fn archive_image_residue(&self)->Result<Value,String>{
  let _source_guard=guard_directory(&self.root.join("staging"))?;
  let entries:Vec<_>=self.image_storage_entries()?.into_iter().filter(|e|!e.protected).collect();
  if entries.is_empty(){return Ok(json!({"directory":null,"archived":0,"retained":0,"status":self.image_storage_status()?}))}
  let total=entries.iter().try_fold(0u64,|n,e|n.checked_add(e.size).ok_or("staging size overflow"))?;
  if total>crate::image_execution::IMAGE_STAGING_BUDGET{return Err("遗留文件超过单次备份上限；原文件已保留".into())}
  crate::image_execution::require_disk_room(crate::image_execution::available_disk_bytes(&self.root)?,total)?;
  let base=self.root.join("recovery-exports");fs::create_dir_all(&base).map_err(|e|e.to_string())?;let _base_guard=guard_directory(&base)?;
  let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
  let folder=base.join(format!("received-residue-{}-{stamp}",std::process::id()));fs::create_dir(&folder).map_err(|e|e.to_string())?;let _folder_guard=guard_directory(&folder)?;
  let mut manifest=Vec::new();
  for entry in &entries{
   let source=self.root.join("staging").join(&entry.name);let target=folder.join(&entry.name);let mut input=open_blob(&source)?;
   let mut output=fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e|e.to_string())?;
   let mut hash=Sha256::new();let mut bytes=0u64;let mut buf=[0u8;65536];
   loop{let n=input.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}bytes+=n as u64;if bytes>entry.size{return Err("遗留文件已变化，未移除原文件".into())}output.write_all(&buf[..n]).map_err(|e|e.to_string())?;hash.update(&buf[..n]);}
   output.sync_all().map_err(|e|e.to_string())?;drop(output);let digest=format!("{:x}",hash.finalize());
   if bytes!=entry.size||digest_file(&target)?!=(bytes,digest.clone()){return Err("恢复副本校验失败，未移除原文件".into())}
   manifest.push(json!({"path":entry.name,"bytes":bytes,"sha256":digest}));
  }
  #[cfg(test)]
  if self.root.join("test-residue-manifest-failure").exists(){return Err("isolated manifest failure; sources retained".into())}
  let data=serde_json::to_vec_pretty(&json!({"format":"quickgen-received-residue-v1","files":manifest,"note":"Preserved received-image residues; this is not a complete profile backup."})).map_err(|e|e.to_string())?;
  let path=folder.join("manifest.json");let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&path).map_err(|e|e.to_string())?;file.write_all(&data).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
  if fs::read(&path).map_err(|e|e.to_string())?!=data{return Err("恢复清单校验失败，原文件保留".into())}
  let mut removed=0;
  for entry in &manifest{
   let name=entry["path"].as_str().unwrap();let source=self.root.join("staging").join(name);
   let expected=(entry["bytes"].as_u64().unwrap(),entry["sha256"].as_str().unwrap().to_string());
   if digest_file(&source).is_ok_and(|actual|actual==expected)&&fs::remove_file(&source).is_ok(){removed+=1}
  }
  Ok(json!({"directory":folder,"archived":entries.len(),"retained":entries.len()-removed,"status":self.image_storage_status()?}))
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn unknown_residue_is_verified_before_removal_and_retry_is_empty(){
  let root=std::env::temp_dir().join(format!("quickgen-residue-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let service=ResourceService::open(&root).unwrap();let name="image-received-unknown.partial";let bytes=b"preserve incomplete provider response";
  fs::write(root.join("staging").join(name),bytes).unwrap();fs::write(root.join("staging/unrelated"),b"keep").unwrap();
  assert_eq!(service.image_storage_status().unwrap()["residue_files"],1);
  fs::write(root.join("test-residue-manifest-failure"),b"isolated only").unwrap();assert!(service.archive_image_residue().unwrap_err().contains("sources retained"));assert_eq!(fs::read(root.join("staging").join(name)).unwrap(),bytes);
  fs::remove_file(root.join("test-residue-manifest-failure")).unwrap();
  let result=service.archive_image_residue().unwrap();let folder=std::path::PathBuf::from(result["directory"].as_str().unwrap());
  assert_eq!(fs::read(folder.join(name)).unwrap(),bytes);assert!(folder.join("manifest.json").is_file());assert!(!root.join("staging").join(name).exists());assert_eq!(fs::read(root.join("staging/unrelated")).unwrap(),b"keep");
  assert_eq!(service.archive_image_residue().unwrap()["archived"],0);
 }
}
