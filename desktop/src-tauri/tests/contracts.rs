use quickgen_desktop::contracts::*;
use std::collections::{BTreeMap, BTreeSet};

fn fixture(name: &str) -> Registration {
    let json = match name {
        "image" => include_str!("../../../docs/quickgen/contracts/image-registration.json"),
        _ => include_str!("../../../docs/quickgen/contracts/harness-registration.json"),
    };
    serde_json::from_str(json).unwrap()
}
fn connection() -> Connection {
    Connection { schema_version: 1, id: "local-harness".into(), adapter_id: "mock".into(), profile_id: "test".into(), credential_ref: None, status: ConnectionStatus::Ready }
}
fn resource(id: &str, kind: ResourceKind) -> Resource {
    Resource { schema_version: 1, id: id.into(), blob_id: format!("blob-{id}"), byte_length: 100,
        mime: if kind == ResourceKind::Image { "image/png" } else { "text/plain" }.into(),
        media: if kind == ResourceKind::Image { Media::Image { width: 20, height: 30 } } else { Media::Text { encoding: "utf-8".into() } },
        provenance: Provenance { task_id: None, parent_ids: vec![] } }
}
fn request(reg: &Registration, id: &str) -> TaskRequest {
    TaskRequest { schema_version: 1, submission_id: format!("submit-{id}"), batch_id: Some("batch-1".into()),
        preset_id: reg.preset.id.clone(), capability: reg.capability.reference.clone(),
        inputs: vec![ResourceRef { role: "source".into(), resource_id: id.into() }], parameters: reg.preset.parameters.clone() }
}
fn supported() -> BTreeSet<ResourceKind> { [ResourceKind::Image,ResourceKind::Text].into() }

#[test]
fn image_batch_is_two_independent_valid_requests_with_one_batch() {
    let reg = fixture("image");
    assert_eq!(reg.validate(&connection(), None), Ok(RegistrationAction::Create));
    let resources = [resource("first",ResourceKind::Image),resource("second",ResourceKind::Image)].into_iter().map(|r|(r.id.clone(),r)).collect();
    let first = request(&reg,"first"); let second = request(&reg,"second");
    for req in [&first,&second] { req.validate(&reg.capability,&connection(),&resources,&supported()).unwrap(); }
    assert_ne!(first.submission_id,second.submission_id); assert_eq!(first.batch_id,second.batch_id);
    let mut accidental_together=first.clone(); accidental_together.inputs.extend(second.inputs);
    assert_eq!(accidental_together.validate(&reg.capability,&connection(),&resources,&supported()),Err(ContractError::InvalidInputs));
}

#[test]
fn harness_text_registration_request_and_archived_result_use_same_contract() {
    let reg=fixture("text"); reg.validate(&connection(),None).unwrap();
    let mut resources:BTreeMap<_,_>=[resource("notes",ResourceKind::Text)].into_iter().map(|r|(r.id.clone(),r)).collect();
    request(&reg,"notes").validate(&reg.capability,&connection(),&resources,&supported()).unwrap();
    let result=TaskResult { schema_version:1, outputs:vec![ResourceRef {role:"result".into(),resource_id:"actions".into()}],summary:None };
    assert_eq!(result.validate(&reg.capability,&resources,&supported()),Err(ContractError::MissingResource));
    resources.insert("actions".into(),resource("actions",ResourceKind::Text));
    result.validate(&reg.capability,&resources,&supported()).unwrap();
    let image_only=[ResourceKind::Image].into();
    assert_eq!(request(&reg,"notes").validate(&reg.capability,&connection(),&resources,&image_only),Err(ContractError::UnsupportedKind));
}

#[test]
fn registration_is_idempotent_and_never_silently_redefines_a_version() {
    let original=fixture("text"); let mut repeated=original.clone();
    repeated.preset.name="新导入名称".into();
    assert_eq!(repeated.validate(&connection(),Some(&original.capability)),Ok(RegistrationAction::KeepExisting));
    repeated.capability.execution.cancel=true;
    assert_eq!(repeated.validate(&connection(),Some(&original.capability)),Err(ContractError::VersionConflict));
    repeated.capability.reference.revision="v2".into(); repeated.preset.capability.revision="v2".into();
    assert_eq!(repeated.validate(&connection(),None),Ok(RegistrationAction::Create));
    assert_eq!(original.preset.capability.revision,"v1");
}

#[test]
fn disconnected_wrong_connection_and_missing_version_rejected() {
    let reg=fixture("text");let mut conn=connection();
    for state in [ConnectionStatus::Offline,ConnectionStatus::Unverified,ConnectionStatus::Revoked] {
        conn.status=state;assert_eq!(reg.validate(&conn,None),Err(ContractError::ConnectionUnavailable));
    }
    conn=connection();conn.id="other".into();assert_eq!(reg.validate(&conn,None),Err(ContractError::CapabilityMismatch));
    let mut req=request(&reg,"notes");req.capability.revision="missing".into();
    assert_eq!(req.validate(&reg.capability,&connection(),&BTreeMap::new(),&supported()),Err(ContractError::CapabilityMismatch));
}

#[test]
fn wrong_roles_kinds_missing_resources_and_invalid_parameters_rejected() {
    let reg=fixture("text");let resources=[resource("notes",ResourceKind::Text),resource("photo",ResourceKind::Image)].into_iter().map(|r|(r.id.clone(),r)).collect();
    let mut req=request(&reg,"notes");req.inputs[0].role="typo".into();
    assert_eq!(req.validate(&reg.capability,&connection(),&resources,&supported()),Err(ContractError::InvalidInputs));
    assert_eq!(request(&reg,"photo").validate(&reg.capability,&connection(),&resources,&supported()),Err(ContractError::InvalidInputs));
    assert_eq!(request(&reg,"missing").validate(&reg.capability,&connection(),&resources,&supported()),Err(ContractError::MissingResource));
    for value in [ParameterValue::Text(" ".into()),ParameterValue::Boolean(true),ParameterValue::Text("x".repeat(65))] {
        req=request(&reg,"notes");req.parameters.insert("language".into(),value);
        assert_eq!(req.validate(&reg.capability,&connection(),&resources,&supported()),Err(ContractError::InvalidParameters));
    }
    req=request(&reg,"notes");req.parameters.clear();assert_eq!(reg.capability.validate_parameters(&req.parameters),Err(ContractError::InvalidParameters));
    req=request(&reg,"notes");req.parameters.insert("unknown".into(),ParameterValue::Integer(1));assert_eq!(reg.capability.validate_parameters(&req.parameters),Err(ContractError::InvalidParameters));
}

#[test]
fn unknown_versions_types_fields_and_path_like_ids_rejected() {
    let mut reg=fixture("image");reg.schema_version=2;assert_eq!(reg.validate(&connection(),None),Err(ContractError::UnsupportedVersion));
    let mut json=serde_json::to_value(fixture("image")).unwrap();json["capability"]["inputs"][0]["kinds"][0]="video".into();
    assert!(serde_json::from_value::<Registration>(json).is_err());
    let mut json=serde_json::to_value(fixture("image")).unwrap();json["shell_command"]="do not execute".into();
    assert!(serde_json::from_value::<Registration>(json).is_err());
    let mut r=resource("a",ResourceKind::Image);r.blob_id="../secret".into();assert_eq!(r.validate(),Err(ContractError::InvalidIdentity));
}

#[test]
fn state_machine_requires_archive_and_keeps_terminal_attempts_terminal() {
    use TaskState::*;
    assert_eq!(Queued.transition(Succeeded,true),Err(ContractError::InvalidTransition));
    Queued.transition(Running,false).unwrap();Running.transition(Finalizing,false).unwrap();
    assert_eq!(Finalizing.transition(Succeeded,false),Err(ContractError::OutputsNotArchived));
    Finalizing.transition(Succeeded,true).unwrap();Running.transition(Interrupted,false).unwrap();
    for terminal in [Succeeded,Failed,Cancelled,Interrupted] { assert_eq!(terminal.transition(Running,false),Err(ContractError::InvalidTransition)); }
    assert_eq!(Finalizing.transition(Running,true),Err(ContractError::InvalidTransition));
}

#[test]
fn stale_or_wrong_attempt_events_cannot_override_current_state() {
    let current=TaskEvent {schema_version:1,task_id:"task-1".into(),attempt:2,event_version:7,state:TaskState::Running,failure:None};
    let mut event=current.clone();assert!(!event.is_newer_than(&current));
    event.event_version=8;assert!(event.is_newer_than(&current));
    event.attempt=1;assert!(!event.is_newer_than(&current));
    event.attempt=2;event.task_id="other".into();assert!(!event.is_newer_than(&current));
}

#[test]
fn scene_references_resources_without_owning_content_and_rejects_dangling_selection() {
    let resources=[resource("photo",ResourceKind::Image)].into_iter().map(|r|(r.id.clone(),r)).collect();
    let mut scene=BoardScene {schema_version:1,id:"main".into(),revision:1,items:vec![Placement {id:"item-1".into(),resource_id:"photo".into(),x:4.,y:5.}],camera_x:0.,camera_y:0.,zoom:1.,selected_item_ids:vec!["item-1".into()]};
    scene.validate(&resources).unwrap();scene.selected_item_ids.push("missing".into());assert_eq!(scene.validate(&resources),Err(ContractError::InvalidInputs));
    scene.selected_item_ids.clear();scene.items[0].resource_id="missing".into();assert_eq!(scene.validate(&resources),Err(ContractError::MissingResource));
}
