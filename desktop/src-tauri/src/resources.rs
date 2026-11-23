//! Bounded image intake. All IO and decoding belongs on the service worker.
use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use std::{fs::File, io::{Cursor, Read}, path::Path};
use std::{fs, io::Write, path::PathBuf};
use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};
use crate::{store::Store, contracts::{Resource, Media, Provenance}};
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 32_000_000;
pub const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 16_384;
fn check_dimensions(width:u32,height:u32,bytes:u64)->Result<(),String> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION || width as u64 * height as u64 > MAX_PIXELS || bytes > MAX_DECODE_BYTES {
        Err("image pixel/decode limit exceeded".into())
    } else { Ok(()) }
}

pub struct ValidatedImage { pub bytes: Vec<u8>, pub width: u32, pub height: u32, pub mime: &'static str }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImage {
    pub resource: Resource,
    pub display_name: String,
    pub imported_at_ms: u64,
    pub digest: String,
}

/// Owned by one service worker. No caller-supplied blob path is accepted.
pub struct ResourceService { pub(crate) store: Store, pub(crate) root: PathBuf, pub(crate) drag_cleanup_cursor:usize, _directory_guards: Vec<File>, _writer_lock: File, _database_guard: File }
fn guard_database(path:&Path)->Result<File,String> {
    let mut options=fs::OpenOptions::new(); options.read(true).write(true).create(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3).custom_flags(0x00200000);
    }
    let file=options.open(path).map_err(|e|e.to_string())?;
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if file.metadata().map_err(|e|e.to_string())?.file_attributes() & 0x400 != 0 {return Err("redirected database rejected".into());}
    }
    Ok(file)
}
fn writer_lock(path:&Path)->Result<File,String> {
    let mut options=fs::OpenOptions::new(); options.read(true).write(true).create(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0).custom_flags(0x00200000);
    }
    let file=options.open(path).map_err(|e|format!("resource writer lock: {e}"))?;
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if file.metadata().map_err(|e|e.to_string())?.file_attributes() & 0x400 != 0 {return Err("redirected writer lock".into());}
    }
    file.try_lock().map_err(|e|format!("resource writer lock: {e}"))?;
    Ok(file)
}
#[cfg(test)]
fn archive_checkpoint(point:&str) {
    if std::env::var("QUICKGEN_ARCHIVE_CRASH_POINT").ok().as_deref()==Some(point) {std::process::exit(74);}
}
#[cfg(not(test))]
fn archive_checkpoint(_: &str) {}
#[cfg(windows)]
pub(crate) fn guard_directory(path:&Path)->Result<File,String> {
    use std::os::windows::fs::{OpenOptionsExt, MetadataExt};
    // Open the directory itself, never follow a reparse point; deny delete/rename.
    let file=fs::OpenOptions::new().read(true).share_mode(3).custom_flags(0x02000000|0x00200000).open(path).map_err(|e|e.to_string())?;
    if file.metadata().map_err(|e|e.to_string())?.file_attributes() & 0x400 != 0 {return Err("reparse directory rejected".into());}
    Ok(file)
}
#[cfg(not(windows))]
pub(crate) fn guard_directory(path:&Path)->Result<File,String> {File::open(path).map_err(|e|e.to_string())}
/// Native shell entry points retain this guard while opening a fixed host path.
pub fn guard_export_directory(path:&Path)->Result<File,String>{guard_directory(path)}

pub(crate) fn open_blob(path:&Path)->Result<File,String> {
    let mut options=fs::OpenOptions::new(); options.read(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000);
    }
    let file=options.open(path).map_err(|e|e.to_string())?;
    let metadata=file.metadata().map_err(|e|e.to_string())?;
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {return Err("reparse blob rejected".into());}
    }
    if !metadata.is_file() {return Err("blob is not a regular file".into());}
    Ok(file)
}
#[derive(Debug, Default, Serialize)]
pub struct RecoveryReport {
    pub orphan_blobs: Vec<String>, pub staged_files: Vec<String>,
    pub unavailable_resources: Vec<(String,String)>,
}
impl ResourceService {
    /// Advisory only: no automatic deletion, and callers must recheck at collection.
    pub fn can_collect(&self,id:&str)->Result<bool,String> {
        self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM resources r WHERE r.resource_id=?1 AND r.state='retired' AND NOT EXISTS(SELECT 1 FROM resource_refs ref WHERE ref.resource_id=r.resource_id))",[id],|r|r.get(0)).map_err(|e|e.to_string())
    }
    pub fn retain(&mut self, kind:&str, owner:&str, resource:&str)->Result<(),String> {
        if kind.is_empty() || owner.is_empty() {return Err("reference owner is required".into());}
        self.read(resource)?;
        self.store.connection.execute("INSERT OR IGNORE INTO resource_refs VALUES (?1,?2,?3)",rusqlite::params![kind,owner,resource]).map_err(|e|e.to_string())?; Ok(())
    }
    pub fn release(&mut self,kind:&str,owner:&str,resource:&str)->Result<(),String> {
        self.store.connection.execute("DELETE FROM resource_refs WHERE owner_kind=?1 AND owner_id=?2 AND resource_id=?3",rusqlite::params![kind,owner,resource]).map_err(|e|e.to_string())?; Ok(())
    }
    /// Retiring is explicit and never deletes content. Actual collection is deferred.
    pub fn retire(&mut self,id:&str)->Result<(),String> {
        let n=self.store.connection.execute("UPDATE resources SET state='retired' WHERE resource_id=?1 AND NOT EXISTS(SELECT 1 FROM resource_refs WHERE resource_id=?1)",[id]).map_err(|e|e.to_string())?;
        if n!=1 {return Err("resource absent or still referenced".into());} Ok(())
    }
    /// Run on the same serialized worker as imports. Report only; never auto-delete.
    pub fn recover(&mut self)->Result<RecoveryReport,String> {
        let mut report=RecoveryReport::default();
        for dir in ["blobs","staging"] {
            let base=self.root.join(dir);
            if base.canonicalize().map_err(|e|e.to_string())?!=base {return Err("redirected managed directory".into());}
            for entry in fs::read_dir(&base).map_err(|e|e.to_string())? {
                let entry=entry.map_err(|e|e.to_string())?;
                let key=entry.file_name().to_string_lossy().into_owned();
                if dir=="staging" {report.staged_files.push(key);continue;}
                let count:i64=self.store.connection.query_row("SELECT count(*) FROM blobs WHERE blob_key=?1",[&key],|r|r.get(0)).map_err(|e|e.to_string())?;
                if count==0 {report.orphan_blobs.push(key);}
            }
        }
        let ids:Vec<String>={let mut stmt=self.store.connection.prepare("SELECT resource_id FROM resources WHERE state='available'").map_err(|e|e.to_string())?;
            let rows=stmt.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?;
            rows.collect::<Result<_,_>>().map_err(|e|e.to_string())?};
        for id in ids {if let Err(error)=self.read(&id) {report.unavailable_resources.push((id,error));}}
        Ok(report)
    }
    pub fn open(root: &Path) -> Result<Self,String> {
        fs::create_dir_all(root).map_err(|e|e.to_string())?;
        let root = root.canonicalize().map_err(|e|e.to_string())?;
        let mut guards=vec![guard_directory(&root)?];
        let writer=writer_lock(&root.join("writer.lock"))?;
        // SQLite opens its sidecars itself; reject pre-existing redirects before IO.
        for name in ["metadata.sqlite-journal","metadata.sqlite-wal","metadata.sqlite-shm"] {
            let path=root.join(name);
            if let Ok(metadata)=fs::symlink_metadata(&path) {
                #[cfg(windows)] {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {return Err("redirected SQLite sidecar rejected".into());}
                }
                if !metadata.is_file() {return Err("invalid SQLite sidecar".into());}
            }
        }
        for name in ["blobs", "staging"] {
            fs::create_dir_all(root.join(name)).map_err(|e|e.to_string())?;
            if root.join(name).canonicalize().map_err(|e|e.to_string())? != root.join(name) { return Err("managed directory redirects outside its path".into()); }
            guards.push(guard_directory(&root.join(name))?);
        }
        let database_guard=guard_database(&root.join("metadata.sqlite"))?;
        let store = Store::open(&root.join("metadata.sqlite"))?;
        Ok(Self { store, root, drag_cleanup_cursor:0, _directory_guards:guards, _writer_lock:writer, _database_guard:database_guard })
    }
    pub fn import_file(&mut self, path: &Path) -> Result<StoredImage,String> {
        let image = read_image(path)?;
        self.archive(image, path.file_name().unwrap_or_default().to_string_lossy().into_owned(), Provenance { task_id: None, parent_ids: vec![] })
    }
    pub fn import_batch(&mut self, paths: &[PathBuf]) -> Vec<Result<StoredImage,String>> {
        paths.iter().map(|p|self.import_file(p)).collect()
    }
    pub fn import_bytes(&mut self, bytes: Vec<u8>, extension: &str, name: String, provenance: Provenance) -> Result<StoredImage,String> {
        self.archive(validate_image(bytes,extension)?, name, provenance)
    }
    fn archive(&mut self, image: ValidatedImage, name: String, provenance: Provenance) -> Result<StoredImage,String> {
        self.archive_mapped(image,name,provenance,None)
    }
    pub(crate) fn archive_mapped(&mut self, image: ValidatedImage, name: String, provenance: Provenance, mapping:Option<(&str,&str,&str)>) -> Result<StoredImage,String> {
        self.archive_bound(image,name,provenance,mapping,None)
    }
    pub(crate) fn archive_task_output(&mut self,image:ValidatedImage,task:crate::contracts::Task)->Result<StoredImage,String>{
        let provenance=Provenance{task_id:Some(task.id.clone()),parent_ids:task.request.inputs.iter().map(|r|r.resource_id.clone()).collect()};
        let name=if task.request.capability.connection_id=="internal-mock"{"模拟结果"}else{"生成结果"};
        self.archive_bound(image,name.into(),provenance,None,Some(task))
    }
    fn archive_bound(&mut self, image: ValidatedImage, name: String, provenance: Provenance, mapping:Option<(&str,&str,&str)>,task:Option<crate::contracts::Task>) -> Result<StoredImage,String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?;
        let id = format!("r-{}-{}-{}", std::process::id(), now.as_nanos(), NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
        let blob_id = format!("b-{id}");
        let resource = Resource { schema_version: 1, id, blob_id: blob_id.clone(), mime: image.mime.into(), byte_length: image.bytes.len() as u64, media: Media::Image {width:image.width,height:image.height}, provenance };
        resource.validate().map_err(|e|format!("{e:?}"))?;
        let record = StoredImage {resource, display_name:name, imported_at_ms:now.as_millis() as u64, digest:format!("{:x}",Sha256::digest(&image.bytes))};
        let temp = self.root.join("staging").join(&blob_id);
        let target = self.root.join("blobs").join(&blob_id);
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
        file.write_all(&image.bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?; drop(file);
        archive_checkpoint("staged");
        // hard_link publishes without replacing an existing immutable blob.
        fs::hard_link(&temp,&target).map_err(|e|e.to_string())?;
        archive_checkpoint("published");
        fs::remove_file(&temp).map_err(|e|e.to_string())?;
        let json = serde_json::to_string(&record).map_err(|e|e.to_string())?;
        let tx = self.store.connection.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO blobs VALUES (?1,?2,?3)", rusqlite::params![blob_id,record.digest,record.resource.byte_length as i64]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO resources VALUES (?1,?2,?3,'available')",rusqlite::params![record.resource.id,record.resource.blob_id,json]).map_err(|e|e.to_string())?;
        if let Some((run,key,digest))=mapping {
            tx.execute("INSERT INTO migration_id_maps VALUES (?1,?2,?3,?4)",rusqlite::params![run,key,digest,record.resource.id]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO resource_refs VALUES ('migration',?1,?2)",rusqlite::params![format!("{run}:{key}"),record.resource.id]).map_err(|e|e.to_string())?;
        }
        if let Some(mut task)=task {
            use crate::contracts::{TaskState,ResourceRef,TaskResult};
            task.state.transition(TaskState::Succeeded,true).map_err(|e|format!("{e:?}"))?;
            task.outputs=vec![ResourceRef{role:"result".into(),resource_id:record.resource.id.clone()}];
            TaskResult{schema_version:1,outputs:task.outputs.clone(),summary:Some(record.display_name.clone())}.validate(&task.capability_snapshot,&std::collections::BTreeMap::from([(record.resource.id.clone(),record.resource.clone())]),&std::collections::BTreeSet::from([crate::contracts::ResourceKind::Image])).map_err(|e|format!("{e:?}"))?;
            task.state=TaskState::Succeeded;task.event_version+=1;task.failure=None;
            let updated=tx.execute("UPDATE attempts SET state='succeeded',event_version=?3 WHERE task_id=?1 AND attempt=?2 AND state='finalizing'",rusqlite::params![task.id,task.attempt,task.event_version as i64]).map_err(|e|e.to_string())?;
            if updated!=1{return Err("task no longer finalizing".into());}
            tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO resource_refs VALUES ('task_output',?1,?2)",rusqlite::params![format!("{}:{}",task.id,task.attempt),record.resource.id]).map_err(|e|e.to_string())?;
            tx.execute("DELETE FROM task_failures WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt]).map_err(|e|e.to_string())?;
            tx.execute("DELETE FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt]).map_err(|e|e.to_string())?;
        }
        archive_checkpoint("before_commit");
        tx.commit().map_err(|e|e.to_string())?;
        archive_checkpoint("committed");
        Ok(record)
    }
    pub fn read(&self, id: &str) -> Result<(StoredImage,Vec<u8>),String> {
        self.read_stored(id,true)
    }
    pub(crate) fn read_stored(&self,id:&str,available_only:bool)->Result<(StoredImage,Vec<u8>),String>{
        let json: String = self.store.connection.query_row("SELECT metadata_json FROM resources WHERE resource_id=?1 AND (?2=0 OR state='available')",rusqlite::params![id,available_only],|r|r.get(0)).map_err(|e|e.to_string())?;
        let record: StoredImage = serde_json::from_str(&json).map_err(|e|e.to_string())?;
        record.resource.validate().map_err(|e|format!("{e:?}"))?;
        let key = &record.resource.blob_id;
        if !key.starts_with("b-r-") || !key.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-') {return Err("invalid blob key".into());}
        let path = self.root.join("blobs").join(key);
        if path.canonicalize().map_err(|e|e.to_string())? != path {return Err("redirected blob rejected".into());}
        let file = open_blob(&path)?;
        let mut bytes=Vec::new(); file.take(MAX_BYTES+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        if bytes.len() as u64 != record.resource.byte_length || format!("{:x}",Sha256::digest(&bytes)) != record.digest {return Err(format!("resource {id}: blob integrity mismatch"));}
        Ok((record,bytes))
    }
}

pub fn read_image(path: &Path) -> Result<ValidatedImage, String> {
    read_image_checked(path, || {})
}
fn read_image_checked(path: &Path, after_open: impl FnOnce()) -> Result<ValidatedImage, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() > MAX_BYTES { return Err("image exceeds 20 MiB or is not a file".into()); }
    after_open();
    let mut bytes = Vec::new();
    (&file).take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() || bytes.len() as u64 != after.len() {
        return Err("source changed during import".into());
    }
    validate_image(bytes, path.extension().and_then(|x| x.to_str()).unwrap_or(""))
}

pub fn validate_image(bytes: Vec<u8>, extension: &str) -> Result<ValidatedImage, String> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES {return Err("image exceeds 20 MiB or is empty".into());}
    let (width,height,mime)=crate::decode_worker::validate(&bytes,extension)?;
    let mime=match mime.as_str(){"image/png"=>"image/png","image/jpeg"=>"image/jpeg","image/webp"=>"image/webp",_=>return Err("invalid decoder MIME".into())};
    check_dimensions(width,height,0)?;
    Ok(ValidatedImage{bytes,width,height,mime})
}
pub(crate) fn decode_local(bytes: Vec<u8>, extension: &str) -> Result<ValidatedImage, String> {
    decode_pixels_local(bytes,extension).map(|(record,_)|record)
}
/// Only called within the budgeted decoder process for production clipboard output.
pub(crate) fn decode_dib_local(bytes: Vec<u8>,extension:&str)->Result<Vec<u8>,String>{
    let (record,pixels)=decode_pixels_local(bytes,extension)?;
    crate::clipboard_pixels::rgba_to_dibv5(record.width,record.height,pixels.to_rgba8().as_raw()).map_err(String::from)
}
fn decode_pixels_local(bytes: Vec<u8>, extension: &str) -> Result<(ValidatedImage,image::DynamicImage), String> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES { return Err("image exceeds 20 MiB or is empty".into()); }
    let format = image::guess_format(&bytes).map_err(|e| e.to_string())?;
    let mime = match (format, extension.to_ascii_lowercase().as_str()) {
        (ImageFormat::Png, "png") => "image/png",
        (ImageFormat::Jpeg, "jpg" | "jpeg") => "image/jpeg",
        (ImageFormat::WebP, "webp") => "image/webp",
        _ => return Err("image content and PNG/JPEG/WebP extension must match".into()),
    };
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION); limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES); reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width,height,decoder.total_bytes())?;
    // Actually consume the decoder; valid headers alone are not sufficient.
    let pixels=image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    Ok((ValidatedImage { bytes, width, height, mime },pixels))
}

#[cfg(test)] mod tests {
    use super::*;
    fn encoded(format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(3, 2).write_to(&mut out, format).unwrap(); out.into_inner()
    }
    #[test] fn clipboard_decode_reuses_validation_and_pixels() {
        let bytes=encoded(ImageFormat::Png);
        let dib=decode_dib_local(bytes.clone(),"png").unwrap();
        assert_eq!(dib.len(),124+3*2*4);
        assert_eq!(&dib[4..8],&3u32.to_le_bytes());
        assert_eq!(&dib[8..12],&(-2i32).to_le_bytes());
        assert!(decode_dib_local(bytes,"jpg").is_err());
        assert!(decode_dib_local(vec![0;16],"png").is_err());
    }
    #[test] fn decode_supported_formats_and_reject_false_extension() {
        for (format, ext) in [(ImageFormat::Png,"png"),(ImageFormat::Jpeg,"jpg"),(ImageFormat::WebP,"webp")] {
            let bytes = encoded(format); let img = validate_image(bytes.clone(), ext).unwrap();
            assert_eq!((img.width,img.height),(3,2));
            assert!(validate_image(bytes, "gif").is_err());
        }
    }
    #[test] fn damaged_body_and_oversize_rejected() {
        let mut bytes = encoded(ImageFormat::Png); bytes.truncate(bytes.len()/2);
        assert!(validate_image(bytes, "png").is_err());
        assert!(validate_image(vec![0;MAX_BYTES as usize+1], "png").is_err());
    }
    fn isolated() -> PathBuf {
        static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let serial=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("quickgen-resource-{}-{}-{serial}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&p).unwrap(); p
    }
    #[test] fn single_writer_released_on_close() {
        let dir=isolated(); let service=ResourceService::open(&dir).unwrap();
        assert!(ResourceService::open(&dir).is_err()); drop(service);
        assert!(ResourceService::open(&dir).is_ok());
    }
    #[cfg(windows)]
    #[test] fn live_managed_directories_cannot_be_replaced() {
        let dir=isolated(); let service=ResourceService::open(&dir).unwrap();
        assert!(fs::rename(dir.join("blobs"),dir.join("replaced")).is_err());
        assert!(fs::rename(dir.join("metadata.sqlite"),dir.join("replaced.sqlite")).is_err());
        drop(service);
        fs::rename(dir.join("blobs"),dir.join("replaced")).unwrap();
    }
    #[cfg(windows)]
    #[test] fn junction_managed_directory_is_rejected() {
        let dir=isolated(); let outside=isolated();
        fs::write(outside.join("keep"),b"unchanged").unwrap();
        let status=std::process::Command::new("cmd.exe").args(["/d","/c","mklink","/J"])
            .arg(dir.join("blobs")).arg(&outside).output().unwrap();
        assert!(status.status.success(),"{}",String::from_utf8_lossy(&status.stderr));
        assert!(ResourceService::open(&dir).is_err());
        assert_eq!(fs::read(outside.join("keep")).unwrap(),b"unchanged");
        assert!(!outside.join("metadata.sqlite").exists());
    }
    #[test] fn archive_process_crashes_recover_without_losing_saved_resource() {
        if let Ok(dir)=std::env::var("QUICKGEN_ARCHIVE_CRASH_DIR") {
            let mut service=ResourceService::open(Path::new(&dir)).unwrap();
            service.import_bytes(encoded(ImageFormat::Png),"png","crash".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
            panic!("crash checkpoint was not reached");
        }
        for point in ["staged","published","before_commit","committed"] {
            let dir=isolated();
            let saved={let mut service=ResourceService::open(&dir).unwrap();
                service.import_bytes(encoded(ImageFormat::Png),"png","saved".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap()};
            let status=std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact","resources::tests::archive_process_crashes_recover_without_losing_saved_resource","--nocapture"])
                .env("QUICKGEN_ARCHIVE_CRASH_DIR",&dir).env("QUICKGEN_ARCHIVE_CRASH_POINT",point).status().unwrap();
            assert_eq!(status.code(),Some(74));
            let mut service=ResourceService::open(&dir).unwrap();
            assert!(service.read(&saved.resource.id).is_ok());
            let report=service.recover().unwrap(); assert!(report.unavailable_resources.is_empty());
            let count:i64=service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get(0)).unwrap();
            assert_eq!(count,if point=="committed" {2}else{1});
            assert_eq!(report.staged_files.len(),if ["staged","published"].contains(&point){1}else{0});
            assert_eq!(report.orphan_blobs.len(),if ["published","before_commit"].contains(&point){1}else{0});
        }
    }
    #[test] fn batch_archive_survives_source_removal_and_reopen() {
        let p = isolated(); let source = p.join("original.png"); let bad = p.join("bad.png");
        let bytes = encoded(ImageFormat::Png); fs::write(&source,&bytes).unwrap(); fs::write(&bad,b"broken").unwrap();
        let mut service = ResourceService::open(&p.join("managed")).unwrap();
        let batch = service.import_batch(&[source.clone(),bad,source.clone()]);
        assert!(batch[1].is_err());
        let first = batch[0].as_ref().unwrap(); let second = batch[2].as_ref().unwrap();
        assert_ne!(first.resource.id,second.resource.id);
        assert_eq!(first.display_name,"original.png");
        fs::rename(source,p.join("moved.png")).unwrap(); drop(service);
        let service = ResourceService::open(&p.join("managed")).unwrap();
        assert_eq!(service.read(&first.resource.id).unwrap().1,bytes);
        assert_eq!(service.read(&second.resource.id).unwrap().1,bytes);
        assert!(service.read("../../moved.png").is_err());
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    }
    #[test] fn archive_sql_failure_leaves_no_available_resource() {
        let p=isolated(); let mut service=ResourceService::open(&p).unwrap();
        service.store.connection.execute_batch("CREATE TRIGGER reject_resource BEFORE INSERT ON resources BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        assert!(service.import_bytes(encoded(ImageFormat::Png),"png","test".into(),Provenance{task_id:None,parent_ids:vec![]}).is_err());
        assert_eq!(service.store.connection.query_row("SELECT count(*) FROM blobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(fs::read_dir(p.join("blobs")).unwrap().count(),1);
    }
    #[test] fn exact_limits_and_oversized_encoded_dimensions() {
        assert!(check_dimensions(8000,4000,MAX_DECODE_BYTES).is_ok());
        assert!(check_dimensions(8000,4001,100).is_err());
        assert!(check_dimensions(1,1,MAX_DECODE_BYTES+1).is_err());
        assert!(check_dimensions(MAX_DIMENSION,1,100).is_ok());
        assert!(check_dimensions(MAX_DIMENSION+1,1,100).is_err());
        let mut out=Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(MAX_DIMENSION+1,1).write_to(&mut out,ImageFormat::Png).unwrap();
        assert!(validate_image(out.into_inner(),"png").is_err());
        let mut exact=encoded(ImageFormat::Png); exact.resize(MAX_BYTES as usize,0);
        assert!(validate_image(exact.clone(),"png").is_ok());
        exact.push(0); assert!(validate_image(exact,"png").is_err());
    }
    #[test] fn changed_source_rejected_before_archive() {
        let dir=isolated(); let p=dir.join("changing.png"); fs::write(&p,encoded(ImageFormat::Png)).unwrap();
        let result=read_image_checked(&p,|| {fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"changed").unwrap();});
        assert_eq!(result.err().unwrap(),"source changed during import");
    }
    #[test] fn references_block_retirement_and_recovery_preserves_evidence() {
        let dir=isolated(); let mut service=ResourceService::open(&dir).unwrap();
        let record=service.import_bytes(encoded(ImageFormat::Png),"png","sample".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
        let id=&record.resource.id;
        assert!(!service.can_collect(id).unwrap());
        service.retain("task","job1",id).unwrap(); service.retain("scene","item1",id).unwrap();
        assert!(service.retire(id).is_err()); service.release("scene","item1",id).unwrap();
        assert!(!service.can_collect(id).unwrap());
        assert!(service.retire(id).is_err());
        fs::write(dir.join("staging/interrupted"),b"partial").unwrap();
        fs::write(dir.join("blobs/orphan"),b"uncommitted").unwrap();
        let blob=dir.join("blobs").join(&record.resource.blob_id);
        fs::rename(&blob,dir.join("missing-backup")).unwrap();
        let report=service.recover().unwrap(); assert_eq!(report.orphan_blobs,vec!["orphan"]);
        assert_eq!(report.staged_files,vec!["interrupted"]); assert_eq!(report.unavailable_resources.len(),1);
        assert_eq!(report.unavailable_resources[0].0,*id);
        fs::rename(dir.join("missing-backup"),&blob).unwrap();
        assert!(service.read(id).is_ok()); service.release("task","job1",id).unwrap(); service.retire(id).unwrap();
        assert!(blob.exists()); assert!(service.read(id).is_err());
        assert!(service.can_collect(id).unwrap()); assert!(!service.can_collect("unknown").unwrap());
    }
}
