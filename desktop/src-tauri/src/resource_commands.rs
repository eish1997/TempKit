//! Lazy, bounded resource worker. The old board remains authoritative until activation.
use std::sync::{Mutex,mpsc::{self,SyncSender}};
use quickgen_desktop::resources::ResourceService;
use tauri::Manager;
use base64::Engine;
#[cfg(feature="desktop-diagnostics")]
#[path="performance_trace.rs"]mod performance_trace;
type Reply=Result<serde_json::Value,String>;
enum Operation {ImageBudget(Option<u64>),ImageStorage(String),
#[cfg(feature="desktop-diagnostics")] GenerationTest(String),
ReadImageBinding(quickgen_desktop::contracts::CapabilityRef),UnbindImagePreset(String,quickgen_desktop::contracts::CapabilityRef),BindImagePreset(String,String,quickgen_desktop::image_protocol::ImageEditPolicy,quickgen_desktop::contracts::Parameters),ImageStart(String),ImageShutdown,ImageRetrySave(String,u32,u64),GatewayProbe(String,mpsc::Sender<Result<(quickgen_desktop::gateway_profiles::GatewayProfile,quickgen_desktop::credential_vault::Secret,String),String>>),GatewayProbeCheck(String,String,Option<Vec<String>>),GatewayList,GatewayConfigure(quickgen_desktop::gateway_profiles::GatewayProfile,quickgen_desktop::credential_vault::Secret),GatewayRotate(String,String,String,quickgen_desktop::credential_vault::Secret),GatewayRevoke(String),CopyRead(String,mpsc::Sender<Result<(String,Vec<u8>),String>>),PrepareDrag(String,mpsc::Sender<Result<quickgen_desktop::drag_exports::DragCopy,String>>),Notes,NoteCreate,NoteGet(String,bool),NotePatch(String,quickgen_desktop::notes::NotePatch),
#[cfg(feature="desktop-diagnostics")] ReviewFault(bool),
#[cfg(feature="desktop-diagnostics")] NotesFault(String,String,bool),
Read(String),ReadExternal(String),Import(String,String),Prepare(serde_json::Value),Verify(String),Activate(serde_json::Value),Storage,Export,Submit(Vec<quickgen_desktop::tasks::MockSubmission>),Control(quickgen_desktop::tasks::TaskControl),Feed(Option<u64>),Tasks,Scene,SaveScene(u64,serde_json::Value),Place(quickgen_desktop::scenes::OutputPlacement)}
struct ReadRequest {operation:Operation,reply:mpsc::Sender<Reply>,#[cfg(feature="desktop-diagnostics")] queued:Option<std::time::Instant>}
#[cfg(feature="desktop-diagnostics")]
impl Operation {
 fn trace_name(&self)->&'static str {
  match self {
   Self::ImageBudget(..)=>"image_budget",Self::ImageStorage(..)=>"image_storage",Self::GenerationTest(..)=>"fixture",
   Self::ReadImageBinding(..)=>"binding_read",Self::UnbindImagePreset(..)=>"binding_remove",Self::BindImagePreset(..)=>"binding_save",
   Self::ImageStart(..)=>"image_start",Self::ImageShutdown=>"image_shutdown",Self::ImageRetrySave(..)=>"image_retry_save",
   Self::GatewayProbe(..)=>"gateway_probe",Self::GatewayProbeCheck(..)=>"gateway_probe_check",Self::GatewayList=>"gateway_list",
   Self::GatewayConfigure(..)=>"gateway_configure",Self::GatewayRotate(..)=>"gateway_rotate",Self::GatewayRevoke(..)=>"gateway_revoke",
   Self::CopyRead(..)=>"copy_read",Self::PrepareDrag(..)=>"drag_prepare",Self::Notes=>"notes_list",Self::NoteCreate=>"note_create",
   Self::NoteGet(..)=>"note_get",Self::NotePatch(..)=>"note_patch",Self::ReviewFault(..)=>"review_fault",Self::NotesFault(..)=>"notes_fault",
   Self::Read(..)=>"resource_read",Self::ReadExternal(..)=>"external_read",Self::Import(..)=>"resource_import",
   Self::Prepare(..)=>"migration_prepare",Self::Verify(..)=>"migration_verify",Self::Activate(..)=>"migration_activate",
   Self::Storage=>"storage_status",Self::Export=>"recovery_export",Self::Submit(..)=>"task_submit",Self::Control(..)=>"task_control",
   Self::Feed(..)=>"task_feed",Self::Tasks=>"task_snapshot",Self::Scene=>"scene_snapshot",Self::SaveScene(..)=>"scene_save",Self::Place(..)=>"result_place",
  }
 }
}
#[derive(Default)]
pub struct ResourceHost {sender:Mutex<Option<SyncSender<ReadRequest>>>}
// Native file drops use the same bounded worker and full decoder as imports.
// Reading the selected source does not archive it or start any task.
#[tauri::command]
pub async fn read_image(window:tauri::WebviewWindow,path:String)->Reply {
    if window.label()!="main"{return Err("main only".into());}
    dispatch(window,Operation::ReadExternal(path)).await
}
fn read_external(path:&str)->Reply {
    let path=std::path::Path::new(path);
    let image=quickgen_desktop::resources::read_image(path)?;
    Ok(serde_json::json!({"name":path.file_name().unwrap_or_default().to_string_lossy(),"data":format!("data:{};base64,{}",image.mime,base64::engine::general_purpose::STANDARD.encode(image.bytes))}))
}
fn authorized(label:&str,id:&str)->Result<(),String> {
    if label!="main" {return Err("resource access is restricted to the main board".into());}
    if !id.starts_with("r-")||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-') {return Err("invalid resource ID".into());} Ok(())
}
#[tauri::command]
pub async fn resource_read(window:tauri::WebviewWindow,id:String)->Reply {
    authorized(window.label(),&id)?;
    dispatch(window,Operation::Read(id)).await
}
#[tauri::command]
pub async fn migration_prepare(window:tauri::WebviewWindow,snapshot:serde_json::Value)->Reply {
    if window.label()!="main"{return Err("main only".into());}
    dispatch(window,Operation::Prepare(snapshot)).await
}
async fn dispatch(window:tauri::WebviewWindow,operation:Operation)->Reply {
    let host=window.state::<ResourceHost>();
    let (reply,receive)=mpsc::channel();
    {
        let mut sender=host.sender.lock().map_err(|e|e.to_string())?;
        if sender.is_none() {
            let root=crate::runtime_paths::data(window.app_handle())?.join("resource-store");
            let (tx,rx)=mpsc::sync_channel::<ReadRequest>(16);
            let app=window.app_handle().clone();
            std::thread::Builder::new().name("quickgen-resources".into()).spawn(move|| {
                let mut service=ResourceService::open(&root).and_then(|mut service|{service.recover_mock_tasks()?;Ok(service)});
                #[cfg(feature="desktop-diagnostics")] let mut note_read_fault:Option<String>=None;
                let mut scheduling=service.is_ok();
                let mut active=service.is_ok();
                let mut scheduler_error:Option<String>=None;
                let mut image_worker=quickgen_desktop::image_execution::ImageExecution::default();
                let mut image_shutdown=false;
                let mut pending_request=None;
                let mut request_burst=0usize;
                loop {
                    let mut wake=false;
                    let received=if let Some(request)=pending_request.take(){Ok(request)}else if active||image_worker.needs_poll() {rx.recv_timeout(std::time::Duration::from_millis(25))}else{rx.recv().map_err(|_|mpsc::RecvTimeoutError::Disconnected)};
                    let request=match received {
                        Ok(request)=>Some(request),
                        Err(mpsc::RecvTimeoutError::Timeout)=>None,
                        Err(mpsc::RecvTimeoutError::Disconnected)=>break,
                    };
                    if let Some(request)=request {
                    #[cfg(feature="desktop-diagnostics")] let timing={let name=match &request.operation{Operation::NotePatch(..)=>"note",Operation::Feed(..)=>"feed",Operation::Tasks=>"query",_=>"other"};let operation=request.operation.trace_name();performance_trace::record_operation(&format!("{name}.queue"),Some(operation),request.queued);(name,operation,performance_trace::start())};
                    wake=matches!(&request.operation,Operation::Submit(_)|Operation::Control(_)|Operation::ImageRetrySave(..));
                    let result=match &mut service {
                        Ok(service)=>match request.operation {
                            Operation::ImageBudget(limit)=>(||{if let Some(limit)=limit{service.establish_image_budget(limit)?;}serde_json::to_value(service.image_budget_status()?).map_err(|e|e.to_string())})(),
                            Operation::ImageStorage(action)=>match action.as_str(){
                             "status"=>service.image_storage_status(),
                             "drag-status"=>service.drag_storage_status(),
                             "drag-clean"=>service.clean_drag_copies(),
                             "drag-archive"=>service.archive_drag_files(),
                             "clean"|"archive" if image_worker.busy()&&image_worker.needs_poll()=>Err("图片任务正在接收结果，请稍后整理暂存".into()),
                             "clean"=>service.clean_received_copies(),
                             "archive"=>service.archive_image_residue(),
                             _=>Err("unknown storage action".into()),
                            },
                            #[cfg(feature="desktop-diagnostics")]
                            Operation::GenerationTest(action)=>(||{match action.as_str(){
                             "performance"=>Ok(serde_json::json!({"mode":quickgen_desktop::generation_fixture::performance_mode()})),
                             "pause-scheduling" if !image_worker.busy()=>{scheduling=false;Ok(serde_json::json!(null))},
                             "resume-scheduling"=>{scheduling=true;wake=true;Ok(serde_json::json!(null))},
                             "performance-timings"=>Ok(performance_trace::take()),
                             "setup"=>{let reference=quickgen_desktop::generation_fixture::setup(service)?;crate::storage::bind_image_preset(&app,"clay",reference,std::collections::BTreeMap::new())?;quickgen_desktop::generation_fixture::fault(service,true)},
                             "archive-fault"=>quickgen_desktop::generation_fixture::archive_fault(service,true),
                             "archive-clear"=>quickgen_desktop::generation_fixture::archive_fault(service,false),
                             "download-start"=>quickgen_desktop::generation_fixture::start_download(service),
                             "download-clear"=>quickgen_desktop::generation_fixture::clear_download(service),
                             "drag-fixture"=>quickgen_desktop::generation_fixture::drag_fixture(service),
                             "fault"=>quickgen_desktop::generation_fixture::fault(service,true),
                             "clear"=>quickgen_desktop::generation_fixture::fault(service,false),
                             _=>Err("unknown fixture action".into()),
                            }})(),
                            Operation::ReadImageBinding(reference)=>service.image_binding(&reference).and_then(|binding|serde_json::to_value(binding).map_err(|e|e.to_string())),
                            Operation::UnbindImagePreset(id,expected)=>crate::storage::unbind_image_preset(&app,&id,expected).map(|_|serde_json::json!(null)),
                            Operation::BindImagePreset(connection_id,preset_id,policy,parameters)=>{
                             (||{
                              if parameters.keys().any(|key|!matches!(key.as_str(),"size"|"quality")){return Err("Only size and quality may be stored as image options".into())}
                              let config=crate::storage::load_config(app.clone())?;
                              let card=config["abilities"].as_array().ok_or("Invalid abilities")?.iter().find(|card|card["id"].as_str()==Some(&preset_id)).ok_or("预设已不存在")?;
                              let binding=service.register_image_binding(&connection_id,policy)?;
                              let mut task_parameters=parameters.clone();task_parameters.insert("prompt".into(),quickgen_desktop::contracts::ParameterValue::Text(card["prompt"].as_str().ok_or("预设指令无效")?.into()));
                              let preset=quickgen_desktop::contracts::Preset{schema_version:1,id:preset_id.clone(),name:card["name"].as_str().ok_or("预设名称无效")?.into(),capability:binding.capability.reference.clone(),parameters:task_parameters,cover_resource_id:None};
                              preset.validate(&binding.capability).map_err(|_|"预设参数与模型能力不匹配")?;
                              for(key,allowed)in [("size",&binding.policy.sizes),("quality",&binding.policy.qualities)]{if let Some(value)=parameters.get(key){if !matches!(value,quickgen_desktop::contracts::ParameterValue::Text(value) if allowed.contains(value)){return Err("预设参数不在模型配置中".into())}}}
                              crate::storage::bind_image_preset(&app,&preset_id,binding.capability.reference.clone(),parameters)?;
                              serde_json::to_value(binding).map_err(|e|e.to_string())
                             })()
                            },
                            Operation::ImageStart(id)=>if image_shutdown{Err("Image execution is shutting down".into())}else{image_worker.start(service,&id).map(|_|serde_json::json!(null))},
                            Operation::ImageRetrySave(id,attempt,version)=>image_worker.retry_task_save(service,&id,attempt,version).and_then(|task|serde_json::to_value(task).map_err(|e|e.to_string())),
                            Operation::ImageShutdown=>image_worker.check_shutdown(service).map(|_|{image_shutdown=true;serde_json::json!(null)}),
                            Operation::GatewayProbe(id,reply)=>{
                             let result=(||{let secret=service.gateway_secret(&id)?;let connection=service.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;Ok((service.gateway_profile(&connection.profile_id)?,secret,connection.credential_ref.ok_or("credential missing")?))})();
                             reply.send(result).map(|_|serde_json::json!(null)).map_err(|_|"connection requester closed".into())
                            },
                            Operation::GatewayProbeCheck(id,reference,models)=>{let authenticated=models.is_some();service.record_gateway_check_version(&id,&reference,models).map(|_|{if authenticated{wake=true;}serde_json::json!({"active":true})})},
                            Operation::GatewayRotate(id,reference,account,token)=>service.rotate_gateway_credential(&id,&reference,&account,token).and_then(|v|serde_json::to_value(v).map_err(|_|"connection response unavailable".into())),
                            Operation::GatewayList=>service.gateway_connections().and_then(|v|serde_json::to_value(v).map_err(|_|"connection list unavailable".into())),
                            Operation::GatewayConfigure(profile,token)=>service.configure_gateway(profile,token).and_then(|v|serde_json::to_value(v).map_err(|_|"connection response unavailable".into())),
                            Operation::GatewayRevoke(id)=>service.revoke_gateway(&id).map(|_|serde_json::json!({"revoked":true})),
                            Operation::CopyRead(id,reply)=>reply.send(service.read(&id).map(|(record,bytes)|(record.resource.mime,bytes))).map(|_|serde_json::json!(null)).map_err(|_|"copy requester closed".to_string()),
                            Operation::PrepareDrag(id,reply)=>{let result=service.prepare_drag_copy(&id);reply.send(result).map(|_|serde_json::json!(null)).map_err(|_|"drag requester closed".to_string())},
                            #[cfg(feature="desktop-diagnostics")]
                            Operation::ReviewFault(enabled)=>service.review_archive_fault(enabled),
                            #[cfg(feature="desktop-diagnostics")]
                            Operation::NotesFault(id,stage,enabled)=>{
                             if stage=="read"{note_read_fault=if enabled{Some(id)}else{None};Ok(serde_json::json!({"enabled":enabled}))}
                             else if stage=="write"{service.notes_write_fault(&id,enabled)}else{Err("unknown note fault stage".into())}
                            },
                            Operation::Notes=>service.notes_list().and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string())),
                            Operation::NoteCreate=>service.note_create().and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string())),
                            Operation::NoteGet(id,_editor)=>{
                             #[cfg(feature="desktop-diagnostics")]
                             if _editor&&note_read_fault.as_deref()==Some(id.as_str()){Err("isolated note read failure".into())}else{service.note_get(&id).and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string()))}
                             #[cfg(not(feature="desktop-diagnostics"))]
                             service.note_get(&id).and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string()))
                            },
                            Operation::NotePatch(id,patch)=>service.note_patch(&id,patch).and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string())),
                            Operation::ReadExternal(path)=>read_external(&path),
                            Operation::Read(id)=>service.read(&id).map(|(record,bytes)|serde_json::json!({"record":record,"data":format!("data:{};base64,{}",record.resource.mime,base64::engine::general_purpose::STANDARD.encode(bytes))})),
                            Operation::Import(data,name)=>decode_import(&data).and_then(|(bytes,extension)|service.import_bytes(bytes,extension,name,quickgen_desktop::contracts::Provenance{task_id:None,parent_ids:vec![]})).and_then(|record|serde_json::to_value(record).map_err(|e|e.to_string())),
                            Operation::Prepare(mut snapshot)=>crate::storage::migration_backup(&app).and_then(|backup|{
                                let object=snapshot.as_object_mut().ok_or("scene must be an object")?;object.insert("configBackup".into(),backup);
                                service.prepare_migration(snapshot).map(|id|serde_json::json!({"migrationId":id}))
                            }),
                            Operation::Verify(id)=>service.copy_migration(&id).and_then(|_|service.verify_migration(&id)),
                            Operation::Storage=>service.active_scene().map(|scene|serde_json::json!({"scene":scene,"activationAllowed":true,"isolated":crate::runtime_paths::isolated().is_some()})),
                            Operation::Export=>crate::storage::migration_backup(&app).and_then(|config|service.export_recovery(config)),
                            Operation::Activate(mut snapshot)=>crate::storage::migration_backup(&app).and_then(|backup|{
                                snapshot.as_object_mut().ok_or("scene must be an object")?.insert("configBackup".into(),backup);
                                let id=service.prepare_migration(snapshot)?;service.copy_migration(&id)?;service.activate_migration(&id)
                            }),
                            Operation::Submit(submissions)=>crate::storage::load_config(app.clone()).and_then(|config|serde_json::to_value(submissions.into_iter().map(|submission|service.admit_board_submission(submission,&config)).collect::<Vec<_>>()).map_err(|e|e.to_string())),
                            Operation::Tasks=>service.task_snapshot().and_then(|tasks|serde_json::to_value(tasks).map_err(|e|e.to_string())),
                            Operation::Control(command)=>service.control_mock(command).and_then(|task|serde_json::to_value(task).map_err(|e|e.to_string())),
                            Operation::Feed(after)=>service.task_feed(after).and_then(|feed|serde_json::to_value(feed).map_err(|e|e.to_string())).map(|mut feed|{feed["scheduler_error"]=serde_json::json!(scheduler_error);feed["image_work"]=serde_json::json!(image_worker.status());feed}),
                            Operation::Scene=>service.scene_snapshot().map(|scene|serde_json::json!(scene)),
                            Operation::SaveScene(revision,scene)=>service.save_scene(revision,scene),
                            Operation::Place(p)=>service.place_task_output(p.revision,&p.task_id,p.attempt,&p.resource_id,p.x,p.y),
                        },
                        Err(error)=>Err(error.clone()),
                    };
                    #[cfg(feature="desktop-diagnostics")] performance_trace::record_operation(&format!("{}.service",timing.0),Some(timing.1),timing.2);
                    let _=request.reply.send(result);
                    // Preserve FIFO and durable replies, but do not insert a
                    // potentially slow archive commit before every queued UI
                    // request. Bound the burst so completion cannot starve.
                    request_burst+=1;
                    if !wake&&request_burst<4 {
                        if let Ok(next)=rx.try_recv(){pending_request=Some(next);continue}
                    }
                    }
                    request_burst=0;
                    if let Ok(service)=&mut service {
                        // poll never waits for HTTP/decoding; failed saves retain their
                        // payload and are exposed separately from durable task events.
                        #[cfg(feature="desktop-diagnostics")] let poll_time=performance_trace::start();
                        match image_worker.poll(service){Ok(Some(_))=>wake=true,Ok(None)=>{},Err(_)=>{}}
                        #[cfg(feature="desktop-diagnostics")] performance_trace::record("worker.poll",poll_time);
                    }
                    if scheduling&&(active||wake) {
                        if let Ok(service)=&mut service {
                            #[cfg(feature="desktop-diagnostics")] let schedule_time=performance_trace::start();
                            let step=if image_worker.busy(){Ok(false)}else{service.tick_mock().and_then(|mock_work|{
                                if image_shutdown{Ok(mock_work)}else{image_worker.start_next(service).map(|network_work|mock_work||network_work)}
                            })};
                            #[cfg(feature="desktop-diagnostics")] performance_trace::record("worker.schedule",schedule_time);
                            match step{
                                Ok(work)=>active=work,
                                Err(error)=>{
                                // A storage failure needs recovery, never a busy retry loop.
                                eprintln!("mock scheduler stopped: {error}");scheduler_error=Some(error);scheduling=false;active=false;
                                }
                            }
                        }
                    }
                }
            }).map_err(|e|e.to_string())?;
            *sender=Some(tx);
        }
        sender.as_ref().unwrap().try_send(ReadRequest{operation,reply,#[cfg(feature="desktop-diagnostics")] queued:performance_trace::start()}).map_err(|e|format!("resource worker busy or stopped: {e}"))?;
    }
    tauri::async_runtime::spawn_blocking(move||receive.recv().map_err(|e|e.to_string())?).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn recovery_export(window:tauri::WebviewWindow)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Export).await
}
#[tauri::command]
pub async fn storage_status(window:tauri::WebviewWindow)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Storage).await
}
#[tauri::command]
pub async fn migration_activate(window:tauri::WebviewWindow,snapshot:serde_json::Value)->Reply{
    // R4 isolated rehearsal and native acceptance passed. Activation still
    // verifies the original backup and every mapped body before committing.
    if window.label()!="main"{return Err("main only".into());}
    dispatch(window,Operation::Activate(snapshot)).await
}
#[tauri::command]
pub async fn resource_import(window:tauri::WebviewWindow,data:String,name:String)->Reply{
    if window.label()!="main"||data.len()>28*1024*1024||name.trim().is_empty()||name.len()>1024{return Err("invalid image import".into());}
    dispatch(window,Operation::Import(data,name)).await
}
fn decode_import(data:&str)->Result<(Vec<u8>,&'static str),String>{
    let (header,payload)=data.split_once(',').ok_or("invalid image data")?;
    let extension=match header {"data:image/png;base64"=>"png","data:image/jpeg;base64"=>"jpg","data:image/webp;base64"=>"webp",_=>return Err("unsupported image data".into())};
    let bytes=base64::engine::general_purpose::STANDARD.decode(payload).map_err(|e|e.to_string())?;
    if bytes.is_empty()||bytes.len()>20*1024*1024{return Err("image exceeds 20 MiB or is empty".into())}Ok((bytes,extension))
}
#[tauri::command]
pub async fn scene_snapshot(window:tauri::WebviewWindow)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Scene).await
}
#[tauri::command]
pub async fn scene_save(window:tauri::WebviewWindow,revision:u64,scene:serde_json::Value)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::SaveScene(revision,scene)).await
}
#[tauri::command]
pub async fn scene_place_output(window:tauri::WebviewWindow,placement:quickgen_desktop::scenes::OutputPlacement)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Place(placement)).await
}
#[tauri::command]
pub async fn task_submit(window:tauri::WebviewWindow,submissions:Vec<quickgen_desktop::tasks::MockSubmission>)->Reply{
    if window.label()!="main"||submissions.is_empty()||submissions.len()>100{return Err("invalid task batch".into());}
    dispatch(window,Operation::Submit(submissions)).await
}
#[tauri::command]
pub async fn task_feed(window:tauri::WebviewWindow,after:Option<u64>)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Feed(after)).await
}
#[tauri::command]
pub async fn task_control(window:tauri::WebviewWindow,command:quickgen_desktop::tasks::TaskControl)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Control(command)).await
}
#[tauri::command]
pub async fn task_snapshot(window:tauri::WebviewWindow)->Reply{
    if window.label()!="main"{return Err("main only".into());}dispatch(window,Operation::Tasks).await
}
#[tauri::command]
pub async fn migration_verify(window:tauri::WebviewWindow,id:String)->Reply{
    if window.label()!="main"||!id.starts_with("migration-")||id.len()!=74||!id[10..].bytes().all(|b|b.is_ascii_hexdigit()){return Err("invalid migration request".into());}
    dispatch(window,Operation::Verify(id)).await
}
#[cfg(test)] mod tests {
    #[test]fn import_transport_rejects_arbitrary_paths_and_types(){
        for data in ["C:/private.png","https://example.com/image.png","data:image/svg+xml;base64,PHN2Zz4=","data:image/png;base64,","data:image/png;base64,???"]{assert!(super::decode_import(data).is_err());}
        assert_eq!(super::decode_import("data:image/png;base64,AQID").unwrap(),(vec![1,2,3],"png"));
        // Transport parsing is followed by actual decode in ResourceService.
    }
    #[test] fn only_main_and_opaque_ids_allowed() {
        assert!(super::authorized("main","r-123-456").is_ok());
        for id in ["../file","https://example.com","file:///a","r-../secret",""] {assert!(super::authorized("main",id).is_err());}
        assert!(super::authorized("manager","r-123").is_err());
    }
}


#[tauri::command]
pub async fn review_archive_fault(window:tauri::WebviewWindow,enabled:bool)->Reply {
    if window.label()!="main"||crate::runtime_paths::isolated().is_none(){return Err("isolated diagnostic only".into())}
    #[cfg(feature="desktop-diagnostics")]
    {dispatch(window,Operation::ReviewFault(enabled)).await}
    #[cfg(not(feature="desktop-diagnostics"))]
    {let _=enabled;Err("diagnostic feature disabled".into())}
}

fn note_authorized(label:&str,id:&str)->Result<(),String>{
 if !id.starts_with("note-")||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("invalid note ID".into())}
 if label!="main"&&label!=id{return Err("note window identity mismatch".into())}Ok(())
}
#[tauri::command]
pub async fn notes_list(window:tauri::WebviewWindow)->Reply{
 if window.label()!="main"{return Err("main only".into())}dispatch(window,Operation::Notes).await
}
#[tauri::command]
pub async fn note_create(window:tauri::WebviewWindow)->Reply{
 if window.label()!="main"{return Err("main only".into())}dispatch(window,Operation::NoteCreate).await
}
#[tauri::command]
pub async fn note_get(window:tauri::WebviewWindow,id:String)->Reply{
 note_authorized(window.label(),&id)?;let editor=window.label()!="main";dispatch(window,Operation::NoteGet(id,editor)).await
}
#[tauri::command]
pub async fn note_patch(window:tauri::WebviewWindow,id:String,patch:quickgen_desktop::notes::NotePatch)->Reply{
 note_authorized(window.label(),&id)?;let app=window.app_handle().clone();
 #[cfg(feature="desktop-diagnostics")] let dispatch_time=performance_trace::start();
 let value=dispatch(window,Operation::NotePatch(id,patch)).await?;
 #[cfg(feature="desktop-diagnostics")] performance_trace::record("note.dispatch",dispatch_time);
 #[cfg(feature="desktop-diagnostics")] let emit_time=performance_trace::start();
 use tauri::Emitter;let _=app.emit("note-changed",&value);
 #[cfg(feature="desktop-diagnostics")] performance_trace::record("note.emit",emit_time);
 Ok(value)
}
#[cfg(test)]mod note_permissions {
 #[test]fn windows_cannot_edit_other_notes(){
  assert!(super::note_authorized("main","note-a").is_ok());
  assert!(super::note_authorized("note-a","note-a").is_ok());
  for (label,id) in [("note-a","note-b"),("manager","note-a"),("main","../private"),("main","note-a/b")]{assert!(super::note_authorized(label,id).is_err())}
 }
}

#[tauri::command]
pub async fn notes_test_fault(window:tauri::WebviewWindow,id:String,stage:String,enabled:bool)->Reply{
 if window.label()!="main"||crate::runtime_paths::isolated().is_none()||crate::board::diagnostic_mode()!="notes-fault"{return Err("isolated notes fault diagnostic only".into())}
 note_authorized(window.label(),&id)?;
 #[cfg(feature="desktop-diagnostics")]{dispatch(window,Operation::NotesFault(id,stage,enabled)).await}
 #[cfg(not(feature="desktop-diagnostics"))]{let _=(stage,enabled);Err("diagnostic feature disabled".into())}
}


static DRAG_ACTIVE:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub(crate) fn drag_active()->bool{DRAG_ACTIVE.load(std::sync::atomic::Ordering::Acquire)}
struct DragActive;
impl Drop for DragActive{fn drop(&mut self){DRAG_ACTIVE.store(false,std::sync::atomic::Ordering::Release);}}
#[tauri::command]
pub async fn resource_drag_out(window:tauri::WebviewWindow,id:String)->Reply{
 authorized(window.label(),&id)?;
 if DRAG_ACTIVE.swap(true,std::sync::atomic::Ordering::AcqRel){return Err("drag already active".into())}
 let active=DragActive;
 let (send,receive)=mpsc::channel();
 dispatch(window.clone(),Operation::PrepareDrag(id,send)).await?;
 let copy=receive.recv().map_err(|e|e.to_string())??;
 let (finish,wait)=mpsc::channel();let target=window.clone();
 window.run_on_main_thread(move||{
  let _active=active;
  let result=(||->Reply{
   target.hide().map_err(|e|e.to_string())?;
   #[cfg(windows)]let outcome=quickgen_desktop::native_file_drag::drag_export_copy(&copy.path,crate::runtime_paths::isolated().map(|root|root.join("drag-diagnostic.json")).as_deref());
   #[cfg(not(windows))]let outcome:Result<(),String>=Err("native drag requires Windows".into());
   // Restore regardless of whether OLE copied, cancelled, or failed.
   let restored=target.show().map_err(|e|format!("window restoration failed: {e}"));
   restored?;
   outcome.map(|value|serde_json::json!({"outcome":format!("{value:?}")}))
  })();
  drop(copy);let _=finish.send(result);
 }).map_err(|e|e.to_string())?;
 tauri::async_runtime::spawn_blocking(move||wait.recv().map_err(|e|e.to_string())?).await.map_err(|e|e.to_string())?
}


static COPY_ACTIVE:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
struct CopyActive;
impl Drop for CopyActive{fn drop(&mut self){COPY_ACTIVE.store(false,std::sync::atomic::Ordering::Release);}}
#[tauri::command]
pub async fn resource_copy_image(window:tauri::WebviewWindow,id:String)->Reply{
 authorized(window.label(),&id)?;
 if COPY_ACTIVE.swap(true,std::sync::atomic::Ordering::AcqRel){return Err("image copy already active".into())}
 let active=CopyActive;
 let(send,receive)=mpsc::channel();dispatch(window.clone(),Operation::CopyRead(id,send)).await?;
 let(mime,bytes)=receive.recv().map_err(|e|e.to_string())??;
 let extension=match mime.as_str(){"image/png"=>"png","image/jpeg"=>"jpg","image/webp"=>"webp",_=>return Err("unsupported clipboard image".into())};
 let dib=tauri::async_runtime::spawn_blocking(move||quickgen_desktop::decode_worker::clipboard_dib(&bytes,extension)).await.map_err(|e|e.to_string())??;
 let(send,receive)=mpsc::channel();let target=window.clone();
 window.run_on_main_thread(move||{
  let _active=active;
  #[cfg(windows)]let result=target.hwnd().map_err(|e|e.to_string()).and_then(|hwnd|quickgen_desktop::native_clipboard::write_dib(hwnd,&dib));
  #[cfg(not(windows))]let result:Result<(),String>=Err("image clipboard requires Windows".into());
  let _=send.send(result.map(|_|serde_json::json!({"copied":true})));
 }).map_err(|e|e.to_string())?;
 tauri::async_runtime::spawn_blocking(move||receive.recv().map_err(|e|e.to_string())?).await.map_err(|e|e.to_string())?
}

fn gateway_authorized(label:&str)->Result<(),String>{if label=="main"||label=="manager"{Ok(())}else{Err("connection settings are restricted to the main panel".into())}}
#[tauri::command]
pub async fn gateway_connections(window:tauri::WebviewWindow)->Reply{
 gateway_authorized(window.label())?;dispatch(window,Operation::GatewayList).await
}
#[tauri::command]
pub async fn gateway_configure(window:tauri::WebviewWindow,profile:quickgen_desktop::gateway_profiles::GatewayProfile,token:String)->Reply{
 let secret=quickgen_desktop::credential_vault::Secret::from_bytes(token.into_bytes());
 gateway_authorized(window.label())?;
 dispatch(window,Operation::GatewayConfigure(profile,secret)).await
}
#[tauri::command]
pub async fn gateway_rotate(window:tauri::WebviewWindow,id:String,expected_ref:String,account_id:String,token:String)->Reply{
 let secret=quickgen_desktop::credential_vault::Secret::from_bytes(token.into_bytes());
 gateway_authorized(window.label())?;
 dispatch(window,Operation::GatewayRotate(id,expected_ref,account_id,secret)).await
}
#[tauri::command]
pub async fn gateway_revoke(window:tauri::WebviewWindow,id:String)->Reply{
 gateway_authorized(window.label())?;dispatch(window,Operation::GatewayRevoke(id)).await
}

static GATEWAY_PROBE_ACTIVE:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
struct GatewayProbeActive;
impl Drop for GatewayProbeActive{fn drop(&mut self){GATEWAY_PROBE_ACTIVE.store(false,std::sync::atomic::Ordering::Release);}}
#[tauri::command]
pub async fn gateway_probe(window:tauri::WebviewWindow,id:String)->Reply{
 gateway_authorized(window.label())?;
 if GATEWAY_PROBE_ACTIVE.swap(true,std::sync::atomic::Ordering::AcqRel){return Err("connection check already running".into())}
 let _active=GatewayProbeActive;
 let(send,receive)=mpsc::channel();dispatch(window.clone(),Operation::GatewayProbe(id.clone(),send)).await?;
 let(profile,secret,reference)=receive.recv().map_err(|_|"connection worker unavailable")??;
 // No network waits on the resource worker or UI thread.
 let result=tauri::async_runtime::spawn_blocking(move||quickgen_desktop::gateway_discovery::discover(&profile,secret)).await.map_err(|_|"connection check worker failed")?;
 // Revocation during the request invalidates even a successful late response.
 dispatch(window,Operation::GatewayProbeCheck(id,reference,result.as_ref().ok().cloned())).await?;
 result.map(|models|serde_json::json!({"models":models,"authenticated":true,"generationVerified":false}))
}

// Host-only entry: future preset/fee admission must authorize before calling.
// Deliberately not registered as a WebView command.
#[allow(dead_code)]
pub(crate) async fn start_authorized_image(window:tauri::WebviewWindow,id:String)->Reply{
 if window.label()!="main"{return Err("main only".into())}dispatch(window,Operation::ImageStart(id)).await
}
pub(crate) async fn image_shutdown(window:tauri::WebviewWindow)->Reply{
 dispatch(window,Operation::ImageShutdown).await
}
#[tauri::command]
pub async fn image_retry_save(window:tauri::WebviewWindow,id:String,attempt:u32,event_version:u64)->Reply{
 if window.label()!="main"{return Err("main only".into())}dispatch(window,Operation::ImageRetrySave(id,attempt,event_version)).await
}

#[tauri::command]
pub async fn gateway_bind_preset(window:tauri::WebviewWindow,connection_id:String,preset_id:String,policy:quickgen_desktop::image_protocol::ImageEditPolicy,parameters:quickgen_desktop::contracts::Parameters)->Reply{
 gateway_authorized(window.label())?;dispatch(window,Operation::BindImagePreset(connection_id,preset_id,policy,parameters)).await
}

#[tauri::command]
pub async fn gateway_image_binding(window:tauri::WebviewWindow,reference:quickgen_desktop::contracts::CapabilityRef)->Reply{
 gateway_authorized(window.label())?;dispatch(window,Operation::ReadImageBinding(reference)).await
}
#[tauri::command]
pub async fn gateway_unbind_preset(window:tauri::WebviewWindow,preset_id:String,expected:quickgen_desktop::contracts::CapabilityRef)->Reply{
 gateway_authorized(window.label())?;dispatch(window,Operation::UnbindImagePreset(preset_id,expected)).await
}

#[tauri::command]
pub async fn generation_test(window:tauri::WebviewWindow,action:String)->Reply{
 if window.label()!="main"||crate::runtime_paths::isolated().is_none()||crate::board::diagnostic_mode()!="generation-recovery"{return Err("isolated generation diagnostic only".into())}
 #[cfg(feature="desktop-diagnostics")]
 {dispatch(window,Operation::GenerationTest(action)).await}
 #[cfg(not(feature="desktop-diagnostics"))]
 {let _=action;Err("diagnostic feature disabled".into())}
}

#[tauri::command]
pub async fn image_budget(window:tauri::WebviewWindow,limit_micros:Option<u64>)->Reply{
 if !matches!(window.label(),"main"|"manager"){return Err("main or manager only".into())}
 dispatch(window,Operation::ImageBudget(limit_micros)).await
}
#[tauri::command]
pub async fn image_storage(window:tauri::WebviewWindow,action:String)->Reply{
 if !matches!(window.label(),"main"|"manager"){return Err("main or manager only".into())}
 dispatch(window,Operation::ImageStorage(action)).await
}
