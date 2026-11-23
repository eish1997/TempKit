use crate::{contracts::*,credential_vault::Secret,gateway_profiles::GatewayProfile,image_protocol::ImageEditPolicy,resources::ResourceService,tasks::MockSubmission};

#[test]
fn cost_scope_rejects_omitted_quality_before_admission(){
 let root=std::env::temp_dir().join(format!("quickgen-cost-scope-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
 let mut host=ResourceService::open(&root).unwrap();
 host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"fixture".into(),base_url:"http://127.0.0.1:1".into()},Secret::from_bytes(b"synthetic-only".to_vec())).unwrap();
 host.record_gateway_check("c",Some(vec!["fixture".into()])).unwrap();
 let binding=host.register_image_binding("c",ImageEditPolicy{model:"fixture".into(),sizes:vec!["1024x1024".into()],qualities:vec!["low".into()],response_format:None}).unwrap();
 host.establish_image_budget(10).unwrap();
 let mut rule=crate::image_cost_rules::ImageCostRule{schema_version:1,capability:binding.capability.reference.clone(),ceiling_micros:1,expires_at_s:crate::image_budget::now_s().unwrap()+3600,evidence:"isolated synthetic pricing; no provider call".into(),scope:crate::image_cost_rules::ImageCostScope{size:"1024x1024".into(),quality:"low".into(),max_prompt_bytes:8,max_input_width:2,max_input_height:2,max_input_bytes:1024}};
 host.install_image_cost_rule(&rule).unwrap();
 // Approval and scope must commit atomically.
 let mut rejected_rule=rule.clone();rejected_rule.evidence="isolated rollback probe".into();
 host.store.connection.execute_batch("CREATE TRIGGER fail_cost_scope BEFORE INSERT ON image_cost_scopes BEGIN SELECT RAISE(ABORT,'isolated fault'); END;").unwrap();
 assert!(host.install_image_cost_rule(&rejected_rule).is_err());
 assert_eq!(host.store.connection.query_row("SELECT count(*) FROM image_cost_approvals",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 host.store.connection.execute_batch("DROP TRIGGER fail_cost_scope;").unwrap();
 let rule_path=root.join("isolated-cost-rule.json");std::fs::write(&rule_path,serde_json::to_vec(&rule).unwrap()).unwrap();
 assert!(crate::image_cost_rules::install_file(&root,&rule_path).is_err(),"live writer cannot be bypassed by installer");
 let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();
 let input=host.import_bytes(png.into_inner(),"png","original".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
 let mut config=serde_json::json!({"abilities":[{"id":"card","name":"fixture","prompt":"test","imageBinding":binding.capability.reference,"imageParameters":{"size":"1024x1024"}}]});
 let submission=MockSubmission{submission_id:"missing-quality".into(),batch_id:None,preset_id:"card".into(),resource_id:input.resource.id.clone(),simulate_failure:false};
 let submitted=host.admit_board_submission(submission.clone(),&config);
 assert!(submitted.is_err(),"a low-only cost estimate must not authorize an omitted quality/service default");
 assert!(host.task_snapshot().unwrap().is_empty());assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);
 config["abilities"][0]["imageParameters"]["quality"]=serde_json::json!("low");
 config["abilities"][0]["prompt"]=serde_json::json!("中文中文");
 assert!(host.admit_board_submission(submission.clone(),&config).is_err(),"prompt limit counts UTF-8 bytes");
 config["abilities"][0]["prompt"]=serde_json::json!("test");
 let mut large=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(3,2).write_to(&mut large,image::ImageFormat::Png).unwrap();
 let larger=host.import_bytes(large.into_inner(),"png","larger".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
 let mut too_large=submission.clone();too_large.resource_id=larger.resource.id.clone();
 assert!(host.admit_board_submission(too_large,&config).is_err());assert!(host.task_snapshot().unwrap().is_empty());
 let task=host.admit_board_submission(submission.clone(),&config).unwrap();
 // A newer rule tightens the limit while this task is queued.
 rule.scope.max_input_width=1;host.install_image_cost_rule(&rule).unwrap();
 drop(host);
 // Reinstalling an old rule is idempotent, not a way to supersede the newer rule.
 crate::image_cost_rules::install_file(&root,&rule_path).unwrap();
 let mut host=ResourceService::open(&root).unwrap();
 assert!(host.prepare_image_dispatch(&task.id).is_err());assert_eq!(host.image_budget_status().unwrap().reserved_micros,0);
 let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&task.id],|r|r.get(0)).unwrap();assert_eq!(count,0);
 assert_eq!(host.admit_board_submission(submission,&config).unwrap(),task,"replay preserves an already accepted task");
 assert!(host.read(&input.resource.id).is_ok());assert!(host.read(&larger.resource.id).is_ok());
 rule.scope.max_input_width=2;rule.expires_at_s+=1;host.install_image_cost_rule(&rule).unwrap();
 let dispatch=host.prepare_image_dispatch(&task.id).unwrap();assert_eq!(dispatch.task.state,TaskState::Running);assert_eq!(host.image_budget_status().unwrap().reserved_micros,1);
 let count:i64=host.store.connection.query_row("SELECT execution_count FROM attempts WHERE task_id=?1",[&task.id],|r|r.get(0)).unwrap();assert_eq!(count,1);
 assert!(host.prepare_image_dispatch(&task.id).is_err());
}
