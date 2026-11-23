//! Host-installed pricing evidence. This is not provider discovery or a bill.
use crate::{contracts::{CapabilityRef,Media,ParameterValue,Resource,TaskRequest},resources::ResourceService};
use rusqlite::OptionalExtension;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};

/// Offline installer: existing isolated store only, exclusive writer lock applies.
pub fn install_file(root:&std::path::Path,path:&std::path::Path)->Result<String,String>{
 use std::io::Read;
 let mut bytes=Vec::new();std::fs::File::open(path).map_err(|_|"cost rule file unavailable")?.take(16385).read_to_end(&mut bytes).map_err(|_|"cost rule file unreadable")?;
 if bytes.len()>16384{return Err("cost rule file exceeds limit".into())}
 let rule:ImageCostRule=serde_json::from_slice(&bytes).map_err(|_|"invalid cost rule file")?;
 if !root.join("metadata.sqlite").is_file(){return Err("existing resource store required".into())}
 ResourceService::open(root)?.install_image_cost_rule(&rule)
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCostScope {
 pub size:String,pub quality:String,pub max_prompt_bytes:u32,
 pub max_input_width:u32,pub max_input_height:u32,pub max_input_bytes:u64,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCostRule {
 pub schema_version:u16,pub capability:CapabilityRef,pub ceiling_micros:u64,
 pub expires_at_s:u64,pub evidence:String,pub scope:ImageCostScope,
}
impl ImageCostScope {
 fn check(&self,request:&TaskRequest,input:&Resource)->Result<(),String>{
  let text=|key:&str|match request.parameters.get(key){Some(ParameterValue::Text(v))=>Some(v.as_str()),_=>None};
  let dimensions=matches!(input.media,Media::Image{width,height} if width<=self.max_input_width && height<=self.max_input_height);
  if request.inputs.len()!=1 || request.inputs[0].resource_id!=input.id || !dimensions || input.byte_length>self.max_input_bytes
   || text("size")!=Some(self.size.as_str()) || text("quality")!=Some(self.quality.as_str())
   || !text("prompt").is_some_and(|v|!v.trim().is_empty() && v.len()<=self.max_prompt_bytes as usize){
   return Err("图片或参数超出已核验费用范围，不会发送请求".into())
  }Ok(())
 }
}
impl ResourceService {
 /// Only a host/operator may install reviewed evidence. No WebView command.
 /// The ceiling must include input, output, currency conversion and gateway retries.
 pub fn install_image_cost_rule(&mut self,rule:&ImageCostRule)->Result<String,String>{
  let binding=self.image_binding(&rule.capability)?;let s=&rule.scope;
  if rule.schema_version!=1 || rule.ceiling_micros==0 || rule.ceiling_micros>1_000_000_000_000
   || rule.expires_at_s<=crate::image_budget::now_s()? || rule.expires_at_s>i64::MAX as u64
   || rule.evidence.trim().is_empty() || rule.evidence.len()>4096
   || s.max_prompt_bytes==0 || s.max_prompt_bytes>32000
   || s.max_input_width==0 || s.max_input_height==0 || s.max_input_width>crate::resources::MAX_DIMENSION || s.max_input_height>crate::resources::MAX_DIMENSION
   || s.max_input_bytes==0 || s.max_input_bytes>crate::resources::MAX_BYTES
   || !binding.policy.sizes.contains(&s.size) || !binding.policy.qualities.contains(&s.quality){return Err("invalid image cost rule".into())}
  let encoded=serde_json::to_string(rule).map_err(|_|"invalid image cost rule")?;
  let id=format!("scoped-{:x}",Sha256::digest(encoded.as_bytes()));
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO image_cost_approvals VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT DO NOTHING",rusqlite::params![id,rule.capability.connection_id,rule.capability.capability_id,rule.capability.revision,rule.ceiling_micros as i64,rule.expires_at_s as i64]).map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO image_cost_scopes VALUES(?1,?2) ON CONFLICT DO NOTHING",rusqlite::params![id,encoded]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(id)
 }
}

/// Also used within the dispatch transaction before reservation/execution_count.
pub(crate) fn check_scope(db:&rusqlite::Connection,request:&TaskRequest,input:&Resource)->Result<(),String>{
 let reference=&request.capability;
 let row:Option<(String,Option<String>)>=db.query_row("SELECT a.id,s.rule_json FROM image_cost_approvals a LEFT JOIN image_cost_scopes s ON s.approval_id=a.id WHERE a.connection_id=?1 AND a.capability_id=?2 AND a.revision=?3 ORDER BY a.rowid DESC LIMIT 1",rusqlite::params![reference.connection_id,reference.capability_id,reference.revision],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
 if let Some((id,encoded))=row {
  if let Some(encoded)=encoded {
   if id!=format!("scoped-{:x}",Sha256::digest(encoded.as_bytes())){return Err("image cost rule integrity mismatch".into())}
   let rule:ImageCostRule=serde_json::from_str(&encoded).map_err(|_|"invalid saved image cost rule")?;
   if rule.capability!=*reference{return Err("image cost rule identity mismatch".into())}
   rule.scope.check(request,input)?;
  }else if id.starts_with("scoped-"){return Err("image cost scope missing".into())}
 }
 Ok(())
}
