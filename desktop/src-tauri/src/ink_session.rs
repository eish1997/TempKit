//! Ephemeral screen annotations. Never written to the resource DB or recovery bundle.
use serde::{Deserialize,Serialize};
use std::{collections::{BTreeMap,BTreeSet,VecDeque},sync::Arc};
pub const COLORS:&[&str]=&["#ed6957","#f4c85b","#79bd91","#70aee4","#b099db","#faf6e9","#484744"];
pub const MAX_HISTORY:usize=100;
const MAX_OBJECTS:usize=10_000;
const MAX_POINTS:usize=2_000_000;
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style{pub color:String,pub width:f64}
impl Default for Style{fn default()->Self{Self{color:COLORS[0].into(),width:3.}}}
impl Style{pub fn validate(&self)->Result<(),String>{if !COLORS.contains(&self.color.as_str())||![2.,3.,6.].contains(&self.width){Err("invalid_ink_style".into())}else{Ok(())}}}
#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point{pub x:f64,pub y:f64}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(tag="type",rename_all="snake_case",deny_unknown_fields)]
pub enum Content{Stroke{points:Vec<Point>,style:Style},Text{x:f64,y:f64,text:String,style:Style}}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object{pub id:String,pub monitor:String,pub content:Content}
#[derive(Clone,Debug,PartialEq,Serialize)]
pub struct Entry{pub order:u64,pub item:Object}
fn finite(n:f64)->bool{n.is_finite()&&n.abs()<=131_072.}
fn valid_id(id:&str)->bool{!id.is_empty()&&id.len()<=128&&id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
impl Object{
 pub fn validate(&self)->Result<(),String>{
  if !valid_id(&self.id)||self.monitor.is_empty()||self.monitor.len()>512||self.monitor.chars().any(char::is_control){return Err("invalid_ink_identity".into())}
  match &self.content{
   Content::Stroke{points,style}=>{style.validate()?;if points.is_empty()||points.len()>4096||points.iter().any(|p|!finite(p.x)||!finite(p.y)){return Err("invalid_ink_stroke".into())}},
   Content::Text{x,y,text,style}=>{style.validate()?;if !finite(*x)||!finite(*y)||text.trim().is_empty()||text.len()>16384||text.chars().any(|c|c.is_control()&&c!='\n'&&c!='\t'){return Err("invalid_ink_text".into())}}
  }Ok(())
 }
 fn points(&self)->usize{match &self.content{Content::Stroke{points,..}=>points.len(),Content::Text{..}=>0}}
}
#[derive(Clone,Debug,Deserialize)]
#[serde(tag="op",rename_all="snake_case",deny_unknown_fields)]
pub enum Mutation{Add{object:Object},Replace{object:Object},Translate{object:Object,dx:f64,dy:f64},Erase{ids:Vec<String>},Clear,Undo,Redo}
#[derive(Clone,Default)]struct Change{before:Vec<Arc<Entry>>,after:Vec<Arc<Entry>>}
#[derive(Serialize)]
pub struct Snapshot{pub active:bool,pub epoch:u64,pub revision:u64,pub style:Style,pub objects:Vec<Entry>,pub can_undo:bool,pub can_redo:bool}
#[derive(Default)]
pub struct Session{pub active:bool,pub epoch:u64,pub revision:u64,pub style:Style,objects:BTreeMap<String,Arc<Entry>>,history:VecDeque<Change>,future:Vec<Change>,next_order:u64,total_points:usize,receipts:BTreeMap<String,u64>}
impl Session{
 pub fn snapshot(&self)->Snapshot{let mut objects:Vec<_>=self.objects.values().map(|v|v.as_ref().clone()).collect();objects.sort_by_key(|e|e.order);Snapshot{active:self.active,epoch:self.epoch,revision:self.revision,style:self.style.clone(),objects,can_undo:!self.history.is_empty(),can_redo:!self.future.is_empty()}}
 pub fn snapshot_for(&self,monitor:Option<&str>)->Snapshot{let mut objects:Vec<_>=self.objects.values().filter(|e|monitor==Some(e.item.monitor.as_str())).map(|e|e.as_ref().clone()).collect();objects.sort_by_key(|e|e.order);Snapshot{active:self.active,epoch:self.epoch,revision:self.revision,style:self.style.clone(),objects,can_undo:!self.history.is_empty(),can_redo:!self.future.is_empty()}}
 pub fn ids_belong_to(&self,ids:&[String],monitor:&str)->bool{ids.iter().all(|id|self.objects.get(id).is_none_or(|e|e.item.monitor==monitor))}
 pub fn set_active(&mut self,active:bool){if self.active!=active{self.active=active;self.epoch+=1;}}
 pub fn set_style(&mut self,style:Style)->Result<(),String>{style.validate()?;if self.style!=style{self.style=style;self.revision+=1;}Ok(())}
 /// Each renderer sends a monotonic stream. A lost reply can be retried without
 /// applying an erase or Undo twice; failed writes never advance the receipt.
 pub fn mutate_once(&mut self,stream:&str,sequence:u64,epoch:u64,revision:u64,op:Mutation)->Result<(),String>{if !valid_id(stream)||sequence==0{return Err("invalid_ink_stream".into())}let previous=self.receipts.get(stream).copied().unwrap_or(0);if sequence<=previous{return Ok(())}if sequence!=previous+1{return Err("ink_stream_gap".into())}if previous==0&&self.receipts.len()>=4096{return Err("ink_stream_limit".into())}self.mutate(epoch,revision,op)?;self.receipts.insert(stream.into(),sequence);Ok(())}
 pub fn mutate(&mut self,epoch:u64,revision:u64,op:Mutation)->Result<(),String>{
  if (!self.active&&!matches!(&op,Mutation::Clear))||epoch!=self.epoch{return Err("ink_mode_changed".into())}if revision!=self.revision{return Err("ink_revision_conflict".into())}
  let change=match op{
   Mutation::Add{object}=>{object.validate()?;if self.objects.contains_key(&object.id){return Err("ink_object_exists".into())}Change{before:vec![],after:vec![Arc::new(Entry{order:self.next_order,item:object})]}},
   Mutation::Replace{object}=>{object.validate()?;let previous=self.objects.get(&object.id).ok_or("ink_object_missing")?;if previous.item.monitor!=object.monitor||!matches!((&previous.item.content,&object.content),(Content::Text{..},Content::Text{..})){return Err("ink_replace_requires_same_text".into())}if previous.item==object{return Ok(())}Change{before:vec![previous.clone()],after:vec![Arc::new(Entry{order:previous.order,item:object})]}},
   Mutation::Translate{object,dx,dy}=>{if !finite(dx)||!finite(dy){return Err("invalid_ink_translation".into())}let previous=self.objects.get(&object.id).ok_or("ink_object_missing")?;if previous.item!=object{return Err("ink_object_changed".into())}if dx==0.&&dy==0.{return Ok(())}let mut moved=object;
    match &mut moved.content{Content::Stroke{points,..}=>for p in points{p.x+=dx;p.y+=dy;},Content::Text{x,y,..}=>{*x+=dx;*y+=dy;}}
    moved.validate()?;Change{before:vec![previous.clone()],after:vec![Arc::new(Entry{order:previous.order,item:moved})]}
   },
   Mutation::Erase{ids}=>{if ids.len()>MAX_OBJECTS||ids.iter().any(|id|!valid_id(id)){return Err("invalid_ink_erase".into())}let ids:BTreeSet<_>=ids.into_iter().collect();let before=ids.into_iter().filter_map(|id|self.objects.get(&id).cloned()).collect::<Vec<_>>();if before.is_empty(){return Ok(())}Change{before,after:vec![]}},
   Mutation::Clear=>{if self.objects.is_empty(){return Ok(())}Change{before:self.objects.values().cloned().collect(),after:vec![]}},
   Mutation::Undo=>{if let Some(change)=self.history.pop_back(){self.apply(&change,false);self.future.push(change);}return Ok(())},
   Mutation::Redo=>{if let Some(change)=self.future.pop(){self.apply(&change,true);self.history.push_back(change);}return Ok(())},
  };
  let count=self.objects.len()-change.before.len()+change.after.len();let points=self.total_points-change.before.iter().map(|e|e.item.points()).sum::<usize>()+change.after.iter().map(|e|e.item.points()).sum::<usize>();
  if count>MAX_OBJECTS||points>MAX_POINTS{return Err("ink_session_limit".into())}
  self.next_order+=1;self.apply(&change,true);self.history.push_back(change);if self.history.len()>MAX_HISTORY{self.history.pop_front();}self.future.clear();Ok(())
 }
 fn apply(&mut self,change:&Change,forward:bool){let (remove,insert)=if forward{(&change.before,&change.after)}else{(&change.after,&change.before)};for e in remove{if let Some(removed)=self.objects.remove(&e.item.id){self.total_points-=removed.item.points();}}for e in insert{self.total_points+=e.item.points();self.objects.insert(e.item.id.clone(),e.clone());}self.revision+=1;}
 pub fn len(&self)->usize{self.objects.len()}
 pub fn is_empty(&self)->bool{self.objects.is_empty()}
}
#[cfg(test)]mod tests{
 use super::*;
 fn stroke(id:usize,monitor:&str)->Object{Object{id:format!("stroke-{id}"),monitor:monitor.into(),content:Content::Stroke{points:vec![Point{x:10.,y:20.},Point{x:80.,y:90.}],style:Style::default()}}}
 fn text(id:usize)->Object{Object{id:format!("text-{id}"),monitor:"left-125".into(),content:Content::Text{x:20.,y:30.,text:"中文\n第二行".into(),style:Style::default()}}}
 fn apply(session:&mut Session,op:Mutation){session.mutate(session.epoch,session.revision,op).unwrap()}
 #[test]fn mode_exit_retains_objects_but_rejects_late_requests_and_new_session_is_empty(){let mut s=Session::default();s.set_active(true);let epoch=s.epoch;apply(&mut s,Mutation::Add{object:stroke(1,"left-125")});s.set_active(false);assert_eq!(s.len(),1);assert!(s.mutate(epoch,s.revision,Mutation::Erase{ids:vec!["stroke-1".into()]}).is_err());s.set_active(true);assert!(s.mutate(epoch,s.revision,Mutation::Add{object:stroke(2,"left-125")}).is_err());assert!(Session::default().is_empty());}
 #[test]fn one_erase_gesture_restores_all_lines_and_text_in_original_order(){let mut s=Session::default();s.set_active(true);apply(&mut s,Mutation::Add{object:stroke(1,"left-125")});apply(&mut s,Mutation::Add{object:text(1)});apply(&mut s,Mutation::Add{object:stroke(2,"right-100")});let before=s.snapshot().objects;let ptr=s.objects["stroke-1"].clone();apply(&mut s,Mutation::Erase{ids:vec!["stroke-1".into(),"text-1".into(),"stroke-1".into()]});assert_eq!(s.len(),1);assert_eq!(s.history.len(),4);apply(&mut s,Mutation::Undo);assert_eq!(s.snapshot().objects,before);assert!(Arc::ptr_eq(&ptr,&s.objects["stroke-1"]));apply(&mut s,Mutation::Redo);assert_eq!(s.len(),1);apply(&mut s,Mutation::Undo);apply(&mut s,Mutation::Add{object:stroke(3,"right-100")});assert!(!s.snapshot().can_redo);}
 #[test]fn edit_text_is_same_object_and_stale_updates_do_not_overwrite(){let mut s=Session::default();s.set_active(true);let old=text(1);apply(&mut s,Mutation::Add{object:old.clone()});let old_revision=s.revision;let mut new=old.clone();if let Content::Text{text,..}=&mut new.content{*text="改名\n换行".into();}apply(&mut s,Mutation::Replace{object:new.clone()});assert_eq!(s.len(),1);assert!(s.mutate(s.epoch,old_revision,Mutation::Replace{object:old.clone()}).is_err());apply(&mut s,Mutation::Undo);assert_eq!(s.snapshot().objects[0].item,old);apply(&mut s,Mutation::Redo);assert_eq!(s.snapshot().objects[0].item,new);}
 #[test]fn capacity_500_lines_50_texts_bounded_history_and_no_monitor_reassignment(){let mut s=Session::default();s.set_active(true);for i in 0..500{apply(&mut s,Mutation::Add{object:stroke(i,if i%2==0{"left-125"}else{"right-100"})});}for i in 0..50{apply(&mut s,Mutation::Add{object:text(i)});}assert_eq!(s.len(),550);assert_eq!(s.history.len(),100);let snapshot=s.snapshot();assert_eq!(snapshot.objects.iter().filter(|e|e.item.monitor=="right-100").count(),250);for _ in 0..100{apply(&mut s,Mutation::Undo);}assert_eq!(s.len(),450);assert!(!s.snapshot().can_undo);for _ in 0..100{apply(&mut s,Mutation::Redo);}assert_eq!(s.snapshot().objects,snapshot.objects);}
 #[test]fn validation_rejects_nonfinite_oversized_unknown_style_and_cross_monitor_replacement_atomically(){let mut s=Session::default();s.set_active(true);let mut bad=stroke(0,"left");if let Content::Stroke{points,..}=&mut bad.content{points[0].x=f64::NAN;}assert!(s.mutate(s.epoch,0,Mutation::Add{object:bad}).is_err());assert_eq!(s.revision,0);apply(&mut s,Mutation::Add{object:text(0)});let mut moved=text(0);moved.monitor="right".into();assert!(s.mutate(s.epoch,s.revision,Mutation::Replace{object:moved}).is_err());assert_eq!(s.snapshot().objects[0].item.monitor,"left-125");let mut bad=text(1);if let Content::Text{text,style,..}=&mut bad.content{*text="x".repeat(16385);style.color="url(example)".into();}assert!(s.mutate(s.epoch,s.revision,Mutation::Add{object:bad}).is_err());assert_eq!(s.len(),1);}
 #[test]fn lost_reply_retry_does_not_undo_twice_and_failures_do_not_advance_stream(){let mut s=Session::default();s.set_active(true);apply(&mut s,Mutation::Add{object:stroke(1,"left")});apply(&mut s,Mutation::Add{object:stroke(2,"left")});let revision=s.revision;s.mutate_once("renderer-1",1,s.epoch,revision,Mutation::Undo).unwrap();assert_eq!(s.len(),1);s.mutate_once("renderer-1",1,s.epoch,revision,Mutation::Undo).unwrap();assert_eq!(s.len(),1);assert!(s.mutate_once("renderer-1",3,s.epoch,s.revision,Mutation::Undo).is_err());assert!(s.mutate_once("renderer-1",2,s.epoch,revision,Mutation::Undo).is_err());s.mutate_once("renderer-1",2,s.epoch,s.revision,Mutation::Undo).unwrap();assert_eq!(s.len(),0);}
 #[test]fn style_changes_advance_snapshot_revision_without_adding_history(){let mut s=Session::default();let old=s.snapshot();s.set_style(Style{color:COLORS[1].into(),width:6.}).unwrap();let current=s.snapshot();assert!(current.revision>old.revision);assert!(!current.can_undo);let revision=s.revision;s.set_style(s.style.clone()).unwrap();assert_eq!(s.revision,revision);}
 #[test]fn clear_all_monitors_is_one_undo_and_works_inactive(){let mut s=Session::default();s.set_active(true);apply(&mut s,Mutation::Add{object:stroke(1,"left")});apply(&mut s,Mutation::Add{object:stroke(2,"right")});apply(&mut s,Mutation::Add{object:text(1)});let before=s.snapshot().objects;s.set_active(false);apply(&mut s,Mutation::Clear);assert!(s.is_empty());assert!(!s.active);let revision=s.revision;apply(&mut s,Mutation::Clear);assert_eq!(s.revision,revision);s.set_active(true);apply(&mut s,Mutation::Undo);assert_eq!(s.snapshot().objects,before);apply(&mut s,Mutation::Redo);assert!(s.is_empty());}
 #[test]fn lost_clear_reply_cannot_erase_later_drawing(){let mut s=Session::default();s.set_active(true);apply(&mut s,Mutation::Add{object:stroke(1,"left")});let old=s.revision;assert!(s.mutate_once("clear",1,s.epoch,0,Mutation::Clear).is_err());assert_eq!(s.len(),1);s.mutate_once("clear",1,s.epoch,old,Mutation::Clear).unwrap();apply(&mut s,Mutation::Add{object:stroke(2,"right")});s.mutate_once("clear",1,s.epoch,s.revision,Mutation::Clear).unwrap();assert_eq!(s.len(),1);assert_eq!(s.snapshot().objects[0].item.id,"stroke-2");}

 #[test]fn translation_preserves_stroke_text_order_style_and_one_undo(){let mut s=Session::default();s.set_active(true);apply(&mut s,Mutation::Add{object:stroke(1,"left-125")});apply(&mut s,Mutation::Add{object:text(1)});let before=s.snapshot().objects;apply(&mut s,Mutation::Translate{object:before[0].item.clone(),dx:30.,dy:-10.});let moved=s.snapshot().objects;assert_eq!(moved[0].order,before[0].order);assert_eq!(moved[1],before[1]);if let Content::Stroke{points,style}=&moved[0].item.content{assert_eq!(points[0],Point{x:40.,y:10.});assert_eq!(*style,Style::default());}else{panic!()};apply(&mut s,Mutation::Undo);assert_eq!(s.snapshot().objects,before);apply(&mut s,Mutation::Redo);assert_eq!(s.snapshot().objects,moved);apply(&mut s,Mutation::Translate{object:before[1].item.clone(),dx:-5.,dy:12.});if let Content::Text{x,y,text,..}=&s.snapshot().objects[1].item.content{assert_eq!((*x,*y),(15.,42.));assert_eq!(text,"中文\n第二行");}else{panic!()}}
 #[test]fn translation_rejects_stale_content_and_nonfinite_without_mutation(){let mut s=Session::default();s.set_active(true);let object=text(1);apply(&mut s,Mutation::Add{object:object.clone()});let before=s.snapshot().objects;assert!(s.mutate(s.epoch,s.revision,Mutation::Translate{object:object.clone(),dx:f64::NAN,dy:0.}).is_err());assert!(s.mutate(s.epoch,s.revision,Mutation::Translate{object:object.clone(),dx:131072.,dy:0.}).is_err());assert_eq!(s.snapshot().objects,before);s.mutate_once("move",1,s.epoch,s.revision,Mutation::Translate{object:object.clone(),dx:20.,dy:30.}).unwrap();let moved=s.snapshot().objects;s.mutate_once("move",1,s.epoch,s.revision,Mutation::Translate{object:object.clone(),dx:20.,dy:30.}).unwrap();assert_eq!(s.snapshot().objects,moved);assert!(s.mutate(s.epoch,s.revision,Mutation::Translate{object,dx:10.,dy:0.}).is_err());assert_eq!(s.snapshot().objects,moved);}

}
