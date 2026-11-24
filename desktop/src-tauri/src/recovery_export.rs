//! Readable recovery bundle. Never mutates or deletes the active data store.
use crate::resources::ResourceService;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::Path};

fn write_verified(path:&Path,bytes:&[u8])->Result<String,String>{
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e|e.to_string())?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
    let hash=format!("{:x}",Sha256::digest(bytes));
    if format!("{:x}",Sha256::digest(fs::read(path).map_err(|e|e.to_string())?))!=hash{return Err("export verification mismatch".into())}Ok(hash)
}
impl ResourceService {
    /// Runs on the single owner worker: no metadata changes between SQLite's
    /// snapshot and the immutable-body copies. Incomplete folders are retained.
    pub fn export_recovery(&self,config_backup:Value)->Result<Value,String>{
        let base=self.root.join("recovery-exports");
        if !base.exists(){fs::create_dir(&base).map_err(|e|e.to_string())?;}
        if base.canonicalize().map_err(|e|e.to_string())?!=base{return Err("redirected export directory".into())}
        let _base_guard=crate::resources::guard_directory(&base)?;
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
        let folder=base.join(format!("recovery-{}-{stamp}",std::process::id()));fs::create_dir(&folder).map_err(|e|e.to_string())?;
        let _folder_guard=crate::resources::guard_directory(&folder)?;
        let blobs=folder.join("blobs");fs::create_dir(&blobs).map_err(|e|e.to_string())?;
        let _blobs_guard=crate::resources::guard_directory(&blobs)?;
        let database=folder.join("metadata.sqlite");
        self.store.connection.execute("VACUUM INTO ?1",[database.to_string_lossy().as_ref()]).map_err(|e|e.to_string())?;
        let copy=rusqlite::Connection::open_with_flags(&database,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
        let integrity:String=copy.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;if integrity!="ok"{return Err(format!("export database integrity: {integrity}"))}
        drop(copy);fs::OpenOptions::new().write(true).open(&database).and_then(|file|file.sync_all()).map_err(|e|e.to_string())?;
        let mut files=vec![json!({"path":"metadata.sqlite","sha256":format!("{:x}",Sha256::digest(fs::read(&database).map_err(|e|e.to_string())?))})];
        let backup_dir=folder.join("migration-backups");fs::create_dir(&backup_dir).map_err(|e|e.to_string())?;
        let _backup_guard=crate::resources::guard_directory(&backup_dir)?;
        let mut backups=self.store.connection.prepare("SELECT source_digest,snapshot_json FROM migration_runs ORDER BY migration_id").map_err(|e|e.to_string())?;
        for row in backups.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())? {
            let (digest,snapshot)=row.map_err(|e|e.to_string())?;
            if digest.len()!=64||!digest.bytes().all(|b|b.is_ascii_hexdigit()){return Err("invalid backup identity".into())}
            let path=self.root.join("migration-backups").join(format!("{digest}.json"));
            if path.canonicalize().map_err(|e|e.to_string())?!=path{return Err("redirected migration backup".into())}
            let bytes=fs::read(path).map_err(|e|e.to_string())?;
            if bytes!=snapshot.as_bytes()||format!("{:x}",Sha256::digest(&bytes))!=digest{return Err("migration backup differs from source".into())}
            let hash=write_verified(&backup_dir.join(format!("{digest}.json")),&bytes)?;
            files.push(json!({"path":format!("migration-backups/{digest}.json"),"sha256":hash}));
        }
        let mut query=self.store.connection.prepare("SELECT resource_id,state FROM resources ORDER BY resource_id").map_err(|e|e.to_string())?;
        let rows=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
        let mut inventory=vec![];
        for row in rows {
            let (id,state)=row.map_err(|e|e.to_string())?;let (record,bytes)=self.read_stored(&id,false)?;
            let hash=write_verified(&blobs.join(&record.resource.blob_id),&bytes)?;
            files.push(json!({"path":format!("blobs/{}",record.resource.blob_id),"sha256":hash}));
            inventory.push(json!({"state":state,"record":record}));
        }
        #[cfg(windows)]
        {
            let artifacts=self.received_image_artifacts()?;
            if !artifacts.is_empty(){
                let staging=folder.join("staging");fs::create_dir(&staging).map_err(|e|e.to_string())?;
                let _guard=crate::resources::guard_directory(&staging)?;
                for (name,bytes) in artifacts{
                    files.push(json!({"path":format!("staging/{name}"),"sha256":write_verified(&staging.join(name),&bytes)?}));
                }
            }
        }
        let notes=self.notes_list()?;
        let report=json!({"notes":notes,"resources":inventory,"tasks":self.task_snapshot()?,"scene":self.scene_snapshot()?,"configBackup":config_backup});
        let bytes=serde_json::to_vec_pretty(&report).map_err(|e|e.to_string())?;
        files.push(json!({"path":"inventory.json","sha256":write_verified(&folder.join("inventory.json"),&bytes)?}));
        let manifest=json!({"format":"quickgen-recovery-v1","databaseSchema":crate::store::SCHEMA_VERSION,"noteDocumentVersion":1,"resourceCount":inventory.len(),"noteCount":notes.len(),"files":files,"legacyRollback":"Old executables cannot read changes created after activation. Keep this entire bundle and the original store; never delete the new database to force rollback."});
        write_verified(&folder.join("manifest.json"),&serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?)?;
        Ok(json!({"directory":folder,"manifest":manifest}))
    }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn recovery_inventory_and_copy_preserve_all_notes(){
  use crate::notes::NotePatch;
  let root=std::env::temp_dir().join(format!("quickgen-note-export-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let blank=service.note_create().unwrap();let edited=service.note_create().unwrap();
  let edited=service.note_patch(&edited.id,NotePatch::Text{revision:0,text:"中文恢复\nsecond line".into()}).unwrap();
  let mut document=edited.document.clone();document["doc"]["content"][0]["content"][0]["marks"]=json!([{"type":"highlight"}]);
  let edited=service.note_patch(&edited.id,NotePatch::Document{revision:edited.text_revision,document}).unwrap();
  let edited=service.note_patch(&edited.id,NotePatch::Theme{revision:0,theme:"paper".into()}).unwrap();
  let edited=service.note_patch(&edited.id,NotePatch::Flags{revision:0,resident:false,always_on_top:true}).unwrap();
  service.note_patch(&edited.id,NotePatch::Geometry{revision:0,x:120.,y:140.,width:320.,height:260.,monitor:"isolated-monitor".into()}).unwrap();
  let removed=service.note_create().unwrap();service.note_patch(&removed.id,NotePatch::Deleted{revision:0,deleted:true}).unwrap();
  let before=service.notes_list().unwrap();let result=service.export_recovery(json!({})).unwrap();
  let folder=std::path::PathBuf::from(result["directory"].as_str().unwrap());
  let report:Value=serde_json::from_slice(&fs::read(folder.join("inventory.json")).unwrap()).unwrap();
  assert_eq!(report["notes"],serde_json::to_value(&before).unwrap());assert_eq!(result["manifest"]["noteCount"],3);
  for item in result["manifest"]["files"].as_array().unwrap(){assert_eq!(format!("{:x}",Sha256::digest(fs::read(folder.join(item["path"].as_str().unwrap())).unwrap())),item["sha256"].as_str().unwrap());}
  // Restore a second copy; never open the sole exported snapshot for writing.
  let restored_root=root.join("isolated-restored-copy");fs::create_dir(&restored_root).unwrap();fs::copy(folder.join("metadata.sqlite"),restored_root.join("metadata.sqlite")).unwrap();
  let restored=ResourceService::open(&restored_root).unwrap();assert_eq!(restored.notes_list().unwrap(),before);assert_eq!(restored.note_get(&blank.id).unwrap().text,"");assert!(restored.note_get(&removed.id).unwrap().deleted);
  assert_eq!(service.notes_list().unwrap(),before);
 }
 #[test]fn export_reopens_with_resources_scene_and_task_history(){
  let root=std::env::temp_dir().join(format!("quickgen-export-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=service.import_bytes(bytes.into_inner(),"png","original".into(),crate::contracts::Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let config=json!({"abilities":[{"id":"clay","name":"mock","prompt":"fixed"}]});
  service.admit_mock(crate::tasks::MockSubmission{submission_id:"export-task".into(),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&config).unwrap();service.tick_mock().unwrap();service.tick_mock().unwrap();
  let before=service.task_snapshot().unwrap();let export=service.export_recovery(config.clone()).unwrap();let folder=std::path::PathBuf::from(export["directory"].as_str().unwrap());assert_eq!(export["manifest"]["resourceCount"],2);
  let restored=ResourceService::open(&folder).unwrap();assert_eq!(restored.task_snapshot().unwrap(),before);assert!(restored.read(&input.resource.id).is_ok());assert!(restored.read(&before[0].outputs[0].resource_id).is_ok());
  assert_eq!(service.task_snapshot().unwrap(),before);assert!(root.join("metadata.sqlite").exists());
  let report:Value=serde_json::from_slice(&fs::read(folder.join("inventory.json")).unwrap()).unwrap();assert_eq!(report["configBackup"],config);
  drop(restored);
  let source=json!({"version":1,"items":[],"jobs":[],"scale":1,"tx":0,"ty":0,"selected":null,"selection":[],"placed":false,"pinned":false,"open":false,"passthrough":false,"light":false,"nextId":1,"jobId":0,"configBackup":config});
  let migration=service.prepare_migration(source.clone()).unwrap();service.copy_migration(&migration).unwrap();let scene=service.activate_migration(&migration).unwrap();
  let full=service.export_recovery(json!({"config":config})).unwrap();let full_dir=std::path::PathBuf::from(full["directory"].as_str().unwrap());
  let restored=ResourceService::open(&full_dir).unwrap();assert_eq!(restored.active_scene().unwrap(),Some(scene));assert_eq!(restored.task_snapshot().unwrap(),before);
  assert_eq!(fs::read_dir(full_dir.join("migration-backups")).unwrap().count(),1);drop(restored);
  fs::write(root.join("blobs").join(&input.resource.blob_id),b"broken").unwrap();
  assert!(service.export_recovery(json!({})).is_err());
  let complete=fs::read_dir(root.join("recovery-exports")).unwrap().filter(|entry|entry.as_ref().unwrap().path().join("manifest.json").exists()).count();assert_eq!(complete,2);
 }
}
