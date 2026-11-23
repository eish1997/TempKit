//! Immutable image protocol policy attached to the existing capability reference.
use crate::{contracts::*,image_protocol::ImageEditPolicy,resources::ResourceService};
use serde::{Serialize,Deserialize};use sha2::{Digest,Sha256};use std::collections::{BTreeMap,BTreeSet};
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageBinding{pub profile_id:String,pub policy:ImageEditPolicy,pub capability:Capability}
/// Host-only dispatch payload. Never serialize or log credentials/request bytes.
pub struct ImageDispatch{
 pub task:Task,
 pub profile:crate::gateway_profiles::GatewayProfile,
 pub secret:crate::credential_vault::Secret,
 pub body:crate::image_protocol::EditBody,
}
impl ResourceService{
 pub(crate) fn pending_image_archive(&self,task:&Task)->Result<Vec<u8>,String>{
  self.store.connection.query_row("SELECT bytes FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt],|r|r.get(0)).map_err(|e|e.to_string())
 }
 pub(crate) fn retry_image_archive(&mut self,expected:&Task)->Result<Task,String>{
  if expected.state!=TaskState::Finalizing||!expected.failure.as_ref().is_some_and(|f|f.stage==FailureStage::Archive){return Err("image archive is not waiting for retry".into())}
  let mut task=expected.clone();task.failure=None;task.event_version=task.event_version.checked_add(1).ok_or("event version overflow")?;
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  let changed=tx.execute("UPDATE attempts SET event_version=?4 WHERE task_id=?1 AND attempt=?2 AND event_version=?3 AND state='finalizing'",rusqlite::params![task.id,task.attempt,expected.event_version as i64,task.event_version as i64]).map_err(|e|e.to_string())?;
  if changed!=1{return Err("task_version_conflict".into())}
  tx.execute("DELETE FROM task_failures WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt]).map_err(|e|e.to_string())?;
  tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(task)
 }
 pub(crate) fn complete_image_archive(&mut self,expected:&Task,outcome:Result<&crate::resources::ValidatedImage,&crate::gateway_discovery::DeliveryFailure>)->Result<Task,String>{
  let current=self.task_snapshot()?.into_iter().find(|t|t.id==expected.id).ok_or("task not found")?;
  if current!=*expected||current.state!=TaskState::Finalizing{return Err("stale archive completion".into())}
  let result=(||{
   let image=outcome.map_err(|failure|failure.message.clone())?;
   let (bytes,mime):(Vec<u8>,String)=self.store.connection.query_row("SELECT bytes,mime FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2",rusqlite::params![expected.id,expected.attempt],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
   if bytes!=image.bytes||mime!=image.mime{return Err("pending image changed during decoding".into())}
   self.archive_task_output(crate::resources::ValidatedImage{bytes:image.bytes.clone(),width:image.width,height:image.height,mime:image.mime},expected.clone())?;
   self.task_snapshot()?.into_iter().find(|t|t.id==expected.id).ok_or("task disappeared".into())
  })();
  match result{Ok(task)=>Ok(task),Err(error)=>self.record_archive_failure(expected,error)}
 }
 /// Single board admission route, preserving historical submissions before
 /// looking at a card that may have been rebound since the original request.
 pub fn admit_board_submission(&mut self,submission:crate::tasks::MockSubmission,config:&serde_json::Value)->Result<Task,String>{
  let exists:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE submission_id=?1)",[&submission.submission_id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if exists{return self.admit_mock(submission,config)}
  let card=config["abilities"].as_array().ok_or("missing abilities")?.iter().find(|card|card["id"].as_str()==Some(&submission.preset_id)).ok_or("preset unavailable")?;
  if card.get("imageBinding").is_some_and(|value|!value.is_null()){
   let reference:CapabilityRef=serde_json::from_value(card["imageBinding"].clone()).map_err(|_|"invalid saved image binding")?;
   self.check_image_budget(&reference)?;
   self.admit_configured_image(submission,config)
  }else{self.admit_mock(submission,config)}
 }
 pub fn fail_queued_image(&mut self,queued:&Task,message:String)->Result<Task,String>{
  let mut task=self.task_snapshot()?.into_iter().find(|task|task.id==queued.id).ok_or("task not found")?;
  if task.state!=TaskState::Queued||task.attempt!=queued.attempt||task.event_version!=queued.event_version{return Err("task_version_conflict".into())}
  task.state=TaskState::Failed;task.event_version=task.event_version.checked_add(1).ok_or("event version overflow")?;
  task.failure=Some(TaskFailure{code:"image_not_sent".into(),stage:FailureStage::Execution,message:message.clone(),outcome_unknown:false});
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  let changed=tx.execute("UPDATE attempts SET state='failed',event_version=?4 WHERE task_id=?1 AND attempt=?2 AND state='queued' AND event_version=?3",rusqlite::params![task.id,task.attempt,queued.event_version as i64,task.event_version as i64]).map_err(|e|e.to_string())?;if changed!=1{return Err("task_version_conflict".into())}
  tx.execute("INSERT INTO task_failures VALUES(?1,?2,'execution',?3)",rusqlite::params![task.id,task.attempt,message]).map_err(|e|e.to_string())?;
  tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(task)
 }
 /// Prepare locally, then durably claim the attempt before returning anything
 /// that a network worker can send. This does not itself authorize a paid call.
 pub fn prepare_image_dispatch(&mut self,id:&str)->Result<ImageDispatch,String>{
  let tasks=self.task_snapshot()?;
  if tasks.iter().any(|t|t.state==TaskState::Running&&t.request.capability.connection_id!="internal-mock"){return Err("image execution already active".into())}
  let mut task=tasks.into_iter().find(|t|t.id==id).ok_or("task not found")?;
  if task.state!=TaskState::Queued{return Err("image task is not queued".into())}
  self.reclaim_received_copies()?;
  crate::image_execution::check_received_capacity(&self.root,self.pending_image_bytes()?,2*crate::resources::MAX_BYTES+32)?;
  let saved:String=self.store.connection.query_row("SELECT binding_json FROM image_task_bindings WHERE task_id=?1",[id],|r|r.get(0)).map_err(|_|"saved image binding unavailable")?;
  let binding:ImageBinding=serde_json::from_str(&saved).map_err(|_|"invalid saved image binding")?;
  if binding.capability!=task.capability_snapshot||binding.capability.reference!=task.request.capability{return Err("saved image capability mismatch".into())}
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==task.request.capability.connection_id).ok_or("connection not found")?;
  if connection.status!=ConnectionStatus::Ready||connection.profile_id!=binding.profile_id{return Err("image connection is not ready".into())}
  let profile=self.gateway_profile(&binding.profile_id)?;profile.bind(&connection)?;
  let secret=self.gateway_secret(&connection.id)?;
  let input=task.request.inputs.first().ok_or("image input missing")?;
  let(record,bytes)=self.read(&input.resource_id)?;
  let boundary=format!("quickgen-{}-{}",task.id,task.attempt);
  // Hash keeps the MIME boundary inside its length budget.
  let boundary=format!("qg-{:x}",Sha256::digest(boundary.as_bytes()));
  let body=crate::image_protocol::build_edit(&binding.policy,&task.request.parameters,&record.resource.mime,&bytes,&boundary)?;
  let previous=task.event_version;task.state=TaskState::Running;task.event_version=previous.checked_add(1).ok_or("event version overflow")?;task.failure=None;
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  crate::image_cost_rules::check_scope(&tx,&task.request,&record.resource)?;
  crate::image_budget::reserve(&tx,&task)?;
  let changed=tx.execute("UPDATE attempts SET state='running',event_version=?4,execution_count=execution_count+1 WHERE task_id=?1 AND attempt=?2 AND event_version=?3 AND state='queued'",rusqlite::params![id,task.attempt,previous as i64,task.event_version as i64]).map_err(|e|e.to_string())?;
  if changed!=1{return Err("task_version_conflict".into())}
  tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![id,serde_json::to_string(&task).map_err(|_|"invalid task")?]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;
  Ok(ImageDispatch{task,profile,secret,body})
 }
 /// Decoder runs outside the resource worker; the worker only commits validated
 /// bytes. On commit failure the caller must retain the output and retry saving.
 pub fn stage_image_output(&mut self,dispatched:&Task,image:&crate::resources::ValidatedImage)->Result<Task,String>{
  let mut task=self.task_snapshot()?.into_iter().find(|t|t.id==dispatched.id).ok_or("task not found")?;
  if task.state!=TaskState::Running||task.attempt!=dispatched.attempt||task.event_version!=dispatched.event_version||task.request!=dispatched.request{return Err("stale image completion".into())}
  let bound:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM image_task_bindings WHERE task_id=?1)",[&task.id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if !bound{return Err("not an image execution".into())}
  crate::image_execution::check_received_capacity(&self.root,self.pending_image_bytes()?,image.bytes.len() as u64)?;
  task.state=TaskState::Finalizing;task.event_version=task.event_version.checked_add(1).ok_or("event version overflow")?;task.failure=None;
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  let changed=tx.execute("UPDATE attempts SET state='finalizing',event_version=?4 WHERE task_id=?1 AND attempt=?2 AND event_version=?3 AND state='running'",rusqlite::params![task.id,task.attempt,dispatched.event_version as i64,task.event_version as i64]).map_err(|e|e.to_string())?;
  if changed!=1{return Err("stale image completion".into())}
  tx.execute("INSERT INTO pending_task_outputs VALUES(?1,?2,?3,?4)",rusqlite::params![task.id,task.attempt,image.bytes,image.mime]).map_err(|e|e.to_string())?;
  tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).map_err(|_|"invalid task")?]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(task)
 }
 /// Transport supplies a redacted classification, never the provider response.
 /// An uncertain submission is terminal until the user explicitly retries.
 pub fn fail_image_dispatch(&mut self,dispatched:&Task,failure:crate::gateway_discovery::DeliveryFailure)->Result<Task,String>{
  let mut task=self.task_snapshot()?.into_iter().find(|t|t.id==dispatched.id).ok_or("task not found")?;
  if task.state!=TaskState::Running||task.attempt!=dispatched.attempt||task.event_version!=dispatched.event_version||task.request!=dispatched.request{return Err("stale image completion".into())}
  let bound:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM image_task_bindings WHERE task_id=?1)",[&task.id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if !bound{return Err("not an image execution".into())}
  let state=if failure.acceptance_unknown{"interrupted"}else{"failed"};
  task.state=if failure.acceptance_unknown{TaskState::Interrupted}else{TaskState::Failed};task.event_version=task.event_version.checked_add(1).ok_or("event version overflow")?;
  task.failure=Some(TaskFailure{code:if failure.acceptance_unknown{"image_outcome_unknown"}else{"image_not_sent"}.into(),stage:FailureStage::Execution,message:failure.message.clone(),outcome_unknown:failure.acceptance_unknown});
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  let changed=tx.execute("UPDATE attempts SET state=?4,event_version=?5 WHERE task_id=?1 AND attempt=?2 AND event_version=?3 AND state='running'",rusqlite::params![task.id,task.attempt,dispatched.event_version as i64,state,task.event_version as i64]).map_err(|e|e.to_string())?;
  if changed!=1{return Err("stale image completion".into())}
  tx.execute("INSERT INTO task_failures VALUES(?1,?2,'execution',?3)",rusqlite::params![task.id,task.attempt,failure.message]).map_err(|e|e.to_string())?;
  tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).map_err(|_|"invalid task")?]).map_err(|e|e.to_string())?;
  if !failure.acceptance_unknown{
   // Only the transport's pre-send classification can release a reservation.
   // Failure state and this receipt commit together; unknown outcomes retain cost.
   tx.execute("INSERT INTO image_budget_releases VALUES(?1,?2,'confirmed_not_sent')",rusqlite::params![task.id,task.attempt]).map_err(|e|e.to_string())?;
  }
  tx.commit().map_err(|e|e.to_string())?;Ok(task)
 }
}
impl ResourceService{
 /// Called only by the host after a successful authenticated discovery. Not an IPC.
 pub fn record_gateway_check(&mut self,id:&str,models:Option<Vec<String>>)->Result<(),String>{
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  if connection.status==ConnectionStatus::Revoked{return Err("connection revoked during check".into())}
  self.gateway_secret(id)?;
  match models{
   Some(models)=>{if models.len()>4096||models.iter().any(|id|id.is_empty()||id.len()>256||id.chars().any(char::is_control)){return Err("invalid verified model list".into())}
    let models:BTreeSet<_>=models.into_iter().collect();self.store.connection.execute("INSERT INTO gateway_checks VALUES(?1,?2) ON CONFLICT(connection_id) DO UPDATE SET models_json=excluded.models_json",rusqlite::params![id,serde_json::to_string(&models).map_err(|_|"invalid models")?]).map_err(|_|"connection check could not be saved")?;},
   None=>{self.store.connection.execute("DELETE FROM gateway_checks WHERE connection_id=?1",[id]).map_err(|_|"connection check could not be cleared")?;}
  }Ok(())
 }
 pub fn register_image_binding(&mut self,id:&str,policy:ImageEditPolicy)->Result<ImageBinding,String>{
  policy.validate()?;
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  if connection.status!=ConnectionStatus::Ready{return Err("connection must pass authentication first".into())}
  let models:String=self.store.connection.query_row("SELECT models_json FROM gateway_checks WHERE connection_id=?1",[id],|r|r.get(0)).map_err(|_|"verified models unavailable")?;
  let models:BTreeSet<String>=serde_json::from_str(&models).map_err(|_|"invalid verified models")?;
  if !models.contains(&policy.model){return Err("model is absent from authenticated model list".into())}
  let profile=self.gateway_profile(&connection.profile_id)?;profile.bind(&connection)?;
  let serialized=serde_json::to_vec(&policy).map_err(|_|"invalid image policy")?;
  // Includes the protocol ID, so future protocol changes cannot alter old bindings.
  let mut hash=Sha256::new();hash.update(b"openai-image-edit-v1\0");hash.update(&serialized);
  let reference=CapabilityRef{connection_id:id.into(),capability_id:"image-edit".into(),revision:format!("{:x}",hash.finalize())};
  let mut parameters=BTreeMap::from([("prompt".into(),ParameterSpec::Text{required:true,max_length:32000})]);
  if !policy.sizes.is_empty(){parameters.insert("size".into(),ParameterSpec::Text{required:false,max_length:256});}
  if !policy.qualities.is_empty(){parameters.insert("quality".into(),ParameterSpec::Text{required:false,max_length:256});}
  let capability=Capability{schema_version:1,reference:reference.clone(),inputs:vec![RoleSpec{role:"source".into(),kinds:vec![ResourceKind::Image],min:1,max:1}],outputs:vec![RoleSpec{role:"result".into(),kinds:vec![ResourceKind::Image],min:1,max:1}],parameters,permissions:BTreeSet::from(["network".into()]),execution:ExecutionTraits{progress:false,cancel:false,query:false,idempotency:false}};
  capability.validate().map_err(|_|"invalid image capability")?;
  let binding=ImageBinding{profile_id:connection.profile_id,policy,capability};let encoded=serde_json::to_string(&binding).map_err(|_|"invalid binding")?;
  self.store.connection.execute("INSERT INTO image_bindings VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",rusqlite::params![reference.connection_id,reference.capability_id,reference.revision,encoded]).map_err(|_|"image binding could not be saved")?;
  let stored=self.image_binding(&reference)?;if serde_json::to_string(&stored).map_err(|_|"invalid binding")?!=encoded{return Err("image binding revision conflict".into())}Ok(stored)
 }
 pub fn image_binding(&self,reference:&CapabilityRef)->Result<ImageBinding,String>{
  reference.validate().map_err(|_|"invalid capability reference")?;
  let text:String=self.store.connection.query_row("SELECT binding_json FROM image_bindings WHERE connection_id=?1 AND capability_id=?2 AND revision=?3",rusqlite::params![reference.connection_id,reference.capability_id,reference.revision],|r|r.get(0)).map_err(|_|"image binding not found")?;
  let binding:ImageBinding=serde_json::from_str(&text).map_err(|_|"invalid image binding")?;
  if binding.capability.reference!=*reference{return Err("image binding identity mismatch".into())}binding.policy.validate()?;binding.capability.validate().map_err(|_|"invalid image capability")?;Ok(binding)
 }
}

#[cfg(test)]
#[path="image_archive_probe.rs"]
mod archive_probe;
#[cfg(test)]mod tests{
 use super::*;use crate::{gateway_profiles::GatewayProfile,credential_vault::Secret};
 #[test]fn rotation_keeps_queued_task_target_and_running_attempt_secret(){
  let root=std::env::temp_dir().join(format!("quickgen-task-rotation-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let mut host=ResourceService::open(&root).unwrap();
  let profile=GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"same".into(),base_url:"http://127.0.0.1:1".into()};
  let connection=host.configure_gateway(profile.clone(),Secret::from_bytes(b"synthetic-old".to_vec())).unwrap();host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
  let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec![],qualities:vec![],response_format:None}).unwrap();host.establish_image_budget(3).unwrap();host.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
  let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let input=host.import_bytes(png.into_inner(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let config=serde_json::json!({"abilities":[{"id":"card","name":"test","prompt":"isolated rotation","imageBinding":binding.capability.reference,"imageParameters":{}}]});
  let mut submission=crate::tasks::MockSubmission{submission_id:"running".into(),batch_id:None,preset_id:"card".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
  let first=host.admit_board_submission(submission.clone(),&config).unwrap();let running=host.prepare_image_dispatch(&first.id).unwrap();submission.submission_id="queued".into();let queued=host.admit_board_submission(submission,&config).unwrap();
  let next=host.rotate_gateway_credential("c",connection.credential_ref.as_deref().unwrap(),"same",Secret::from_bytes(b"synthetic-new".to_vec())).unwrap();
  assert_eq!(running.secret.bytes(),b"synthetic-old");assert_eq!(running.profile,profile);assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==queued.id).unwrap(),queued);
  host.fail_image_dispatch(&running.task,crate::gateway_discovery::DeliveryFailure{message:"isolated unknown result".into(),acceptance_unknown:true}).unwrap();
  assert!(host.prepare_image_dispatch(&queued.id).is_err(),"new credential must authenticate first");
  let mut scheduler=crate::image_execution::ImageExecution::default();
  assert!(!scheduler.start_next(&mut host).unwrap(),"scheduler must wait for rotated credential authentication");
  assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==queued.id).unwrap(),queued,"scheduler must preserve the queued attempt");
  assert_eq!(host.image_budget_status().unwrap().reserved_micros,1);
  host.record_gateway_check_version("c",next.credential_ref.as_deref().unwrap(),Some(vec!["fixture".into()])).unwrap();let dispatched=host.prepare_image_dispatch(&queued.id).unwrap();
  assert_eq!(dispatched.secret.bytes(),b"synthetic-new");assert_eq!(dispatched.profile,profile);assert_eq!(dispatched.task.request,queued.request);assert_eq!(dispatched.task.attempt,queued.attempt);
  assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==first.id).unwrap().state,TaskState::Interrupted);
 }
 #[test]fn configured_submission_replays_original_snapshot_after_card_changes_and_revocation(){
  let root=std::env::temp_dir().join(format!("quickgen-card-admission-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:"http://127.0.0.1:3000".into()},Secret::from_bytes(b"synthetic-only".to_vec())).unwrap();host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
  let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec!["fixture-size".into()],qualities:vec![],response_format:None}).unwrap();
  let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let input=host.import_bytes(png.into_inner(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let mut config=serde_json::json!({"abilities":[{"id":"card","name":"original name","prompt":"quickgen-admission-validation-boundary","imageBinding":binding.capability.reference,"imageParameters":{"size":"fixture-size"}}]});
  let submission=crate::tasks::MockSubmission{submission_id:"original-submission".into(),batch_id:Some("batch".into()),preset_id:"card".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
  let first=host.admit_configured_image(submission.clone(),&config).unwrap();assert_eq!(first.request.parameters["prompt"],ParameterValue::Text("quickgen-admission-validation-boundary".into()));
  let mut invalid=submission.clone();invalid.resource_id="r-other".into();assert_eq!(host.admit_configured_image(invalid,&config).unwrap_err(),"submission_conflict");
  config["abilities"][0]["prompt"]=serde_json::json!("new prompt");config["abilities"][0]["name"]=serde_json::json!("new name");assert_eq!(host.admit_configured_image(submission.clone(),&config).unwrap(),first);
  let mut second=submission.clone();second.submission_id="second-submission".into();
  host.store.connection.execute_batch("CREATE TRIGGER reject_card_admission BEFORE INSERT ON image_task_bindings BEGIN SELECT RAISE(ABORT,'isolated failure'); END;").unwrap();assert!(host.admit_configured_image(second.clone(),&config).is_err());assert_eq!(host.task_snapshot().unwrap().len(),1);host.store.connection.execute_batch("DROP TRIGGER reject_card_admission;").unwrap();
  let next=host.admit_configured_image(second.clone(),&config).unwrap();assert_eq!(next.request.parameters["prompt"],ParameterValue::Text("new prompt".into()));assert_ne!(next.id,first.id);
  host.revoke_gateway("c").unwrap();config["abilities"]=serde_json::json!([]);drop(host);
  let mut host=ResourceService::open(&root).unwrap();assert_eq!(host.admit_configured_image(submission.clone(),&config).unwrap(),first);assert_eq!(host.admit_mock(submission.clone(),&config).unwrap(),first);assert_eq!(host.admit_configured_image(second,&config).unwrap(),next);
  let mut new=submission;new.submission_id="after-removal".into();assert!(host.admit_configured_image(new,&config).is_err());
  assert!(host.execute_next_mock().unwrap().is_none());let executions:i64=host.store.connection.query_row("SELECT sum(execution_count) FROM attempts",[],|r|r.get(0)).unwrap();assert_eq!(executions,0);assert!(host.read(&input.resource.id).is_ok());assert!(host.retire(&input.resource.id).is_err());
 }
 #[test]fn binding_versions_preserve_policy_and_revoked_checks_cannot_restore_readiness(){
  let root=std::env::temp_dir().join(format!("quickgen-binding-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();let profile=GatewayProfile{schema_version:1,profile_id:"profile".into(),connection_id:"connection".into(),account_id:"account".into(),base_url:"http://127.0.0.1:3000".into()};host.configure_gateway(profile,Secret::from_bytes(b"synthetic-token".to_vec())).unwrap();
  let policy=ImageEditPolicy{model:"model-a".into(),sizes:vec!["1024x1024".into()],qualities:vec![],response_format:None};
  assert!(host.register_image_binding("connection",policy.clone()).is_err());
  host.record_gateway_check("connection",Some(vec!["model-a".into()])).unwrap();
  let first=host.register_image_binding("connection",policy.clone()).unwrap();let same=host.register_image_binding("connection",policy.clone()).unwrap();assert_eq!(first.capability,same.capability);
  let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=host.import_bytes(bytes.into_inner(),"png","fixture".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let parameters=BTreeMap::from([("prompt".into(),ParameterValue::Text("fixture".into()))]);
  let preset=Preset{schema_version:1,id:"preset".into(),name:"fixture".into(),capability:first.capability.reference.clone(),parameters:parameters.clone(),cover_resource_id:None};
  let request=TaskRequest{schema_version:1,submission_id:"image-admission".into(),batch_id:None,preset_id:"preset".into(),capability:first.capability.reference.clone(),parameters,inputs:vec![ResourceRef{role:"source".into(),resource_id:input.resource.id.clone()}]};
  let task=host.admit_image_task(request.clone(),&preset).unwrap();assert_eq!(host.admit_image_task(request.clone(),&preset).unwrap(),task);assert!(host.execute_next_mock().unwrap().is_none());assert!(host.retire(&input.resource.id).is_err());
  let executions:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&task.id],|r|r.get(0)).unwrap();assert_eq!(executions,0);
  let mut conflict=request.clone();conflict.parameters.insert("prompt".into(),ParameterValue::Text("changed".into()));assert_eq!(host.admit_image_task(conflict,&preset).unwrap_err(),"submission_conflict");
  host.store.connection.execute_batch("CREATE TRIGGER reject_image_binding BEFORE INSERT ON image_task_bindings BEGIN SELECT RAISE(ABORT,'isolated fault'); END;").unwrap();
  let mut failed=request.clone();failed.submission_id="transaction-failure".into();assert!(host.admit_image_task(failed.clone(),&preset).is_err());assert_eq!(host.task_snapshot().unwrap().len(),1);host.store.connection.execute_batch("DROP TRIGGER reject_image_binding;").unwrap();
  let mut changed=policy.clone();changed.qualities=vec!["high".into()];let second=host.register_image_binding("connection",changed).unwrap();assert_ne!(first.capability.reference,second.capability.reference);
  let mut wrong=policy;wrong.model="unlisted-model".into();assert!(host.register_image_binding("connection",wrong).is_err());
  assert!(host.store.connection.execute("DELETE FROM image_bindings",[]).is_err());assert!(host.store.connection.execute("UPDATE image_bindings SET binding_json='{}'",[]).is_err());drop(host);
  let mut host=ResourceService::open(&root).unwrap();let restored=host.image_binding(&first.capability.reference).unwrap();assert_eq!(host.admit_image_task(request.clone(),&preset).unwrap(),task);assert!(restored.policy.qualities.is_empty());assert_eq!(restored.policy.model,"model-a");
  assert!(host.prepare_image_dispatch(&task.id).is_err());assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);
  host.establish_image_budget(4).unwrap();assert!(host.prepare_image_dispatch(&task.id).is_err());
  assert!(host.approve_image_cost(&first.capability.reference,0,crate::image_budget::now_s().unwrap()+60).is_err());assert!(host.approve_image_cost(&first.capability.reference,1,1).is_err());
  host.approve_image_cost(&first.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
  // A dispatch transaction failure cannot release a sendable payload or count.
  host.store.connection.execute_batch("CREATE TRIGGER reject_dispatch BEFORE UPDATE ON tasks BEGIN SELECT RAISE(ABORT,'isolated dispatch fault'); END;").unwrap();
  assert!(host.prepare_image_dispatch(&task.id).is_err());
  let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&task.id],|r|r.get(0)).unwrap();assert_eq!(count,0);assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);
  host.store.connection.execute_batch("DROP TRIGGER reject_dispatch;").unwrap();
  let dispatch=host.prepare_image_dispatch(&task.id).unwrap();assert_eq!(dispatch.task.state,TaskState::Running);assert!(!dispatch.body.bytes.is_empty());assert!(host.prepare_image_dispatch(&task.id).is_err());
  // Crash before a response: durable running becomes unknown, never queued.
  drop(host);let mut host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();
  let interrupted=host.task_snapshot().unwrap().remove(0);assert_eq!(interrupted.state,TaskState::Interrupted);assert!(interrupted.failure.as_ref().unwrap().outcome_unknown);
  let mut jpeg=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,3).write_to(&mut jpeg,image::ImageFormat::Jpeg).unwrap();
  let output=crate::resources::validate_image(jpeg.into_inner(),"jpg").unwrap();
  assert!(host.stage_image_output(&dispatch.task,&output).is_err());
  assert!(host.control_mock(crate::tasks::TaskControl{task_id:task.id.clone(),attempt:interrupted.attempt,event_version:interrupted.event_version,action:crate::tasks::TaskAction::Retry}).is_err());
  let retry=host.control_mock(crate::tasks::TaskControl{task_id:task.id.clone(),attempt:interrupted.attempt,event_version:interrupted.event_version,action:crate::tasks::TaskAction::Regenerate}).unwrap();assert_eq!(retry.attempt,2);
  let next=host.prepare_image_dispatch(&task.id).unwrap();assert!(host.stage_image_output(&dispatch.task,&output).is_err());
  host.store.connection.execute_batch("CREATE TRIGGER reject_pending BEFORE INSERT ON pending_task_outputs BEGIN SELECT RAISE(ABORT,'isolated save fault'); END;").unwrap();
  assert!(host.stage_image_output(&next.task,&output).is_err());assert_eq!(host.task_snapshot().unwrap()[0].state,TaskState::Running);
  host.store.connection.execute_batch("DROP TRIGGER reject_pending;").unwrap();host.stage_image_output(&next.task,&output).unwrap();assert!(host.stage_image_output(&next.task,&output).is_err());
  host.store.connection.execute_batch("CREATE TRIGGER reject_archive BEFORE INSERT ON resource_refs WHEN NEW.owner_kind='task_output' BEGIN SELECT RAISE(ABORT,'isolated archive fault'); END;").unwrap();
  assert!(host.finalize_mock(&task.id).is_err());drop(host);
  let mut host=ResourceService::open(&root).unwrap();host.recover_mock_tasks().unwrap();let pending=host.task_snapshot().unwrap().remove(0);assert_eq!(pending.state,TaskState::Finalizing);assert_eq!(pending.failure.unwrap().stage,FailureStage::Archive);
  host.store.connection.execute_batch("DROP TRIGGER reject_archive;").unwrap();let done=host.finalize_mock(&task.id).unwrap();assert_eq!(done.state,TaskState::Succeeded);assert!(done.failure.is_none());assert_eq!(host.finalize_mock(&task.id).unwrap(),done);
  let(record,saved)=host.read(&done.outputs[0].resource_id).unwrap();assert_eq!(saved,output.bytes);assert_eq!(record.resource.mime,"image/jpeg");assert_eq!(record.display_name,"生成结果");assert!(host.read(&input.resource.id).is_ok());
  let counts:Vec<i64>=host.store.connection.prepare("SELECT execution_count FROM attempts ORDER BY attempt").unwrap().query_map([],|r|r.get(0)).unwrap().map(Result::unwrap).collect();assert_eq!(counts,vec![1,1]);
  let mut unknown_request=request.clone();unknown_request.submission_id="unknown-response".into();let unknown=host.admit_image_task(unknown_request,&preset).unwrap();
  let sending=host.prepare_image_dispatch(&unknown.id).unwrap();
  let mut queued_request=request.clone();queued_request.submission_id="queued-behind-network".into();let queued=host.admit_image_task(queued_request,&preset).unwrap();assert!(host.prepare_image_dispatch(&queued.id).is_err());
  let failed=host.fail_image_dispatch(&sending.task,crate::gateway_discovery::DeliveryFailure{message:"isolated connection lost after send".into(),acceptance_unknown:true}).unwrap();assert_eq!(failed.state,TaskState::Interrupted);assert!(failed.failure.unwrap().outcome_unknown);assert!(host.prepare_image_dispatch(&unknown.id).is_err());assert!(host.stage_image_output(&sending.task,&output).is_err());
  let unsent=host.prepare_image_dispatch(&queued.id).unwrap();
  host.store.connection.execute_batch("CREATE TRIGGER reject_release BEFORE INSERT ON image_budget_releases BEGIN SELECT RAISE(ABORT,'isolated release failure'); END;").unwrap();
  assert!(host.fail_image_dispatch(&unsent.task,crate::gateway_discovery::DeliveryFailure{message:"known pre-send failure".into(),acceptance_unknown:false}).is_err());assert_eq!(host.image_budget_status().unwrap().reserved_micros,4);assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==queued.id).unwrap().state,TaskState::Running);
  host.store.connection.execute_batch("DROP TRIGGER reject_release;").unwrap();
  let failed=host.fail_image_dispatch(&unsent.task,crate::gateway_discovery::DeliveryFailure{message:"isolated pre-send rejection".into(),acceptance_unknown:false}).unwrap();assert_eq!(failed.state,TaskState::Failed);assert!(!failed.failure.unwrap().outcome_unknown);
  assert_eq!(host.image_budget_status().unwrap().reserved_micros,3);
  assert!(host.fail_image_dispatch(&unsent.task,crate::gateway_discovery::DeliveryFailure{message:"duplicate".into(),acceptance_unknown:false}).is_err());
  drop(host);let mut host=ResourceService::open(&root).unwrap();assert_eq!(host.image_budget_status().unwrap().reserved_micros,3);
  let mut reuse=request.clone();reuse.submission_id="reuse-unsent-allowance".into();let reuse=host.admit_image_task(reuse,&preset).unwrap();let sent=host.prepare_image_dispatch(&reuse.id).unwrap();
  host.fail_image_dispatch(&sent.task,crate::gateway_discovery::DeliveryFailure{message:"unknown acceptance".into(),acceptance_unknown:true}).unwrap();
  assert_eq!(host.image_budget_status().unwrap().reserved_micros,4);host.establish_image_budget(4).unwrap();assert!(host.establish_image_budget(5).is_err());
  assert!(host.store.connection.execute("DELETE FROM image_budget_reservations",[]).is_err());assert!(host.store.connection.execute("DELETE FROM image_budget_releases",[]).is_err());
  let mut exhausted=request.clone();exhausted.submission_id="budget-exhausted".into();let exhausted=host.admit_image_task(exhausted,&preset).unwrap();assert!(host.prepare_image_dispatch(&exhausted.id).err().unwrap().contains("额度不足"));
  host.store.connection.execute("INSERT INTO image_cost_approvals VALUES('isolated-expired',?1,?2,?3,1,1)",rusqlite::params![first.capability.reference.connection_id,first.capability.reference.capability_id,first.capability.reference.revision]).unwrap();assert!(host.prepare_image_dispatch(&exhausted.id).err().unwrap().contains("过期"));
  let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&exhausted.id],|r|r.get(0)).unwrap();assert_eq!(count,0);assert_eq!(host.image_budget_status().unwrap().reserved_micros,4);
  host.record_gateway_check("connection",None).unwrap();assert_eq!(host.gateway_connections().unwrap()[0].status,ConnectionStatus::Unverified);
  host.revoke_gateway("connection").unwrap();assert!(host.record_gateway_check("connection",Some(vec!["model-a".into()])).is_err());assert!(host.register_image_binding("connection",restored.policy).is_err());assert_eq!(host.image_binding(&first.capability.reference).unwrap().capability,first.capability);
 }
}

impl ResourceService{
 /// Resolve a board submission against the host's saved card exactly once.
 /// Replay checks the original submission before consulting mutable configuration.
 pub fn admit_configured_image(&mut self,submission:crate::tasks::MockSubmission,config:&serde_json::Value)->Result<Task,String>{
  use rusqlite::OptionalExtension;
  let encoded=serde_json::to_string(&submission).map_err(|_|"invalid image submission")?;
  let prior:Option<(String,String)>=self.store.connection.query_row("SELECT submission_json,task_json FROM tasks WHERE submission_id=?1",[&submission.submission_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  if let Some((previous,task))=prior{if previous!=encoded{return Err("submission_conflict".into())}return serde_json::from_str(&task).map_err(|_|"invalid saved task".into())}
  if submission.simulate_failure{return Err("fault injection is not allowed for image model tasks".into())}
  let card=config["abilities"].as_array().ok_or("missing abilities")?.iter().find(|card|card["id"].as_str()==Some(&submission.preset_id)).ok_or("preset unavailable")?;
  let reference:CapabilityRef=serde_json::from_value(card["imageBinding"].clone()).map_err(|_|"preset image binding unavailable")?;
  let mut parameters:Parameters=match card.get("imageParameters"){Some(value)=>serde_json::from_value(value.clone()).map_err(|_|"invalid saved image parameters")?,None=>BTreeMap::new()};
  if parameters.keys().any(|key|!matches!(key.as_str(),"size"|"quality")){return Err("invalid saved image option".into())}
  parameters.insert("prompt".into(),ParameterValue::Text(card["prompt"].as_str().ok_or("invalid saved prompt")?.into()));
  let preset=Preset{schema_version:1,id:submission.preset_id.clone(),name:card["name"].as_str().ok_or("invalid saved preset name")?.into(),capability:reference.clone(),parameters:parameters.clone(),cover_resource_id:None};
  let request=TaskRequest{schema_version:1,submission_id:submission.submission_id,batch_id:submission.batch_id,preset_id:submission.preset_id,capability:reference,parameters,inputs:vec![ResourceRef{role:"source".into(),resource_id:submission.resource_id}]};
  self.admit_image_task_encoded(request,&preset,encoded)
 }
 /// Host supplies its saved preset snapshot, never a frontend-authored replacement.
 /// Admission is durable and does not send a request or increase execution_count.
 pub fn admit_image_task(&mut self,request:TaskRequest,preset:&Preset)->Result<Task,String>{
  let encoded=serde_json::to_string(&request).map_err(|_|"invalid image request")?;
  self.admit_image_task_encoded(request,preset,encoded)
 }
 fn admit_image_task_encoded(&mut self,request:TaskRequest,preset:&Preset,encoded:String)->Result<Task,String>{
  use rusqlite::OptionalExtension;
  let prior:Option<(String,String)>=self.store.connection.query_row("SELECT submission_json,task_json FROM tasks WHERE submission_id=?1",[&request.submission_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  if let Some((previous,task))=prior{if previous!=encoded{return Err("submission_conflict".into())}return serde_json::from_str(&task).map_err(|_|"invalid saved task".into())}
  let binding=self.image_binding(&request.capability)?;
  preset.validate(&binding.capability).map_err(|_|"invalid saved image preset")?;
  if request.preset_id!=preset.id||request.parameters!=preset.parameters{return Err("request does not match saved preset".into())}
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==request.capability.connection_id).ok_or("connection not found")?;
  if connection.profile_id!=binding.profile_id{return Err("binding profile mismatch".into())}self.gateway_secret(&connection.id)?;
  let mut resources=BTreeMap::new();
  for input in &request.inputs{let(record,_)=self.read(&input.resource_id)?;resources.insert(record.resource.id.clone(),record.resource);}
  request.validate(&binding.capability,&connection,&resources,&BTreeSet::from([ResourceKind::Image])).map_err(|_|"invalid image task request")?;
  // Admission validates options without constructing transport bytes or inventing a MIME boundary.
  crate::image_protocol::validate_parameters(&binding.policy,&request.parameters)?;
  if let Some(input)=request.inputs.first().and_then(|input|resources.get(&input.resource_id)){
   crate::image_cost_rules::check_scope(&self.store.connection,&request,input)?;
  }
  let id=format!("task-{:x}",Sha256::digest(request.submission_id.as_bytes()));
  let task=Task{schema_version:1,id:id.clone(),request,capability_snapshot:binding.capability.clone(),attempt:1,state:TaskState::Queued,event_version:1,external_run_id:None,outputs:vec![],failure:None};
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO tasks VALUES(?1,?2,?3,?4,?5)",rusqlite::params![id,task.request.submission_id,encoded,serde_json::to_string(&task).map_err(|_|"invalid task")?,serde_json::to_string(preset).map_err(|_|"invalid preset")?]).map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO attempts(task_id,attempt,state,event_version) VALUES(?1,1,'queued',1)",[&id]).map_err(|e|e.to_string())?;
  for input in &task.request.inputs{tx.execute("INSERT INTO resource_refs VALUES('task',?1,?2)",rusqlite::params![id,input.resource_id]).map_err(|e|e.to_string())?;}
  tx.execute("INSERT INTO image_task_bindings VALUES(?1,?2)",rusqlite::params![id,serde_json::to_string(&binding).map_err(|_|"invalid binding")?]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(task)
 }
}
