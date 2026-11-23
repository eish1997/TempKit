//! Conservative CNY budget reservations, not provider billing measurements.
//! Only the host may install a verified per-attempt ceiling and user-approved cap.
use crate::{contracts::{CapabilityRef,Task},resources::ResourceService};
use rusqlite::OptionalExtension;
#[cfg(any(test,feature="desktop-diagnostics"))]
use sha2::{Digest,Sha256};
const MAX_MICROS:u64=1_000_000_000_000;
pub fn now_s()->Result<u64,String>{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_secs()).map_err(|_|"system clock unavailable".into())}
#[derive(serde::Serialize)]
pub struct ImageBudgetStatus{pub limit_micros:Option<u64>,pub reserved_micros:u64}
impl ResourceService{
 /// No default amount: calling this requires an explicit user-authorized cap.
 /// This phase has no reset/raise API; restart cannot replenish spent allowance.
 pub fn establish_image_budget(&mut self,limit_micros:u64)->Result<(),String>{
  if limit_micros==0||limit_micros>MAX_MICROS{return Err("invalid image budget".into())}
  let old:Option<i64>=self.store.connection.query_row("SELECT limit_micros FROM image_budget WHERE id=1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  if let Some(old)=old{return if old==limit_micros as i64{Ok(())}else{Err("existing budget cannot be reset or changed".into())}}
  self.store.connection.execute("INSERT INTO image_budget VALUES(1,?1)",[limit_micros as i64]).map_err(|e|e.to_string())?;Ok(())
 }
 /// Must cover every allowed parameter/input for this immutable capability,
 /// including gateway retries. Discovery alone is not evidence of such a ceiling.
 #[cfg(any(test,feature="desktop-diagnostics"))]
 pub fn approve_image_cost(&mut self,reference:&CapabilityRef,ceiling_micros:u64,expires_at_s:u64)->Result<(),String>{
  self.image_binding(reference)?;
  if ceiling_micros==0||ceiling_micros>MAX_MICROS||expires_at_s<=now_s()?||expires_at_s>i64::MAX as u64{return Err("invalid or expired image cost approval".into())}
  let encoded=serde_json::to_vec(&(reference,ceiling_micros,expires_at_s)).map_err(|_|"invalid cost approval")?;let id=format!("{:x}",Sha256::digest(encoded));
  self.store.connection.execute("INSERT INTO image_cost_approvals VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT DO NOTHING",rusqlite::params![id,reference.connection_id,reference.capability_id,reference.revision,ceiling_micros as i64,expires_at_s as i64]).map_err(|e|e.to_string())?;Ok(())
 }
 pub fn image_budget_status(&self)->Result<ImageBudgetStatus,String>{
  let limit:Option<i64>=self.store.connection.query_row("SELECT limit_micros FROM image_budget WHERE id=1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  let reserved:i64=self.store.connection.query_row("SELECT COALESCE(sum(r.amount_micros),0) FROM image_budget_reservations r LEFT JOIN image_budget_releases f ON f.task_id=r.task_id AND f.attempt=r.attempt WHERE f.task_id IS NULL",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  let limit_micros=limit.map(u64::try_from).transpose().map_err(|_|"invalid saved budget")?;
  let reserved_micros=u64::try_from(reserved).map_err(|_|"invalid saved reservations")?;
  Ok(ImageBudgetStatus{limit_micros,reserved_micros})
 }
}
/// Called in the same transaction as Running/execution_count, before any send.
pub(crate) fn reserve(tx:&rusqlite::Transaction<'_>,task:&Task)->Result<(),String>{
 let(id,amount)=available_approval(tx,&task.request.capability)?;
 tx.execute("INSERT INTO image_budget_reservations VALUES(?1,?2,?3,?4)",rusqlite::params![task.id,task.attempt,id,amount]).map_err(|e|e.to_string())?;Ok(())
}

fn available_approval(connection:&rusqlite::Connection,reference:&CapabilityRef)->Result<(String,i64),String>{
 let limit:Option<i64>=connection.query_row("SELECT limit_micros FROM image_budget WHERE id=1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;let limit=limit.ok_or("尚未设置图片调用总额度")?;
 let approval:Option<(String,i64,i64)>=connection.query_row("SELECT id,ceiling_micros,expires_at_s FROM image_cost_approvals WHERE connection_id=?1 AND capability_id=?2 AND revision=?3 ORDER BY rowid DESC LIMIT 1",rusqlite::params![reference.connection_id,reference.capability_id,reference.revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?;
 let(id,amount,expires)=approval.ok_or("模型费用上限尚未核验或已过期")?;
 #[cfg(not(any(test,feature="desktop-diagnostics")))]
 if !id.starts_with("scoped-"){return Err("模型费用规则缺少适用范围，不会发送请求".into())}
 // Never fall back to an older, cheaper approval when the latest one expires.
 if expires<=now_s()? as i64{return Err("模型费用上限尚未核验或已过期".into())}
 let reserved:i64=connection.query_row("SELECT COALESCE(sum(r.amount_micros),0) FROM image_budget_reservations r LEFT JOIN image_budget_releases f ON f.task_id=r.task_id AND f.attempt=r.attempt WHERE f.task_id IS NULL",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if reserved.checked_add(amount).filter(|total|*total<=limit).is_none(){return Err("剩余图片调用额度不足，不会发送请求".into())}
 Ok((id,amount))
}
impl ResourceService{
 pub fn check_image_budget(&self,reference:&CapabilityRef)->Result<(),String>{available_approval(&self.store.connection,reference).map(|_|())}
}
#[cfg(test)]mod settings_tests{
 use super::*;
 #[test]fn setting_total_cap_never_authorizes_unverified_model_or_resets_after_restart(){
  let root=std::env::temp_dir().join(format!("quickgen-budget-settings-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  assert!(host.establish_image_budget(0).is_err());assert!(host.establish_image_budget(MAX_MICROS+1).is_err());assert_eq!(host.image_budget_status().unwrap().limit_micros,None);
  host.establish_image_budget(2_000_001).unwrap();host.establish_image_budget(2_000_001).unwrap();assert!(host.establish_image_budget(3_000_000).is_err());
  drop(host);let mut host=ResourceService::open(&root).unwrap();let status=host.image_budget_status().unwrap();assert_eq!(status.limit_micros,Some(2_000_001));assert_eq!(status.reserved_micros,0);
  assert!(host.establish_image_budget(1).is_err());assert!(host.task_snapshot().unwrap().is_empty());
  let reference=CapabilityRef{connection_id:"unverified".into(),capability_id:"image".into(),revision:"1".into()};
  assert!(host.check_image_budget(&reference).unwrap_err().contains("费用上限尚未核验"));
 }
}
