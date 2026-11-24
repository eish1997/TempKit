//! Current-user DPAPI vault. Only opaque references belong in configuration/events.
use std::{fs,io::{Read,Write},path::{Path,PathBuf}};
use sha2::{Digest,Sha256};
use std::os::windows::{fs::OpenOptionsExt,io::AsRawHandle};
use windows::{core::PCWSTR,Win32::{Foundation::{HLOCAL,LocalFree},Security::Cryptography::*}};
#[cfg(test)]thread_local!{static TEST_PARTIAL_WRITE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
fn write_encrypted(file:&mut fs::File,bytes:&[u8])->std::io::Result<()>{
 #[cfg(test)]if TEST_PARTIAL_WRITE.with(|fault|fault.get()){file.write_all(&bytes[..8.min(bytes.len())])?;return Err(std::io::Error::other("isolated partial write failure"));}
 file.write_all(bytes).and_then(|_|file.sync_all())
}
pub struct Secret(Vec<u8>);
impl Secret{pub fn from_bytes(bytes:Vec<u8>)->Self{Self(bytes)} pub fn bytes(&self)->&[u8]{&self.0}}
impl Drop for Secret{fn drop(&mut self){for byte in &mut self.0{unsafe{std::ptr::write_volatile(byte,0)}}}}
struct Output(CRYPT_INTEGER_BLOB);
impl Drop for Output{fn drop(&mut self){unsafe{if !self.0.pbData.is_null(){for i in 0..self.0.cbData as usize{std::ptr::write_volatile(self.0.pbData.add(i),0)}let _=LocalFree(Some(HLOCAL(self.0.pbData.cast())));}}}}
pub(crate) fn crypt(bytes:&[u8],binding:&[u8],protect:bool)->Result<Vec<u8>,String>{
 if bytes.is_empty()||bytes.len()>16384||binding.len()>4096{return Err("invalid credential envelope".into())}
 let input=CRYPT_INTEGER_BLOB{cbData:bytes.len() as u32,pbData:bytes.as_ptr() as *mut u8};let entropy=CRYPT_INTEGER_BLOB{cbData:binding.len() as u32,pbData:binding.as_ptr() as *mut u8};let mut output=Output(CRYPT_INTEGER_BLOB::default());
 unsafe{if protect{CryptProtectData(&input,PCWSTR::null(),Some(&entropy),None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output.0)}else{CryptUnprotectData(&input,None,Some(&entropy),None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output.0)}.map_err(|_|"credential protection unavailable or identity mismatch".to_string())?;
 if output.0.pbData.is_null()||output.0.cbData>16384{return Err("invalid protected credential".into())}Ok(std::slice::from_raw_parts(output.0.pbData,output.0.cbData as usize).to_vec())}
}
pub struct CredentialVault{root:PathBuf,_guard:fs::File}
impl CredentialVault{
 pub fn open(root:&Path)->Result<Self,String>{fs::create_dir_all(root).map_err(|_|"credential directory unavailable")?;let guard=crate::resources::guard_directory(root)?;Ok(Self{root:root.canonicalize().map_err(|_|"credential directory unavailable")?,_guard:guard})}
 fn location(&self,binding:&str)->Result<(PathBuf,Vec<u8>),String>{
  if binding.is_empty()||binding.len()>512||binding.chars().any(char::is_control){return Err("invalid credential reference".into())}
  let entropy=format!("QuickGen credential v1|{}|{}",self.root.display(),binding).into_bytes();let key=format!("{:x}.dpapi",Sha256::digest(binding.as_bytes()));Ok((self.root.join(key),entropy))
 }
 pub fn save_new(&self,binding:&str,token:&[u8])->Result<(),String>{
  if token.is_empty()||token.len()>4096||!token.iter().all(|b|b.is_ascii_graphic()){return Err("invalid credential value".into())}
  let(path,entropy)=self.location(binding)?;let encrypted=crypt(token,&entropy,true)?;
  // Exclusive ownership until durable completion. DELETE access lets failure
  // cleanup target this exact newly-created file rather than resolving a path.
  let mut file=fs::OpenOptions::new().write(true).access_mode(0x40000000|0x00010000).share_mode(0).create_new(true).open(path).map_err(|_|"credential reference exists or cannot be created")?;
  if write_encrypted(&mut file,&encrypted).is_err(){
   use windows::Win32::{Foundation::HANDLE,Storage::FileSystem::{SetFileInformationByHandle,FileDispositionInfo,FILE_DISPOSITION_INFO}};
   let disposition=FILE_DISPOSITION_INFO{DeleteFile:true};
   unsafe{SetFileInformationByHandle(HANDLE(file.as_raw_handle()),FileDispositionInfo,(&disposition as *const FILE_DISPOSITION_INFO).cast(),std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32)}.map_err(|_|"凭据保存失败且未能清除半成品；已保留加密文件".to_string())?;
   return Err("credential could not be saved".into())
  }
  Ok(())
 }
 pub fn load(&self,binding:&str)->Result<Secret,String>{
  let(path,entropy)=self.location(binding)?;let file=crate::resources::open_blob(&path).map_err(|_|"credential missing or inaccessible")?;let mut encrypted=Vec::new();file.take(16385).read_to_end(&mut encrypted).map_err(|_|"credential read failed")?;
  let secret=Secret(crypt(&encrypted,&entropy,false)?);if secret.0.is_empty()||secret.0.len()>4096||!secret.0.iter().all(|b|b.is_ascii_graphic()){return Err("invalid stored credential".into())}Ok(secret)
 }
 pub fn remove(&self,binding:&str)->Result<(),String>{
  let(path,_)=self.location(binding)?;
  match fs::remove_file(path){Ok(())=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(_)=>Err("connection revoked but credential removal failed; retry removal".into())}
 }
 pub fn contains(&self,binding:&str)->Result<bool,String>{
  let(path,_)=self.location(binding)?;
  path.try_exists().map_err(|_|"credential presence unavailable".into())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn partial_credential_write_can_retry_without_touching_existing_credentials(){
  let root=std::env::temp_dir().join(format!("quickgen-vault-partial-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let vault=CredentialVault::open(&root).unwrap();
  vault.save_new("existing",b"synthetic-existing").unwrap();
  TEST_PARTIAL_WRITE.with(|fault|fault.set(true));let result=vault.save_new("new",b"synthetic-new");TEST_PARTIAL_WRITE.with(|fault|fault.set(false));assert!(result.is_err());
  assert_eq!(vault.load("existing").unwrap().bytes(),b"synthetic-existing");
  let(path,_)=vault.location("new").unwrap();assert!(!path.exists(),"partial credential left behind after write failure");
  vault.save_new("new",b"synthetic-new").unwrap();assert_eq!(vault.load("new").unwrap().bytes(),b"synthetic-new");assert!(vault.save_new("existing",b"replacement").is_err());assert_eq!(vault.load("existing").unwrap().bytes(),b"synthetic-existing");
 }
 #[test]fn isolated_vault_roundtrip_and_wrong_binding(){
  let root=std::env::temp_dir().join(format!("quickgen-vault-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let vault=CredentialVault::open(&root).unwrap();let binding="connection-a/account-a/token-v1";let token=b"synthetic-test-token";
  vault.save_new(binding,token).unwrap();assert_eq!(vault.load(binding).unwrap().bytes(),token);assert!(vault.save_new(binding,token).is_err());
  let(path,entropy)=vault.location(binding).unwrap();let bytes=fs::read(path).unwrap();assert!(!bytes.windows(token.len()).any(|part|part==token));assert!(crypt(&bytes,b"wrong identity",false).is_err());
  let mut damaged=bytes;damaged[0]^=255;assert!(crypt(&damaged,&entropy,false).is_err());assert!(vault.load("connection-b/account-a/token-v1").is_err());assert!(vault.save_new("another",b"bad\r\nheader").is_err());
 }
}
