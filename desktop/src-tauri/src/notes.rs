//! Notes share the resource worker's database; views never own durable text.
use crate::resources::ResourceService;
use serde::{Deserialize,Serialize};
use rusqlite::OptionalExtension;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
 pub id:String,pub text:String,
 pub document:serde_json::Value,pub theme:String,pub theme_revision:u64,
 pub resident:bool,pub always_on_top:bool,
 pub x:f64,pub y:f64,pub width:f64,pub height:f64,pub monitor:String,
 pub revision:u64,pub text_revision:u64,pub geometry_revision:u64,pub flags_revision:u64,
 pub deleted:bool,pub created_at:u64,pub updated_at:u64,
}
#[derive(Debug,Deserialize)]
#[serde(tag="field",rename_all="snake_case",deny_unknown_fields)]
pub enum NotePatch {
 Text{revision:u64,text:String},
 Document{revision:u64,document:serde_json::Value},
 Theme{revision:u64,theme:String},
 Geometry{revision:u64,x:f64,y:f64,width:f64,height:f64,monitor:String},
 Flags{revision:u64,resident:bool,always_on_top:bool},
 Deleted{revision:u64,deleted:bool},
}
fn now()->Result<u64,String>{Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis() as u64)}
// Reserve the side-tab margin. Choose a free nearby slot first; when full,
// minimize overlap while avoiding identical origins. Existing notes never move.
fn new_note_position(area:[f64;4],width:f64,height:f64,notes:Vec<&Note>)->(f64,f64){
 let left=area[0]+32.;let top=area[1];let right=area[0]+area[2]-width;let bottom=area[1]+area[3]-height;
 let anchor=((area[0]+80.).min(right).max(left),(top+80.).min(bottom).max(top));
 let score=|x:f64,y:f64|{let mut overlap=0.;let mut identical=0;for n in &notes{overlap+=((x+width+12.).min(n.x+n.width+12.)-(x-32.).max(n.x-32.)).max(0.)*((y+height+12.).min(n.y+n.height+12.)-y.max(n.y)).max(0.);if (x-n.x).abs()<12.&&(y-n.y).abs()<12.{identical+=1;}}(identical,overlap,(x-anchor.0).powi(2)+(y-anchor.1).powi(2))};
 let mut best=anchor;let mut best_score=score(best.0,best.1);
 for iy in 0..=(((bottom-top)/32.).ceil()as usize).min(512){for ix in 0..=(((right-left)/32.).ceil()as usize).min(512){let x=(left+ix as f64*32.).min(right);let y=(top+iy as f64*32.).min(bottom);let candidate=score(x,y);if candidate<best_score{best=(x,y);best_score=candidate;}}}best
}
impl ResourceService {
 #[cfg(feature="desktop-diagnostics")]
 pub fn notes_write_fault(&mut self,id:&str,enabled:bool)->Result<serde_json::Value,String>{
  if !id.starts_with("note-")||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("invalid note fault target".into())}
  self.store.connection.execute_batch("DROP TRIGGER IF EXISTS temp.notes_test_reject_write;").map_err(|e|e.to_string())?;
  if enabled{self.store.connection.execute_batch(&format!("CREATE TEMP TRIGGER notes_test_reject_write BEFORE UPDATE ON notes WHEN OLD.note_id='{id}' BEGIN SELECT RAISE(ABORT,'isolated note write failure'); END;")).map_err(|e|e.to_string())?;}
  Ok(serde_json::json!({"enabled":enabled}))
 }
 pub fn notes_list(&self)->Result<Vec<Note>,String>{
  let mut q=self.store.connection.prepare("SELECT note_json FROM notes ORDER BY rowid").map_err(|e|e.to_string())?;
  let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
  rows.map(|r|serde_json::from_str(&r.map_err(|e|e.to_string())?).map_err(|e|e.to_string())).collect()
 }
 pub fn note_get(&self,id:&str)->Result<Note,String>{
  let row:Option<String>=self.store.connection.query_row("SELECT note_json FROM notes WHERE note_id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  serde_json::from_str(&row.ok_or("note_not_found")?).map_err(|e|e.to_string())
 }
 pub fn note_create(&mut self)->Result<Note,String>{self.note_insert(80.,80.,280.,240.,String::new())}
 pub fn note_create_in(&mut self,area:[f64;4],monitor:String)->Result<Note,String>{
  if area.iter().any(|v|!v.is_finite())||area[2]<192.||area[3]<120.{return Err("invalid note work area".into())}
  let width=280f64.min(area[2]-32.);let height=240f64.min(area[3]);
  let existing=self.notes_list()?;let (x,y)=new_note_position(area,width,height,existing.iter().filter(|n|!n.deleted&&n.resident&&(n.monitor==monitor||n.monitor.is_empty())).collect());
  self.note_insert(x,y,width,height,monitor)
 }
 fn note_insert(&mut self,x:f64,y:f64,width:f64,height:f64,monitor:String)->Result<Note,String>{
  static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
  let stamp=now()?;let id=format!("note-{}-{}-{}",std::process::id(),stamp,NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
  let note=Note{id,text:String::new(),document:crate::note_document::from_text(""),theme:"default".into(),theme_revision:0,resident:true,always_on_top:false,x,y,width,height,monitor,revision:0,text_revision:0,geometry_revision:0,flags_revision:0,deleted:false,created_at:stamp,updated_at:stamp};
  self.store.connection.execute("INSERT INTO notes VALUES(?1,?2)",rusqlite::params![note.id,serde_json::to_string(&note).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(note)
 }
 pub fn note_patch(&mut self,id:&str,patch:NotePatch)->Result<Note,String>{
  let mut note=self.note_get(id)?;
  if note.deleted&&!matches!(&patch,NotePatch::Deleted{deleted:false,..}){return Err("note_deleted".into())}
  let is_document=matches!(&patch,NotePatch::Document{..});
  let conflict=||"note_revision_conflict".to_string();
  match patch {
   NotePatch::Text{revision,text}=>{if revision!=note.text_revision{return Err(conflict())}if text.len()>1_048_576{return Err("note_text_limit".into())}if note.document!=crate::note_document::from_text(&note.text){return Err("legacy_note_text_write_would_flatten_document".into())}note.document=crate::note_document::from_text(&text);note.text=text;note.text_revision+=1;},
   NotePatch::Document{revision,document}=>{if revision!=note.text_revision{return Err(conflict())}if !crate::note_document::supported(&note.document){return Err("unsupported_note_document_version".into())}let text=crate::note_document::validate(&document)?;note.document=document;note.text=text;note.text_revision+=1;},
   NotePatch::Theme{revision,theme}=>{if revision!=note.theme_revision{return Err(conflict())}if !crate::note_document::COLORS.contains(&theme.as_str()){return Err("invalid_note_theme".into())}note.theme=theme;note.theme_revision+=1;},
   NotePatch::Geometry{revision,x,y,width,height,monitor}=>{if revision!=note.geometry_revision{return Err(conflict())}if ![x,y,width,height].iter().all(|v|v.is_finite())||width<160.||height<120.||width>16384.||height>16384.||monitor.len()>1024{return Err("invalid_note_geometry".into())}note.x=x;note.y=y;note.width=width;note.height=height;note.monitor=monitor;note.geometry_revision+=1;},
   NotePatch::Flags{revision,resident,always_on_top}=>{if revision!=note.flags_revision{return Err(conflict())}note.resident=resident;note.always_on_top=always_on_top;note.flags_revision+=1;},
   NotePatch::Deleted{revision,deleted}=>{if revision!=note.revision{return Err(conflict())}note.deleted=deleted;if !deleted{note.resident=true;}
    // Invalidate all in-flight field edits, including after explicit undo.
    note.text_revision+=1;note.geometry_revision+=1;note.flags_revision+=1;note.theme_revision+=1;
   },
  }
  note.revision+=1;note.updated_at=now()?;
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  if is_document{crate::note_document::stamp_tags(&mut note.document,&mut |tag|{
   let existing:Option<i64>=tx.query_row("SELECT created_at FROM note_tag_births WHERE note_id=?1 AND tag_id=?2",rusqlite::params![id,tag],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
   if let Some(stamp)=existing{return u64::try_from(stamp).map_err(|_|"invalid_note_tag_time".into())}
   tx.execute("INSERT INTO note_tag_births VALUES(?1,?2,?3)",rusqlite::params![id,tag,i64::try_from(note.updated_at).map_err(|_|"note_tag_time_overflow")?]).map_err(|e|e.to_string())?;Ok(note.updated_at)
  })?;}
  tx.execute("UPDATE notes SET note_json=?2 WHERE note_id=?1",rusqlite::params![id,serde_json::to_string(&note).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(note)
 }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn new_notes_use_free_slots_then_stagger_and_survive_restart(){
  let root=std::env::temp_dir().join(format!("quickgen-placement-{}-{}",std::process::id(),now().unwrap()));let mut service=ResourceService::open(&root).unwrap();let area=[-1536.,32.,1536.,832.];let mut notes=vec![];
  for _ in 0..30{let n=service.note_create_in(area,"left-125".into()).unwrap();assert!(n.x>=area[0]+32.&&n.x+n.width<=0.&&n.y>=32.&&n.y+n.height<=864.);assert!(notes.iter().all(|p:&Note|(p.x,p.y)!=(n.x,n.y)));if notes.len()<6{assert!(notes.iter().all(|p:&Note|n.x+n.width<=p.x||p.x+p.width<=n.x||n.y+n.height<=p.y||p.y+p.height<=n.y));}notes.push(n);}
  assert_eq!(service.notes_list().unwrap(),notes);drop(service);let mut service=ResourceService::open(&root).unwrap();assert_eq!(service.notes_list().unwrap(),notes);let n=service.note_create_in([0.,0.,192.,120.],"tiny".into()).unwrap();assert_eq!((n.x,n.y,n.width,n.height),(32.,0.,160.,120.));let other=service.note_create_in(area,"different-monitor".into()).unwrap();assert_eq!((other.x,other.y),(notes[0].x,notes[0].y));
 }
 #[test]fn undo_of_nonresident_note_is_visible_and_keeps_top_and_text(){
  let root=std::env::temp_dir().join(format!("quickgen-notes-undo-{}-{}",std::process::id(),now().unwrap()));
  let mut service=ResourceService::open(&root).unwrap();let n=service.note_create().unwrap();
  let n=service.note_patch(&n.id,NotePatch::Text{revision:0,text:"undo retained".into()}).unwrap();
  let n=service.note_patch(&n.id,NotePatch::Flags{revision:0,resident:false,always_on_top:true}).unwrap();
  let deleted=service.note_patch(&n.id,NotePatch::Deleted{revision:n.revision,deleted:true}).unwrap();
  let restored=service.note_patch(&n.id,NotePatch::Deleted{revision:deleted.revision,deleted:false}).unwrap();
  assert!(restored.resident,"explicit undo must make the note visible");assert!(restored.always_on_top);assert_eq!(restored.text,n.text);assert_eq!((restored.x,restored.y,restored.width,restored.height),(n.x,n.y,n.width,n.height));
 }
 #[test]fn blank_chinese_fields_restart_and_delete_barrier(){
  let root=std::env::temp_dir().join(format!("quickgen-notes-test-{}-{}",std::process::id(),now().unwrap()));
  let mut service=ResourceService::open(&root).unwrap();let a=service.note_create().unwrap();let b=service.note_create().unwrap();
  let a=service.note_patch(&a.id,NotePatch::Text{revision:0,text:"中文便签\n第二行".into()}).unwrap();
  let a=service.note_patch(&a.id,NotePatch::Flags{revision:0,resident:false,always_on_top:true}).unwrap();
  assert_eq!(a.text,"中文便签\n第二行");assert!(service.note_patch(&a.id,NotePatch::Text{revision:0,text:"late".into()}).is_err());
  let deleted=service.note_patch(&a.id,NotePatch::Deleted{revision:a.revision,deleted:true}).unwrap();
  assert!(service.note_patch(&a.id,NotePatch::Text{revision:a.text_revision,text:"late".into()}).is_err());
  let restored=service.note_patch(&a.id,NotePatch::Deleted{revision:deleted.revision,deleted:false}).unwrap();
  assert!(service.note_patch(&a.id,NotePatch::Text{revision:a.text_revision,text:"late".into()}).is_err());
  service.store.connection.execute_batch("CREATE TRIGGER reject_note BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT,'write failed'); END;").unwrap();
  assert!(service.note_patch(&a.id,NotePatch::Text{revision:restored.text_revision,text:"unsaved".into()}).is_err());assert_eq!(service.note_get(&a.id).unwrap(),restored);
  drop(service);let service=ResourceService::open(&root).unwrap();assert_eq!(service.note_get(&a.id).unwrap(),restored);assert_eq!(service.note_get(&b.id).unwrap().text,"");
 }
}

/// Upgrades every legacy record atomically with schema metadata. Unknown future
/// document versions stay byte-for-byte as values; readers must show them read-only.
pub(crate) fn migrate_v15(tx:&rusqlite::Transaction<'_>)->Result<(),String>{
 tx.execute_batch("CREATE TABLE note_tag_births(note_id TEXT NOT NULL REFERENCES notes(note_id),tag_id TEXT NOT NULL,created_at INTEGER NOT NULL,PRIMARY KEY(note_id,tag_id)); CREATE TRIGGER note_tag_birth_no_update BEFORE UPDATE ON note_tag_births BEGIN SELECT RAISE(ABORT,'immutable note tag birth'); END;").map_err(|e|e.to_string())?;
 let rows={let mut q=tx.prepare("SELECT note_id,note_json FROM notes").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?};
 for (id,encoded) in rows{let mut value:serde_json::Value=serde_json::from_str(&encoded).map_err(|e|e.to_string())?;
  if value.get("document").is_none(){let text=value["text"].as_str().ok_or("legacy note text missing")?;if text.len()>crate::note_document::TEXT_LIMIT{return Err("legacy_note_text_limit".into())}value["document"]=crate::note_document::from_text(text);}
  if value.get("theme").is_none(){value["theme"]=serde_json::json!("default");}
  if value.get("theme_revision").is_none(){value["theme_revision"]=serde_json::json!(0);}
  tx.execute("UPDATE notes SET note_json=?2 WHERE note_id=?1",rusqlite::params![id,serde_json::to_string(&value).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
 }
 tx.execute_batch("INSERT INTO schema_migrations VALUES(15,strftime('%Y-%m-%dT%H:%M:%fZ','now')); PRAGMA user_version=15;").map_err(|e|e.to_string())?;Ok(())
}
#[cfg(test)]mod rich_tests{
 use super::*;use serde_json::json;
 fn root()->std::path::PathBuf{std::env::temp_dir().join(format!("quickgen-rich-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))}
 #[test]fn rich_document_theme_versions_failure_and_legacy_guard(){
  let root=root();let mut host=ResourceService::open(&root).unwrap();let n=host.note_create().unwrap();
  let mut doc=crate::note_document::from_text("中文\n\n尾行");doc["doc"]["content"][0]["content"][0]["marks"]=json!([{"type":"strong"}]);
  let a=host.note_patch(&n.id,NotePatch::Document{revision:0,document:doc.clone()}).unwrap();assert_eq!(a.text,"中文\n\n尾行");
  let themed=host.note_patch(&n.id,NotePatch::Theme{revision:0,theme:"paper".into()}).unwrap();assert_eq!(themed.text_revision,a.text_revision);assert_eq!(themed.document,doc);
  assert!(host.note_patch(&n.id,NotePatch::Text{revision:1,text:"flatten".into()}).is_err());
  assert!(host.note_patch(&n.id,NotePatch::Document{revision:0,document:doc.clone()}).is_err());
  host.store.connection.execute_batch("CREATE TRIGGER reject_rich_write BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT,'isolated rich fault'); END;").unwrap();
  assert!(host.note_patch(&n.id,NotePatch::Document{revision:1,document:crate::note_document::from_text("unsaved")}).is_err());assert_eq!(host.note_get(&n.id).unwrap(),themed);
  host.store.connection.execute_batch("DROP TRIGGER reject_rich_write;").unwrap();let deleted=host.note_patch(&n.id,NotePatch::Deleted{revision:themed.revision,deleted:true}).unwrap();let restored=host.note_patch(&n.id,NotePatch::Deleted{revision:deleted.revision,deleted:false}).unwrap();
  assert!(host.note_patch(&n.id,NotePatch::Theme{revision:themed.theme_revision,theme:"blue".into()}).is_err());assert!(host.note_patch(&n.id,NotePatch::Document{revision:themed.text_revision,document:doc}).is_err());drop(host);
  let host=ResourceService::open(&root).unwrap();assert_eq!(host.note_get(&n.id).unwrap(),restored);
 }
 #[test]fn schema14_upgrade_is_lossless_and_failure_rolls_back_every_record(){
  let root=root();let mut host=ResourceService::open(&root).unwrap();let mut prior=vec![];
  for (i,text) in ["","中文\n\n末尾\n","a\r\nb"].iter().enumerate(){let n=host.note_create().unwrap();let n=host.note_patch(&n.id,NotePatch::Text{revision:0,text:(*text).into()}).unwrap();let n=if i==2{host.note_patch(&n.id,NotePatch::Deleted{revision:n.revision,deleted:true}).unwrap()}else{n};let mut old=serde_json::to_value(&n).unwrap();for key in ["document","theme","theme_revision"]{old.as_object_mut().unwrap().remove(key);}let bytes=old.to_string();host.store.connection.execute("UPDATE notes SET note_json=?2 WHERE note_id=?1",rusqlite::params![n.id,bytes]).unwrap();prior.push((n.id,bytes));}
  host.store.connection.execute_batch("DROP TRIGGER note_tag_birth_no_update; DROP TABLE note_tag_births; DELETE FROM schema_migrations WHERE version=15; PRAGMA user_version=14;").unwrap();host.store.connection.execute_batch(&format!("CREATE TRIGGER reject_upgrade BEFORE UPDATE ON notes WHEN OLD.note_id='{}' BEGIN SELECT RAISE(ABORT,'migration interruption'); END;",prior[1].0)).unwrap();drop(host);
  assert!(ResourceService::open(&root).is_err());let c=rusqlite::Connection::open(root.join("metadata.sqlite")).unwrap();assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),14);
  for (id,bytes) in &prior{assert_eq!(c.query_row("SELECT note_json FROM notes WHERE note_id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),*bytes);}c.execute_batch("DROP TRIGGER reject_upgrade").unwrap();drop(c);
  let host=ResourceService::open(&root).unwrap();for (id,bytes) in prior{let mut upgraded=serde_json::to_value(host.note_get(&id).unwrap()).unwrap();let doc=upgraded.as_object_mut().unwrap().remove("document").unwrap();assert_eq!(crate::note_document::validate(&doc).unwrap(),upgraded["text"].as_str().unwrap());for key in ["theme","theme_revision"]{upgraded.as_object_mut().unwrap().remove(key);}assert_eq!(upgraded,serde_json::from_str::<serde_json::Value>(&bytes).unwrap());}
 }
 #[test]fn tag_time_is_host_owned_survives_rename_remove_undo_and_failed_save(){
  let root=root();let mut host=ResourceService::open(&root).unwrap();let n=host.note_create().unwrap();let doc=json!({"version":1,"doc":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"tag","attrs":{"id":"tag1","label":"原标签","createdAt":1,"color":"yellow"}}]}]}});
  let n=host.note_patch(&n.id,NotePatch::Document{revision:0,document:doc}).unwrap();let born=n.document["doc"]["content"][0]["content"][0]["attrs"]["createdAt"].as_u64().unwrap();assert!(born>1);
  let mut renamed=n.document.clone();renamed["doc"]["content"][0]["content"][0]["attrs"]["label"]=json!("新标签");renamed["doc"]["content"][0]["content"][0]["attrs"]["createdAt"]=json!(0);
  let n=host.note_patch(&n.id,NotePatch::Document{revision:n.text_revision,document:renamed.clone()}).unwrap();assert_eq!(n.document["doc"]["content"][0]["content"][0]["attrs"]["createdAt"],born);
  let n=host.note_patch(&n.id,NotePatch::Document{revision:n.text_revision,document:crate::note_document::from_text("")}).unwrap();
  let n=host.note_patch(&n.id,NotePatch::Document{revision:n.text_revision,document:renamed.clone()}).unwrap();assert_eq!(n.document["doc"]["content"][0]["content"][0]["attrs"]["createdAt"],born);
  renamed["doc"]["content"][0]["content"][0]["attrs"]["id"]=json!("tag2");host.store.connection.execute_batch("CREATE TRIGGER reject_stamp BEFORE UPDATE ON notes BEGIN SELECT RAISE(ABORT,'isolated failure'); END;").unwrap();assert!(host.note_patch(&n.id,NotePatch::Document{revision:n.text_revision,document:renamed}).is_err());assert_eq!(host.store.connection.query_row("SELECT count(*) FROM note_tag_births",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(host.note_get(&n.id).unwrap(),n);
 }
 #[test]fn future_document_is_preserved_and_cannot_be_overwritten(){let root=root();let mut host=ResourceService::open(&root).unwrap();let mut n=host.note_create().unwrap();n.document=json!({"version":99,"future":{"data":"preserve"}});host.store.connection.execute("UPDATE notes SET note_json=?2 WHERE note_id=?1",rusqlite::params![n.id,serde_json::to_string(&n).unwrap()]).unwrap();assert_eq!(host.note_get(&n.id).unwrap(),n);assert!(host.note_patch(&n.id,NotePatch::Document{revision:0,document:crate::note_document::from_text("bad")}).is_err());assert!(host.note_patch(&n.id,NotePatch::Text{revision:0,text:"bad".into()}).is_err());assert_eq!(host.note_get(&n.id).unwrap(),n);}
}
