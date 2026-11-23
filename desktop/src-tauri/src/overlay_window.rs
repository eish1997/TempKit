//! A transparent desktop utility must not masquerade as an opaque full-screen
//! application window to native occlusion tracking. Tao rewrites EXSTYLE during
//! input-mode changes, so preserve the tool-window role at the style boundary.
use windows::Win32::{Foundation::{HWND,LPARAM,LRESULT,WPARAM},UI::{Shell::{DefSubclassProc,RemoveWindowSubclass,SetWindowSubclass},WindowsAndMessaging::*}};
const SUBCLASS_ID:usize=0x51474f43;
fn tool_style(style:u32)->u32{(style|WS_EX_TOOLWINDOW.0)&!WS_EX_APPWINDOW.0}
unsafe extern "system" fn preserve_role(hwnd:HWND,message:u32,wparam:WPARAM,lparam:LPARAM,_id:usize,_data:usize)->LRESULT{
 if message==WM_STYLECHANGING&&wparam.0 as i32==GWL_EXSTYLE.0&&!((lparam.0 as *mut STYLESTRUCT).is_null()){
  let styles=&mut *(lparam.0 as *mut STYLESTRUCT);styles.styleNew=tool_style(styles.styleNew);
  return LRESULT(0)
 }
 if message==WM_NCDESTROY{let _=RemoveWindowSubclass(hwnd,Some(preserve_role),SUBCLASS_ID);}
 DefSubclassProc(hwnd,message,wparam,lparam)
}
/// Call on the creating UI thread, before showing the main window.
pub fn install(hwnd:HWND)->Result<(),String>{unsafe{
 if !SetWindowSubclass(hwnd,Some(preserve_role),SUBCLASS_ID,0).as_bool(){return Err("Cannot preserve overlay window role".into())}
 let previous=GetWindowLongW(hwnd,GWL_EXSTYLE) as u32;
 SetWindowLongW(hwnd,GWL_EXSTYLE,tool_style(previous) as i32);
 if GetWindowLongW(hwnd,GWL_EXSTYLE) as u32&WS_EX_TOOLWINDOW.0==0{let _=RemoveWindowSubclass(hwnd,Some(preserve_role),SUBCLASS_ID);return Err("Cannot set overlay tool-window role".into())}
 SetWindowPos(hwnd,None,0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE|SWP_FRAMECHANGED).map_err(|e|e.to_string())?;
 Ok(())
}}
#[cfg(test)]mod tests{
 use super::*;use windows::core::w;
 #[test]fn overlay_role_survives_native_style_rewrites_without_forcing_passthrough(){unsafe{
  // Hidden native fixture: no desktop/UI changes or production data.
  let hwnd=CreateWindowExW(WS_EX_WINDOWEDGE|WS_EX_ACCEPTFILES,w!("STATIC"),w!("isolated overlay style fixture"),WS_OVERLAPPED,0,0,1920,1080,None,None,None,None).unwrap();
  let before=GetWindowLongW(hwnd,GWL_EXSTYLE) as u32;
  assert_eq!(before&(WS_EX_TOOLWINDOW.0|WS_EX_TRANSPARENT.0|WS_EX_LAYERED.0),0,"ordinary Tao-style host lacks an occlusion exemption");
  install(hwnd).unwrap();
  for proposed in [WS_EX_ACCEPTFILES.0,WS_EX_ACCEPTFILES.0|WS_EX_TRANSPARENT.0|WS_EX_LAYERED.0,WS_EX_ACCEPTFILES.0,WS_EX_APPWINDOW.0|WS_EX_NOACTIVATE.0]{
   SetWindowLongW(hwnd,GWL_EXSTYLE,proposed as i32);
   let actual=GetWindowLongW(hwnd,GWL_EXSTYLE) as u32;
   // Windows may restore WINDOWEDGE for this STATIC fixture. Assert the
   // role and input bits we own, not unrelated OS-managed frame bits.
   assert_eq!(actual&(WS_EX_TOOLWINDOW.0|WS_EX_APPWINDOW.0),WS_EX_TOOLWINDOW.0);
   assert_eq!(actual&WS_EX_ACCEPTFILES.0,proposed&WS_EX_ACCEPTFILES.0);
   assert_eq!(actual&(WS_EX_TRANSPARENT.0|WS_EX_LAYERED.0|WS_EX_NOACTIVATE.0),proposed&(WS_EX_TRANSPARENT.0|WS_EX_LAYERED.0|WS_EX_NOACTIVATE.0));
  }
  DestroyWindow(hwnd).unwrap();assert!(!IsWindow(Some(hwnd)).as_bool());
 }}
}
