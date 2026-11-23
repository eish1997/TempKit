//! Isolated protocol/queue regression; never connects to a configured gateway.
use crate::{contracts::*,credential_vault::Secret,gateway_profiles::GatewayProfile,image_execution::ImageExecution,image_protocol::ImageEditPolicy,resources::ResourceService,tasks::{MockSubmission,TaskAction,TaskControl}};
use std::{io::{Read,Write},time::{Duration,Instant}};
use base64::Engine;

#[test]
fn rejected_batch_item_does_not_block_success_or_run_cancelled_item(){
 let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();listener.set_nonblocking(true).unwrap();
 let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let bytes=png.into_inner();
 let mut output=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(4,2).write_to(&mut output,image::ImageFormat::Png).unwrap();
 let body=serde_json::json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(output.into_inner())}]}).to_string();
 let (done,finished)=std::sync::mpsc::channel();
 let server=std::thread::spawn(move||{
  let deadline=Instant::now()+Duration::from_secs(15);let mut requests=0;
  loop{
   if finished.try_recv().is_ok(){break}assert!(Instant::now()<deadline,"batch fixture deadline");
   let(mut stream,_)=match listener.accept(){Ok(s)=>s,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(5));continue},Err(e)=>panic!("{e}")};
   stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut request=Vec::new();let mut buffer=[0u8;4096];
   loop{let n=stream.read(&mut buffer).unwrap();assert!(n>0);request.extend_from_slice(&buffer[..n]);assert!(request.len()<1024*1024);
    if let Some(end)=request.windows(4).position(|v|v==b"\r\n\r\n"){
     let headers=String::from_utf8_lossy(&request[..end]);let length:usize=headers.lines().find_map(|line|line.to_ascii_lowercase().strip_prefix("content-length:").map(|v|v.trim().parse().unwrap())).unwrap();if request.len()>=end+4+length{break}
    }
   }
   assert!(request.starts_with(b"POST /v1/images/edits HTTP/1.1"));requests+=1;assert!(requests<=2,"unexpected resubmission or cancelled execution");
   let(status,response)=if requests==1{("400 Bad Request","{\"error\":\"isolated rejection\"}")}else{("200 OK",body.as_str())};
   write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
  }
  assert_eq!(requests,2);assert!(matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock),"unexpected pending request");
 });
 let root=std::env::temp_dir().join(format!("quickgen-batch-{}-{}",std::process::id(),crate::image_budget::now_s().unwrap()));let mut host=ResourceService::open(&root).unwrap();
 host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"synthetic".into(),base_url:format!("http://{address}")},Secret::from_bytes(b"synthetic-only".to_vec())).unwrap();host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
 let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec![],qualities:vec![],response_format:Some("b64_json".into())}).unwrap();host.establish_image_budget(3).unwrap();host.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
 let input=host.import_bytes(bytes.clone(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();let config=serde_json::json!({"abilities":[{"id":"card","name":"fixture","prompt":"isolated batch","imageBinding":binding.capability.reference,"imageParameters":{}}]});
 let mut tasks=Vec::new();for n in 0..3{tasks.push(host.admit_board_submission(MockSubmission{submission_id:format!("batch-{n}"),batch_id:Some("shared-batch".into()),preset_id:"card".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&config).unwrap());}
 let cancelled=&tasks[2];host.control_mock(TaskControl{task_id:cancelled.id.clone(),attempt:cancelled.attempt,event_version:cancelled.event_version,action:TaskAction::Cancel}).unwrap();
 let mut worker=ImageExecution::default();let deadline=Instant::now()+Duration::from_secs(10);
 loop{worker.poll(&mut host).unwrap();worker.start_next(&mut host).unwrap();let current=host.task_snapshot().unwrap();if !worker.busy()&&current.iter().all(|t|matches!(t.state,TaskState::Interrupted|TaskState::Succeeded|TaskState::Cancelled)){break}assert!(Instant::now()<deadline,"batch did not settle");std::thread::sleep(Duration::from_millis(5));}
 for _ in 0..5{assert!(!worker.start_next(&mut host).unwrap());assert!(worker.poll(&mut host).unwrap().is_none());}
 done.send(()).unwrap();server.join().unwrap();let current=host.task_snapshot().unwrap();assert_eq!(current.iter().filter(|t|t.state==TaskState::Interrupted).count(),1);assert_eq!(current.iter().filter(|t|t.state==TaskState::Succeeded).count(),1);assert_eq!(current.iter().find(|t|t.id==cancelled.id).unwrap().state,TaskState::Cancelled);assert!(current.iter().all(|t|t.attempt==1));
 let successful=current.iter().find(|t|t.state==TaskState::Succeeded).unwrap();assert_eq!(successful.outputs.len(),1);assert_ne!(successful.outputs[0].resource_id,input.resource.id);assert_ne!(host.read(&successful.outputs[0].resource_id).unwrap().1,bytes);
 let initial=serde_json::json!({"version":1,"resourceSchemaVersion":1,"items":[],"jobs":[],"selection":[],"scale":1,"tx":0,"ty":0});host.initialize_scene(initial).unwrap();
 let placed=host.place_task_output(0,&successful.id,1,&successful.outputs[0].resource_id,1.,2.).unwrap();
 assert_eq!(placed["items"][0]["simulated"],false);assert_eq!(placed["items"][0]["name"],"生成结果");
 // Reproduce a legacy persisted real result; read repair keeps identity/revision.
 let mut legacy=placed.clone();legacy["items"][0]["simulated"]=serde_json::json!(true);legacy["items"][0]["name"]=serde_json::json!("原图 · 能力（模拟结果）");
 host.store.connection.execute("UPDATE scenes SET scene_json=?1 WHERE scene_id='main'",[legacy.to_string()]).unwrap();
 let repaired=host.scene_snapshot().unwrap().unwrap();assert_eq!(repaired["revision"],placed["revision"]);assert_eq!(repaired["items"][0]["id"],placed["items"][0]["id"]);assert_eq!(repaired["items"][0]["simulated"],false);assert_eq!(repaired["items"][0]["name"],"原图 · 能力（生成结果）");
 let saved=host.save_scene(1,legacy).unwrap();assert_eq!(saved["items"],repaired["items"]);
 let mut removed=saved.clone();removed["items"]=serde_json::json!([]);let removed=host.save_scene(2,removed).unwrap();
 assert_eq!(host.place_task_output(3,&successful.id,1,&successful.outputs[0].resource_id,1.,2.).unwrap(),removed);
 assert_eq!(host.store.connection.query_row("SELECT count(*) FROM result_placements",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 let rejected=current.iter().find(|t|t.state==TaskState::Interrupted).unwrap();assert!(rejected.failure.as_ref().unwrap().outcome_unknown);assert!(rejected.outputs.is_empty());assert!(current.iter().find(|t|t.id==cancelled.id).unwrap().outputs.is_empty());
 assert_eq!(host.image_budget_status().unwrap().reserved_micros,2);assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);
 let counts:Vec<i64>=host.store.connection.prepare("SELECT execution_count FROM attempts ORDER BY execution_count").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();assert_eq!(counts,vec![0,1,1]);
 drop(host);let host=ResourceService::open(&root).unwrap();assert_eq!(host.task_snapshot().unwrap(),current);assert_eq!(host.image_budget_status().unwrap().reserved_micros,2);assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);
}
