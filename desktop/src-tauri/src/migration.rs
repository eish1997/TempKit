//! Legacy snapshot inventory. Preparation never activates the new store.
use serde::{Serialize,Deserialize};
use serde_json::Value;
use sha2::{Digest,Sha256};
use std::collections::BTreeSet;
use crate::{resources::{ResourceService,StoredImage,validate_image},contracts::Provenance};
use rusqlite::OptionalExtension;
use base64::Engine;

impl ResourceService {
    pub fn active_scene(&self)->Result<Option<Value>,String>{
        let active:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM migration_runs WHERE state='active')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !active{return Ok(None)}
        self.scene_snapshot()?.map(Some).ok_or("active migration has no scene; preserve store and restore from backup".into())
    }
    /// Caller freezes/export-checks the final source. Backup and byte verification
    /// are repeated immediately before this atomic authority change.
    pub fn activate_migration(&mut self,id:&str)->Result<Value,String>{
        let active:Option<String>=self.store.connection.query_row("SELECT migration_id FROM migration_runs WHERE state='active'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        if let Some(active)=active {
            if active!=id{return Err("another migration is already active".into())}
            return self.active_scene()?.ok_or("active scene missing".into());
        }
        if self.scene_snapshot()?.is_some(){return Err("scene exists without active migration; refusing overwrite".into())}
        let scene=self.verify_migration(id)?;let refs=self.validate_scene(&scene)?;
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO scenes VALUES('main',0,?1)",[scene.to_string()]).map_err(|e|e.to_string())?;
        crate::scenes::retain_scene(&tx,&refs)?;
        let updated=tx.execute("UPDATE migration_runs SET state='active',last_error=NULL WHERE migration_id=?1 AND state='verified'",[id]).map_err(|e|e.to_string())?;
        if updated!=1{return Err("migration is not verified".into())}
        tx.commit().map_err(|e|e.to_string())?;Ok(scene)
    }
    pub fn verify_migration(&mut self,id:&str)->Result<Value,String>{
        let result=self.verify_migration_inner(id);
        if let Err(error)=&result {self.invalidate_migration(id,error)?;}
        result
    }
    fn invalidate_migration(&mut self,id:&str,error:&str)->Result<(),String>{
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM migration_candidates WHERE migration_id=?1 AND EXISTS(SELECT 1 FROM migration_runs WHERE migration_id=?1 AND state!='active')",[id]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE migration_runs SET state='copying',last_error=?2 WHERE migration_id=?1 AND state!='active'",rusqlite::params![id,error]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())
    }
    /// Before activation, rollback returns the exact saved source and preserves all copies.
    pub fn rollback_prepared_migration(&mut self,id:&str)->Result<Value,String>{
        let source:String=self.store.connection.query_row("SELECT snapshot_json FROM migration_runs WHERE migration_id=?1 AND state!='active'",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let snapshot:Value=serde_json::from_str(&source).map_err(|e|e.to_string())?;
        let digest=format!("{:x}",Sha256::digest(source.as_bytes()));
        backup_snapshot(&self.root,&digest,source.as_bytes())?;
        self.invalidate_migration(id,"Preparation rolled back; legacy source remains authoritative")?;
        Ok(snapshot)
    }
    fn verify_migration_inner(&mut self,id:&str)->Result<Value,String>{
        let encoded:String=self.store.connection.query_row("SELECT inventory_json FROM migration_runs WHERE migration_id=?1 AND state!='active'",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let inventory:MigrationInventory=serde_json::from_str(&encoded).map_err(|e|e.to_string())?;
        let source=serde_json::to_vec(&inventory.snapshot).map_err(|e|e.to_string())?;
        if format!("{:x}",Sha256::digest(&source))!=inventory.source_digest{return Err("snapshot digest mismatch".into());}
        backup_snapshot(&self.root,&inventory.source_digest,&source)?;
        let mut mapping=std::collections::BTreeMap::new();
        for entry in &inventory.images {
            let resource:String=self.store.connection.query_row("SELECT resource_id FROM migration_id_maps WHERE migration_id=?1 AND legacy_key=?2",rusqlite::params![id,entry.legacy_key],|r|r.get(0)).map_err(|e|format!("{}: {e}",entry.legacy_key))?;
            let (_,bytes)=self.read(&resource)?;
            let original=base64::engine::general_purpose::STANDARD.decode(entry.source.split_once(',').ok_or("invalid source")?.1).map_err(|e|e.to_string())?;
            if bytes!=original{return Err(format!("{}: copied content differs",entry.legacy_key));}
            mapping.insert(entry.legacy_key.clone(),resource);
        }
        let count:i64=self.store.connection.query_row("SELECT count(*) FROM migration_id_maps WHERE migration_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if count as usize!=inventory.images.len(){return Err("migration mapping count mismatch".into());}
        let mut scene=inventory.snapshot.clone();
        // Config/covers stay in the independently verified source backup, not in
        // every subsequent board save. The live config remains its authority.
        scene.as_object_mut().unwrap().remove("configBackup");
        for item in scene["items"].as_array_mut().ok_or("missing items")? {
            let key=format!("item:{}",item["id"].as_str().unwrap());
            let object=item.as_object_mut().unwrap();object.remove("src");object.insert("resourceId".into(),Value::String(mapping[&key].clone()));
        }
        scene["jobs"]=Value::Array(inventory.historical_jobs.clone());
        for job in scene["jobs"].as_array_mut().unwrap(){
            let old=job["id"].as_u64().unwrap();let key=format!("job:{old}:input");
            let input=job["input"].as_object_mut().unwrap();input.remove("src");input.insert("resourceId".into(),Value::String(mapping[&key].clone()));
            job["legacyTaskId"]=Value::String(format!("legacy-{old}"));
            if let Some(output)=mapping.get(&format!("item:result-{old}")){job["outputResourceIds"]=serde_json::json!([output]);}
            else{job["outputResourceIds"]=serde_json::json!([]);}
        }
        scene["revision"]=serde_json::json!(0);scene["resourceSchemaVersion"]=serde_json::json!(1);
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        // Preserve output provenance even when the input has left the visible board.
        for job in &inventory.historical_jobs {
            let old=job["id"].as_u64().unwrap();
            if let Some(output)=mapping.get(&format!("item:result-{old}")) {
                let json:String=tx.query_row("SELECT metadata_json FROM resources WHERE resource_id=?1",[output],|r|r.get(0)).map_err(|e|e.to_string())?;
                let mut record:StoredImage=serde_json::from_str(&json).map_err(|e|e.to_string())?;
                record.resource.provenance=Provenance{task_id:Some(format!("legacy-{old}")),parent_ids:vec![mapping[&format!("job:{old}:input")].clone()]};
                tx.execute("UPDATE resources SET metadata_json=?2 WHERE resource_id=?1",rusqlite::params![output,serde_json::to_string(&record).unwrap()]).map_err(|e|e.to_string())?;
            }
        }
        tx.execute("INSERT INTO migration_candidates(migration_id,scene_json) VALUES(?1,?2) ON CONFLICT(migration_id) DO UPDATE SET scene_json=excluded.scene_json",rusqlite::params![id,serde_json::to_string(&scene).unwrap()]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE migration_runs SET state='verified',last_error=NULL WHERE migration_id=?1 AND state!='active'",[id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;Ok(scene)
    }
    pub fn prepare_migration(&mut self,snapshot:Value)->Result<String,String>{
        let inventory=MigrationInventory::prepare(snapshot)?;
        let id=format!("migration-{}",inventory.source_digest);
        let source=serde_json::to_string(&inventory.snapshot).map_err(|e|e.to_string())?;
        backup_snapshot(&self.root,&inventory.source_digest,source.as_bytes())?;
        let encoded=serde_json::to_string(&inventory).map_err(|e|e.to_string())?;
        self.store.connection.execute("INSERT OR IGNORE INTO migration_runs VALUES (?1,?2,?3,?4,'created',NULL)",rusqlite::params![id,inventory.source_digest,source,encoded]).map_err(|e|e.to_string())?;
        Ok(id)
    }
    pub fn copy_migration(&mut self,id:&str)->Result<Vec<StoredImage>,String>{
        let encoded:String=self.store.connection.query_row("SELECT inventory_json FROM migration_runs WHERE migration_id=?1 AND state!='active'",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let inventory:MigrationInventory=serde_json::from_str(&encoded).map_err(|e|e.to_string())?;
        self.store.connection.execute("UPDATE migration_runs SET state='copying',last_error=NULL WHERE migration_id=?1 AND state IN ('created','copying')",[id]).map_err(|e|e.to_string())?;
        let mut records=Vec::new();
        for entry in inventory.images {
            let result=(||{
                let digest=format!("{:x}",Sha256::digest(entry.source.as_bytes()));
                let mapped:Option<(String,String)>=self.store.connection.query_row("SELECT resource_id,source_digest FROM migration_id_maps WHERE migration_id=?1 AND legacy_key=?2",rusqlite::params![id,entry.legacy_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
                if let Some((resource,old_digest))=mapped {
                    if digest!=old_digest{return Err("migration mapping content conflict".into());}
                    return self.read(&resource).map(|(record,_)|record);
                }
                let (header,base64)=entry.source.split_once(',').ok_or("invalid image encoding")?;
                if base64.len()>((crate::resources::MAX_BYTES+2)/3*4) as usize{return Err("legacy image exceeds 20MiB".into());}
                if !matches!(header,"data:image/png;base64"|"data:image/jpeg;base64"|"data:image/webp;base64"){return Err("unsupported image encoding".into())}
                let bytes=base64::engine::general_purpose::STANDARD.decode(base64).map_err(|e|e.to_string())?;
                // Historical browser storage sometimes labeled JPEG bytes as
                // PNG. Preserve the original snapshot and bytes; only migration
                // normalizes MIME from actual supported content before decoding.
                let ext=match image::guess_format(&bytes).map_err(|e|e.to_string())?{image::ImageFormat::Png=>"png",image::ImageFormat::Jpeg=>"jpg",image::ImageFormat::WebP=>"webp",_=>return Err("unsupported legacy image content".into())};
                self.archive_mapped(validate_image(bytes,ext)?,entry.display["name"].as_str().unwrap_or("").into(),Provenance{task_id:None,parent_ids:vec![]},Some((id,&entry.legacy_key,&digest)))
            })();
            match result {Ok(record)=>records.push(record),Err(error)=>{
                let error=format!("{}: {error}",entry.legacy_key);
                self.invalidate_migration(id,&error)?;
                return Err(error);
            }}
        }
        Ok(records)
    }
}
fn backup_snapshot(root:&std::path::Path,digest:&str,bytes:&[u8])->Result<(),String>{
    use std::io::Write;
    let directory=root.join("migration-backups");std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
    if directory.canonicalize().map_err(|e|e.to_string())?!=directory{return Err("redirected backup directory".into());}
    let target=directory.join(format!("{digest}.json"));
    if !target.exists(){
        let temp=directory.join(format!("{digest}-{}.tmp",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos()));
        let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(|e|e.to_string())?;
        file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
        std::fs::hard_link(&temp,&target).map_err(|e|e.to_string())?;
        std::fs::remove_file(temp).map_err(|e|e.to_string())?;
    }
    if target.canonicalize().map_err(|e|e.to_string())?!=target{return Err("redirected snapshot backup".into());}
    let restored=std::fs::read(&target).map_err(|e|e.to_string())?;
    if restored!=bytes{return Err("existing snapshot backup differs; original retained".into());}
    let _:Value=serde_json::from_slice(&restored).map_err(|e|e.to_string())?;
    Ok(())
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct LegacyImage {pub legacy_key:String,pub source:String,pub display:Value}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct MigrationInventory {
    pub source_digest:String,
    pub snapshot:Value,
    pub images:Vec<LegacyImage>,
    pub historical_jobs:Vec<Value>,
}
impl MigrationInventory {
    pub fn prepare(snapshot:Value)->Result<Self,String>{
        if snapshot["version"]!=1 {return Err("unsupported legacy scene version".into());}
        let items=snapshot["items"].as_array().ok_or("missing legacy items")?;
        let jobs=snapshot["jobs"].as_array().ok_or("missing legacy jobs")?;
        for field in ["scale","tx","ty"] {if !snapshot[field].as_f64().is_some_and(|x|x.is_finite()){return Err(format!("invalid scene {field}"));}}
        if snapshot["scale"].as_f64().unwrap()<=0. {return Err("invalid scene scale".into());}
        for field in ["placed","pinned","open","passthrough","light"] {if !snapshot[field].is_boolean(){return Err(format!("missing scene {field}"));}}
        for field in ["nextId","jobId"] {if snapshot[field].as_u64().is_none(){return Err(format!("invalid scene {field}"));}}
        let mut ids=BTreeSet::new();let mut images=Vec::new();
        for item in items {
            let id=item["id"].as_str().ok_or("missing item ID")?;
            if !ids.insert(id.to_string()){return Err(format!("duplicate item {id}"));}
            images.push(image(format!("item:{id}"),item)?);
        }
        for selected in snapshot["selection"].as_array().ok_or("missing selection")? {
            if !selected.as_str().is_some_and(|id|ids.contains(id)){return Err("dangling legacy selection".into());}
        }
        if !snapshot["selected"].is_null() && !snapshot["selected"].as_str().is_some_and(|id|ids.contains(id)){return Err("dangling selected item".into());}
        let mut job_ids=BTreeSet::new();let mut historical_jobs=Vec::new();
        for job in jobs {
            let id=job["id"].as_u64().ok_or("invalid legacy job ID")?;
            if !job_ids.insert(id){return Err(format!("duplicate job {id}"));}
            images.push(image(format!("job:{id}:input"),&job["input"])?);
            if !job["abilitySnapshot"].is_object()||!job["preset"].is_string(){return Err(format!("job {id} has no ability snapshot"));}
            let state=job["state"].as_str().ok_or("job state absent")?;
            if !["queued","running","done","failed","cancelled","interrupted"].contains(&state){return Err(format!("unknown legacy state {state}"));}
            let mut historical=job.clone();
            if ["queued","running"].contains(&state){historical["state"]=Value::String("interrupted".into());}
            historical["simulated"]=Value::Bool(true);historical_jobs.push(historical);
        }
        let bytes=serde_json::to_vec(&snapshot).map_err(|e|e.to_string())?;
        Ok(Self{source_digest:format!("{:x}",Sha256::digest(bytes)),snapshot,images,historical_jobs})
    }
}
fn image(key:String,value:&Value)->Result<LegacyImage,String>{
    let source=value["src"].as_str().ok_or_else(||format!("{key}: missing image source"))?;
    if !["data:image/png;base64,","data:image/jpeg;base64,","data:image/webp;base64,"].iter().any(|prefix|source.starts_with(prefix)){return Err(format!("{key}: unsupported source; export requires embedded image"));}
    for field in ["x","y"] {if !value[field].as_f64().is_some_and(|n|n.is_finite()){return Err(format!("{key}: invalid {field}"));}}
    if !value["name"].is_string(){return Err(format!("{key}: missing display name"));}
    Ok(LegacyImage{legacy_key:key,source:source.into(),display:value.clone()})
}
#[cfg(test)]mod tests{
    use super::*;use serde_json::json;
    #[test]fn backup_corruption_and_write_failure_preserve_original(){
        let root=std::env::temp_dir().join(format!("quickgen-backup-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();let root=root.canonicalize().unwrap();
        backup_snapshot(&root,"test",b"{\"keep\":1}").unwrap();backup_snapshot(&root,"test",b"{\"keep\":1}").unwrap();
        let file=root.join("migration-backups/test.json");std::fs::write(&file,b"damaged backup").unwrap();
        assert!(backup_snapshot(&root,"test",b"{\"keep\":1}").is_err());assert_eq!(std::fs::read(file).unwrap(),b"damaged backup");
        let blocked=root.join("blocked");std::fs::create_dir(&blocked).unwrap();std::fs::write(blocked.join("migration-backups"),b"keep me").unwrap();
        assert!(backup_snapshot(&blocked,"test",b"{}").is_err());assert_eq!(std::fs::read(blocked.join("migration-backups")).unwrap(),b"keep me");
    }
    fn scene()->Value{json!({"version":1,"items":[{"id":"local-1","name":"原图","src":"data:image/png;base64,AQID","x":-2,"y":3,"addedAt":123,"order":2,"custom":"preserve"}],"jobs":[],"scale":1,"tx":2,"ty":3,"placed":true,"pinned":false,"open":true,"passthrough":false,"light":false,"nextId":3,"jobId":0,"selected":"local-1","selection":["local-1"]})}
    fn valid_scene()->Value{let mut s=scene();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();s["items"][0]["src"]=json!(format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())));s}
    fn temp()->std::path::PathBuf{std::env::temp_dir().join(format!("quickgen-migration-recovery-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))}
    #[test]fn scene_json_preserves_exact_camera_numbers(){
        for value in [1.3720290902350643_f64,1.3720290902350645,-1468.8216411479225,-125.78947099942673] {
            let encoded=serde_json::to_string(&value).unwrap();
            let decoded:Value=serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded.as_f64().unwrap().to_bits(),value.to_bits(),"camera round trip: {encoded}");
        }
    }
    #[test]fn legacy_mislabeled_jpeg_preserves_body_and_backup_but_normalizes_resource_mime(){
        let root=temp();let mut service=ResourceService::open(&root).unwrap();let mut source=scene();
        let mut body=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,2).write_to(&mut body,image::ImageFormat::Jpeg).unwrap();let body=body.into_inner();
        source["items"][0]["src"]=json!(format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(&body)));
        assert!(validate_image(body.clone(),"png").is_err());
        let id=service.prepare_migration(source.clone()).unwrap();let records=service.copy_migration(&id).unwrap();assert_eq!(records[0].resource.mime,"image/jpeg");
        let (_,copied)=service.read(&records[0].resource.id).unwrap();assert_eq!(copied,body);service.verify_migration(&id).unwrap();
        assert_eq!(service.rollback_prepared_migration(&id).unwrap(),source);
    }
    #[test]fn activation_and_scene_are_atomic_and_reopen_selects_new_authority(){
        let root=temp();let mut service=ResourceService::open(&root).unwrap();let mut source=valid_scene();source["configBackup"]=json!({"covers":{"keep":"original"}});
        let id=service.prepare_migration(source.clone()).unwrap();service.copy_migration(&id).unwrap();assert!(service.active_scene().unwrap().is_none());
        service.store.connection.execute_batch("CREATE TRIGGER reject_activation BEFORE UPDATE OF state ON migration_runs WHEN NEW.state='active' BEGIN SELECT RAISE(ABORT,'activation failed'); END;").unwrap();
        assert!(service.activate_migration(&id).is_err());assert!(service.scene_snapshot().unwrap().is_none());assert!(service.active_scene().unwrap().is_none());
        service.store.connection.execute_batch("DROP TRIGGER reject_activation").unwrap();
        let active=service.activate_migration(&id).unwrap();assert!(active.get("configBackup").is_none());assert_eq!(active["items"][0]["x"],source["items"][0]["x"]);
        let mut moved=active.clone();moved["items"][0]["x"]=json!(999);let moved=service.save_scene(0,moved).unwrap();
        drop(service);let mut service=ResourceService::open(&root).unwrap();assert_eq!(service.active_scene().unwrap(),Some(moved.clone()));assert_eq!(service.activate_migration(&id).unwrap(),moved);
        assert!(service.rollback_prepared_migration(&id).is_err());
        let backup:String=service.store.connection.query_row("SELECT snapshot_json FROM migration_runs WHERE migration_id=?1",[&id],|r|r.get(0)).unwrap();assert_eq!(serde_json::from_str::<Value>(&backup).unwrap(),source);
    }
    #[test]fn empty_scene_verifies_and_damaged_input_blocks_candidate(){
        let root=temp();let mut service=ResourceService::open(&root).unwrap();let mut empty=scene();empty["items"]=json!([]);empty["selection"]=json!([]);empty["selected"]=Value::Null;
        let run=service.prepare_migration(empty).unwrap();assert!(service.copy_migration(&run).unwrap().is_empty());assert_eq!(service.verify_migration(&run).unwrap()["items"],json!([]));
        let damaged=service.prepare_migration(scene()).unwrap();assert!(service.copy_migration(&damaged).is_err());assert!(service.verify_migration(&damaged).is_err());
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM migration_candidates WHERE migration_id=?1",[damaged],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]fn committed_item_crash_resumes_without_duplicate_mapping(){
        if let Ok(root)=std::env::var("QUICKGEN_MIGRATION_CRASH_ROOT"){
            let mut service=ResourceService::open(std::path::Path::new(&root)).unwrap();
            service.copy_migration(&std::env::var("QUICKGEN_MIGRATION_CRASH_RUN").unwrap()).unwrap();panic!("checkpoint not reached");
        }
        let root=temp();let mut source=valid_scene();let mut second=source["items"][0].clone();second["id"]=json!("local-2");source["items"].as_array_mut().unwrap().push(second);
        let run={let mut service=ResourceService::open(&root).unwrap();service.prepare_migration(source).unwrap()};
        let status=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","migration::tests::committed_item_crash_resumes_without_duplicate_mapping","--nocapture"])
            .env("QUICKGEN_MIGRATION_CRASH_ROOT",&root).env("QUICKGEN_MIGRATION_CRASH_RUN",&run).env("QUICKGEN_ARCHIVE_CRASH_POINT","committed").status().unwrap();assert_eq!(status.code(),Some(74));
        let mut service=ResourceService::open(&root).unwrap();let old:String=service.store.connection.query_row("SELECT resource_id FROM migration_id_maps",[],|r|r.get(0)).unwrap();
        let records=service.copy_migration(&run).unwrap();assert_eq!(records.len(),2);assert_eq!(records[0].resource.id,old);assert_ne!(records[0].resource.id,records[1].resource.id);service.verify_migration(&run).unwrap();
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    }
    #[test]fn damaged_verified_candidate_is_invalidated_and_rollback_preserves_source(){
        let root=temp();let source=valid_scene();let mut service=ResourceService::open(&root).unwrap();let run=service.prepare_migration(source.clone()).unwrap();let records=service.copy_migration(&run).unwrap();service.verify_migration(&run).unwrap();
        let blob=root.join("blobs").join(&records[0].resource.blob_id);let original=std::fs::read(&blob).unwrap();std::fs::write(&blob,b"damage").unwrap();
        assert!(service.verify_migration(&run).is_err());
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM migration_candidates",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        let status:(String,String)=service.store.connection.query_row("SELECT state,last_error FROM migration_runs",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(status.0,"copying");assert!(status.1.contains("integrity"));
        std::fs::write(&blob,original).unwrap();service.verify_migration(&run).unwrap();assert_eq!(service.rollback_prepared_migration(&run).unwrap(),source);assert!(blob.exists());
        let mut updated=source.clone();updated["items"][0]["x"]=json!(999);let newer=service.prepare_migration(updated.clone()).unwrap();assert_ne!(run,newer);service.copy_migration(&newer).unwrap();assert_eq!(service.verify_migration(&newer).unwrap()["items"][0]["x"],999);assert_eq!(service.rollback_prepared_migration(&run).unwrap(),source);
    }
    #[test]fn persisted_mapping_is_atomic_and_repeated_copy_is_idempotent(){
        let root=std::env::temp_dir().join(format!("quickgen-migration-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut s=scene();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        s["items"][0]["src"]=json!(format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())));
        let mut service=ResourceService::open(&root).unwrap();let id=service.prepare_migration(s.clone()).unwrap();
        assert_eq!(service.prepare_migration(s).unwrap(),id);
        service.store.connection.execute_batch("CREATE TRIGGER fail_mapping BEFORE INSERT ON migration_id_maps BEGIN SELECT RAISE(ABORT,'injected map failure'); END;").unwrap();
        assert!(service.copy_migration(&id).is_err());
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        service.store.connection.execute_batch("DROP TRIGGER fail_mapping;").unwrap();
        let first=service.copy_migration(&id).unwrap();drop(service);
        let mut service=ResourceService::open(&root).unwrap();let again=service.copy_migration(&id).unwrap();assert_eq!(first[0].resource.id,again[0].resource.id);
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(service.store.connection.query_row("SELECT state FROM migration_runs",[],|r|r.get::<_,String>(0)).unwrap(),"copying");
        assert!(!service.can_collect(&first[0].resource.id).unwrap());
    }
    #[test]fn verified_candidate_preserves_history_positions_and_output_provenance(){
        let root=std::env::temp_dir().join(format!("quickgen-verify-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut source=scene();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,4).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        source["items"][0]["src"]=json!(format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())));
        let input=source["items"][0].clone();let mut output=input.clone();output["id"]=json!("result-1");output["result"]=json!(true);output["x"]=json!(240);
        source["items"].as_array_mut().unwrap().push(output);
        source["jobs"]=json!([{"id":1,"input":input,"state":"done","preset":"clay","abilitySnapshot":{"name":"旧卡片","prompt":"旧提示"},"x":240,"y":3},{"id":2,"input":input,"state":"failed","preset":"clay","abilitySnapshot":{"name":"旧卡片","prompt":"旧提示"},"x":400,"y":3}]);
        let mut service=ResourceService::open(&root).unwrap();let id=service.prepare_migration(source.clone()).unwrap();
        assert!(service.verify_migration(&id).is_err());service.copy_migration(&id).unwrap();let candidate=service.verify_migration(&id).unwrap();
        assert_eq!(candidate["items"][1]["x"],240);assert_eq!(candidate["selection"],source["selection"]);assert_eq!(candidate["jobs"][1]["state"],"failed");assert!(candidate["items"][0].get("src").is_none());
        let output=candidate["items"][1]["resourceId"].as_str().unwrap();let (record,_)=service.read(output).unwrap();assert_eq!(record.resource.provenance.task_id.as_deref(),Some("legacy-1"));assert_eq!(record.resource.provenance.parent_ids[0],candidate["jobs"][0]["input"]["resourceId"].as_str().unwrap());
        assert_eq!(service.store.connection.query_row("SELECT state FROM migration_runs WHERE migration_id=?1",[&id],|r|r.get::<_,String>(0)).unwrap(),"verified");
        let saved=service.store.connection.query_row("SELECT snapshot_json FROM migration_runs WHERE migration_id=?1",[&id],|r|r.get::<_,String>(0)).unwrap();assert_eq!(serde_json::from_str::<Value>(&saved).unwrap(),source);
        drop(service);let mut service=ResourceService::open(&root).unwrap();assert_eq!(service.verify_migration(&id).unwrap(),candidate);
    }
    #[test]fn stable_inventory_preserves_every_source_field(){let s=scene();let a=MigrationInventory::prepare(s.clone()).unwrap();let b=MigrationInventory::prepare(s.clone()).unwrap();assert_eq!(a.source_digest,b.source_digest);assert_eq!(a.snapshot,s);assert_eq!(a.images[0].display["custom"],"preserve");}
    #[test]fn queued_history_is_interrupted_without_mutating_source(){let mut s=scene();s["jobs"]=json!([{"id":1,"input":s["items"][0],"state":"running","preset":"clay","abilitySnapshot":{"name":"旧能力","prompt":"旧指令"}}]);let a=MigrationInventory::prepare(s.clone()).unwrap();assert_eq!(a.historical_jobs[0]["state"],"interrupted");assert_eq!(a.snapshot,s);assert_eq!(a.images.len(),2);}
    #[test]fn dangling_and_duplicate_items_block_preparation(){let mut s=scene();s["selection"]=json!(["gone"]);assert!(MigrationInventory::prepare(s).is_err());let mut s=scene();let item=s["items"][0].clone();s["items"].as_array_mut().unwrap().push(item);assert!(MigrationInventory::prepare(s).is_err());}
}
