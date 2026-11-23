//! Host profile data referenced by the existing Connection contract; never contains a token.
use serde::{Serialize,Deserialize};
use crate::contracts::{Connection,ConnectionStatus};
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayProfile {
 pub schema_version:u16,
 pub profile_id:String,
 pub connection_id:String,
 pub account_id:String,
 pub base_url:String,
}
impl GatewayProfile {
 pub fn validate(&self)->Result<(),String>{
  if self.connection_id=="internal-mock"{return Err("internal mock connection ID is reserved".into())}
  let reference=Connection{schema_version:self.schema_version,id:self.connection_id.clone(),adapter_id:"new-api".into(),profile_id:self.profile_id.clone(),credential_ref:Some(self.account_id.clone()),status:ConnectionStatus::Unverified};
  reference.validate().map_err(|_|"invalid profile identity".to_string())?;
  let url=url::Url::parse(&self.base_url).map_err(|_|"invalid gateway address".to_string())?;
  if !url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some(){return Err("gateway address must not contain credentials, query or fragment".into())}
  let local=match url.host(){Some(url::Host::Ipv4(ip))=>ip.is_loopback(),Some(url::Host::Ipv6(ip))=>ip.is_loopback(),Some(url::Host::Domain(host))=>host=="localhost",None=>false};
  if url.scheme()!="https"&&!(url.scheme()=="http"&&local){return Err("non-loopback gateways require HTTPS".into())}
  if url.host().is_none()||url.path()!="/"{return Err("gateway address must be an origin without endpoint path".into())}
  Ok(())
 }
 pub fn bind(&self,connection:&Connection)->Result<(),String>{
  self.validate()?;connection.validate().map_err(|_|"invalid connection".to_string())?;
  if connection.id!=self.connection_id||connection.profile_id!=self.profile_id||connection.adapter_id!="new-api"{return Err("connection profile mismatch".into())}Ok(())
 }
}
/// Insert-only version semantics, later persisted transactionally by the host catalogue.
pub fn insert_version(profiles:&mut std::collections::BTreeMap<String,GatewayProfile>,profile:GatewayProfile)->Result<(),String>{
 profile.validate()?;
 if let Some(old)=profiles.get(&profile.profile_id){if old!=&profile{return Err("profile version conflict; create a new profile ID".into())}return Ok(())}
 profiles.insert(profile.profile_id.clone(),profile);Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 fn profile()->GatewayProfile{GatewayProfile{schema_version:1,profile_id:"gateway-v1".into(),connection_id:"gateway".into(),account_id:"account-a".into(),base_url:"http://127.0.0.1:3000".into()}}
 #[test]fn address_security_and_local_http(){
  for address in ["http://127.0.0.1:3000","http://[::1]:3000","http://localhost:3000","https://example.com"]{let mut p=profile();p.base_url=address.into();assert!(p.validate().is_ok(),"{address}")}
  for address in ["http://example.com","file:///x","https://user:secret@example.com","https://example.com/?key=secret","https://example.com/#secret","https://example.com/v1/images/edits"]{let mut p=profile();p.base_url=address.into();assert!(p.validate().is_err(),"{address}")}
 }
 #[test]fn version_never_retargets_old_account_or_address(){
  let mut profiles=std::collections::BTreeMap::new();let old=profile();insert_version(&mut profiles,old.clone()).unwrap();insert_version(&mut profiles,old.clone()).unwrap();
  let mut changed=old.clone();changed.account_id="account-b".into();assert!(insert_version(&mut profiles,changed.clone()).is_err());changed.account_id=old.account_id.clone();changed.base_url="https://other.example".into();assert!(insert_version(&mut profiles,changed.clone()).is_err());
  changed.profile_id="gateway-v2".into();insert_version(&mut profiles,changed).unwrap();assert_eq!(profiles["gateway-v1"],old);
  let connection=Connection{schema_version:1,id:"gateway".into(),profile_id:"gateway-v2".into(),adapter_id:"new-api".into(),credential_ref:None,status:ConnectionStatus::Unverified};assert!(old.bind(&connection).is_err());
 }
 #[test]fn rejects_token_in_serialized_profile(){let mut value=serde_json::to_value(profile()).unwrap();value["token"]=serde_json::json!("secret");assert!(serde_json::from_value::<GatewayProfile>(value).is_err())}
 #[test]fn gateway_cannot_claim_mock_executor_identity(){let mut value=profile();value.connection_id="internal-mock".into();assert!(value.validate().is_err());}
}

impl crate::resources::ResourceService {
 pub fn save_gateway_profile(&mut self,profile:&GatewayProfile)->Result<(),String>{
  profile.validate()?;
  let tx=self.store.connection.transaction().map_err(|e|e.to_string())?;
  use rusqlite::OptionalExtension;
  let old:Option<String>=tx.query_row("SELECT profile_json FROM gateway_profiles WHERE profile_id=?1",[&profile.profile_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  if let Some(old)=old {
   let prior:GatewayProfile=serde_json::from_str(&old).map_err(|e|e.to_string())?;
   if prior!=*profile{return Err("profile version conflict; create a new profile ID".into())}
  }else{tx.execute("INSERT INTO gateway_profiles VALUES(?1,?2)",rusqlite::params![profile.profile_id,serde_json::to_string(profile).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;}
  tx.commit().map_err(|e|e.to_string())
 }
 pub fn gateway_profile(&self,id:&str)->Result<GatewayProfile,String>{
  let text:String=self.store.connection.query_row("SELECT profile_json FROM gateway_profiles WHERE profile_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  let profile:GatewayProfile=serde_json::from_str(&text).map_err(|e|e.to_string())?;profile.validate()?;Ok(profile)
 }
}
#[cfg(test)]mod persistence_tests{
 use super::*;
 #[test]fn restart_and_sql_constraints_keep_historical_target(){
  let root=std::env::temp_dir().join(format!("quickgen-profile-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let profile=GatewayProfile{schema_version:1,profile_id:"v1".into(),connection_id:"local".into(),account_id:"a".into(),base_url:"http://127.0.0.1:3000".into()};
  let mut host=crate::resources::ResourceService::open(&root).unwrap();host.save_gateway_profile(&profile).unwrap();host.save_gateway_profile(&profile).unwrap();
  let mut changed=profile.clone();changed.base_url="https://other.example".into();assert!(host.save_gateway_profile(&changed).is_err());
  assert!(host.store.connection.execute("UPDATE gateway_profiles SET profile_json='{}'",[]).is_err());assert!(host.store.connection.execute("DELETE FROM gateway_profiles",[]).is_err());drop(host);
  let host=crate::resources::ResourceService::open(&root).unwrap();assert_eq!(host.gateway_profile("v1").unwrap(),profile);
 }
}
