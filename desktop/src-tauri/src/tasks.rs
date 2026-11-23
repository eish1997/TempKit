//! Durable mock-only admission; configuration remains the editable card authority.
use crate::{contracts::*,resources::ResourceService};
use serde::{Serialize,Deserialize};
use std::collections::{BTreeMap,BTreeSet};
use sha2::{Sha256,Digest};
use rusqlite::OptionalExtension;
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MockSubmission {pub submission_id:String,pub batch_id:Option<String>,pub preset_id:String,pub resource_id:String,pub simulate_failure:bool}
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TaskAction {Cancel,Retry,Regenerate,RetryArchive}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskControl {pub task_id:String,pub attempt:u32,pub event_version:u64,pub action:TaskAction}
#[derive(Debug,Serialize,Deserialize)]
pub struct TaskFeed {pub cursor:u64,pub tasks:Vec<Task>}
impl ResourceService {
    #[cfg(feature="desktop-diagnostics")]
    pub fn review_archive_fault(&mut self,enabled:bool)->Result<serde_json::Value,String>{
        self.store.connection.execute_batch(if enabled {"CREATE TRIGGER IF NOT EXISTS review_reject_archive BEFORE INSERT ON resource_refs WHEN NEW.owner_kind='task_output' BEGIN SELECT RAISE(ABORT,'Isolated review: archive unavailable'); END;"}else{"DROP TRIGGER IF EXISTS review_reject_archive;"}).map_err(|e|e.to_string())?;
        let executions:i64=self.store.connection.query_row("SELECT COALESCE(sum(execution_count),0) FROM attempts",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        let placements:i64=self.store.connection.query_row("SELECT count(*) FROM result_placements",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        Ok(serde_json::json!({"executions":executions,"placements":placements}))
    }
    /// None gives a consistent full snapshot and watermark; Some replays the
    /// durable log after that watermark, including intermediate/old attempts.
    pub fn task_feed(&self,after:Option<u64>)->Result<TaskFeed,String>{
        if let Some(after)=after {
            let mut query=self.store.connection.prepare("SELECT sequence,task_json FROM task_events WHERE sequence>?1 ORDER BY sequence LIMIT 1000").map_err(|e|e.to_string())?;
            let rows=query.query_map([i64::try_from(after).map_err(|_|"invalid cursor")?],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
            let mut feed=TaskFeed{cursor:after,tasks:vec![]};
            for row in rows {let (cursor,task)=row.map_err(|e|e.to_string())?;feed.cursor=u64::try_from(cursor).map_err(|_|"invalid stored cursor")?;feed.tasks.push(serde_json::from_str(&task).map_err(|e|e.to_string())?);}
            Ok(feed)
        }else{
            // ResourceService owns the only write connection on one host thread.
            let cursor=self.store.connection.query_row("SELECT COALESCE(MAX(sequence),0) FROM task_events",[],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?;
            let cursor=u64::try_from(cursor).map_err(|_|"invalid stored cursor")?;
            Ok(TaskFeed{cursor,tasks:self.task_snapshot()?})
        }
    }
    pub fn control_mock(&mut self,command:TaskControl)->Result<Task,String>{
        let mut task=self.task_snapshot()?.into_iter().find(|t|t.id==command.task_id).ok_or("task not found")?;
        if task.attempt!=command.attempt||task.event_version!=command.event_version{return Err("task_version_conflict".into())}
        match command.action {
            TaskAction::RetryArchive=>{
                #[cfg(windows)]
                if task.request.capability.connection_id!="internal-mock"{return self.retry_image_archive(&task)}
                return self.finalize_mock(&task.id)
            },
            TaskAction::Cancel=>{
                if task.state!=TaskState::Queued{return Err("Only queued mock tasks can be cancelled; execution has not confirmed cancellation".into())}
                task.state.transition(TaskState::Cancelled,false).map_err(|e|format!("{e:?}"))?;
                task.state=TaskState::Cancelled;
            },
            TaskAction::Retry|TaskAction::Regenerate=>{
                if matches!(command.action,TaskAction::Retry)&&task.request.capability.connection_id!="internal-mock"&&task.failure.as_ref().is_some_and(|failure|failure.outcome_unknown){return Err("原请求可能已执行并计费，请明确确认重新生成".into())}
                if !matches!(task.state,TaskState::Failed|TaskState::Interrupted|TaskState::Cancelled){return Err("Only unsuccessful terminal tasks can be retried".into())}
                for input in &task.request.inputs {self.read(&input.resource_id)?;}
                task.attempt=task.attempt.checked_add(1).ok_or("attempt overflow")?;
                task.state=TaskState::Queued;task.outputs.clear();task.external_run_id=None;task.failure=None;
            },
        }
        task.event_version=task.event_version.checked_add(1).ok_or("event version overflow")?;
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        match command.action {
            TaskAction::Retry|TaskAction::Regenerate=>{tx.execute("INSERT INTO attempts(task_id,attempt,state,event_version) VALUES(?1,?2,'queued',?3)",rusqlite::params![task.id,task.attempt,task.event_version as i64]).map_err(|e|e.to_string())?;},
            TaskAction::Cancel=>{let changed=tx.execute("UPDATE attempts SET state='cancelled',event_version=?4 WHERE task_id=?1 AND attempt=?2 AND event_version=?3 AND state='queued'",rusqlite::params![task.id,task.attempt,command.event_version as i64,task.event_version as i64]).map_err(|e|e.to_string())?;if changed!=1{return Err("task_version_conflict".into())}},
            TaskAction::RetryArchive=>unreachable!(),
        }
        tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;Ok(task)
    }
    /// Called once when the host worker starts, before it can execute queued work.
    /// Unknown execution outcomes are never automatically submitted again.
    pub fn recover_mock_tasks(&mut self)->Result<(),String>{
        let tasks=self.task_snapshot()?;
        #[cfg(windows)]
        let received:std::collections::BTreeSet<String>=tasks.iter().filter(|task|task.state==TaskState::Running&&self.has_received_image(task)).map(|task|task.id.clone()).collect();
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        for mut task in tasks {
            // A received response has a save-only worker after restart. Keep the
            // exact snapshot so its artifact and UI retry version remain valid.
            #[cfg(windows)]
            if received.contains(&task.id){continue}
            let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2)",rusqlite::params![task.id,task.attempt],|r|r.get(0)).map_err(|e|e.to_string())?;
            let reason=match task.state {
                TaskState::Running|TaskState::WaitingUser=>Some("Execution outcome unknown after restart; explicit retry required"),
                TaskState::Finalizing if !pending=>Some("Pending output missing after restart; execution was not repeated"),
                TaskState::Finalizing=>{
                    // Retain explicit archive failure across restarts. Older builds
                    // stored only the SQL row: publish it once into the durable feed.
                    let error:Option<String>=tx.query_row("SELECT message FROM task_failures WHERE task_id=?1 AND attempt=?2 AND stage='archive'",rusqlite::params![task.id,task.attempt],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
                    if let Some(error)=error {if task.failure.is_none(){
                        task.failure=Some(archive_failure(error));task.event_version+=1;
                        tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
                        tx.execute("UPDATE attempts SET event_version=?3 WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt,task.event_version as i64]).map_err(|e|e.to_string())?;
                    }}
                    None
                },
                _=>None,
            };
            if let Some(reason)=reason {
                task.state.transition(TaskState::Interrupted,false).map_err(|e|format!("{e:?}"))?;
                task.state=TaskState::Interrupted;task.event_version+=1;
                task.failure=Some(TaskFailure{code:"execution_interrupted".into(),stage:FailureStage::Execution,message:reason.into(),outcome_unknown:true});
                tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
                tx.execute("UPDATE attempts SET state='interrupted',event_version=?3 WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt,task.event_version as i64]).map_err(|e|e.to_string())?;
                tx.execute("INSERT INTO task_failures VALUES(?1,?2,'recovery',?3) ON CONFLICT(task_id,attempt) DO UPDATE SET stage='recovery',message=excluded.message",rusqlite::params![task.id,task.attempt,reason]).map_err(|e|e.to_string())?;
            }
        }
        tx.commit().map_err(|e|e.to_string())
    }
    /// One serial scheduling step. Failed archives wait for explicit retry;
    /// they do not monopolize the worker or invoke the executor again.
    pub fn tick_mock(&mut self)->Result<bool,String>{
        let pending:Option<String>=self.store.connection.query_row("SELECT a.task_id FROM attempts a LEFT JOIN task_failures f ON f.task_id=a.task_id AND f.attempt=a.attempt WHERE a.state='finalizing' AND f.task_id IS NULL AND json_extract((SELECT task_json FROM tasks WHERE task_id=a.task_id),'$.request.capability.connection_id')='internal-mock' ORDER BY a.rowid LIMIT 1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        if let Some(id)=pending {
            if let Err(error)=self.finalize_mock(&id){
                let recorded:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM task_failures WHERE task_id=?1 AND stage='archive')",[&id],|r|r.get(0)).map_err(|e|e.to_string())?;
                if !recorded{return Err(error)}
            }
            return Ok(true)
        }
        self.execute_next_mock().map(|task|task.is_some())
    }
    pub fn finalize_mock(&mut self,id:&str)->Result<Task,String>{
        let task=self.task_snapshot()?.into_iter().find(|t|t.id==id).ok_or("task not found")?;
        if task.state==TaskState::Succeeded{return Ok(task)}
        if task.state!=TaskState::Finalizing{return Err("task is not finalizing".into());}
        let result:Result<Task,String>=(||{
            let (bytes,mime):(Vec<u8>,String)=self.store.connection.query_row("SELECT bytes,mime FROM pending_task_outputs WHERE task_id=?1 AND attempt=?2",rusqlite::params![id,task.attempt],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
            let extension=match mime.as_str(){"image/png"=>"png","image/jpeg"=>"jpg","image/webp"=>"webp",_=>return Err("unsupported pending image type".into())};
            let image=crate::resources::validate_image(bytes,extension)?;
            self.archive_task_output(image,task.clone())?;
            self.task_snapshot()?.into_iter().find(|t|t.id==id).ok_or("task disappeared".into())
        })();
        if let Err(error)=&result {
            self.record_archive_failure(&task,error.clone())?;
        }
        result
    }
    pub(crate) fn record_archive_failure(&mut self,task:&Task,error:String)->Result<Task,String>{
        let current=self.task_snapshot()?.into_iter().find(|t|t.id==task.id).ok_or("task not found")?;
        if current!=*task||task.state!=TaskState::Finalizing{return Err("stale archive completion".into())}
        let id=&task.id;
            let mut failed=task.clone();failed.failure=Some(archive_failure(error.clone()));failed.event_version+=1;
            let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO task_failures VALUES(?1,?2,'archive',?3) ON CONFLICT(task_id,attempt) DO UPDATE SET stage='archive',message=excluded.message",rusqlite::params![id,task.attempt,error]).map_err(|e|e.to_string())?;
            tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![id,serde_json::to_string(&failed).unwrap()]).map_err(|e|e.to_string())?;
            tx.execute("UPDATE attempts SET event_version=?3 WHERE task_id=?1 AND attempt=?2",rusqlite::params![id,task.attempt,failed.event_version as i64]).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e|e.to_string())?;Ok(failed)
    }
    pub fn task_snapshot(&self)->Result<Vec<Task>,String>{
        let mut query=self.store.connection.prepare("SELECT task_json FROM tasks ORDER BY rowid").map_err(|e|e.to_string())?;
        let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        rows.map(|r|serde_json::from_str(&r.map_err(|e|e.to_string())?).map_err(|e|e.to_string())).collect()
    }
    pub fn admit_mock(&mut self,submission:MockSubmission,config:&serde_json::Value)->Result<Task,String>{
        let encoded=serde_json::to_string(&submission).map_err(|e|e.to_string())?;
        let previous:Option<(String,String)>=self.store.connection.query_row("SELECT submission_json,task_json FROM tasks WHERE submission_id=?1",[&submission.submission_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
        if let Some((old,task))=previous {if old!=encoded{return Err("submission_conflict".into());}return serde_json::from_str(&task).map_err(|e|e.to_string());}
        let card=config["abilities"].as_array().ok_or("missing abilities")?.iter().find(|c|c["id"].as_str()==Some(&submission.preset_id)).ok_or("preset unavailable")?;
        if card.get("imageBinding").is_some_and(|binding|!binding.is_null()){return Err("此预设已绑定图片模型；真实执行入口尚未启用，不会改用模拟执行器".into())}
        let name=card["name"].as_str().filter(|s|!s.trim().is_empty()).ok_or("invalid card name")?;
        let prompt=card["prompt"].as_str().ok_or("invalid card prompt")?;
        let card_snapshot=serde_json::json!({"id":submission.preset_id,"name":name,"prompt":prompt});
        let revision=format!("{:x}",Sha256::digest(serde_json::to_vec(&card_snapshot).unwrap()));
        let capability=Capability{schema_version:1,reference:CapabilityRef{connection_id:"internal-mock".into(),capability_id:"image-mock".into(),revision},
            inputs:vec![RoleSpec{role:"source".into(),kinds:vec![ResourceKind::Image],min:1,max:1}],outputs:vec![RoleSpec{role:"result".into(),kinds:vec![ResourceKind::Image],min:1,max:1}],
            parameters:BTreeMap::from([("prompt".into(),ParameterSpec::Text{required:true,max_length:2000}),("simulate_failure".into(),ParameterSpec::Boolean{required:true})]),permissions:BTreeSet::new(),execution:ExecutionTraits{progress:false,cancel:true,query:false,idempotency:false}};
        let request=TaskRequest{schema_version:1,submission_id:submission.submission_id.clone(),batch_id:submission.batch_id,preset_id:submission.preset_id,capability:capability.reference.clone(),inputs:vec![ResourceRef{role:"source".into(),resource_id:submission.resource_id.clone()}],parameters:BTreeMap::from([("prompt".into(),ParameterValue::Text(prompt.into())),("simulate_failure".into(),ParameterValue::Boolean(submission.simulate_failure))])};
        let (record,_)=self.read(&submission.resource_id)?;
        let resources=BTreeMap::from([(record.resource.id.clone(),record.resource)]);
        let connection=Connection{schema_version:1,id:"internal-mock".into(),adapter_id:"mock".into(),profile_id:"internal".into(),credential_ref:None,status:ConnectionStatus::Ready};
        request.validate(&capability,&connection,&resources,&BTreeSet::from([ResourceKind::Image])).map_err(|e|format!("{e:?}"))?;
        let id=format!("task-{:x}",Sha256::digest(submission.submission_id.as_bytes()));
        let task=Task{schema_version:1,id:id.clone(),request,capability_snapshot:capability,attempt:1,state:TaskState::Queued,event_version:1,external_run_id:None,outputs:vec![],failure:None};
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO tasks VALUES (?1,?2,?3,?4,?5)",rusqlite::params![id,submission.submission_id,encoded,serde_json::to_string(&task).unwrap(),card_snapshot.to_string()]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO attempts(task_id,attempt,state,event_version) VALUES (?1,1,'queued',1)",[&id]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO resource_refs VALUES ('task',?1,?2)",rusqlite::params![id,submission.resource_id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;Ok(task)
    }
    pub fn admit_mock_batch(&mut self,submissions:Vec<MockSubmission>,config:&serde_json::Value)->Vec<Result<Task,String>>{
        submissions.into_iter().map(|s|self.admit_mock(s,config)).collect()
    }
    /// Execute one queued mock, persisting its output before any resource archival.
    /// A finalizing task is never selected here, including after process restart.
    pub fn execute_next_mock(&mut self)->Result<Option<Task>,String>{
        let Some(mut task)=self.task_snapshot()?.into_iter().find(|t|t.state==TaskState::Queued&&t.request.capability.connection_id=="internal-mock"&&t.capability_snapshot.reference.connection_id=="internal-mock") else{return Ok(None)};
        task.state.transition(TaskState::Running,false).map_err(|e|format!("{e:?}"))?;
        task.state=TaskState::Running;task.event_version+=1;
        {
            let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
            tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
            tx.execute("UPDATE attempts SET state='running',event_version=?3,execution_count=execution_count+1 WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt,task.event_version as i64]).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e|e.to_string())?;
        }
        // UI's fault switch injects one initial failure; explicit retry retains
        // the frozen request but exercises normal execution in its new attempt.
        let failed=task.attempt==1&&task.request.parameters.get("simulate_failure")==Some(&ParameterValue::Boolean(true));
        let output=if failed {Err("Requested simulated execution failure".to_string())}else{mock_png(&task)};
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        match output {
            Ok(bytes)=>{
                task.state.transition(TaskState::Finalizing,false).map_err(|e|format!("{e:?}"))?;task.state=TaskState::Finalizing;
                tx.execute("INSERT INTO pending_task_outputs VALUES(?1,?2,?3,'image/png')",rusqlite::params![task.id,task.attempt,bytes]).map_err(|e|e.to_string())?;
            },
            Err(error)=>{task.state=TaskState::Failed;task.failure=Some(TaskFailure{code:"execution_failed".into(),stage:FailureStage::Execution,message:error.clone(),outcome_unknown:false});tx.execute("INSERT INTO task_failures VALUES(?1,?2,'execution',?3)",rusqlite::params![task.id,task.attempt,error]).map_err(|e|e.to_string())?;}
        }
        task.event_version+=1;
        tx.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![task.id,serde_json::to_string(&task).unwrap()]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE attempts SET state=?3,event_version=?4 WHERE task_id=?1 AND attempt=?2",rusqlite::params![task.id,task.attempt,if task.state==TaskState::Failed{"failed"}else{"finalizing"},task.event_version as i64]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;Ok(Some(task))
    }
}
fn archive_failure(message:String)->TaskFailure{TaskFailure{code:"archive_failed".into(),stage:FailureStage::Archive,message,outcome_unknown:false}}
fn mock_png(task:&Task)->Result<Vec<u8>,String>{
    // Deterministic bounded synthetic image, intentionally distinct from input.
    // Labeling and provenance are provided by the archived result metadata/UI.
    let digest=Sha256::digest(task.id.as_bytes());
    let image=image::RgbImage::from_fn(64,64,|x,y|if (x/8+y/8)%2==0 {image::Rgb([digest[0],digest[1],digest[2]])}else{image::Rgb([240,240,240])});
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgb8(image).write_to(&mut bytes,image::ImageFormat::Png).map_err(|e|e.to_string())?;Ok(bytes.into_inner())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn restart_interrupts_unknown_outcomes_and_only_archives_saved_output(){
  let root=std::env::temp_dir().join(format!("quickgen-recovery-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=service.import_bytes(bytes.into_inner(),"png","input".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let config=serde_json::json!({"abilities":[{"id":"clay","name":"mock","prompt":"frozen"}]});
  let mut tasks=vec![];
  for n in 0..5 {tasks.push(service.admit_mock(MockSubmission{submission_id:format!("recover-{n}"),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&config).unwrap());}
  // Durable snapshots of interrupted execution boundaries in this isolated DB.
  for (index,state,label) in [(0,TaskState::Running,"running"),(1,TaskState::WaitingUser,"waiting_user"),(2,TaskState::Finalizing,"finalizing")] {
   tasks[index].state=state;tasks[index].event_version=2;
   service.store.connection.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![tasks[index].id,serde_json::to_string(&tasks[index]).unwrap()]).unwrap();
   service.store.connection.execute("UPDATE attempts SET state=?2,event_version=2,execution_count=1 WHERE task_id=?1",rusqlite::params![tasks[index].id,label]).unwrap();
  }
  assert_eq!(service.execute_next_mock().unwrap().unwrap().id,tasks[3].id);
  drop(service);let mut service=ResourceService::open(&root).unwrap();service.recover_mock_tasks().unwrap();
  let recovered=service.task_snapshot().unwrap();
  assert!(recovered[..3].iter().all(|t|t.state==TaskState::Interrupted&&t.event_version==3));
  assert_eq!(recovered[3].state,TaskState::Finalizing);assert_eq!(recovered[4].state,TaskState::Queued);
  service.recover_mock_tasks().unwrap();assert_eq!(recovered,service.task_snapshot().unwrap());
  assert!(service.tick_mock().unwrap());assert_eq!(service.task_snapshot().unwrap()[3].state,TaskState::Succeeded);
  assert_eq!(service.store.connection.query_row("SELECT sum(execution_count) FROM attempts",[],|r|r.get::<_,i64>(0)).unwrap(),4);
  assert!(service.tick_mock().unwrap());assert!(service.tick_mock().unwrap());assert!(!service.tick_mock().unwrap());
  assert_eq!(service.task_snapshot().unwrap()[4].state,TaskState::Succeeded);
  assert_eq!(service.store.connection.query_row("SELECT sum(execution_count) FROM attempts",[],|r|r.get::<_,i64>(0)).unwrap(),5);
  let retry=TaskControl{task_id:recovered[0].id.clone(),attempt:1,event_version:3,action:TaskAction::Retry};
  let before_retry=service.task_feed(None).unwrap();
  service.store.connection.execute_batch("CREATE TRIGGER reject_retry BEFORE INSERT ON attempts BEGIN SELECT RAISE(ABORT,'retry failed'); END;").unwrap();
  assert!(service.control_mock(retry.clone()).is_err());assert_eq!(service.task_snapshot().unwrap()[0],recovered[0]);
  assert!(service.task_feed(Some(before_retry.cursor)).unwrap().tasks.is_empty());
  service.store.connection.execute_batch("DROP TRIGGER reject_retry").unwrap();
  let retried=service.control_mock(retry.clone()).unwrap();assert_eq!(retried.attempt,2);assert_eq!(retried.request,recovered[0].request);
  assert_eq!(service.control_mock(retry).unwrap_err(),"task_version_conflict");
  let cancel=TaskControl{task_id:retried.id.clone(),attempt:2,event_version:retried.event_version,action:TaskAction::Cancel};
  let cancelled=service.control_mock(cancel.clone()).unwrap();assert_eq!(cancelled.state,TaskState::Cancelled);
  assert_eq!(service.control_mock(cancel).unwrap_err(),"task_version_conflict");
  assert!(!service.tick_mock().unwrap());
  assert_eq!(service.store.connection.query_row("SELECT state FROM attempts WHERE task_id=?1 AND attempt=1",[&retried.id],|r|r.get::<_,String>(0)).unwrap(),"interrupted");
  let queued=service.control_mock(TaskControl{task_id:cancelled.id.clone(),attempt:2,event_version:cancelled.event_version,action:TaskAction::Retry}).unwrap();assert_eq!(queued.attempt,3);
  service.tick_mock().unwrap();let finalizing=service.task_snapshot().unwrap()[0].clone();
  assert!(service.control_mock(TaskControl{task_id:finalizing.id.clone(),attempt:3,event_version:finalizing.event_version,action:TaskAction::Cancel}).is_err());
  let success=service.control_mock(TaskControl{task_id:finalizing.id.clone(),attempt:3,event_version:finalizing.event_version,action:TaskAction::RetryArchive}).unwrap();assert_eq!(success.state,TaskState::Succeeded);
  assert!(service.control_mock(TaskControl{task_id:success.id.clone(),attempt:3,event_version:success.event_version,action:TaskAction::Retry}).is_err());
  assert_eq!(service.store.connection.query_row("SELECT sum(execution_count) FROM attempts",[],|r|r.get::<_,i64>(0)).unwrap(),6);
  let replay=service.task_feed(Some(before_retry.cursor)).unwrap();
  assert_eq!(replay.tasks.iter().map(|t|(t.attempt,t.state)).collect::<Vec<_>>(),vec![(2,TaskState::Queued),(2,TaskState::Cancelled),(3,TaskState::Queued),(3,TaskState::Running),(3,TaskState::Finalizing),(3,TaskState::Succeeded)]);
  assert!(replay.tasks.windows(2).all(|t|t[0].event_version<t[1].event_version));
  drop(service);let service=ResourceService::open(&root).unwrap();assert_eq!(service.task_feed(Some(before_retry.cursor)).unwrap().tasks,replay.tasks);assert!(service.task_feed(Some(replay.cursor)).unwrap().tasks.is_empty());
 }
 #[test]fn admission_deduplicates_freezes_card_and_keeps_batch_failures_independent(){
  let root=std::env::temp_dir().join(format!("quickgen-admission-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=service.import_bytes(bytes.into_inner(),"png","input".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let config=serde_json::json!({"abilities":[{"id":"clay","name":"旧卡片","prompt":"旧指令"}]});
  let submission=MockSubmission{submission_id:"submit-1".into(),batch_id:Some("batch-1".into()),preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
  let first=service.admit_mock(submission.clone(),&config).unwrap();assert!(service.retire(&input.resource.id).is_err());drop(service);
  let mut service=ResourceService::open(&root).unwrap();let changed=serde_json::json!({"abilities":[]});assert_eq!(service.admit_mock(submission.clone(),&changed).unwrap(),first);
  let mut conflict=submission.clone();conflict.simulate_failure=true;assert_eq!(service.admit_mock(conflict,&config).unwrap_err(),"submission_conflict");
  let mut good=submission.clone();good.submission_id="submit-2".into();let mut bad=good.clone();bad.submission_id="submit-3".into();bad.resource_id="missing".into();
  let batch=service.admit_mock_batch(vec![good,bad],&config);assert!(batch[0].is_ok());assert!(batch[1].is_err());
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM tasks",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  service.store.connection.execute_batch("CREATE TRIGGER reject_attempt BEFORE INSERT ON attempts BEGIN SELECT RAISE(ABORT,'attempt write failed'); END;").unwrap();
  let mut failed=submission.clone();failed.submission_id="transaction-failure".into();assert!(service.admit_mock(failed.clone(),&config).is_err());
  assert_eq!(service.task_snapshot().unwrap().len(),2);
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resource_refs WHERE owner_kind='task'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  service.store.connection.execute_batch("DROP TRIGGER reject_attempt").unwrap();
  let invalid=serde_json::json!({"abilities":[{"id":"clay","name":"bad","prompt":""}]});assert!(service.admit_mock(failed.clone(),&invalid).is_err());
  assert_eq!(service.admit_mock(failed,&config).unwrap().state,TaskState::Queued);
  let mut failing=submission.clone();failing.submission_id="execute-failure".into();failing.simulate_failure=true;service.admit_mock(failing,&config).unwrap();
  for _ in 0..3 {assert_eq!(service.execute_next_mock().unwrap().unwrap().state,TaskState::Finalizing);}
  assert_eq!(service.execute_next_mock().unwrap().unwrap().state,TaskState::Failed);
  let bytes:Vec<u8>=service.store.connection.query_row("SELECT bytes FROM pending_task_outputs LIMIT 1",[],|r|r.get(0)).unwrap();
  let image=crate::resources::validate_image(bytes,"png").unwrap();assert_eq!((image.width,image.height),(64,64));
  drop(service);let mut service=ResourceService::open(&root).unwrap();assert!(service.execute_next_mock().unwrap().is_none());
  service.store.connection.execute_batch("CREATE TRIGGER reject_output BEFORE INSERT ON resource_refs WHEN NEW.owner_kind='task_output' BEGIN SELECT RAISE(ABORT,'archive write failed'); END;").unwrap();
  assert!(service.finalize_mock(&first.id).is_err());
  let reported=serde_json::to_value(&service.task_snapshot().unwrap()[0]).unwrap();
  assert_eq!(reported["failure"]["stage"],"archive","archive error must reach snapshot/feed, not just a private SQL row");
  assert!(reported["failure"]["message"].as_str().unwrap().contains("archive write failed"));
  let feed=service.task_feed(None).unwrap();
  let failed=service.task_snapshot().unwrap()[0].clone();
  assert!(service.control_mock(TaskControl{task_id:first.id.clone(),attempt:failed.attempt,event_version:failed.event_version,action:TaskAction::RetryArchive}).is_err());
  let event=service.task_feed(Some(feed.cursor)).unwrap();assert_eq!(event.tasks.len(),1);assert!(event.tasks[0].event_version>failed.event_version);assert!(event.tasks[0].failure.is_some());
  // A pre-fix archive row has no task JSON failure: restart must publish it once.
  let mut legacy=service.task_snapshot().unwrap()[0].clone();legacy.failure=None;
  service.store.connection.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![first.id,serde_json::to_string(&legacy).unwrap()]).unwrap();
  service.recover_mock_tasks().unwrap();let restored=service.task_snapshot().unwrap()[0].clone();assert!(restored.failure.is_some());
  service.recover_mock_tasks().unwrap();assert_eq!(service.task_snapshot().unwrap()[0],restored);
  assert_eq!(service.task_snapshot().unwrap()[0].state,TaskState::Finalizing);
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM pending_task_outputs",[],|r|r.get::<_,i64>(0)).unwrap(),3);
  // Scheduler skips the failed archive but still attempts the next output.
  assert!(service.tick_mock().unwrap());
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM task_failures WHERE stage='archive'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  service.store.connection.execute_batch("DROP TRIGGER reject_output").unwrap();
  drop(service);let mut service=ResourceService::open(&root).unwrap();
  let completed=service.finalize_mock(&first.id).unwrap();assert_eq!(completed.state,TaskState::Succeeded);
  assert!(completed.failure.is_none());
  assert_eq!(completed.outputs.len(),1);assert_ne!(completed.outputs[0].resource_id,input.resource.id);
  let (record,bytes)=service.read(&completed.outputs[0].resource_id).unwrap();
  assert_eq!(record.display_name,"模拟结果");assert_eq!(record.resource.provenance.task_id,Some(first.id.clone()));
  assert_eq!(record.resource.provenance.parent_ids,vec![input.resource.id.clone()]);
  assert_eq!(crate::resources::validate_image(bytes,"png").unwrap().width,64);
  assert_eq!(service.finalize_mock(&first.id).unwrap(),completed);
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  assert_eq!(service.store.connection.query_row("SELECT count(*) FROM task_failures WHERE task_id=?1",[&first.id],|r|r.get::<_,i64>(0)).unwrap(),0);
  assert_eq!(service.store.connection.query_row("SELECT sum(execution_count) FROM attempts",[],|r|r.get::<_,i64>(0)).unwrap(),4);
 }
}

#[cfg(test)]mod executor_boundary_tests{
 use super::*;
 #[test]fn mock_executor_does_not_consume_network_queue(){
  let root=std::env::temp_dir().join(format!("quickgen-executor-boundary-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=host.import_bytes(bytes.into_inner(),"png","fixture".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();let config=serde_json::json!({"abilities":[{"id":"clay","name":"fixture","prompt":"fixture"}]});
  let mut network=host.admit_mock(MockSubmission{submission_id:"network-fixture".into(),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&config).unwrap();
  let mut bound=config.clone();bound["abilities"][0]["imageBinding"]=serde_json::json!({"connection_id":"gateway","capability_id":"image-edit","revision":"v1"});
  assert!(host.admit_mock(MockSubmission{submission_id:"bound-card".into(),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&bound).is_err());assert_eq!(host.task_snapshot().unwrap().len(),1);
  network.request.capability.connection_id="network-fixture".into();network.capability_snapshot.reference.connection_id="network-fixture".into();
  host.store.connection.execute("UPDATE tasks SET task_json=?2 WHERE task_id=?1",rusqlite::params![network.id,serde_json::to_string(&network).unwrap()]).unwrap();
  assert!(host.execute_next_mock().unwrap().is_none(),"network queue must not run synthetic executor");
  let executions:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&network.id],|r|r.get(0)).unwrap();assert_eq!(executions,0);
  let local=host.admit_mock(MockSubmission{submission_id:"local-fixture".into(),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id,simulate_failure:false},&config).unwrap();
  assert_eq!(host.execute_next_mock().unwrap().unwrap().id,local.id);
  assert_eq!(host.task_snapshot().unwrap().into_iter().find(|t|t.id==network.id).unwrap().state,TaskState::Queued);
 }
}
