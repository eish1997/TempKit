//! Full-fidelity board document with optimistic concurrency and durable placement receipts.
use crate::{contracts::{BoardScene,Placement,TaskState},resources::ResourceService};
use serde_json::{Value,json};
use rusqlite::{OptionalExtension,Transaction};
use std::collections::{BTreeMap,BTreeSet};
#[derive(Debug,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputPlacement {pub revision:u64,pub task_id:String,pub attempt:u32,pub resource_id:String,pub x:f64,pub y:f64}

impl ResourceService {
    pub fn scene_snapshot(&self)->Result<Option<Value>,String>{
        let value:Option<String>=self.store.connection.query_row("SELECT scene_json FROM scenes WHERE scene_id='main'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        value.map(|v|{let mut scene=serde_json::from_str(&v).map_err(|e|e.to_string())?;self.normalize_result_labels(&mut scene)?;Ok(scene)}).transpose()
    }
    // Repair only existing, host-proven real outputs from older scene writers.
    // Read normalization does not create placements or change the scene revision.
    fn normalize_result_labels(&self,scene:&mut Value)->Result<(),String>{
        if let Some(items)=scene["items"].as_array_mut(){for item in items{
            if item["simulated"]!=true{continue}
            let (Some(id),Some(attempt),Some(resource))=(item["taskId"].as_str(),item["attempt"].as_u64(),item["resourceId"].as_str())else{continue};
            let raw:Option<String>=self.store.connection.query_row("SELECT task_json FROM tasks WHERE task_id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            let Some(raw)=raw else{continue};let task:crate::contracts::Task=serde_json::from_str(&raw).map_err(|e|e.to_string())?;
            if task.request.capability.connection_id=="internal-mock"||u64::from(task.attempt)!=attempt||task.state!=TaskState::Succeeded||!task.outputs.iter().any(|o|o.resource_id==resource){continue}
            let name=item["name"].as_str().and_then(|name|if name=="模拟结果"{Some("生成结果".to_string())}else{name.strip_suffix("（模拟结果）").map(|prefix|format!("{prefix}（生成结果）"))});
            item["simulated"]=json!(false);if let Some(name)=name{item["name"]=json!(name)}
        }}Ok(())
    }
    /// Internal initialization; activation coordinates this with its durable marker.
    pub fn initialize_scene(&mut self,mut scene:Value)->Result<Value,String>{
        if self.scene_snapshot()?.is_some(){return Err("scene already initialized".into())}
        scene["revision"]=json!(0);let refs=self.validate_scene(&scene)?;
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO scenes VALUES('main',0,?1)",[scene.to_string()]).map_err(|e|e.to_string())?;
        retain_scene(&tx,&refs)?;tx.commit().map_err(|e|e.to_string())?;Ok(scene)
    }
    pub fn save_scene(&mut self,expected:u64,mut scene:Value)->Result<Value,String>{
        self.normalize_result_labels(&mut scene)?;
        if scene["revision"].as_u64()!=Some(expected){return Err("scene_version_conflict".into())}
        scene["revision"]=json!(expected.checked_add(1).ok_or("scene revision overflow")?);
        let refs=self.validate_scene(&scene)?;
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        write_scene(&tx,expected,&scene,&refs)?;tx.commit().map_err(|e|e.to_string())?;Ok(scene)
    }
    /// The receipt survives user deletion of an item: replay cannot resurrect it.
    pub fn place_task_output(&mut self,expected:u64,id:&str,attempt:u32,resource:&str,x:f64,y:f64)->Result<Value,String>{
        let mut scene=self.scene_snapshot()?.ok_or("scene not initialized")?;
        let placed:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM result_placements WHERE task_id=?1 AND attempt=?2 AND resource_id=?3)",rusqlite::params![id,attempt,resource],|r|r.get(0)).map_err(|e|e.to_string())?;
        if placed{return Ok(scene)}
        if scene["revision"].as_u64()!=Some(expected){return Err("scene_version_conflict".into())}
        if !x.is_finite()||!y.is_finite(){return Err("invalid placement coordinates".into())}
        let task=self.task_snapshot()?.into_iter().find(|t|t.id==id).ok_or("task not found")?;
        if task.attempt!=attempt||task.state!=TaskState::Succeeded||!task.outputs.iter().any(|r|r.resource_id==resource){return Err("output does not belong to successful attempt".into())}
        let placement=format!("result-{id}-{attempt}");
        let simulated=task.request.capability.connection_id=="internal-mock";let label=if simulated{"模拟结果"}else{"生成结果"};
        let name=scene["jobs"].as_array().and_then(|jobs|jobs.iter().find(|job|job["hostTaskId"].as_str()==Some(id)||job["submissionId"].as_str()==Some(&task.request.submission_id)))
            .map(|job|format!("{} · {}（{label}）",job["input"]["name"].as_str().unwrap_or("图片"),job["abilitySnapshot"]["name"].as_str().unwrap_or("能力"))).unwrap_or(label.into());
        let order=scene["nextId"].as_u64().unwrap_or(1);let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis() as u64;
        scene["nextId"]=json!(order.checked_add(1).ok_or("scene item counter overflow")?);
        let items=scene["items"].as_array_mut().ok_or("missing items")?;
        if items.iter().any(|i|i["id"].as_str()==Some(&placement)){return Err("placement identity conflict".into())}
        items.push(json!({"id":placement,"resourceId":resource,"name":name,"result":true,"simulated":simulated,"taskId":id,"attempt":attempt,"x":x,"y":y,"addedAt":now,"order":order}));
        scene["revision"]=json!(expected.checked_add(1).ok_or("scene revision overflow")?);
        let refs=self.validate_scene(&scene)?;
        let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
        write_scene(&tx,expected,&scene,&refs)?;
        tx.execute("INSERT INTO result_placements VALUES(?1,?2,?3,?4)",rusqlite::params![id,attempt,resource,placement]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;Ok(scene)
    }
    pub(crate) fn validate_scene(&self,scene:&Value)->Result<BTreeSet<String>,String>{
        if scene["version"]!=1||scene["resourceSchemaVersion"]!=1{return Err("unsupported scene version".into())}
        let mut refs=BTreeSet::new();let mut resources=BTreeMap::new();let mut items=vec![];
        for item in scene["items"].as_array().ok_or("missing scene items")? {
            if item.get("src").is_some(){return Err("inline scene image rejected".into())}
            let resource=item["resourceId"].as_str().ok_or("missing resource ID")?.to_string();refs.insert(resource.clone());
            items.push(Placement{id:item["id"].as_str().ok_or("missing item ID")?.into(),resource_id:resource,x:item["x"].as_f64().ok_or("missing x")?,y:item["y"].as_f64().ok_or("missing y")?});
        }
        for job in scene["jobs"].as_array().ok_or("missing scene jobs")? {
            if job["input"].get("src").is_some(){return Err("inline job image rejected".into())}
            if let Some(input)=job["input"]["resourceId"].as_str(){refs.insert(input.into());}
            if let Some(outputs)=job["outputResourceIds"].as_array(){for output in outputs{refs.insert(output.as_str().ok_or("invalid job output")?.into());}}
        }
        // Saves validate authoritative metadata, without reading all image bodies
        // on every drag. Blob integrity is checked by import/recovery/resource_read.
        for id in &refs {
            let encoded:String=self.store.connection.query_row("SELECT metadata_json FROM resources WHERE resource_id=?1 AND state='available'",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
            let record:crate::resources::StoredImage=serde_json::from_str(&encoded).map_err(|e|e.to_string())?;
            record.resource.validate().map_err(|e|format!("{e:?}"))?;resources.insert(id.clone(),record.resource);
        }
        let selected=scene["selection"].as_array().ok_or("missing selection")?.iter().map(|v|v.as_str().map(String::from).ok_or("invalid selection".to_string())).collect::<Result<Vec<_>,_>>()?;
        BoardScene{schema_version:1,id:"main".into(),revision:scene["revision"].as_u64().ok_or("invalid revision")?,items,camera_x:scene["tx"].as_f64().ok_or("invalid tx")?,camera_y:scene["ty"].as_f64().ok_or("invalid ty")?,zoom:scene["scale"].as_f64().ok_or("invalid scale")?,selected_item_ids:selected}.validate(&resources).map_err(|e|format!("{e:?}"))?;
        Ok(refs)
    }
}
pub(crate) fn retain_scene(tx:&Transaction,refs:&BTreeSet<String>)->Result<(),String>{
    tx.execute("DELETE FROM resource_refs WHERE owner_kind='scene' AND owner_id='main'",[]).map_err(|e|e.to_string())?;
    for id in refs {tx.execute("INSERT INTO resource_refs VALUES('scene','main',?1)",[id]).map_err(|e|e.to_string())?;}Ok(())
}
fn write_scene(tx:&Transaction,expected:u64,scene:&Value,refs:&BTreeSet<String>)->Result<(),String>{
    let changed=tx.execute("UPDATE scenes SET revision=?2,scene_json=?3 WHERE scene_id='main' AND revision=?1",rusqlite::params![expected as i64,scene["revision"].as_u64().unwrap() as i64,scene.to_string()]).map_err(|e|e.to_string())?;
    if changed!=1{return Err("scene_version_conflict".into())}retain_scene(tx,refs)
}
#[cfg(test)]mod tests {
 use super::*;
 use crate::{contracts::Provenance,tasks::MockSubmission};
 #[test]fn concurrent_save_and_result_receipt_are_atomic_and_survive_restart(){
  let root=std::env::temp_dir().join(format!("quickgen-scenes-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut service=ResourceService::open(&root).unwrap();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let input=service.import_bytes(bytes.into_inner(),"png","input".into(),Provenance{task_id:None,parent_ids:vec![]}).unwrap();
  let scene=json!({"version":1,"resourceSchemaVersion":1,"items":[{"id":"input","resourceId":input.resource.id,"x":12,"y":34,"name":"原图","extra":"keep"}],"jobs":[],"selection":["input"],"scale":1,"tx":2,"ty":3,"pinned":true,"custom":{"keep":true}});
  let original=service.initialize_scene(scene).unwrap();assert!(service.initialize_scene(original.clone()).is_err());assert!(service.retire(&input.resource.id).is_err());
  let task=service.admit_mock(MockSubmission{submission_id:"scene-result".into(),batch_id:None,preset_id:"clay".into(),resource_id:input.resource.id.clone(),simulate_failure:false},&json!({"abilities":[{"id":"clay","name":"mock","prompt":"fixed"}]})).unwrap();
  service.tick_mock().unwrap();service.tick_mock().unwrap();let task=service.task_snapshot().unwrap().into_iter().find(|t|t.id==task.id).unwrap();let output=&task.outputs[0].resource_id;
  let mut moved=original.clone();moved["items"][0]["x"]=json!(222);let moved=service.save_scene(0,moved).unwrap();
  assert_eq!(service.save_scene(0,original.clone()).unwrap_err(),"scene_version_conflict");
  assert_eq!(service.place_task_output(0,&task.id,1,output,240.,34.).unwrap_err(),"scene_version_conflict");
  service.store.connection.execute_batch("CREATE TRIGGER reject_placement BEFORE INSERT ON result_placements BEGIN SELECT RAISE(ABORT,'placement failed'); END;").unwrap();
  assert!(service.place_task_output(1,&task.id,1,output,240.,34.).is_err());assert_eq!(service.scene_snapshot().unwrap().unwrap(),moved);
  service.store.connection.execute_batch("DROP TRIGGER reject_placement").unwrap();
  let placed=service.place_task_output(1,&task.id,1,output,240.,34.).unwrap();assert_eq!(placed["items"][0]["x"],222);assert_eq!(placed["custom"],original["custom"]);assert_eq!(placed["items"].as_array().unwrap().len(),2);
  assert_eq!(service.place_task_output(1,&task.id,1,output,999.,999.).unwrap(),placed);
  let mut removed=placed.clone();removed["items"].as_array_mut().unwrap().pop();let removed=service.save_scene(2,removed).unwrap();
  drop(service);let mut service=ResourceService::open(&root).unwrap();assert_eq!(service.place_task_output(0,&task.id,1,output,0.,0.).unwrap(),removed);
  let mut invalid=removed.clone();invalid["items"][0]["resourceId"]=json!("missing");assert!(service.save_scene(3,invalid).is_err());assert_eq!(service.scene_snapshot().unwrap().unwrap(),removed);
 }
}
