//! Call on the native UI thread with validated DIBV5 bytes from the bounded decoder.
use windows::Win32::{Foundation::{HWND,HGLOBAL,HANDLE,GlobalFree},System::{Memory::{GlobalAlloc,GlobalLock,GlobalUnlock,GMEM_MOVEABLE},DataExchange::{OpenClipboard,CloseClipboard,EmptyClipboard,SetClipboardData}}};
struct Memory(Option<HGLOBAL>);
impl Drop for Memory{fn drop(&mut self){if let Some(handle)=self.0.take(){unsafe{let _=GlobalFree(Some(handle));}}}}
struct Clipboard;
impl Drop for Clipboard{fn drop(&mut self){unsafe{let _=CloseClipboard();}}}
pub fn write_dib(owner:HWND,dib:&[u8])->Result<(),String>{
 if owner.0.is_null()||dib.len()<124||dib.len() as u64>124+crate::resources::MAX_PIXELS*4{return Err("invalid clipboard image or owner".into())}
 unsafe{
  // Allocate before emptying the clipboard, so allocation failures preserve prior contents.
  let handle=GlobalAlloc(GMEM_MOVEABLE,dib.len()).map_err(|e|e.to_string())?;let mut memory=Memory(Some(handle));
  let pointer=GlobalLock(handle);if pointer.is_null(){return Err("clipboard memory lock failed".into())}
  std::ptr::copy_nonoverlapping(dib.as_ptr(),pointer.cast::<u8>(),dib.len());
  // A zero return can mean the lock count reached zero; the buffer is no longer used here.
  let _=GlobalUnlock(handle);
  OpenClipboard(Some(owner)).map_err(|_|"剪贴板正被其他程序占用，请重试".to_string())?;let _clipboard=Clipboard;
  EmptyClipboard().map_err(|e|e.to_string())?;
  SetClipboardData(17,Some(HANDLE(handle.0))).map_err(|e|format!("图片未写入剪贴板：{e}"))?; // CF_DIBV5
  memory.0=None; // Ownership transferred only on successful SetClipboardData.
  Ok(())
 }
}
