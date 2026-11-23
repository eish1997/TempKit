//! Native COPY-only OLE operation. Caller owns the export copy lifetime and UI restoration.
use std::{path::Path, os::windows::ffi::OsStrExt};
use windows::core::{implement, BOOL, HRESULT, PCWSTR};
use windows::Win32::{Foundation::{DRAGDROP_S_CANCEL,DRAGDROP_S_DROP,DRAGDROP_S_USEDEFAULTCURSORS},System::{Com::{IDataObject,IBindCtx},Ole::{IDropSource,IDropSource_Impl,DROPEFFECT,DROPEFFECT_COPY,DoDragDrop,OleInitialize,OleUninitialize},SystemServices::{MODIFIERKEYS_FLAGS,MK_LBUTTON}},UI::Shell::{IShellItem,SHCreateItemFromParsingName,BHID_DataObject}};

use std::sync::{Arc,atomic::{AtomicU32,Ordering}};
use windows::Win32::System::Ole::{IDropSourceNotify,IDropSourceNotify_Impl};
#[derive(Default)]
struct DragTrace{queries:AtomicU32,first_keys:AtomicU32,last_keys:AtomicU32,feedback:AtomicU32,accepted_feedback:AtomicU32,targets:AtomicU32}
#[implement(IDropSource,IDropSourceNotify)]
struct CopySource{trace:Arc<DragTrace>}
impl IDropSource_Impl for CopySource_Impl {
 fn QueryContinueDrag(&self, escape:BOOL, keys:MODIFIERKEYS_FLAGS)->HRESULT {
  if self.trace.queries.fetch_add(1,Ordering::Relaxed)==0{self.trace.first_keys.store(keys.0,Ordering::Relaxed);}
  self.trace.last_keys.store(keys.0,Ordering::Relaxed);
  if escape.as_bool(){DRAGDROP_S_CANCEL}else if keys.0 & MK_LBUTTON.0 == 0 {DRAGDROP_S_DROP}else{HRESULT(0)}
 }
 fn GiveFeedback(&self,effect:DROPEFFECT)->HRESULT{self.trace.feedback.fetch_add(1,Ordering::Relaxed);if effect.0!=0{self.trace.accepted_feedback.fetch_add(1,Ordering::Relaxed);}DRAGDROP_S_USEDEFAULTCURSORS}
}
impl IDropSourceNotify_Impl for CopySource_Impl{
 fn DragEnterTarget(&self,_:windows::Win32::Foundation::HWND)->windows::core::Result<()>{self.trace.targets.fetch_add(1,Ordering::Relaxed);Ok(())}
 fn DragLeaveTarget(&self)->windows::core::Result<()>{Ok(())}
}
struct OleScope;
impl Drop for OleScope{fn drop(&mut self){unsafe{OleUninitialize()}}}
#[derive(Debug,PartialEq,Eq)]
pub enum DropOutcome{Copied,Cancelled}
// ResourceService deliberately uses canonical extended paths. Shell parsing does
// not accept that namespace. Convert only at this boundary and verify identity;
// retain the original guarded export lease throughout the OLE operation.
fn shell_path(path:&Path)->Result<Vec<u16>,String>{
 use std::os::windows::ffi::OsStringExt;
 let canonical=path.canonicalize().map_err(|e|format!("export path resolution failed: {e}"))?;
 let units:Vec<u16>=canonical.as_os_str().encode_wide().collect();
 let prefix:Vec<u16>="\\\\?\\".encode_utf16().collect();
 let unc:Vec<u16>="UNC\\".encode_utf16().collect();
 let converted=if units.starts_with(&prefix){
  let tail=&units[4..];
  if tail.starts_with(&unc){let mut out=vec![92,92];out.extend_from_slice(&tail[4..]);out}
  else if tail.len()>=3&&tail[1]==58&&tail[2]==92{tail.to_vec()}
  else{return Err("unsupported Shell path namespace".into())}
 }else{units};
 let shell=std::path::PathBuf::from(std::ffi::OsString::from_wide(&converted));
 if shell.canonicalize().map_err(|e|format!("Shell path resolution failed: {e}"))?!=canonical{return Err("Shell path changes export identity".into())}
 if converted.contains(&0){return Err("invalid export path".into())}
 Ok(converted.into_iter().chain(Some(0)).collect())
}
/// Must run on a dedicated STA or the native UI thread, never the resource worker.
/// `path` must be a validated export COPY, not a managed original. No cleanup here:
/// Explorer may retain the data object and read the file after this call returns.
pub fn drag_export_copy(path:&Path,diagnostic:Option<&Path>)->Result<DropOutcome,String>{
 if !path.is_absolute()||!path.is_file(){return Err("export copy is unavailable".into())}
 let wide=shell_path(path)?;
 unsafe{
  OleInitialize(None).map_err(|e|format!("OLE initialization failed: {e}"))?;
  let _scope=OleScope;
  let item:IShellItem=SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()),None::<&IBindCtx>).map_err(|e|format!("Shell export item failed: {e}"))?;
  let data:IDataObject=item.BindToHandler(None::<&IBindCtx>,&BHID_DataObject).map_err(|e|format!("Shell export data failed: {e}"))?;
  let trace=Arc::new(DragTrace::default());
  let source:IDropSource=CopySource{trace:trace.clone()}.into();let mut effect=DROPEFFECT(0);
  let started=std::time::Instant::now();
  let result=DoDragDrop(&data,&source,DROPEFFECT_COPY,&mut effect);
  // Only the explicitly isolated runtime supplies this path. No clipboard data,
  // file names, target window identity, or user content is recorded.
  if let Some(file)=diagnostic{
   let value=serde_json::json!({"elapsedMs":started.elapsed().as_millis(),"hresult":result.0,"effect":effect.0,"queries":trace.queries.load(Ordering::Relaxed),"firstKeys":trace.first_keys.load(Ordering::Relaxed),"lastKeys":trace.last_keys.load(Ordering::Relaxed),"feedback":trace.feedback.load(Ordering::Relaxed),"acceptedFeedback":trace.accepted_feedback.load(Ordering::Relaxed),"targets":trace.targets.load(Ordering::Relaxed)});
   if let Ok(bytes)=serde_json::to_vec_pretty(&value){let _=std::fs::write(file,bytes);}
  }
  if result==DRAGDROP_S_CANCEL{return Ok(DropOutcome::Cancelled)}
  if result==DRAGDROP_S_DROP && effect==DROPEFFECT_COPY{return Ok(DropOutcome::Copied)}
  Err(format!("Drag did not confirm a copy: {result:?}, effect {}",effect.0))
 }
}

#[cfg(test)]
mod shell_tests {
 use super::*;
 #[test]fn canonical_export_binds_after_boundary_conversion(){
  let dir=std::env::temp_dir().join(format!("quickgen-shell-fixed-{}",std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();let path=dir.join("测试 image.png");std::fs::write(&path,b"isolated test file").unwrap();
  let canonical=path.canonicalize().unwrap();let wide=shell_path(&canonical).unwrap();
  unsafe{OleInitialize(None).unwrap();let _scope=OleScope;
   let item:IShellItem=SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()),None::<&IBindCtx>).unwrap();
   let data:IDataObject=item.BindToHandler(None::<&IBindCtx>,&BHID_DataObject).unwrap();
   use windows::Win32::{System::{Com::{FORMATETC,DVASPECT_CONTENT,TYMED_HGLOBAL},Ole::ReleaseStgMedium},UI::Shell::{HDROP,DragQueryFileW}};
   let format=FORMATETC{cfFormat:15,ptd:std::ptr::null_mut(),dwAspect:DVASPECT_CONTENT.0,lindex:-1,tymed:TYMED_HGLOBAL.0 as u32};
   let mut medium=data.GetData(&format).unwrap();
   let drop=HDROP(medium.u.hGlobal.0);
   let count=DragQueryFileW(drop,u32::MAX,None);
   let size=DragQueryFileW(drop,0,None);let mut name=vec![0u16;size as usize+1];
   DragQueryFileW(drop,0,Some(&mut name));ReleaseStgMedium(&mut medium);
   assert_eq!(count,1);name.pop();
   use std::os::windows::ffi::OsStringExt;
   let exported=std::path::PathBuf::from(std::ffi::OsString::from_wide(&name));
   assert_eq!(exported.canonicalize().unwrap(),canonical);
   assert_eq!(std::fs::read(exported).unwrap(),b"isolated test file");
  }
  assert_eq!(std::fs::read(&canonical).unwrap(),b"isolated test file");
 }
 #[test]fn diagnose_canonical_shell_path(){
  let dir=std::env::temp_dir().join(format!("quickgen-shell-path-{}",std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();let plain=dir.join("test.png");std::fs::write(&plain,b"isolated shell path probe").unwrap();
  let canonical=plain.canonicalize().unwrap();
  unsafe{OleInitialize(None).unwrap();let _scope=OleScope;
   for (name,path) in [("plain",plain),("canonical",canonical)] {
    let wide:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
    let item:windows::core::Result<IShellItem>=SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()),None::<&IBindCtx>);
    match item {Ok(item)=>{let data:windows::core::Result<IDataObject>=item.BindToHandler(None::<&IBindCtx>,&BHID_DataObject);println!("{name}: create OK; bind {:?}",data.map(|_|()));},Err(e)=>println!("{name}: create {e}")}
   }
  }
 }
}
