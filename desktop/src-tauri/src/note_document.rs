//! Versioned, HTML-free note document. The host owns validation and plain projection.
use serde_json::{Value,json};
use std::collections::BTreeSet;
pub const TEXT_LIMIT:usize=1_048_576;
pub const STRUCTURE_LIMIT:usize=128*1024*1024;
pub const COLORS:&[&str]=&["default","yellow","green","blue","pink","purple","paper"];
pub fn from_text(text:&str)->Value{json!({"version":1,"doc":{"type":"doc","content":text.split('\n').map(|line|if line.is_empty(){json!({"type":"paragraph"})}else{json!({"type":"paragraph","content":[{"type":"text","text":line}]})}).collect::<Vec<_>>()}})}
pub fn supported(value:&Value)->bool{value["version"].as_u64()==Some(1)}
fn fields(value:&Value,allowed:&[&str])->Result<(),String>{let obj=value.as_object().ok_or("invalid_note_document_object")?;if obj.keys().any(|key|!allowed.contains(&key.as_str())){return Err("unknown_note_document_field".into())}Ok(())}
fn id(value:&Value)->Result<String,String>{let id=value.as_str().ok_or("invalid_note_node_id")?;if id.is_empty()||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_'){return Err("invalid_note_node_id".into())}Ok(id.into())}
struct Check{nodes:usize,ids:BTreeSet<String>,lines:Vec<String>,bytes:usize}
impl Check{
 fn node(&mut self,n:&Value,parent:&str,depth:usize)->Result<String,String>{
  self.nodes+=1;if self.nodes>2_100_000||depth>4{return Err("note_document_complexity_limit".into())}
  fields(n,&["type","attrs","content","text","marks"])?;
  let kind=n["type"].as_str().ok_or("invalid_note_node")?;
  let allowed=match parent{"root"=>kind=="doc","doc"=>matches!(kind,"paragraph"|"blockquote"|"horizontal_rule"),"blockquote"=>kind=="paragraph","paragraph"=>matches!(kind,"text"|"tag"|"hard_break"),_=>false};
  if !allowed{return Err("invalid_note_node_nesting".into())}
  if let Some(marks)=n.get("marks"){if !matches!(kind,"text"|"tag"|"hard_break"){return Err("invalid_note_marks".into())}let marks=marks.as_array().ok_or("invalid_note_marks")?;if marks.len()>2{return Err("invalid_note_marks".into())}let mut seen=BTreeSet::new();for mark in marks{fields(mark,&["type"])?;let k=mark["type"].as_str().ok_or("invalid_note_mark")?;if !matches!(k,"strong"|"highlight")||!seen.insert(k){return Err("invalid_note_mark".into())}}}
  if kind!="text"&&n.get("text").is_some(){return Err("unexpected_note_text".into())}
  let empty=json!({});let attrs=n.get("attrs").unwrap_or(&empty);
  match kind{
   "paragraph"=>{fields(attrs,&["todo","done"])?;for key in ["todo","done"]{if attrs.get(key).is_some_and(|v|!v.is_boolean()){return Err("invalid_note_paragraph".into())}}if attrs["done"]==true&&attrs["todo"]!=true{return Err("invalid_note_todo".into())}},
   "blockquote"=>{fields(attrs,&["id"])?;if !self.ids.insert(id(&attrs["id"])?){return Err("duplicate_note_node_id".into())}},
   "tag"=>{fields(attrs,&["id","label","createdAt","color"])?;if !self.ids.insert(id(&attrs["id"])?){return Err("duplicate_note_node_id".into())}let label=attrs["label"].as_str().ok_or("invalid_note_tag")?;if label.is_empty()||label.len()>4096||label.chars().any(char::is_control){return Err("invalid_note_tag".into())}if attrs["createdAt"].as_u64().is_none()||!COLORS.contains(&attrs["color"].as_str().unwrap_or("default")){return Err("invalid_note_tag_metadata".into())}},
   _=>fields(attrs,&[])?,
  }
  let leaf=matches!(kind,"text"|"tag"|"hard_break"|"horizontal_rule");
  if leaf&&n.get("content").is_some(){return Err("unexpected_note_content".into())}
  let mut text=match kind{"text"=>{let t=n["text"].as_str().ok_or("invalid_note_text")?;if t.is_empty(){return Err("empty_note_text_node".into())}t.to_string()},"tag"=>attrs["label"].as_str().unwrap().into(),"hard_break"=>"\n".into(),_=>String::new()};
  if !leaf{let children=match n.get("content"){None if kind=="paragraph"=>&[][..],Some(v)=>v.as_array().ok_or("invalid_note_content")?.as_slice(),_=>return Err("missing_note_content".into())};if children.is_empty()&&kind!="paragraph"{return Err("empty_note_block".into())}for child in children{text.push_str(&self.node(child,kind,depth+1)?);if text.len()>TEXT_LIMIT{return Err("note_text_limit".into())}}}
  if matches!(kind,"paragraph"|"horizontal_rule"){self.bytes+=text.len()+usize::from(!self.lines.is_empty());if self.bytes>TEXT_LIMIT{return Err("note_text_limit".into())}self.lines.push(text);return Ok(String::new())}
  Ok(text)
 }
}
pub fn validate(document:&Value)->Result<String,String>{
 if !supported(document){return Err("unsupported_note_document_version".into())}
 fields(document,&["version","doc"])?;
 if serde_json::to_vec(document).map_err(|e|e.to_string())?.len()>STRUCTURE_LIMIT{return Err("note_document_size_limit".into())}
 let mut check=Check{nodes:0,ids:BTreeSet::new(),lines:vec![],bytes:0};check.node(&document["doc"],"root",0)?;Ok(check.lines.join("\n"))
}
/// New IDs obtain host time. Existing IDs retain their original birth even after undo.
pub fn stamp_tags<F>(document:&mut Value,lookup:&mut F)->Result<(),String> where F:FnMut(&str)->Result<u64,String>{
 fn walk<F>(n:&mut Value,lookup:&mut F)->Result<(),String> where F:FnMut(&str)->Result<u64,String>{
  if n["type"]=="tag"{let stamp=lookup(n["attrs"]["id"].as_str().ok_or("invalid_note_tag")?)?;n["attrs"]["createdAt"]=json!(stamp);}
  if let Some(children)=n.get_mut("content").and_then(Value::as_array_mut){for child in children{walk(child,lookup)?;}}Ok(())
 }walk(&mut document["doc"],lookup)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn plain_text_roundtrip_preserves_empty_lines_crlf_and_limit(){for text in ["","\n\n","中文\n\n尾行\n","一\r\n二"]{assert_eq!(validate(&from_text(text)).unwrap(),text)}let limit="x".repeat(TEXT_LIMIT);assert_eq!(validate(&from_text(&limit)).unwrap(),limit);assert!(validate(&from_text(&(limit+"x"))).is_err());}
 #[test]fn structured_content_projects_only_user_text(){let d=json!({"version":1,"doc":{"type":"doc","content":[{"type":"blockquote","attrs":{"id":"q1"},"content":[{"type":"paragraph","attrs":{"todo":true,"done":true},"content":[{"type":"text","text":"内容","marks":[{"type":"strong"}]},{"type":"tag","attrs":{"id":"t1","label":"标签","createdAt":42,"color":"paper"}}]}]},{"type":"horizontal_rule"},{"type":"paragraph"}]}});assert_eq!(validate(&d).unwrap(),"内容标签\n\n");let mut bad=d.clone();bad["doc"]["content"][0]["attrs"]["onclick"]=json!("x");assert!(validate(&bad).is_err());let mut bad=d.clone();bad["version"]=json!(2);assert_eq!(validate(&bad).unwrap_err(),"unsupported_note_document_version");}
 #[test]fn unsafe_nodes_marks_and_duplicates_are_rejected(){for bad in [json!({"type":"script","text":"x"}),json!({"type":"text","text":"x","marks":[{"type":"link","attrs":{"href":"https://example.com"}}]})]{let mut d=from_text("x");d["doc"]["content"][0]["content"][0]=bad;assert!(validate(&d).is_err());}}
}
