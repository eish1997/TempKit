//! Opt-in local diagnosis, not a desktop responsiveness acceptance test.
use super::*;
use std::time::Instant;

#[test]
#[ignore = "explicit local archive timing probe; no network requests"]
fn measure_archive_host_occupancy() {
 let root=std::env::temp_dir().join(format!("quickgen-archive-probe-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
 let mut host=ResourceService::open(&root).unwrap();
 host.configure_gateway(crate::gateway_profiles::GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:"http://127.0.0.1:1".into()},crate::credential_vault::Secret::from_bytes(b"synthetic-only".to_vec())).unwrap();
 host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
 let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec![],qualities:vec![],response_format:None}).unwrap();
 host.establish_image_budget(20).unwrap();host.approve_image_cost(&binding.capability.reference,1,crate::image_budget::now_s().unwrap()+3600).unwrap();
 let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();
 let input=host.import_bytes(png.into_inner(),"png","probe original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
 let config=serde_json::json!({"abilities":[{"id":"probe","name":"probe","prompt":"synthetic timing probe","imageBinding":binding.capability.reference,"imageParameters":{}}]});
 let mut note=host.note_create().unwrap();let mut samples=Vec::new();
 for side in [32u32,2048] {for sample in 0..5 {
  // Deterministic incompressible pixels; each result differs so dedup cannot hide writes.
  let mut seed=0x12345678u32+sample;let mut pixels=vec![0u8;(side*side*3) as usize];
  for byte in &mut pixels {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;*byte=seed as u8;}
  let rgb=image::RgbImage::from_raw(side,side,pixels).unwrap();let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgb8(rgb).write_to(&mut png,image::ImageFormat::Png).unwrap();
  let validated=crate::resources::validate_image(png.into_inner(),"png").unwrap();
  let submission=crate::tasks::MockSubmission{submission_id:format!("probe-{side}-{sample}"),batch_id:None,preset_id:"probe".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
  let task=host.admit_board_submission(submission,&config).unwrap();
  // Only prepares the request and durable dispatch claim; never invokes transport.
  let dispatch=host.prepare_image_dispatch(&task.id).unwrap();
  let begin=Instant::now();let pending=host.stage_image_output(&dispatch.task,&validated).unwrap();let stage_ms=begin.elapsed().as_secs_f64()*1000.;
  let begin=Instant::now();let done=host.complete_image_archive(&pending,Ok(&validated)).unwrap();let archive_ms=begin.elapsed().as_secs_f64()*1000.;
  assert_eq!(done.state,TaskState::Succeeded);
  let begin=Instant::now();host.task_snapshot().unwrap();let query_ms=begin.elapsed().as_secs_f64()*1000.;
  let begin=Instant::now();note=host.note_patch(&note.id,crate::notes::NotePatch::Text{revision:note.text_revision,text:format!("probe {side} {sample}")}).unwrap();let note_ms=begin.elapsed().as_secs_f64()*1000.;
  assert_eq!(host.read(&done.outputs[0].resource_id).unwrap().1,validated.bytes);assert!(host.read(&input.resource.id).is_ok());
  samples.push(serde_json::json!({"side":side,"sample":sample,"bytes":validated.bytes.len(),"stage_ms":stage_ms,"archive_ms":archive_ms,"query_after_ms":query_ms,"note_after_ms":note_ms}));
 }}
 let report=serde_json::json!({"root":root,"network_requests":0,"samples":samples,"limitation":"Direct synchronous service occupancy, not queued IPC, memory peak, WebView input latency or historical baseline. Decode/encode excluded; no performance acceptance inferred."});
 let path=root.join("archive-probe.json");std::fs::write(&path,serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("ARCHIVE_PROBE={}",path.display());
}
