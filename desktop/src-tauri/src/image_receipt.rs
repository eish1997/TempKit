//! Signed output URLs are protected recovery material, never frontend state.
use std::{path::{Path,PathBuf},io::{Read,Write}};
use crate::contracts::Task;
pub(crate) fn path(root:&Path,task:&Task)->PathBuf{crate::image_execution::received_path(root,task).with_extension("url.dpapi")}
fn binding(task:&Task)->Vec<u8>{use sha2::{Digest,Sha256};format!("QuickGen image output URL v1|{:x}",Sha256::digest(serde_json::to_vec(task).expect("serializable task"))).into_bytes()}
pub(crate) fn read(root:&Path,task:&Task)->Result<String,String>{
 let file=crate::resources::open_blob(&path(root,task)).map_err(|_|"图片下载恢复记录不可读取")?;
 let mut bytes=Vec::new();file.take(16385).read_to_end(&mut bytes).map_err(|_|"图片下载恢复记录不可读取")?;
 let decoded=crate::credential_vault::crypt(&bytes,&binding(task),false).map_err(|_|"图片下载恢复记录校验失败")?;
 let remote=String::from_utf8(decoded).map_err(|_|"图片下载恢复记录格式错误")?;
 crate::gateway_discovery::image_download_url(&remote)?;Ok(remote)
}
pub(crate) fn save(root:&Path,task:&Task,remote:&str,pending:u64)->Result<(),String>{
 crate::gateway_discovery::image_download_url(remote)?;
 let _guard=crate::resources::guard_directory(&root.join("staging"))?;
 let target=path(root,task);
 if target.exists(){return if read(root,task)?==remote{Ok(())}else{Err("图片下载恢复记录冲突".into())}}
 let encrypted=crate::credential_vault::crypt(remote.as_bytes(),&binding(task),true).map_err(|_|"图片下载恢复记录无法保护")?;
 crate::image_execution::check_received_capacity(root,pending,encrypted.len() as u64)?;
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"clock unavailable")?.as_nanos();
 let partial=target.with_extension(format!("{stamp}.partial"));
 let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&partial).map_err(|_|"图片下载恢复记录无法保存")?;
 let result=file.write_all(&encrypted).and_then(|_|file.sync_all());drop(file);
 if result.is_err(){let _=std::fs::remove_file(&partial);return Err("图片下载恢复记录无法保存".into())}
 std::fs::rename(partial,target).map_err(|_|"图片下载恢复记录无法发布".into())
}
pub(crate) fn remove(root:&Path,task:&Task){let _=std::fs::remove_file(path(root,task));}
