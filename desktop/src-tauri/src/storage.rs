use tauri::Emitter;
use base64::Engine;
use serde_json::{Value,json};
use std::{sync::Mutex,fs,io::Write,path::PathBuf};
static LOCK:Mutex<()>=Mutex::new(());
fn directory(app:&tauri::AppHandle)->Result<PathBuf,String>{crate::runtime_paths::data(app)}
fn defaults()->Value{json!({"version":1,"theme":"system","layout":{"edge":"top","fraction":0.5,"width":280,"height":220,"heightIndex":2},"abilities":[{"id":"clay","name":"白模转写实","prompt":"将白模场景转成写实效果","cover":""},{"id":"material","name":"材质替换","prompt":"修改物体材质与表面属性","cover":""}]})}
fn validate(section:&str,value:&Value)->Result<(),String>{
 match section{
 "theme"=>{if !["system","light","dark"].contains(&value.as_str().unwrap_or("")){return Err("Invalid theme".into())}},
 "ink"=>{serde_json::from_value::<quickgen_desktop::ink_session::Style>(value.clone()).map_err(|e|e.to_string())?.validate()?;},
 "layout"=>{if !["top","left","right"].contains(&value["edge"].as_str().unwrap_or("")) {return Err("Invalid edge".into())}
 for (key,min,max) in [("fraction",0.,1.),("width",120.,20000.),("height",120.,20000.),("heightIndex",0.,3.)] {let n=value[key].as_f64().ok_or("Invalid layout")?;if !n.is_finite()||n<min||n>max||(key=="heightIndex"&&n.fract()!=0.){return Err("Invalid layout range".into())}}
 },
 "abilities"=>{let entries=value.as_array().ok_or("Invalid abilities")?;let mut ids=std::collections::HashSet::new();for item in entries{
 for (key,max) in [("id",128),("name",24),("prompt",2000)] {let text=item[key].as_str().ok_or("Invalid ability")?;if text.trim().is_empty()||text.chars().count()>max{return Err("能力字段不能为空或超出长度".into())}}
 if !item["id"].as_str().unwrap().chars().all(|c|c.is_ascii_alphanumeric()||c=='-'){return Err("Invalid ability ID".into())}
 if let Some(file)=item["coverFile"].as_str(){if file.contains('/')||file.contains('\\')||file.contains("..") {return Err("Invalid cover file".into())}}
 if !ids.insert(item["id"].as_str().unwrap()){return Err("能力重复".into())}
 if let Some(cover)=item["cover"].as_str(){if cover.len()>7*1024*1024||(!cover.is_empty()&&!cover.starts_with("data:image/")){return Err("封面无效".into())}}
 }},_=>return Err("Invalid config section".into())}
;Ok(())
}
fn read_dir(dir:&std::path::Path)->Result<Value,String>{
 let file=dir.join("config.json");
 if !file.exists(){return Ok(defaults())}
 let bytes=fs::read(&file).map_err(|e|e.to_string())?;
 match serde_json::from_slice::<Value>(&bytes){Ok(v) if v["version"]==1 && validate("layout",&v["layout"]).is_ok() && validate("abilities",&v["abilities"]).is_ok()=>hydrate(dir,v),_=>{
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis();
 fs::rename(file,dir.join(format!("config.corrupt-{stamp}.json"))).map_err(|e|e.to_string())?;
 let mut v=defaults();v["recoveryWarning"]=json!("配置损坏，已保留原文件并恢复默认");Ok(v)
 }}
}
#[tauri::command]
pub fn load_config(app:tauri::AppHandle)->Result<Value,String>{let _guard=LOCK.lock().map_err(|e|e.to_string())?;read_dir(&directory(&app)?)}
pub(crate) fn image_configuration(card:&Value)->Value{json!({"imageBinding":card.get("imageBinding").filter(|v|!v.is_null()).cloned().unwrap_or(Value::Null),"imageParameters":card.get("imageParameters").filter(|v|!v.is_null()).cloned().unwrap_or(json!({}))})}
fn check_image_configuration(card:&Value,expected:&Value)->Result<(),String>{
 if image_configuration(card)!=*expected{return Err("模型配置冲突：已保存配置已改变，请重新核对后操作".into())}Ok(())
}
// Update only the selected card under the same lock as all other config writes.
pub(crate) fn bind_image_preset(app:&tauri::AppHandle,id:&str,capability:quickgen_desktop::contracts::CapabilityRef,parameters:quickgen_desktop::contracts::Parameters,expected:Value)->Result<(),String>{
 bind_image_preset_dir(&directory(app)?,id,capability,parameters,expected)?;
 app.emit("config-changed","abilities").map_err(|e|e.to_string())
}
fn bind_image_preset_dir(dir:&std::path::Path,id:&str,capability:quickgen_desktop::contracts::CapabilityRef,parameters:quickgen_desktop::contracts::Parameters,expected:Value)->Result<(),String>{
 let _guard=LOCK.lock().map_err(|e|e.to_string())?;
 let mut config=read_dir(dir)?;
 let card=config["abilities"].as_array_mut().ok_or("Invalid abilities")?.iter_mut().find(|card|card["id"].as_str()==Some(id)).ok_or("预设已不存在")?;
 check_image_configuration(card,&expected)?;
 card["imageBinding"]=serde_json::to_value(capability).map_err(|e|e.to_string())?;
 card["imageParameters"]=serde_json::to_value(parameters).map_err(|e|e.to_string())?;
 write_dir(dir,config)
}
pub(crate) fn unbind_image_preset(app:&tauri::AppHandle,id:&str,expected:Value)->Result<(),String>{
 unbind_image_preset_dir(&directory(app)?,id,expected)?;app.emit("config-changed","abilities").map_err(|e|e.to_string())
}
fn unbind_image_preset_dir(dir:&std::path::Path,id:&str,expected:Value)->Result<(),String>{
 let _guard=LOCK.lock().map_err(|e|e.to_string())?;let mut config=read_dir(dir)?;
 let card=config["abilities"].as_array_mut().ok_or("Invalid abilities")?.iter_mut().find(|card|card["id"].as_str()==Some(id)).ok_or("预设已不存在")?;
 check_image_configuration(card,&expected)?;
 let card=card.as_object_mut().ok_or("Invalid ability")?;card.remove("imageBinding");card.remove("imageParameters");write_dir(dir,config)
}
pub fn migration_backup(app:&tauri::AppHandle)->Result<Value,String>{
 let _guard=LOCK.lock().map_err(|e|e.to_string())?;let dir=directory(app)?;
 let config=if dir.join("config.json").exists(){serde_json::from_slice::<Value>(&fs::read(dir.join("config.json")).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?}else{defaults()};
 if config["version"]!=1{return Err("Unsupported config version".into())}
 validate("layout",&config["layout"])?;validate("abilities",&config["abilities"])?;
 let mut covers=serde_json::Map::new();
 for ability in config["abilities"].as_array().unwrap(){if let Some(name)=ability["coverFile"].as_str(){
  let base=dir.join("covers").canonicalize().map_err(|e|e.to_string())?;
  let path=base.join(name).canonicalize().map_err(|e|e.to_string())?;
  if path.parent()!=Some(base.as_path()){return Err("Cover backup path escapes covers".into())}
  let bytes=fs::read(path).map_err(|e|e.to_string())?;covers.insert(name.into(),json!(base64::engine::general_purpose::STANDARD.encode(bytes)));
 }}
 Ok(json!({"config":config,"covers":covers}))
}
// Keep the complete read/append/write transaction in the host, across webview reloads.
fn add_ability_dir(dir:&std::path::Path,ability:Value)->Result<bool,String>{
 let _guard=LOCK.lock().map_err(|e|e.to_string())?;
 validate("abilities",&json!([ability.clone()]))?;
 let mut config=read_dir(dir)?;
 let entries=config["abilities"].as_array_mut().ok_or("Invalid abilities")?;
 if entries.iter().any(|entry|entry["id"]==ability["id"]){return Ok(false)}
 entries.push(ability);config.as_object_mut().unwrap().remove("recoveryWarning");
 write_dir(dir,config)?;Ok(true)
}
#[tauri::command]
pub fn add_ability(app:tauri::AppHandle,ability:Value)->Result<(),String>{
 if add_ability_dir(&directory(&app)?,ability)?{app.emit("config-changed","abilities").map_err(|e|e.to_string())?;}
 Ok(())
}
#[tauri::command]
pub fn save_config(app:tauri::AppHandle,section:String,value:Value)->Result<(),String>{
 let _guard=LOCK.lock().map_err(|e|e.to_string())?;
 validate(&section,&value)?;
 let mut config=read_dir(&directory(&app)?)?;config[&section]=value;config.as_object_mut().unwrap().remove("recoveryWarning");
 let dir=directory(&app)?;fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 write_dir(&dir,config)?;
 app.emit("config-changed",section).map_err(|e|e.to_string())?;Ok(())
}

fn hydrate(dir:&std::path::Path,mut config:Value)->Result<Value,String>{
 if validate("theme",&config["theme"]).is_err(){config["theme"]=json!("system")}
 if validate("ink",&config["ink"]).is_err(){config["ink"]=serde_json::to_value(quickgen_desktop::ink_session::Style::default()).map_err(|e|e.to_string())?;}
 for item in config["abilities"].as_array_mut().unwrap(){
  if let Some(name)=item["coverFile"].as_str(){let path=dir.join("covers").join(name);match fs::read(path){Ok(bytes)=>{let mime=if name.ends_with(".png"){"image/png"}else if name.ends_with(".webp"){"image/webp"}else{"image/jpeg"};item["cover"]=json!(format!("data:{};base64,{}",mime,base64::engine::general_purpose::STANDARD.encode(bytes)));},Err(_)=>{item["cover"]=json!("");item["coverWarning"]=json!("封面副本缺失");}}}
 }Ok(config)
}
fn write_dir(dir:&std::path::Path,mut config:Value)->Result<(),String>{
 fs::create_dir_all(dir.join("covers")).map_err(|e|e.to_string())?;
 for item in config["abilities"].as_array_mut().unwrap(){
  let cover=item["cover"].as_str().unwrap_or("");if cover.is_empty(){continue}
  let (prefix,encoded)=cover.split_once(',').ok_or("Invalid cover data")?;
  let ext=match prefix{"data:image/png;base64"=>"png","data:image/jpeg;base64"=>"jpg","data:image/webp;base64"=>"webp",_=>return Err("Invalid cover type".into())};
  let data=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_|"Invalid cover encoding")?;
  if data.len()>5*1024*1024{return Err("封面超过 5 MB".into())}
  let name=format!("{}.{}",item["id"].as_str().unwrap(),ext);
  let target=dir.join("covers").join(&name);let temp=target.with_extension("tmp");
  let mut out=fs::File::create(&temp).map_err(|e|e.to_string())?;out.write_all(&data).map_err(|e|e.to_string())?;out.sync_all().map_err(|e|e.to_string())?;drop(out);fs::rename(temp,target).map_err(|e|e.to_string())?;
  item["coverFile"]=json!(name);item["cover"]=json!("");
 }
 let temp=dir.join("config.tmp");let mut file=fs::File::create(&temp).map_err(|e|e.to_string())?;
 file.write_all(&serde_json::to_vec_pretty(&config).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);
 fs::rename(temp,dir.join("config.json")).map_err(|e|e.to_string())
}
#[cfg(test)] mod tests{
 use super::*;
 #[test]fn image_binding_updates_only_selected_card_and_preserves_other_settings(){
  let dir=temp();let mut config=defaults();config["theme"]=json!("dark");config["abilities"][0]["cover"]=json!("data:image/png;base64,AQID");write_dir(&dir,config).unwrap();let before=read_dir(&dir).unwrap();
  let capability=quickgen_desktop::contracts::CapabilityRef{connection_id:"fixture".into(),capability_id:"image-edit".into(),revision:"v1".into()};
  let options=std::collections::BTreeMap::from([("size".into(),quickgen_desktop::contracts::ParameterValue::Text("fixture-size".into()))]);
  bind_image_preset_dir(&dir,"clay",capability.clone(),options.clone(),image_configuration(&before["abilities"][0])).unwrap();let after=read_dir(&dir).unwrap();
  assert_eq!(after["theme"],before["theme"]);assert_eq!(after["layout"],before["layout"]);assert_eq!(after["abilities"][1],before["abilities"][1]);
  for key in ["name","prompt","cover","coverFile"]{assert_eq!(after["abilities"][0][key],before["abilities"][0][key]);}
  assert_eq!(after["abilities"][0]["imageParameters"]["size"],"fixture-size");assert_eq!(after["abilities"][0]["imageBinding"]["connection_id"],"fixture");
  assert!(bind_image_preset_dir(&dir,"missing",capability.clone(),options,image_configuration(&after["abilities"][0])).is_err());assert_eq!(read_dir(&dir).unwrap(),after);
  let mut stale=capability.clone();stale.revision="old".into();assert!(unbind_image_preset_dir(&dir,"clay",json!({"imageBinding":stale,"imageParameters":{}})).is_err());assert_eq!(read_dir(&dir).unwrap(),after);
  unbind_image_preset_dir(&dir,"clay",image_configuration(&after["abilities"][0])).unwrap();assert_eq!(read_dir(&dir).unwrap(),before);
 }

 #[test]fn image_configuration_compare_and_swap_rejects_stale_bind_unbind_and_parameters(){
  let dir=temp();write_dir(&dir,defaults()).unwrap();let original=read_dir(&dir).unwrap();let empty=image_configuration(&original["abilities"][0]);
  let reference=quickgen_desktop::contracts::CapabilityRef{connection_id:"fixture".into(),capability_id:"image-edit".into(),revision:"v1".into()};
  let params=std::collections::BTreeMap::new();
  bind_image_preset_dir(&dir,"clay",reference.clone(),params.clone(),empty.clone()).unwrap();
  let bound=read_dir(&dir).unwrap();let baseline=image_configuration(&bound["abilities"][0]);
  assert!(bind_image_preset_dir(&dir,"clay",reference.clone(),params.clone(),empty).unwrap_err().contains("冲突"));
  let changed=std::collections::BTreeMap::from([("size".into(),quickgen_desktop::contracts::ParameterValue::Text("new-size".into()))]);
  bind_image_preset_dir(&dir,"clay",reference.clone(),changed.clone(),baseline.clone()).unwrap();let latest=read_dir(&dir).unwrap();
  assert!(unbind_image_preset_dir(&dir,"clay",baseline.clone()).is_err());
  let mut rebind=reference.clone();rebind.revision="v2".into();
  assert!(bind_image_preset_dir(&dir,"clay",rebind.clone(),params.clone(),baseline).is_err());assert_eq!(read_dir(&dir).unwrap(),latest);
  let expected=image_configuration(&latest["abilities"][0]);let mut unrelated=latest.clone();unrelated["abilities"][1]["imageBinding"]=json!(rebind);unrelated["abilities"][0]["name"]=json!("新名称");write_dir(&dir,unrelated.clone()).unwrap();
  bind_image_preset_dir(&dir,"clay",rebind,changed,expected.clone()).unwrap();let rebound=read_dir(&dir).unwrap();assert_eq!(rebound["abilities"][1],unrelated["abilities"][1]);assert_eq!(rebound["abilities"][0]["name"],"新名称");
  assert!(unbind_image_preset_dir(&dir,"clay",expected).is_err());
  unbind_image_preset_dir(&dir,"clay",image_configuration(&rebound["abilities"][0])).unwrap();
  assert!(bind_image_preset_dir(&dir,"clay",reference,params,image_configuration(&rebound["abilities"][0])).is_err());
 }
 fn temp()->PathBuf{let dir=std::env::temp_dir().join(format!("quickgen-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir_all(&dir).unwrap();dir}
 #[test]fn theme_roundtrip_and_legacy_config(){
  let dir=temp();let mut cfg=defaults();cfg["theme"]=json!("dark");write_dir(&dir,cfg).unwrap();assert_eq!(read_dir(&dir).unwrap()["theme"],"dark");
  let mut old=defaults();old.as_object_mut().unwrap().remove("theme");write_dir(&dir,old).unwrap();let restored=read_dir(&dir).unwrap();assert_eq!(restored["theme"],"system");assert!(restored.get("recoveryWarning").is_none());
  assert!(validate("theme",&json!("blue")).is_err());assert!(validate("theme",&json!("light")).is_ok());
 }
 #[test]fn semantic_corruption_is_preserved(){let dir=temp();let mut cfg=defaults();cfg["layout"]["heightIndex"]=json!(9);fs::write(dir.join("config.json"),cfg.to_string()).unwrap();let restored=read_dir(&dir).unwrap();assert_eq!(restored["layout"]["heightIndex"],2);assert!(restored["recoveryWarning"].is_string());assert!(fs::read_dir(&dir).unwrap().any(|e|e.unwrap().file_name().to_string_lossy().starts_with("config.corrupt-")));}
 #[test]fn truncated_json_is_preserved_byte_for_byte_and_next_save_recovers(){
  let dir=temp();let damaged=b"{\"version\":1,\"abilities\":[";
  fs::write(dir.join("config.json"),damaged).unwrap();
  let recovered=read_dir(&dir).unwrap();
  assert_eq!(recovered["layout"],defaults()["layout"]);
  assert_eq!(recovered["abilities"],defaults()["abilities"]);
  assert!(recovered["recoveryWarning"].as_str().unwrap().contains("配置损坏"));
  let backups:Vec<_>=fs::read_dir(&dir).unwrap().map(|e|e.unwrap().path()).filter(|p|p.file_name().unwrap().to_string_lossy().starts_with("config.corrupt-")).collect();
  assert_eq!(backups.len(),1);assert_eq!(fs::read(&backups[0]).unwrap(),damaged);
  assert!(add_ability_dir(&dir,json!({"id":"after-recovery","name":"恢复后新增","prompt":"处理","cover":""})).unwrap());
  let saved=read_dir(&dir).unwrap();assert_eq!(saved["abilities"].as_array().unwrap().len(),3);
  assert!(saved.get("recoveryWarning").is_none());assert_eq!(fs::read(&backups[0]).unwrap(),damaged);
 }
 #[test]fn layout_and_cover_survive_disk_roundtrip(){let dir=temp();let mut cfg=defaults();cfg["layout"]["width"]=json!(120);cfg["abilities"][0]["cover"]=json!("data:image/png;base64,AQID");write_dir(&dir,cfg).unwrap();let disk:Value=serde_json::from_slice(&fs::read(dir.join("config.json")).unwrap()).unwrap();assert_eq!(disk["abilities"][0]["cover"],"");assert!(dir.join("covers/clay.png").exists());let restored=read_dir(&dir).unwrap();assert_eq!(restored["layout"]["width"],120);assert_eq!(restored["abilities"][0]["cover"],"data:image/png;base64,AQID");}
 #[test]fn traversal_and_duplicate_ids_rejected(){let mut cfg=defaults();cfg["abilities"][0]["coverFile"]=json!("../outside.png");assert!(validate("abilities",&cfg["abilities"]).is_err());cfg=defaults();cfg["abilities"][1]["id"]=json!("clay");assert!(validate("abilities",&cfg["abilities"]).is_err());}
 #[test]fn write_failure_does_not_erase_previous_config(){let dir=temp();write_dir(&dir,defaults()).unwrap();let old=fs::read(dir.join("config.json")).unwrap();fs::create_dir(dir.join("config.tmp")).unwrap();assert!(write_dir(&dir,defaults()).is_err());assert_eq!(fs::read(dir.join("config.json")).unwrap(),old);}
 #[test]fn concurrent_additions_survive_reload(){
  let dir=temp();write_dir(&dir,defaults()).unwrap();
  let barrier=std::sync::Arc::new(std::sync::Barrier::new(8));
  let jobs:Vec<_>=(0..8).map(|i|{let dir=dir.clone();let barrier=barrier.clone();std::thread::spawn(move||{
   barrier.wait();add_ability_dir(&dir,json!({"id":format!("new-{i}"),"name":"测试","prompt":"处理","cover":""})).unwrap()
  })}).collect();
  for job in jobs{assert!(job.join().unwrap())}
  let restored=read_dir(&dir).unwrap();let entries=restored["abilities"].as_array().unwrap();assert_eq!(entries.len(),10);
  for i in 0..8{assert!(entries.iter().any(|a|a["id"]==format!("new-{i}")))}
 }
 #[test]fn duplicate_addition_preserves_existing_fields(){
  let dir=temp();write_dir(&dir,defaults()).unwrap();
  assert!(!add_ability_dir(&dir,json!({"id":"clay","name":"覆盖","prompt":"替换","cover":""})).unwrap());
  assert_eq!(read_dir(&dir).unwrap()["abilities"][0]["name"],"白模转写实");
 }
 #[test]fn failed_addition_preserves_config_and_can_retry(){
  let dir=temp();write_dir(&dir,defaults()).unwrap();let before=fs::read(dir.join("config.json")).unwrap();
  fs::create_dir(dir.join("config.tmp")).unwrap();let ability=json!({"id":"new","name":"新增","prompt":"处理","cover":""});
  assert!(add_ability_dir(&dir,ability.clone()).is_err());assert_eq!(fs::read(dir.join("config.json")).unwrap(),before);
  fs::remove_dir(dir.join("config.tmp")).unwrap();assert!(add_ability_dir(&dir,ability).unwrap());assert_eq!(read_dir(&dir).unwrap()["abilities"].as_array().unwrap().len(),3);
 }
}
