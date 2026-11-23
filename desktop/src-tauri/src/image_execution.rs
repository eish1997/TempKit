//! One host-owned network worker. No retries and no frontend credentials.
//! Starting work requires a separate host authorization/fee gate.
use crate::{contracts::Task,gateway_discovery::DeliveryFailure,resources::{ResourceService,ValidatedImage}};
type Outcome=Result<ValidatedImage,DeliveryFailure>;
fn download_received(root:&std::path::Path,task:&Task,remote:&str,pending:u64)->Outcome{
 let result=(||{
  crate::image_receipt::save(root,task,remote,pending)?;
  let bytes=if received_path(root,task).exists(){read_received(&received_path(root,task))?}else{
   #[cfg(test)]
   let fixture=std::fs::read_to_string(root.join("test-image-download-address")).ok();
   #[cfg(test)]
   let bytes=match fixture{Some(address)=>crate::gateway_discovery::download_fixture(&address)?,None=>crate::gateway_discovery::download_image(remote)?};
   #[cfg(all(not(test),feature="desktop-diagnostics"))]
   let bytes=match crate::generation_fixture::download(root,remote){Some(result)=>result?,None=>crate::gateway_discovery::download_image(remote)?};
   #[cfg(all(not(test),not(feature="desktop-diagnostics")))]
   let bytes=crate::gateway_discovery::download_image(remote)?;
   bytes
  };
  // Keep successfully downloaded bytes in memory if disk saving fails; poll
  // retries that local save, without downloading or generating again.
  let _=persist_received(root,task,&bytes,pending);
  Ok(bytes)
 })();
 result.map_err(|message|DeliveryFailure{message,acceptance_unknown:true}).and_then(|bytes|receive(crate::image_protocol::ImageOutput::Inline(bytes)))
}
struct Work{archive:bool,task:Task,thread:Option<std::thread::JoinHandle<(Outcome,Option<String>)>>,saved:Option<Outcome>,remote:Option<String>,save_failed:bool}
#[derive(Default)]
pub struct ImageExecution{work:Option<Work>}
#[derive(serde::Serialize)]
pub struct ImageWorkStatus{pub task_id:String,pub attempt:u32,pub event_version:u64,pub save_failed:bool}
impl ImageExecution{
 pub fn busy(&self)->bool{self.work.is_some()}
 pub fn needs_poll(&self)->bool{self.work.as_ref().is_some_and(|work|!work.save_failed)}
 /// Exit can preserve a paused result only when the exact current task has a
 /// verified durable recovery source. Mere file existence is insufficient.
 pub fn check_shutdown(&self,service:&ResourceService)->Result<(),String>{
  let Some(work)=self.work.as_ref()else{return Ok(())};
  if !work.save_failed||work.thread.is_some(){return Err("图片任务仍在接收结果，请稍后重试退出".into())}
  if !service.task_snapshot()?.iter().any(|task|task==&work.task){return Err("图片任务版本已变化，请同步后重试退出".into())}
  if let Some(Ok(image))=&work.saved{
   let saved=if work.archive{service.pending_image_archive(&work.task)}else{read_received(&received_path(&service.root,&work.task))};
   if saved.is_ok_and(|bytes|bytes==image.bytes){return Ok(())}
  }else if work.archive{
   if service.pending_image_archive(&work.task).is_ok_and(|bytes|!bytes.is_empty()){return Ok(())}
  }else if let Ok(remote)=crate::image_receipt::read(&service.root,&work.task){
   if work.remote.as_ref().is_none_or(|expected|expected==&remote){return Ok(())}
  }
  Err("图片结果尚无可验证的恢复副本，请重试保存后再退出".into())
 }
 pub fn status(&self)->Option<ImageWorkStatus>{self.work.as_ref().map(|work|ImageWorkStatus{task_id:work.task.id.clone(),attempt:work.task.attempt,event_version:work.task.event_version,save_failed:work.save_failed})}
 /// One FIFO scheduling step. Every start rechecks and atomically reserves cost.
 pub fn start_next(&mut self,service:&mut ResourceService)->Result<bool,String>{
  if self.busy(){return Ok(false)}
  let tasks=service.task_snapshot()?;
  if let Some(task)=tasks.iter().find(|task|task.state==crate::contracts::TaskState::Running&&service.has_received_image(task)){
   let task=task.clone();let path=received_path(&service.root,&task);
   if !path.exists(){
    // A persisted URL is a completed generation receipt. Restart waits for an
    // explicit download/save retry rather than sending another edit request.
    let remote=crate::image_receipt::read(&service.root,&task).ok();
    self.work=Some(Work{archive:false,task,thread:None,saved:Some(Err(DeliveryFailure{message:"图片下载尚未完成，请重试保存".into(),acceptance_unknown:true})),remote,save_failed:true});return Ok(true)
   }
   // Recovery uses the existing single slot, with no HTTP and no new attempt.
   let thread=std::thread::Builder::new().name("quickgen-image-recovery".into()).spawn(move||{
    let outcome=read_received(&path).map_err(|_|DeliveryFailure{message:"Received image recovery failed; saved artifact retained, generation was not repeated".into(),acceptance_unknown:true}).and_then(|bytes|receive(crate::image_protocol::ImageOutput::Inline(bytes)));
    (outcome,None)
   }).map_err(|_|"Image recovery worker could not start")?;
   self.work=Some(Work{archive:false,task,thread:Some(thread),saved:None,remote:None,save_failed:false});return Ok(true)
  }
  if let Some(task)=tasks.iter().find(|task|task.state==crate::contracts::TaskState::Finalizing&&task.request.capability.connection_id!="internal-mock"&&task.failure.is_none()){
   let task=task.clone();let bytes=match service.pending_image_archive(&task){Ok(bytes)=>bytes,Err(error)=>{service.record_archive_failure(&task,error)?;return Ok(true)}};
   #[cfg(test)] let test_root=service.root.clone();
   let spawned=std::thread::Builder::new().name("quickgen-image-archive-decode".into()).spawn(move||{
    #[cfg(test)]{
     let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
     while test_root.join("test-archive-decode-wait").exists()&&std::time::Instant::now()<deadline{std::thread::sleep(std::time::Duration::from_millis(10));}
    }
    (receive(crate::image_protocol::ImageOutput::Inline(bytes)),None)
   });
   match spawned{Ok(thread)=>self.work=Some(Work{archive:true,task,thread:Some(thread),saved:None,remote:None,save_failed:false}),Err(_)=>{service.record_archive_failure(&task,"Archive decoder could not start; received result retained".into())?;}}
   return Ok(true)
  }
  // A rotated credential must be reauthenticated without failing or retrying
  // an already-admitted attempt. Skip waiting connections so other ready work
  // can progress; revoked/missing connections still take the failure path.
  let connections=service.gateway_connections()?;
  let Some(task)=tasks.into_iter().find(|task|task.state==crate::contracts::TaskState::Queued&&task.request.capability.connection_id!="internal-mock"&&!connections.iter().any(|connection|connection.id==task.request.capability.connection_id&&connection.status==crate::contracts::ConnectionStatus::Unverified))else{return Ok(false)};
  if let Err(error)=self.start(service,&task.id){service.fail_queued_image(&task,error)?;}
  Ok(true)
 }
 pub fn retry_task_save(&mut self,service:&mut ResourceService,id:&str,attempt:u32,event_version:u64)->Result<Option<Task>,String>{
  let work=self.work.as_ref().ok_or("no image output awaiting save")?;
  if work.task.id!=id||work.task.attempt!=attempt||work.task.event_version!=event_version{return Err("task_version_conflict".into())}self.retry_save(service)
 }
 /// Called only after authorization; never from the generic mock scheduler.
 pub fn start(&mut self,service:&mut ResourceService,id:&str)->Result<(),String>{
  if self.busy(){return Err("image worker busy or waiting for result save".into())}
  let pending_bytes=service.pending_image_bytes()?;
  let dispatch=service.prepare_image_dispatch(id)?;
  let task=dispatch.task.clone();
  let root=service.root.clone();
  let spawned=std::thread::Builder::new().name("quickgen-image-network".into()).spawn(move||{
   let result=match crate::gateway_discovery::submit_edit(&dispatch.profile,dispatch.secret,&dispatch.body){Ok(output)=>output,Err(error)=>return (Err(error),None)};
   if let crate::image_protocol::ImageOutput::Remote(remote)=result{
    if let Err(message)=crate::gateway_discovery::image_download_url(&remote){return (Err(DeliveryFailure{message,acceptance_unknown:true}),None)}
    let outcome=download_received(&root,&dispatch.task,&remote,pending_bytes);return (outcome,Some(remote))
   }
   // Persist the bounded response before decoding, so a decoder/process crash
   // can recover the same bytes. Disk failure keeps the response in memory;
   // poll must persist it successfully before committing the task.
   if let crate::image_protocol::ImageOutput::Inline(bytes)=&result{let _=persist_received(&root,&dispatch.task,bytes,pending_bytes);}
   (receive(result),None)
  });
  let (thread,saved)=match spawned{
   Ok(thread)=>(Some(thread),None),
   Err(_)=>(None,Some(Err(DeliveryFailure{message:"Image worker could not start; request not sent".into(),acceptance_unknown:false}))),
  };
  self.work=Some(Work{archive:false,task,thread,saved,remote:None,save_failed:false});Ok(())
 }
 /// Nonblocking: a failed save keeps the same bytes in memory and stops polling
 /// storage. It requires retry_save, never another network submission.
 pub fn poll(&mut self,service:&mut ResourceService)->Result<Option<Task>,String>{
  let Some(work)=self.work.as_mut()else{return Ok(None)};
  if work.save_failed{return Ok(None)}
  if let Some(thread)=work.thread.as_ref(){if !thread.is_finished(){return Ok(None)}}
  if let Some(thread)=work.thread.take(){let(outcome,remote)=thread.join().unwrap_or_else(|_|(Err(DeliveryFailure{message:"Image worker stopped; execution outcome unknown".into(),acceptance_unknown:true}),None));work.saved=Some(outcome);work.remote=remote;}
  let outcome=work.saved.as_ref().ok_or("image worker has no result")?;
  if outcome.is_err()&&(work.remote.is_some()||crate::image_receipt::path(&service.root,&work.task).exists()){
   work.save_failed=true;return Err("图片结果下载或保存未完成；请重试保存，不会重新生成".into())
  }
  let result=if work.archive{service.complete_image_archive(&work.task,outcome.as_ref())}else{match outcome{
   Ok(image)=>service.pending_image_bytes().and_then(|pending|persist_received(&service.root,&work.task,&image.bytes,pending)).and_then(|_|service.stage_image_output(&work.task,image)),
   Err(failure)=>service.fail_image_dispatch(&work.task,DeliveryFailure{message:failure.message.clone(),acceptance_unknown:failure.acceptance_unknown}),
  }};
  match result{Ok(task)=>{if work.saved.as_ref().is_some_and(|outcome|outcome.is_ok()){remove_received(&service.root,&work.task);}self.work=None;Ok(Some(task))},Err(error)=>{work.save_failed=true;Err(error)}}
 }
 pub fn retry_save(&mut self,service:&mut ResourceService)->Result<Option<Task>,String>{
  let work=self.work.as_mut().ok_or("no image output awaiting save")?;
  if !work.save_failed{return Err("image output is not waiting for save retry".into())}
  if work.saved.as_ref().is_some_and(|outcome|outcome.is_err())&&(work.remote.is_some()||crate::image_receipt::path(&service.root,&work.task).exists()){
   let remote=match &work.remote{Some(remote)=>remote.clone(),None=>crate::image_receipt::read(&service.root,&work.task)?};
   let root=service.root.clone();let task=work.task.clone();let pending=service.pending_image_bytes()?;
   let thread=std::thread::Builder::new().name("quickgen-image-download-retry".into()).spawn(move||{let result=download_received(&root,&task,&remote,pending);(result,Some(remote))}).map_err(|_|"图片下载重试无法启动")?;
   work.thread=Some(thread);work.saved=None;work.save_failed=false;return Ok(None)
  }
  work.save_failed=false;self.poll(service)
 }
}
// Files contain only output bytes and their checksum. Task identity comes from
// the host's immutable snapshot, never from a path supplied by the WebView.
pub(crate) fn received_path(root:&std::path::Path,task:&Task)->std::path::PathBuf{
 use sha2::{Digest,Sha256};
 let identity=serde_json::to_vec(task).expect("serializable task");
 root.join("staging").join(format!("image-received-{:x}.bin",Sha256::digest(identity)))
}
fn read_received(path:&std::path::Path)->Result<Vec<u8>,String>{
 use std::io::Read;use sha2::{Digest,Sha256};
 let file=crate::resources::open_blob(path)?;
 let mut data=Vec::new();file.take(crate::resources::MAX_BYTES+33).read_to_end(&mut data).map_err(|e|e.to_string())?;
 if data.len()<33||data.len() as u64>crate::resources::MAX_BYTES+32{return Err("invalid received image size".into())}
 if Sha256::digest(&data[32..]).as_slice()!=&data[..32]{return Err("received image checksum mismatch".into())}
 Ok(data.split_off(32))
}
fn persist_received(root:&std::path::Path,task:&Task,bytes:&[u8],pending:u64)->Result<(),String>{
 use std::io::Write;use sha2::{Digest,Sha256};
 if bytes.len() as u64>crate::resources::MAX_BYTES{return Err("received image too large".into())}
 let _guard=crate::resources::guard_directory(&root.join("staging"))?;
 let path=received_path(root,task);
 if path.exists(){check_received_capacity(root,pending,bytes.len() as u64)?;return if read_received(&path)?==bytes{Ok(())}else{Err("received image differs from saved output".into())}}
 // Include the temporary overlap between the received file and SQLite copy.
 check_received_capacity(root,pending,2*bytes.len() as u64+32)?;
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
 let partial=path.with_extension(format!("{stamp}.partial"));
 let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&partial).map_err(|e|e.to_string())?;
 let written=(||{
  file.write_all(&Sha256::digest(bytes))?;
  // Unit-test-only fault after a real partial write, never in desktop builds.
  #[cfg(test)]
  if root.join("test-received-write-failure").exists(){return Err(std::io::Error::from_raw_os_error(112))}
  file.write_all(bytes)?;file.sync_all()
 })();
 drop(file);
 if let Err(error)=written{let _=std::fs::remove_file(&partial);return Err(error.to_string())}
 std::fs::rename(partial,path).map_err(|e|e.to_string())
}
fn remove_received(root:&std::path::Path,task:&Task){
 // Only after the SQLite commit. Failure leaves a harmless recovery artifact.
 let _=std::fs::remove_file(received_path(root,task));
  crate::image_receipt::remove(root,task);
}
pub const IMAGE_STAGING_BUDGET:u64=256*1024*1024;
const DISK_HEADROOM:u64=64*1024*1024;
#[cfg(test)]
thread_local!{static TEST_AVAILABLE_BYTES:std::cell::Cell<Option<u64>>=const{std::cell::Cell::new(None)};}
pub(crate) fn available_disk_bytes(root:&std::path::Path)->Result<u64,String>{
 #[cfg(test)]
 if let Some(value)=TEST_AVAILABLE_BYTES.with(|v|v.get()){return Ok(value)}
 use std::os::windows::ffi::OsStrExt;
 let path:Vec<u16>=root.as_os_str().encode_wide().chain(Some(0)).collect();let mut available=0u64;
 unsafe{windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(windows::core::PCWSTR(path.as_ptr()),Some(&mut available),None,None)}.map_err(|_|"无法确认图片暂存盘的可用空间")?;
 Ok(available)
}
pub(crate) fn require_disk_room(available:u64,additional:u64)->Result<(),String>{
 if additional.checked_add(DISK_HEADROOM).is_none_or(|needed|available<needed){return Err("磁盘可用空间不足；已有结果保留，请释放空间后重试保存".into())}Ok(())
}

pub(crate) fn check_received_capacity(root:&std::path::Path,pending:u64,additional:u64)->Result<(),String>{
 let _guard=crate::resources::guard_directory(&root.join("staging"))?;
 let mut used=pending;let mut count=0usize;
 for entry in std::fs::read_dir(root.join("staging")).map_err(|e|e.to_string())?{
  let entry=entry.map_err(|e|e.to_string())?;
  if !entry.file_name().to_string_lossy().starts_with("image-received-"){continue}
  count+=1;if count>1024{return Err("图片暂存文件数量已达上限；请保留恢复副本后处理残留".into())}
  let file=crate::resources::open_blob(&entry.path())?;
  used=used.checked_add(file.metadata().map_err(|e|e.to_string())?.len()).ok_or("image staging size overflow")?;
 }
 if used.checked_add(additional).filter(|total|*total<=IMAGE_STAGING_BUDGET).is_none(){return Err("图片暂存容量不足；已有结果保留，请先完成保存或处理恢复残留".into())}
 require_disk_room(available_disk_bytes(root)?,additional)
}
impl ResourceService{
 pub(crate) fn pending_image_bytes(&self)->Result<u64,String>{
  let bytes:i64=self.store.connection.query_row("SELECT COALESCE(sum(length(bytes)),0) FROM pending_task_outputs",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  u64::try_from(bytes).map_err(|_|"invalid pending image size".into())
 }
 /// Only called before a new network dispatch, while the single worker is idle.
 /// Unknown/corrupt files and partial crash remnants are retained and counted.
 pub(crate) fn reclaim_received_copies(&self)->Result<usize,String>{
  use rusqlite::OptionalExtension;
  let _guard=crate::resources::guard_directory(&self.root.join("staging"))?;
  let mut query=self.store.connection.prepare("SELECT task_json FROM task_events WHERE json_extract(task_json,'$.state')='running' AND json_extract(task_json,'$.request.capability.connection_id')!='internal-mock'").map_err(|e|e.to_string())?;
  let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;let mut removed=0;
  for row in rows{
   let task:Task=serde_json::from_str(&row.map_err(|e|e.to_string())?).map_err(|_|"invalid task event")?;
   let path=received_path(&self.root,&task);if !path.exists(){continue}
   let bytes=match read_received(&path){Ok(bytes)=>bytes,Err(_)=>continue};
   let pending:Option<Vec<u8>>=self.store.connection.query_row("SELECT bytes FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
   let mut safe=pending.as_ref().is_some_and(|saved|saved==&bytes);
   if !safe{
    let owner=format!("{}:{}",task.id,task.attempt);
    let mut refs=self.store.connection.prepare("SELECT resource_id FROM resource_refs WHERE owner_kind='task_output' AND owner_id=?1").map_err(|e|e.to_string())?;
    for id in refs.query_map([owner],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?{
     if self.read(&id.map_err(|e|e.to_string())?).is_ok_and(|(_,saved)|saved==bytes){safe=true;break}
    }
   }
   if safe&&std::fs::remove_file(path).is_ok(){removed+=1}
  }Ok(removed)
 }
 pub(crate) fn received_image_artifacts(&self)->Result<Vec<(String,Vec<u8>)>,String>{
  use sha2::{Digest,Sha256};
  let mut files=Vec::new();
  for task in self.task_snapshot()?{
   if task.state!=crate::contracts::TaskState::Running||task.request.capability.connection_id=="internal-mock"{continue}
   let receipt=crate::image_receipt::path(&self.root,&task);
   if receipt.exists(){
    use std::io::Read;
    crate::image_receipt::read(&self.root,&task)?;
    let mut encrypted=Vec::new();crate::resources::open_blob(&receipt)?.take(16385).read_to_end(&mut encrypted).map_err(|_|"图片恢复记录不可读取")?;
    files.push((receipt.file_name().unwrap().to_string_lossy().into_owned(),encrypted));
   }
   let path=received_path(&self.root,&task);if !path.exists(){continue}
   let bytes=read_received(&path)?;let mut encoded=Sha256::digest(&bytes).to_vec();encoded.extend_from_slice(&bytes);
   files.push((path.file_name().unwrap().to_string_lossy().into_owned(),encoded));
  }Ok(files)
 }
 pub(crate) fn has_received_image(&self,task:&Task)->bool{
  task.request.capability.connection_id!="internal-mock"&&(received_path(&self.root,task).exists()||crate::image_receipt::path(&self.root,task).exists())
 }

}
fn receive(output:crate::image_protocol::ImageOutput)->Outcome{
 let bytes=match output{
  crate::image_protocol::ImageOutput::Inline(bytes)=>bytes,
  // URL receipt persistence and downloading happen before this decoder.
  crate::image_protocol::ImageOutput::Remote(_)=>return Err(DeliveryFailure{message:"Remote image download is not configured; do not resubmit generation automatically".into(),acceptance_unknown:true}),
 };
 let extension=match image::guess_format(&bytes){Ok(image::ImageFormat::Png)=>"png",Ok(image::ImageFormat::Jpeg)=>"jpg",Ok(image::ImageFormat::WebP)=>"webp",_=>return Err(DeliveryFailure{message:"Provider output is not a supported image".into(),acceptance_unknown:true})};
 crate::resources::validate_image(bytes,extension).map_err(|_|DeliveryFailure{message:"Provider image failed bounded decoding".into(),acceptance_unknown:true})
}
#[cfg(test)]mod tests{
 use super::*;use crate::{contracts::*,gateway_profiles::GatewayProfile,credential_vault::Secret,image_protocol::ImageEditPolicy};
 use std::{collections::BTreeMap,io::{Read,Write},time::{Duration,Instant}};use base64::Engine;
 fn fs_remove_and_restore_receipt_for_shutdown_test(worker:&ImageExecution,host:&ResourceService,path:&std::path::Path){
  let encrypted=std::fs::read(path).unwrap();std::fs::remove_file(path).unwrap();assert!(worker.check_shutdown(host).is_err(),"memory URL alone cannot guarantee recovery");
  std::fs::write(path,b"damaged receipt").unwrap();assert!(worker.check_shutdown(host).is_err(),"corrupt receipt cannot permit exit");std::fs::write(path,encrypted).unwrap();assert!(worker.check_shutdown(host).is_ok());
 }
 #[test]fn slow_network_keeps_host_available_and_save_retry_never_resends(){run_save_recovery(false,false,false)}
 #[test]fn received_image_survives_restart_after_database_save_failure(){run_save_recovery(true,false,false)}
 #[test]fn persistent_database_failure_keeps_recovered_host_and_save_retry_available(){run_save_recovery(true,true,false)}
 fn finish_received_recovery(host:&mut ResourceService){
  let mut worker=ImageExecution::default();assert!(worker.start_next(host).unwrap());
  let deadline=Instant::now()+Duration::from_secs(5);
  loop{if worker.poll(host).unwrap().is_some(){break}assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
 }
 #[test]fn partial_disk_write_failure_preserves_response_and_retry_never_resends(){run_save_recovery(false,false,true)}
 #[test]fn remote_receipt_survives_restart_and_download_retry_never_regenerates(){
  let root=std::env::temp_dir().join(format!("quickgen-url-receipt-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:"http://127.0.0.1:1".into()},Secret::from_bytes(b"synthetic-token".to_vec())).unwrap();host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
  let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec![],qualities:vec![],response_format:None}).unwrap();
  let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(5,4).write_to(&mut png,image::ImageFormat::Png).unwrap();let bytes=png.into_inner();
  let input=host.import_bytes(bytes.clone(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let config=serde_json::json!({"abilities":[{"id":"fixture","name":"fixture","prompt":"fixture edit","imageBinding":binding.capability.reference,"imageParameters":{}}]});
  host.establish_image_budget(1).unwrap();host.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
  let task=host.admit_board_submission(crate::tasks::MockSubmission{submission_id:"url-receipt".into(),batch_id:None,preset_id:"fixture".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&config).unwrap();
  // Simulate the durable dispatch followed by a received URL, without calling
  // any model. Only download GETs below can reach a live loopback socket.
  let dispatch=host.prepare_image_dispatch(&task.id).unwrap();let running=dispatch.task;drop(dispatch.secret);
  let remote="https://file.302.ai/gpt/imgs/fixture.png?signature=synthetic-private";
  crate::image_receipt::save(&root,&running,remote,0).unwrap();crate::image_receipt::save(&root,&running,remote,0).unwrap();
  assert!(crate::image_receipt::save(&root,&running,"https://file.302.ai/other",0).is_err());
  let encrypted=std::fs::read(crate::image_receipt::path(&root,&running)).unwrap();assert!(!encrypted.windows(9).any(|part|part==b"signature"));
  assert_eq!(crate::image_receipt::read(&root,&running).unwrap(),remote);
  let mut other=running.clone();other.event_version+=1;std::fs::write(crate::image_receipt::path(&root,&other),&encrypted).unwrap();assert!(crate::image_receipt::read(&root,&other).is_err());std::fs::remove_file(crate::image_receipt::path(&root,&other)).unwrap();
  drop(host);let mut host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();assert_eq!(host.task_snapshot().unwrap()[0],running);
  let artifacts=host.received_image_artifacts().unwrap();assert_eq!(artifacts.len(),1);assert_eq!(artifacts[0].1,encrypted);
  assert_eq!(host.image_storage_status().unwrap()["protected_files"],1);assert_eq!(host.archive_image_residue().unwrap()["archived"],0);
  let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();std::fs::write(root.join("test-image-download-address"),format!("http://{address}/fixture")).unwrap();
  let output=bytes.clone();let server=std::thread::spawn(move||{
   for success in [false,true]{let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut headers=Vec::new();let mut byte=[0u8;1];while !headers.ends_with(b"\r\n\r\n"){stream.read_exact(&mut byte).unwrap();headers.push(byte[0]);assert!(headers.len()<16384)}
    let request=String::from_utf8(headers).unwrap().to_ascii_lowercase();assert!(request.starts_with("get /fixture http/1.1"));assert!(!request.contains("authorization:"));
    if success{write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",output.len()).unwrap();stream.write_all(&output).unwrap()}else{stream.write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap()}
   }
  });
  let mut worker=ImageExecution::default();assert!(worker.start_next(&mut host).unwrap());assert!(worker.status().unwrap().save_failed);assert!(!worker.needs_poll());
  assert!(worker.check_shutdown(&host).is_ok(),"verified receipt must permit normal exit");
  let receipt_path=crate::image_receipt::path(&root,&running);fs_remove_and_restore_receipt_for_shutdown_test(&worker,&host,&receipt_path);
  assert!(worker.retry_task_save(&mut host,&running.id,running.attempt,running.event_version+1).is_err());
  worker.retry_save(&mut host).unwrap();let deadline=Instant::now()+Duration::from_secs(5);while worker.needs_poll(){let _=worker.poll(&mut host);assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
  assert!(worker.status().unwrap().save_failed);assert_eq!(host.task_snapshot().unwrap()[0],running);assert!(host.note_create().is_ok());
  drop(worker);drop(host);let mut host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();let mut worker=ImageExecution::default();assert!(worker.start_next(&mut host).unwrap());assert!(worker.status().unwrap().save_failed);
  worker.retry_save(&mut host).unwrap();let deadline=Instant::now()+Duration::from_secs(5);while worker.busy(){worker.poll(&mut host).unwrap();assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}server.join().unwrap();
  assert!(!crate::image_receipt::path(&root,&running).exists());finish_received_recovery(&mut host);
  let done=host.task_snapshot().unwrap().remove(0);assert_eq!(done.state,TaskState::Succeeded);assert_eq!(done.attempt,1);assert_eq!(host.image_budget_status().unwrap().reserved_micros,1);
  let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&running.id],|r|r.get(0)).unwrap();assert_eq!(count,1);assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);
 }
 fn run_save_recovery(restart:bool,persistent:bool,disk_fault:bool){
  let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
  let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let bytes=png.into_inner();
  let response=serde_json::json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(&bytes),"url":"https://unused.invalid/must-not-fetch.png"}]}).to_string();
  let (received,wait_received)=std::sync::mpsc::channel();let(release,wait_release)=std::sync::mpsc::channel();let(finished,wait_finished)=std::sync::mpsc::channel();
  let server=std::thread::spawn(move||{
   let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut request=Vec::new();let mut buffer=[0u8;4096];
   loop{let n=stream.read(&mut buffer).unwrap();assert!(n>0);request.extend_from_slice(&buffer[..n]);
    if let Some(end)=request.windows(4).position(|v|v==b"\r\n\r\n"){
     let headers=String::from_utf8_lossy(&request[..end]);let length:usize=headers.lines().find_map(|line|line.to_ascii_lowercase().strip_prefix("content-length:").map(|v|v.trim().parse().unwrap())).unwrap();if request.len()>=end+4+length{break}
    }
   }
   assert!(request.starts_with(b"POST /v1/images/edits HTTP/1.1"));received.send(()).unwrap();wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
   write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
   // Listener remains open while save retry runs: any second request is observed.
   wait_finished.recv_timeout(Duration::from_secs(10)).unwrap();listener.set_nonblocking(true).unwrap();assert!(matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
  });
  static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
  let serial=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
  let root=std::env::temp_dir().join(format!("quickgen-network-save-{}-{}-{serial}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:format!("http://{address}")},Secret::from_bytes(b"synthetic-token".to_vec())).unwrap();host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
  let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec![],qualities:vec![],response_format:Some("b64_json".into())}).unwrap();
  let input=host.import_bytes(bytes.clone(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();let parameters=BTreeMap::from([("prompt".into(),ParameterValue::Text("fixture edit".into()))]);
  let preset=Preset{schema_version:1,id:"preset".into(),name:"fixture".into(),capability:binding.capability.reference.clone(),parameters:parameters.clone(),cover_resource_id:None};
  let config=serde_json::json!({"abilities":[{"id":preset.id,"name":preset.name,"prompt":"fixture edit","imageBinding":preset.capability,"imageParameters":{}}]});
  let submission=crate::tasks::MockSubmission{submission_id:"network-test".into(),batch_id:None,preset_id:"preset".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
  assert!(host.admit_board_submission(submission.clone(),&config).is_err());assert!(host.task_snapshot().unwrap().is_empty());
  host.establish_image_budget(1).unwrap();host.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
  let task=host.admit_board_submission(submission.clone(),&config).unwrap();assert_eq!(host.admit_board_submission(submission.clone(),&config).unwrap(),task);
  let mut second=submission;second.submission_id="queued-over-budget".into();let queued=host.admit_board_submission(second,&config).unwrap();
  let mut worker=ImageExecution::default();
  TEST_AVAILABLE_BYTES.with(|v|v.set(Some(0)));let no_disk=worker.start(&mut host,&task.id);TEST_AVAILABLE_BYTES.with(|v|v.set(None));
  assert!(no_disk.unwrap_err().contains("磁盘可用空间不足"));assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);
  let full=root.join("staging/image-received-capacity.partial");let file=std::fs::File::create(&full).unwrap();file.set_len(256*1024*1024).unwrap();drop(file);
  let blocked=worker.start(&mut host,&task.id);std::fs::remove_file(&full).unwrap();assert!(blocked.is_err(),"full staging must reject before dispatch");
  assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);assert_eq!(host.task_snapshot().unwrap().iter().find(|t|t.id==task.id).unwrap().state,TaskState::Queued);
  assert!(worker.start_next(&mut host).unwrap());wait_received.recv_timeout(Duration::from_secs(5)).unwrap();
  assert!(worker.check_shutdown(&host).is_err(),"in-flight network must not be discarded");
  let running=worker.work.as_ref().unwrap().task.clone();
  let start=Instant::now();assert!(worker.poll(&mut host).unwrap().is_none());assert!(host.note_create().is_ok());assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);assert!(start.elapsed()<Duration::from_secs(1));assert!(worker.start(&mut host,&task.id).is_err());
  if disk_fault{std::fs::write(root.join("test-received-write-failure"),b"isolated test only").unwrap();}
  host.store.connection.execute_batch("CREATE TRIGGER reject_network_save BEFORE INSERT ON pending_task_outputs BEGIN SELECT RAISE(ABORT,'isolated save failure'); END;").unwrap();release.send(()).unwrap();
  let deadline=Instant::now()+Duration::from_secs(5);loop{match worker.poll(&mut host){Err(_)=>break,Ok(None)=>{assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10))},Ok(Some(_))=>panic!("fault was bypassed")}}
  if disk_fault{
   assert!(!received_path(&root,&running).exists());
   assert!(worker.check_shutdown(&host).is_err(),"memory-only result must block exit");
   assert_eq!(std::fs::read_dir(root.join("staging")).unwrap().filter(|entry|entry.as_ref().unwrap().file_name().to_string_lossy().starts_with("image-received-")).count(),0,"failed partial write must not accumulate debris");
   std::fs::remove_file(root.join("test-received-write-failure")).unwrap();
   assert!(worker.retry_save(&mut host).is_err(),"database fault still active after disk fault cleared");
  }
  assert!(worker.busy());assert!(!worker.needs_poll());assert!(worker.status().unwrap().save_failed);assert!(worker.retry_task_save(&mut host,&task.id,99,99).is_err());assert!(worker.poll(&mut host).unwrap().is_none());
  {
   assert!(worker.check_shutdown(&host).is_ok(),"verified image bytes allow recoverable exit");
   let path=received_path(&root,&running);let saved=std::fs::read(&path).unwrap();std::fs::write(&path,b"corrupt").unwrap();assert!(worker.check_shutdown(&host).is_err());std::fs::write(&path,saved).unwrap();
  }
  assert_eq!(host.reclaim_received_copies().unwrap(),0);assert!(received_path(&root,&running).exists(),"uncommitted received output must be protected");
  assert_eq!(host.image_storage_status().unwrap()["protected_files"],1);assert_eq!(host.archive_image_residue().unwrap()["archived"],0);assert!(received_path(&root,&running).exists());
  let file=std::fs::File::create(&full).unwrap();file.set_len(IMAGE_STAGING_BUDGET).unwrap();drop(file);
  assert!(worker.retry_save(&mut host).unwrap_err().contains("容量不足"));assert!(received_path(&root,&running).exists());assert!(!worker.needs_poll());std::fs::remove_file(&full).unwrap();
  if !persistent{host.store.connection.execute_batch("DROP TRIGGER reject_network_save;").unwrap();}
  if restart&&!persistent {
   let backup=host.export_recovery(serde_json::json!({})).unwrap();
   let folder=std::path::PathBuf::from(backup["directory"].as_str().unwrap());
   assert!(backup["manifest"]["files"].as_array().unwrap().iter().any(|v|v["path"].as_str().unwrap().starts_with("staging/image-received-")));
   let mut restored=ResourceService::open(&folder).unwrap();restored.recover_mock_tasks().unwrap();finish_received_recovery(&mut restored);
   assert_eq!(restored.task_snapshot().unwrap().into_iter().find(|t|t.id==task.id).unwrap().state,TaskState::Finalizing);drop(restored);
   drop(worker);drop(host);host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();finish_received_recovery(&mut host);
   assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==task.id).unwrap().state,TaskState::Finalizing);
   worker=ImageExecution::default();
  }else if !persistent{assert_eq!(worker.retry_save(&mut host).unwrap().unwrap().state,TaskState::Finalizing);assert!(!worker.busy());}
  if persistent{
   drop(worker);drop(host);host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();
   assert!(host.note_create().is_ok());assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);
   worker=ImageExecution::default();assert!(worker.start_next(&mut host).unwrap());
   let deadline=Instant::now()+Duration::from_secs(5);loop{match worker.poll(&mut host){Err(_)=>break,Ok(None)=>{assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10))},Ok(Some(_))=>panic!("persistent fault was bypassed")}}
   assert!(worker.status().unwrap().save_failed);assert!(!worker.needs_poll());
   host.store.connection.execute_batch("DROP TRIGGER reject_network_save;").unwrap();
   assert_eq!(worker.retry_save(&mut host).unwrap().unwrap().state,TaskState::Finalizing);
  }
  assert!(!host.tick_mock().unwrap(),"mock scheduler must not synchronously decode network output");
  host.store.connection.execute_batch("CREATE TRIGGER reject_image_archive BEFORE INSERT ON resource_refs WHEN NEW.owner_kind='task_output' BEGIN SELECT RAISE(ABORT,'isolated archive failure'); END;").unwrap();
  let gate=root.join("test-archive-decode-wait");std::fs::write(&gate,b"isolated decoder hold").unwrap();
  let responsive=Instant::now();assert!(worker.start_next(&mut host).unwrap());assert!(worker.busy());let archive_snapshot=worker.work.as_ref().unwrap().task.clone();
  assert!(worker.poll(&mut host).unwrap().is_none());assert!(host.note_create().is_ok());assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);assert!(responsive.elapsed()<Duration::from_secs(1));
  std::fs::remove_file(gate).unwrap();
  let deadline=Instant::now()+Duration::from_secs(5);let failed_archive=loop{if let Some(done)=worker.poll(&mut host).unwrap(){break done}assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));};
  assert_eq!(failed_archive.state,TaskState::Finalizing);assert_eq!(failed_archive.failure.as_ref().unwrap().stage,FailureStage::Archive);assert!(!worker.busy());
  let decoded=receive(crate::image_protocol::ImageOutput::Inline(bytes.clone())).unwrap();assert!(host.complete_image_archive(&archive_snapshot,Ok(&decoded)).is_err(),"late archive completion must not clear newer failure");
  host.store.connection.execute_batch("DROP TRIGGER reject_image_archive;").unwrap();
  let retry=host.control_mock(crate::tasks::TaskControl{task_id:task.id.clone(),attempt:failed_archive.attempt,event_version:failed_archive.event_version,action:crate::tasks::TaskAction::RetryArchive}).unwrap();
  assert_eq!(retry.state,TaskState::Finalizing);assert!(retry.failure.is_none());assert!(retry.event_version>failed_archive.event_version);
  assert!(host.complete_image_archive(&archive_snapshot,Ok(&decoded)).is_err());
  finish_received_recovery(&mut host);
  assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==task.id).unwrap().state,TaskState::Succeeded);
  assert!(worker.start_next(&mut host).unwrap());let rejected=host.task_snapshot().unwrap().into_iter().find(|t|t.id==queued.id).unwrap();assert_eq!(rejected.state,TaskState::Failed);assert!(!rejected.failure.unwrap().outcome_unknown);assert!(!worker.busy());
  drop(host);let mut host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();let done=host.finalize_mock(&task.id).unwrap();assert_eq!(done.state,TaskState::Succeeded);assert_eq!(host.read(&done.outputs[0].resource_id).unwrap().1,bytes);assert_eq!(host.finalize_mock(&task.id).unwrap(),done);
  assert!(!received_path(&root,&running).exists(),"successful save removes its received file");
  // Reproduce a crash/failed unlink after a durable commit, plus an unknown partial.
  persist_received(&root,&running,&bytes,host.pending_image_bytes().unwrap()).unwrap();
  let unknown=root.join("staging/image-received-unknown.partial");std::fs::write(&unknown,b"unassociated crash residue").unwrap();
  assert_eq!(host.reclaim_received_copies().unwrap(),1);assert!(!received_path(&root,&running).exists());assert!(unknown.exists());assert_eq!(host.reclaim_received_copies().unwrap(),0);
  assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);assert_eq!(host.read(&done.outputs[0].resource_id).unwrap().1,bytes);
  let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&task.id],|r|r.get(0)).unwrap();assert_eq!(count,1);assert_eq!(host.image_budget_status().unwrap().reserved_micros,1);finished.send(()).unwrap();server.join().unwrap();
 }
 #[test]fn disk_budget_boundary_and_query_are_checked(){
  assert!(require_disk_room(DISK_HEADROOM+10,10).is_ok());assert!(require_disk_room(DISK_HEADROOM+9,10).is_err());assert!(require_disk_room(u64::MAX,u64::MAX).is_err());
  assert!(available_disk_bytes(&std::env::temp_dir()).is_ok());
 }
 #[test]fn received_artifact_rejects_damage_and_oversize(){
  use sha2::{Digest,Sha256};
  let root=std::env::temp_dir().join(format!("quickgen-received-integrity-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  std::fs::create_dir_all(&root).unwrap();let path=root.join("fixture.bin");
  let bytes=b"test output";let mut encoded=Sha256::digest(bytes).to_vec();encoded.extend_from_slice(bytes);
  std::fs::write(&path,&encoded).unwrap();assert_eq!(read_received(&path).unwrap(),bytes);
  encoded[32]^=1;std::fs::write(&path,encoded).unwrap();assert!(read_received(&path).unwrap_err().contains("checksum"));
  let f=std::fs::File::create(&path).unwrap();f.set_len(crate::resources::MAX_BYTES+33).unwrap();drop(f);assert!(read_received(&path).unwrap_err().contains("size"));
 }
 #[test]fn unsupported_or_invalid_outputs_do_not_trigger_fetch_or_resubmit(){
  assert!(receive(crate::image_protocol::ImageOutput::Remote("https://invalid.example/image".into())).err().unwrap().acceptance_unknown);
  assert!(receive(crate::image_protocol::ImageOutput::Inline(vec![1,2,3])).err().unwrap().acceptance_unknown);
 }
}
