//! Diagnostic-only localhost image provider. Never accepts a remote endpoint.
use crate::{resources::ResourceService,contracts::CapabilityRef};
use std::{io::{Read,Write},time::{Duration,Instant}};
const REMOTE:&str="https://file.302.ai/gpt/imgs/quickgen-isolated-fixture.png?signature=synthetic";
pub fn performance_mode()->Option<String>{std::env::var("QUICKGEN_PERFORMANCE").ok().filter(|v|matches!(v.as_str(),"small"|"large"))}
pub fn setup(service:&mut ResourceService)->Result<CapabilityRef,String>{
 if !service.task_snapshot()?.is_empty(){return Err("fixture requires no prior tasks".into())}
 let listener=std::net::TcpListener::bind("127.0.0.1:0").map_err(|e|e.to_string())?;
 let address=listener.local_addr().map_err(|e|e.to_string())?;
 listener.set_nonblocking(true).map_err(|e|e.to_string())?;
 start_download(service)?;
 let response=serde_json::json!({"data":[{"url":REMOTE}]}).to_string();
 let log=service.root.join("fixture-requests.txt");let performance=performance_mode().is_some();
 std::fs::write(service.root.join("staging/image-received-fixture.partial"),b"isolated incomplete response residue").map_err(|e|e.to_string())?;
 std::thread::Builder::new().name("isolated-image-fixture".into()).spawn(move||{
  let until=Instant::now()+Duration::from_secs(120);
  while Instant::now()<until{
   let (mut stream,_)=match listener.accept(){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(20));continue},Err(_)=>break};
   let _=stream.set_read_timeout(Some(Duration::from_secs(5)));let mut data=Vec::new();let mut buf=[0u8;4096];let mut valid=false;
   while data.len()<24*1024*1024{
    let n=match stream.read(&mut buf){Ok(0)|Err(_)=>break,Ok(n)=>n};data.extend_from_slice(&buf[..n]);
    if let Some(end)=data.windows(4).position(|v|v==b"\r\n\r\n"){
     let headers=String::from_utf8_lossy(&data[..end]);let length=headers.lines().find_map(|line|line.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v|v.trim().parse::<usize>().ok()));
     if data.starts_with(b"GET /v1/models HTTP/1.1"){
      let models=r#"{"data":[{"id":"fixture-image"}]}"#;
      let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",models.len(),models);
      break
     }
     if length.is_some_and(|n|data.len()>=end+4+n){valid=data.starts_with(b"POST /v1/images/edits HTTP/1.1");break}
    }
   }
   if data.starts_with(b"GET /v1/models HTTP/1.1"){continue}
   if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&log){let _=writeln!(file,"{}",if valid{"edit"}else{"invalid"});let _=file.sync_all();}
   if valid{if performance{std::thread::sleep(Duration::from_secs(3));}let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response);}
  }
 }).map_err(|e|e.to_string())?;
 service.configure_gateway(crate::gateway_profiles::GatewayProfile{schema_version:1,profile_id:"fixture".into(),connection_id:"fixture".into(),account_id:"synthetic".into(),base_url:format!("http://{address}")},crate::credential_vault::Secret::from_bytes(b"synthetic-fixture-token".to_vec()))?;
 service.record_gateway_check("fixture",Some(vec!["fixture-image".into()]))?;
 let binding=service.register_image_binding("fixture",crate::image_protocol::ImageEditPolicy{model:"fixture-image".into(),sizes:vec![],qualities:vec![],response_format:None})?;
 service.establish_image_budget(2)?;service.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s()?+3600)?;
 Ok(binding.capability.reference)
}
pub fn fault(service:&mut ResourceService,enabled:bool)->Result<serde_json::Value,String>{
 service.store.connection.execute_batch(if enabled{"CREATE TRIGGER IF NOT EXISTS generation_reject_save BEFORE INSERT ON pending_task_outputs BEGIN SELECT RAISE(ABORT,'isolated received image save failure'); END;"}else{"DROP TRIGGER IF EXISTS generation_reject_save;"}).map_err(|e|e.to_string())?;
 let executions:i64=service.store.connection.query_row("SELECT COALESCE(sum(execution_count),0) FROM attempts",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let requests=std::fs::read_to_string(service.root.join("fixture-requests.txt")).unwrap_or_default().lines().count();
 let downloads=std::fs::read_to_string(service.root.join("fixture-downloads.txt")).unwrap_or_default().lines().count();
 Ok(serde_json::json!({"executions":executions,"requests":requests,"downloads":downloads,"budget":service.image_budget_status()?}))
}

pub fn start_download(service:&ResourceService)->Result<serde_json::Value,String>{
 let listener=std::net::TcpListener::bind("127.0.0.1:0").map_err(|e|e.to_string())?;let address=listener.local_addr().map_err(|e|e.to_string())?;listener.set_nonblocking(true).map_err(|e|e.to_string())?;
 std::fs::write(service.root.join("fixture-download-address"),format!("http://{address}/output")).map_err(|e|e.to_string())?;
 let root=service.root.clone();let mut png=std::io::Cursor::new(Vec::new());
 let image=if performance_mode().as_deref()==Some("large"){
  let mut seed=0x12345678u32;let mut pixels=vec![0u8;2048*2048*3];for byte in &mut pixels{seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;*byte=seed as u8;}
  image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(2048,2048,pixels).unwrap())
 }else{image::DynamicImage::new_rgb8(8,6)};
 image.write_to(&mut png,image::ImageFormat::Png).map_err(|e|e.to_string())?;let bytes=png.into_inner();
 std::thread::Builder::new().name("isolated-download-fixture".into()).spawn(move||{
  let deadline=Instant::now()+Duration::from_secs(120);
  while Instant::now()<deadline{
   let(mut stream,_)=match listener.accept(){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(20));continue},Err(_)=>break};let _=stream.set_read_timeout(Some(Duration::from_secs(5)));
   let mut headers=Vec::new();let mut byte=[0u8;1];while !headers.ends_with(b"\r\n\r\n")&&headers.len()<16384{if stream.read_exact(&mut byte).is_err(){break}headers.push(byte[0]);}
   let headers=String::from_utf8_lossy(&headers).to_ascii_lowercase();let valid=headers.starts_with("get /output http/1.1")&&!headers.contains("authorization:")&&!headers.contains("cookie:");
   if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(root.join("fixture-downloads.txt")){let _=writeln!(file,"{}",if valid{"get"}else{"invalid"});let _=file.sync_all();}
   if valid&&root.join("fixture-download-ready").exists(){let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len());let _=stream.write_all(&bytes);}else{let _=stream.write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");}
  }
 }).map_err(|e|e.to_string())?;Ok(serde_json::json!({"started":true}))
}
pub fn clear_download(service:&ResourceService)->Result<serde_json::Value,String>{std::fs::write(service.root.join("fixture-download-ready"),b"ready").map_err(|e|e.to_string())?;Ok(serde_json::json!({"ready":true}))}
pub(crate) fn download(root:&std::path::Path,remote:&str)->Option<Result<Vec<u8>,String>>{
 // This override only exists in diagnostic builds and the explicitly selected
 // isolated recovery fixture. It cannot accept a caller-supplied remote URL.
 if std::env::var("QUICKGEN_DIAGNOSTIC").as_deref()!=Ok("generation-recovery")||remote!=REMOTE||!std::env::args().any(|a|a=="--isolated-data"){return None}
 Some(std::fs::read_to_string(root.join("fixture-download-address")).map_err(|_|"isolated download server unavailable".to_string()).and_then(|address|crate::gateway_discovery::download_fixture(&address)))
}

pub fn archive_fault(service:&mut ResourceService,enabled:bool)->Result<serde_json::Value,String>{
 service.store.connection.execute_batch(if enabled{"CREATE TRIGGER IF NOT EXISTS generation_reject_archive BEFORE INSERT ON resource_refs WHEN NEW.owner_kind='task_output' BEGIN SELECT RAISE(ABORT,'isolated image archive failure'); END;"}else{"DROP TRIGGER IF EXISTS generation_reject_archive;"}).map_err(|e|e.to_string())?;Ok(serde_json::json!({"enabled":enabled}))
}
pub fn drag_fixture(service:&mut ResourceService)->Result<serde_json::Value,String>{
 let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(4,3).write_to(&mut png,image::ImageFormat::Png).map_err(|e|e.to_string())?;
 let image=service.import_bytes(png.into_inner(),"png","isolated drag fixture".into(),crate::contracts::Provenance{task_id:None,parent_ids:vec![]})?;
 drop(service.prepare_drag_copy(&image.resource.id)?);
 std::fs::write(service.root.join("drag-exports/pending-fixture"),b"isolated incomplete drag copy").map_err(|e|e.to_string())?;
 service.drag_storage_status()
}
