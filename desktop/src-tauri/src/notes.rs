//! Notes share the resource worker's database; views never own durable text.
use crate::resources::ResourceService;
use serde::{Deserialize,Serialize};
use rusqlite::OptionalExtension;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
 pub id:String,pub text:String,pub resident:bool,pub always_on_top:bool,
 pub x:f64,pub y:f64,pub width:f64,pub height:f64,pub monitor:String,
 pub revision:u64,pub text_revision:u64,pub geometry_revision:u64,pub flags_revision:u64,
 pub deleted:bool,pub created_at:u64,pub updated_at:u64,
}
#[derive(Debug,Deserialize)]
#[serde(tag="field",rename_all="snake_case",deny_unknown_fields)]
pub enum NotePatch {
 Text{revision:u64,text:String},
 Geometry{revision:u64,x:f64,y:f64,width:f64,height:f64,monitor:String},
 Flags{revision:u64,resident:bool,always_on_top:bool},
 Deleted{revision:u64,deleted:bool},
}
fn now()->Result<u64,String>{Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis() as u64)}
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
 pub fn note_create(&mut self)->Result<Note,String>{
  static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
  let stamp=now()?;let id=format!("note-{}-{}-{}",std::process::id(),stamp,NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed));
  let note=Note{id,text:String::new(),resident:true,always_on_top:false,x:80.,y:80.,width:280.,height:240.,monitor:String::new(),revision:0,text_revision:0,geometry_revision:0,flags_revision:0,deleted:false,created_at:stamp,updated_at:stamp};
  self.store.connection.execute("INSERT INTO notes VALUES(?1,?2)",rusqlite::params![note.id,serde_json::to_string(&note).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(note)
 }
 pub fn note_patch(&mut self,id:&str,patch:NotePatch)->Result<Note,String>{
  let mut note=self.note_get(id)?;
  if note.deleted&&!matches!(&patch,NotePatch::Deleted{deleted:false,..}){return Err("note_deleted".into())}
  let conflict=||"note_revision_conflict".to_string();
  match patch {
   NotePatch::Text{revision,text}=>{if revision!=note.text_revision{return Err(conflict())}if text.len()>1_048_576{return Err("note_text_limit".into())}note.text=text;note.text_revision+=1;},
   NotePatch::Geometry{revision,x,y,width,height,monitor}=>{if revision!=note.geometry_revision{return Err(conflict())}if ![x,y,width,height].iter().all(|v|v.is_finite())||width<160.||height<120.||width>16384.||height>16384.||monitor.len()>1024{return Err("invalid_note_geometry".into())}note.x=x;note.y=y;note.width=width;note.height=height;note.monitor=monitor;note.geometry_revision+=1;},
   NotePatch::Flags{revision,resident,always_on_top}=>{if revision!=note.flags_revision{return Err(conflict())}note.resident=resident;note.always_on_top=always_on_top;note.flags_revision+=1;},
   NotePatch::Deleted{revision,deleted}=>{if revision!=note.revision{return Err(conflict())}note.deleted=deleted;if !deleted{note.resident=true;}
    // Invalidate all in-flight field edits, including after explicit undo.
    note.text_revision+=1;note.geometry_revision+=1;note.flags_revision+=1;
   },
  }
  note.revision+=1;note.updated_at=now()?;
  self.store.connection.execute("UPDATE notes SET note_json=?2 WHERE note_id=?1",rusqlite::params![id,serde_json::to_string(&note).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(note)
 }
}

#[cfg(test)]mod tests{
 use super::*;
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
