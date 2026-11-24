//! Host-owned immutable connection versions. Configuration is not authentication.
use crate::{contracts::{Connection,ConnectionStatus},gateway_profiles::GatewayProfile,credential_vault::{CredentialVault,Secret},resources::ResourceService};
use rusqlite::OptionalExtension;
fn binding(profile:&GatewayProfile,reference:&str)->Result<String,String>{
 serde_json::to_string(&(&profile.connection_id,&profile.profile_id,&profile.account_id,reference)).map_err(|_|"invalid credential binding".into())
}
impl ResourceService{
 /// Display-only metadata. The profile validator forbids userinfo, query and fragment.
 pub fn gateway_connection_summaries(&self)->Result<Vec<serde_json::Value>,String>{
  self.gateway_connections()?.into_iter().map(|connection|{
   let profile=self.gateway_profile(&connection.profile_id)?;profile.bind(&connection)?;
   let mut value=serde_json::to_value(&connection).map_err(|_|"connection summary unavailable")?;
   value["account_id"]=serde_json::json!(profile.account_id);
   value["base_url"]=serde_json::json!(profile.base_url);
   let mut pending=false;
   if connection.status==ConnectionStatus::Revoked{
    let original:String=self.store.connection.query_row("SELECT connection_json FROM gateway_connections WHERE connection_id=?1",[&connection.id],|r|r.get(0)).map_err(|_|"connection record unavailable")?;
    let original:Connection=serde_json::from_str(&original).map_err(|_|"invalid connection record")?;
    let mut references:Vec<String>=original.credential_ref.into_iter().collect();
    let mut query=self.store.connection.prepare("SELECT credential_ref FROM gateway_credential_rotations WHERE connection_id=?1").map_err(|_|"credential history unavailable")?;
    references.extend(query.query_map([&connection.id],|r|r.get::<_,String>(0)).map_err(|_|"credential history unavailable")?.collect::<Result<Vec<_>,_>>().map_err(|_|"credential history unavailable")?);
    let vault=CredentialVault::open(&self.root.join("credentials"))?;
    for reference in references{pending|=vault.contains(&binding(&profile,&reference)?)?;}
   }
   value["credential_cleanup_pending"]=serde_json::json!(pending);
   Ok(value)
  }).collect()
 }
 pub fn gateway_connections(&self)->Result<Vec<Connection>,String>{
  let mut statement=self.store.connection.prepare("SELECT connection_json,revoked FROM gateway_connections ORDER BY connection_id").map_err(|_|"connection catalogue unavailable")?;
  let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?))).map_err(|_|"connection catalogue unavailable")?;
  let mut result=Vec::new();
  for row in rows{let(text,revoked)=row.map_err(|_|"connection catalogue unavailable")?;let mut connection:Connection=serde_json::from_str(&text).map_err(|_|"invalid connection record")?;
   connection.validate().map_err(|_|"invalid connection record")?;
   let rotated:Option<String>=self.store.connection.query_row("SELECT credential_ref FROM gateway_credential_rotations WHERE connection_id=?1 ORDER BY revision DESC LIMIT 1",[&connection.id],|r|r.get(0)).optional().map_err(|_|"credential version unavailable")?;
   if let Some(reference)=rotated{connection.credential_ref=Some(reference);}
   let checked:bool=self.store.connection.query_row("SELECT EXISTS(SELECT 1 FROM gateway_checks WHERE connection_id=?1)",[&connection.id],|r|r.get(0)).map_err(|_|"connection check unavailable")?;
   connection.status=if revoked{ConnectionStatus::Revoked}else if checked{ConnectionStatus::Ready}else{ConnectionStatus::Unverified};result.push(connection);
  }Ok(result)
 }
 pub fn configure_gateway(&mut self,profile:GatewayProfile,token:Secret)->Result<Connection,String>{
  profile.validate()?;
  if self.gateway_connections()?.iter().any(|c|c.id==profile.connection_id){return Err("connection ID already exists; create a new version".into())}
  // Persist only a reference, with the encrypted credential durable before admission.
  self.save_gateway_profile(&profile)?;
  let reference=format!("credential-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"clock unavailable")?.as_nanos());
  let credential_binding=binding(&profile,&reference)?;
  let connection=Connection{schema_version:1,id:profile.connection_id.clone(),adapter_id:"new-api".into(),profile_id:profile.profile_id.clone(),credential_ref:Some(reference),status:ConnectionStatus::Unverified};
  profile.bind(&connection)?;
  let encoded=serde_json::to_string(&connection).map_err(|_|"invalid connection")?;
  let vault=CredentialVault::open(&self.root.join("credentials"))?;
  vault.save_new(&credential_binding,token.bytes())?;
  let saved:Result<(),String>=(||{
   let tx=self.store.connection.transaction().map_err(|_|"connection transaction unavailable".to_string())?;
   tx.execute("INSERT INTO gateway_connections(connection_id,profile_id,connection_json) VALUES(?1,?2,?3)",rusqlite::params![connection.id,connection.profile_id,encoded]).map_err(|_|"connection could not be saved".to_string())?;
   tx.commit().map_err(|_|"connection could not be committed".to_string())
  })();
  if let Err(error)=saved{
   // Drop/rollback the transaction before checking durable absence. Do not erase
   // a potentially referenced credential when commit/rollback state is unclear.
   let absent=self.store.connection.is_autocommit()&&self.store.connection.query_row("SELECT NOT EXISTS(SELECT 1 FROM gateway_connections WHERE connection_id=?1)",[&connection.id],|row|row.get::<_,bool>(0)).unwrap_or(false);
   if !absent{return Err("连接保存失败，状态无法确认；已保留加密凭据，请保留数据目录".into())}
   vault.remove(&credential_binding).map_err(|_|"连接保存失败，新凭据未能清除；已保留加密文件".to_string())?;
   return Err(error)
  }
  Ok(connection)
 }
 pub fn gateway_secret(&self,id:&str)->Result<Secret,String>{
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  if connection.status==ConnectionStatus::Revoked{return Err("connection revoked".into())}
  let profile=self.gateway_profile(&connection.profile_id)?;profile.bind(&connection)?;
  let reference=connection.credential_ref.as_deref().ok_or("credential not configured")?;
  CredentialVault::open(&self.root.join("credentials"))?.load(&binding(&profile,reference)?)
 }
 pub fn revoke_gateway(&mut self,id:&str)->Result<(),String>{
  // Revoke first, so failed credential removal cannot leave an executable connection.
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  self.store.connection.execute("UPDATE gateway_connections SET revoked=1 WHERE connection_id=?1",[id]).map_err(|_|"connection could not be revoked")?;
  let profile=self.gateway_profile(&connection.profile_id)?;
  let original:String=self.store.connection.query_row("SELECT connection_json FROM gateway_connections WHERE connection_id=?1",[id],|r|r.get(0)).map_err(|_|"connection record unavailable")?;
  let original:Connection=serde_json::from_str(&original).map_err(|_|"invalid connection record")?;
  let mut references:Vec<String>=original.credential_ref.into_iter().collect();
  let mut query=self.store.connection.prepare("SELECT credential_ref FROM gateway_credential_rotations WHERE connection_id=?1").map_err(|_|"credential history unavailable")?;
  references.extend(query.query_map([id],|r|r.get::<_,String>(0)).map_err(|_|"credential history unavailable")?.collect::<Result<Vec<_>,_>>().map_err(|_|"credential history unavailable")?);
  let vault=CredentialVault::open(&self.root.join("credentials"))?;let mut failed=false;
  for reference in references{if vault.remove(&binding(&profile,&reference)?).is_err(){failed=true}}
  if failed{return Err("连接已撤销，部分加密凭据清除失败；请重试清除凭据".into())}
  Ok(())
 }
 /// User explicitly confirms the existing account. Address/account changes must
 /// create a new connection; already-dispatched work keeps its captured secret.
 pub fn rotate_gateway_credential(&mut self,id:&str,expected_ref:&str,account_id:&str,token:Secret)->Result<Connection,String>{
  let mut connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  if connection.status==ConnectionStatus::Revoked{return Err("connection revoked".into())}
  if connection.credential_ref.as_deref()!=Some(expected_ref){return Err("credential version conflict".into())}
  let profile=self.gateway_profile(&connection.profile_id)?;profile.bind(&connection)?;
  if profile.account_id!=account_id{return Err("account mismatch; create a new connection".into())}
  let reference=format!("credential-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"clock unavailable")?.as_nanos());
  let credential_binding=binding(&profile,&reference)?;let vault=CredentialVault::open(&self.root.join("credentials"))?;vault.save_new(&credential_binding,token.bytes())?;
  let saved:Result<(),String>=(||{
   let tx=self.store.connection.transaction().map_err(|_|"rotation transaction unavailable".to_string())?;
   let latest:Option<(i64,String)>=tx.query_row("SELECT revision,credential_ref FROM gateway_credential_rotations WHERE connection_id=?1 ORDER BY revision DESC LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|"credential version unavailable".to_string())?;
   let revision=match latest{Some((revision,prior))=>{if prior!=expected_ref{return Err("credential version conflict".into())}revision.checked_add(1).ok_or("credential revision overflow")?},None=>1};
   let revoked:bool=tx.query_row("SELECT revoked FROM gateway_connections WHERE connection_id=?1",[id],|r|r.get(0)).map_err(|_|"connection unavailable".to_string())?;if revoked{return Err("connection revoked".into())}
   tx.execute("INSERT INTO gateway_credential_rotations VALUES(?1,?2,?3,?4)",rusqlite::params![id,revision,reference,expected_ref]).map_err(|_|"credential rotation could not be saved".to_string())?;
   tx.execute("DELETE FROM gateway_checks WHERE connection_id=?1",[id]).map_err(|_|"connection check could not be invalidated".to_string())?;
   tx.commit().map_err(|_|"credential rotation could not be committed".to_string())
  })();
  if let Err(error)=saved{
   let absent=self.store.connection.is_autocommit()&&self.store.connection.query_row("SELECT NOT EXISTS(SELECT 1 FROM gateway_credential_rotations WHERE credential_ref=?1)",[&reference],|r|r.get::<_,bool>(0)).unwrap_or(false);
   if !absent{return Err("令牌轮换状态无法确认；已保留加密凭据".into())}
   vault.remove(&credential_binding).map_err(|_|"令牌轮换失败，新加密凭据未能清除".to_string())?;return Err(error)
  }
  connection.credential_ref=Some(reference);connection.status=ConnectionStatus::Unverified;Ok(connection)
 }
 pub fn record_gateway_check_version(&mut self,id:&str,expected_ref:&str,models:Option<Vec<String>>)->Result<(),String>{
  let connection=self.gateway_connections()?.into_iter().find(|c|c.id==id).ok_or("connection not found")?;
  if connection.credential_ref.as_deref()!=Some(expected_ref){return Err("credential changed during connection check".into())}
  self.record_gateway_check(id,models)
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn credential_rotation_preserves_identity_and_rejects_stale_authentication(){
  let root=std::env::temp_dir().join(format!("quickgen-rotation-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();let profile=GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"same-account".into(),base_url:"http://127.0.0.1:1".into()};
  let first=host.configure_gateway(profile.clone(),Secret::from_bytes(b"synthetic-old".to_vec())).unwrap();let old=first.credential_ref.as_deref().unwrap();
  let summaries=host.gateway_connection_summaries().unwrap();
  assert_eq!(summaries[0]["account_id"],"same-account");
  assert_eq!(summaries[0]["base_url"],"http://127.0.0.1:1");
  assert_eq!(summaries[0]["id"],first.id);
  assert!(!serde_json::to_string(&summaries).unwrap().contains("synthetic-old"));
  assert_eq!(summaries[0].as_object().unwrap().len(),9);
  host.record_gateway_check_version("c",old,Some(vec!["fixture".into()])).unwrap();let in_flight=host.gateway_secret("c").unwrap();
  assert!(host.rotate_gateway_credential("c",old,"different-account",Secret::from_bytes(b"wrong-account".to_vec())).is_err());
  host.store.connection.execute_batch("CREATE TRIGGER reject_rotation AFTER INSERT ON gateway_credential_rotations BEGIN SELECT RAISE(FAIL,'isolated rotation failure'); END;").unwrap();
  assert!(host.rotate_gateway_credential("c",old,"same-account",Secret::from_bytes(b"synthetic-new".to_vec())).is_err());
  assert_eq!(host.gateway_secret("c").unwrap().bytes(),b"synthetic-old");assert_eq!(host.gateway_connections().unwrap()[0].status,ConnectionStatus::Ready);assert_eq!(std::fs::read_dir(root.join("credentials")).unwrap().count(),1);
  host.store.connection.execute_batch("DROP TRIGGER reject_rotation;").unwrap();
  let next=host.rotate_gateway_credential("c",old,"same-account",Secret::from_bytes(b"synthetic-new".to_vec())).unwrap();assert_eq!(next.status,ConnectionStatus::Unverified);assert_ne!(next.credential_ref,first.credential_ref);
  assert_eq!(host.gateway_profile("p").unwrap(),profile);assert_eq!(next.id,first.id);assert_eq!(next.profile_id,first.profile_id);assert_eq!(in_flight.bytes(),b"synthetic-old");assert_eq!(host.gateway_secret("c").unwrap().bytes(),b"synthetic-new");
  assert!(host.record_gateway_check_version("c",old,Some(vec!["fixture".into()])).is_err());assert_eq!(host.gateway_connections().unwrap()[0].status,ConnectionStatus::Unverified);
  assert!(host.rotate_gateway_credential("c",old,"same-account",Secret::from_bytes(b"stale".to_vec())).is_err());
  host.record_gateway_check_version("c",next.credential_ref.as_deref().unwrap(),Some(vec!["fixture".into()])).unwrap();drop(host);
  let mut host=ResourceService::open(&root).unwrap();assert_eq!(host.gateway_secret("c").unwrap().bytes(),b"synthetic-new");assert_eq!(host.gateway_profile("p").unwrap(),profile);
  assert!(host.record_gateway_check_version("c",old,None).is_err());assert_eq!(host.gateway_connections().unwrap()[0].status,ConnectionStatus::Ready);
  host.revoke_gateway("c").unwrap();assert!(host.gateway_secret("c").is_err());assert_eq!(std::fs::read_dir(root.join("credentials")).unwrap().count(),0);assert!(host.rotate_gateway_credential("c",next.credential_ref.as_deref().unwrap(),"same-account",Secret::from_bytes(b"revive".to_vec())).is_err());
 }
 #[test]fn failed_catalogue_save_removes_only_its_new_credential(){
  let root=std::env::temp_dir().join(format!("quickgen-credential-rollback-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  let original=GatewayProfile{schema_version:1,profile_id:"original".into(),connection_id:"original".into(),account_id:"test".into(),base_url:"http://127.0.0.1:1".into()};
  host.configure_gateway(original,Secret::from_bytes(b"synthetic-original".to_vec())).unwrap();
  let profile=GatewayProfile{schema_version:1,profile_id:"retry".into(),connection_id:"retry".into(),account_id:"test".into(),base_url:"http://127.0.0.1:1".into()};
  host.store.connection.execute_batch("CREATE TRIGGER reject_connection BEFORE INSERT ON gateway_connections BEGIN SELECT RAISE(ABORT,'isolated catalogue failure'); END;").unwrap();
  assert!(host.configure_gateway(profile.clone(),Secret::from_bytes(b"synthetic-retry".to_vec())).is_err());
  assert_eq!(host.gateway_connections().unwrap().len(),1);assert_eq!(host.gateway_secret("original").unwrap().bytes(),b"synthetic-original");
  assert_eq!(std::fs::read_dir(root.join("credentials")).unwrap().count(),1,"failed save left an encrypted orphan");
  host.store.connection.execute_batch("DROP TRIGGER reject_connection; CREATE TRIGGER reject_after_connection AFTER INSERT ON gateway_connections BEGIN SELECT RAISE(FAIL,'isolated late failure'); END;").unwrap();
  assert!(host.configure_gateway(profile.clone(),Secret::from_bytes(b"synthetic-retry".to_vec())).is_err());
  assert_eq!(host.gateway_connections().unwrap().len(),1);assert_eq!(std::fs::read_dir(root.join("credentials")).unwrap().count(),1,"late failure must roll back before cleanup");
  host.store.connection.execute_batch("DROP TRIGGER reject_after_connection;").unwrap();drop(host);
  let mut host=ResourceService::open(&root).unwrap();host.configure_gateway(profile,Secret::from_bytes(b"synthetic-retry".to_vec())).unwrap();
  assert_eq!(host.gateway_secret("retry").unwrap().bytes(),b"synthetic-retry");assert_eq!(host.gateway_secret("original").unwrap().bytes(),b"synthetic-original");
 }
 #[test]fn revoked_summary_recovers_pending_cleanup_after_restart(){
  let root=std::env::temp_dir().join(format!("quickgen-cleanup-summary-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  host.configure_gateway(GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"account".into(),base_url:"http://127.0.0.1:1".into()},Secret::from_bytes(b"synthetic".to_vec())).unwrap();
  host.store.connection.execute("UPDATE gateway_connections SET revoked=1 WHERE connection_id='c'",[]).unwrap();drop(host);
  let mut host=ResourceService::open(&root).unwrap();assert_eq!(host.gateway_connection_summaries().unwrap()[0]["credential_cleanup_pending"],true);
  host.revoke_gateway("c").unwrap();drop(host);
  let host=ResourceService::open(&root).unwrap();assert_eq!(host.gateway_connection_summaries().unwrap()[0]["credential_cleanup_pending"],false);
 }
 #[test]fn credentials_survive_restart_and_revocation_never_retargets_history(){
  let root=std::env::temp_dir().join(format!("quickgen-connection-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let mut host=ResourceService::open(&root).unwrap();
  let profile=GatewayProfile{schema_version:1,profile_id:"profile-a".into(),connection_id:"connection-a".into(),account_id:"account-a".into(),base_url:"http://127.0.0.1:3000".into()};
  let saved=host.configure_gateway(profile.clone(),Secret::from_bytes(b"synthetic-connection-token".to_vec())).unwrap();
  assert_eq!(saved.status,ConnectionStatus::Unverified);
  assert!(!serde_json::to_string(&host.gateway_connections().unwrap()).unwrap().contains("synthetic-connection-token"));
  assert!(host.configure_gateway(profile.clone(),Secret::from_bytes(b"replacement".to_vec())).is_err());drop(host);
  let mut host=ResourceService::open(&root).unwrap();assert_eq!(host.gateway_secret("connection-a").unwrap().bytes(),b"synthetic-connection-token");
  host.revoke_gateway("connection-a").unwrap();host.revoke_gateway("connection-a").unwrap();assert!(host.gateway_secret("connection-a").is_err());
  assert_eq!(host.gateway_connections().unwrap()[0].status,ConnectionStatus::Revoked);
  assert_eq!(host.gateway_profile("profile-a").unwrap(),profile);
  assert!(host.store.connection.execute("UPDATE gateway_connections SET revoked=0",[]).is_err());
  assert!(host.store.connection.execute("DELETE FROM gateway_connections",[]).is_err());
 }
}
