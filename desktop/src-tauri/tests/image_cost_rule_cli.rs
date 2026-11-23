//! Exercises the ordinary library and actual executable; no HTTP request is sent.
use quickgen_desktop::{contracts::*,credential_vault::Secret,gateway_profiles::GatewayProfile,image_cost_rules::{ImageCostRule,ImageCostScope},image_protocol::ImageEditPolicy,resources::ResourceService,tasks::MockSubmission};
use std::{path::Path,process::{Command,Stdio},io::Write};
use sha2::{Digest,Sha256};

fn install(exe:&Path,root:&Path,file:&Path)->std::process::Output{
 Command::new(exe).arg("--isolated-data").arg(root).arg("--install-image-cost-rule").arg(file).output().unwrap()
}
#[test]
#[ignore = "requires an explicitly selected ordinary candidate via QUICKGEN_COST_RULE_TEST_EXE"]
fn ordinary_executable_installs_scoped_rule_without_spending_or_bypassing_lock(){
 let exe=std::env::var_os("QUICKGEN_COST_RULE_TEST_EXE").expect("set QUICKGEN_COST_RULE_TEST_EXE to the ordinary candidate");let exe=Path::new(&exe);
 let root=std::env::temp_dir().join(format!("quickgen-cost-cli-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
 std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join("quickgen-isolated-v1"),b"QuickGen isolated test data v1").unwrap();
 let store=root.join("resource-store");let mut host=ResourceService::open(&store).unwrap();
 host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"synthetic".into(),base_url:"http://127.0.0.1:1".into()},Secret::from_bytes(b"synthetic-no-network".to_vec())).unwrap();
 host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
 let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec!["1024x1024".into()],qualities:vec!["low".into()],response_format:None}).unwrap();host.establish_image_budget(10).unwrap();
 let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();let bytes=png.into_inner();
 // A library integration test is not the same-executable decoder host.
 // Validate with the selected real exe, then seed only this synthetic resource.
 let mut decoder=Command::new(exe).args(["--quickgen-decode","png"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
 decoder.stdin.take().unwrap().write_all(&bytes).unwrap();let decoded=decoder.wait_with_output().unwrap();assert!(decoded.status.success());
 assert!(String::from_utf8_lossy(&decoded.stdout).contains("QG_DECODE:{\"Ok\":[2,2,\"image/png\"]}"));
 let input=quickgen_desktop::resources::StoredImage{resource:Resource{schema_version:1,id:"r-cli-input".into(),blob_id:"b-r-cli-input".into(),mime:"image/png".into(),byte_length:bytes.len() as u64,media:Media::Image{width:2,height:2},provenance:Provenance{task_id:None,parent_ids:vec![]}},display_name:"original".into(),imported_at_ms:1,digest:format!("{:x}",Sha256::digest(&bytes))};
 drop(host);std::fs::write(store.join("blobs").join(&input.resource.blob_id),&bytes).unwrap();
 let db=rusqlite::Connection::open(store.join("metadata.sqlite")).unwrap();
 db.execute("INSERT INTO blobs VALUES(?1,?2,?3)",rusqlite::params![input.resource.blob_id,input.digest,input.resource.byte_length as i64]).unwrap();
 db.execute("INSERT INTO resources VALUES(?1,?2,?3,'available')",rusqlite::params![input.resource.id,input.resource.blob_id,serde_json::to_string(&input).unwrap()]).unwrap();drop(db);
 let host=ResourceService::open(&store).unwrap();
 let config=serde_json::json!({"abilities":[{"id":"card","name":"fixture","prompt":"test","imageBinding":binding.capability.reference,"imageParameters":{"size":"1024x1024","quality":"low"}}]});
 let submission=MockSubmission{submission_id:"ordinary-rule".into(),batch_id:None,preset_id:"card".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
 let rule=ImageCostRule{schema_version:1,capability:binding.capability.reference.clone(),ceiling_micros:1,expires_at_s:quickgen_desktop::image_budget::now_s().unwrap()+3600,evidence:"synthetic isolated CLI fixture, not real pricing".into(),scope:ImageCostScope{size:"1024x1024".into(),quality:"low".into(),max_prompt_bytes:16,max_input_width:2,max_input_height:2,max_input_bytes:1024}};
 let file=root.join("rule.json");std::fs::write(&file,serde_json::to_vec(&rule).unwrap()).unwrap();
 assert!(!install(exe,&root,&file).status.success(),"running writer must block installer");assert!(host.task_snapshot().unwrap().is_empty());drop(host);
 // Simulate an old diagnostic approval, without exposing that API in ordinary builds.
 let db=rusqlite::Connection::open(store.join("metadata.sqlite")).unwrap();
 db.execute("INSERT INTO image_cost_approvals VALUES('legacy-fixture','c','image-edit',?1,1,?2)",rusqlite::params![binding.capability.reference.revision,rule.expires_at_s as i64]).unwrap();drop(db);
 let mut host=ResourceService::open(&store).unwrap();
 assert!(host.admit_board_submission(submission.clone(),&config).unwrap_err().contains("适用范围"));assert!(host.task_snapshot().unwrap().is_empty());drop(host);
 let result=install(exe,&root,&file);assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));assert!(String::from_utf8_lossy(&result.stdout).contains("Installed image cost rule scoped-"));
 assert!(install(exe,&root,&file).status.success(),"same rule is idempotent");
 let invalid=root.join("invalid.json");std::fs::write(&invalid,b"{\"secret\":\"must-not-echo\"}").unwrap();let rejected=install(exe,&root,&invalid);assert!(!rejected.status.success());assert!(!String::from_utf8_lossy(&rejected.stderr).contains("must-not-echo"));
 std::fs::write(&invalid,vec![b' ';16385]).unwrap();assert!(!install(exe,&root,&invalid).status.success());
 let production=Command::new(exe).arg("--install-image-cost-rule").arg(&file).output().unwrap();assert!(!production.status.success());assert!(String::from_utf8_lossy(&production.stderr).contains("requires --isolated-data"));
 let mut host=ResourceService::open(&store).unwrap();assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);assert!(host.task_snapshot().unwrap().is_empty());
 let mut missing=config.clone();missing["abilities"][0]["imageParameters"].as_object_mut().unwrap().remove("quality");assert!(host.admit_board_submission(submission.clone(),&missing).is_err());
 let task=host.admit_board_submission(submission,&config).unwrap();let dispatch=host.prepare_image_dispatch(&task.id).unwrap();assert_eq!(dispatch.task.state,TaskState::Running);assert_eq!(host.image_budget_status().unwrap().reserved_micros,1);assert!(host.prepare_image_dispatch(&task.id).is_err());assert_eq!(host.read(&input.resource.id).unwrap().1,bytes);
 // We only prepared bytes. No transport is started by this test or installer.
 println!("isolated fixture: {}",root.display());
}
