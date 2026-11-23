#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::{Mutex,Arc,atomic::{AtomicBool,Ordering}};
use tauri::{Manager,Emitter};
use windows::Win32::{Foundation::POINT,UI::WindowsAndMessaging::{GetCursorPos,PeekMessageW,MSG,PM_REMOVE,WM_HOTKEY,MessageBoxW,MB_OK,MB_ICONERROR}};
#[path="../overlay_support.rs"] mod overlay_support;
use overlay_support::{ModeHotkey,DisplayArea,panel_bounds};

#[derive(Clone,Copy,Default,serde::Serialize)]
struct Mode { passthrough:bool, pinned:bool }
#[derive(Default)]
struct Probe { mode:Mutex<Mode>, bridge:Mutex<Option<tauri::ipc::Channel<serde_json::Value>>> }
#[derive(Default)]
struct DisplayState(Mutex<Option<DisplayArea>>);
fn sync_display(app:&tauri::AppHandle)->Result<(),String>{
 let monitor=app.primary_monitor().map_err(|e|e.to_string())?.or_else(||app.available_monitors().ok()?.into_iter().next()).ok_or("No monitor available")?;
 let work=monitor.work_area();let area=DisplayArea{x:work.position.x,y:work.position.y,width:work.size.width,height:work.size.height,scale:monitor.scale_factor()};
 let state=app.state::<DisplayState>();let mut previous=state.0.lock().map_err(|e|e.to_string())?;
 if *previous==Some(area){return Ok(())}
 let bounds=panel_bounds(area)?;
 let board=app.get_webview_window("board").ok_or("Missing board")?;let panel=app.get_webview_window("probe-panel").ok_or("Missing panel")?;
 board.set_position(work.position).map_err(|e|e.to_string())?;board.set_size(work.size).map_err(|e|e.to_string())?;
 panel.set_position(tauri::PhysicalPosition::new(bounds.x,bounds.y)).map_err(|e|e.to_string())?;
 panel.set_size(tauri::PhysicalSize::new(bounds.width,bounds.height)).map_err(|e|e.to_string())?;
 app.emit("probe-display-changed",()).map_err(|e|e.to_string())?;
 *previous=Some(area);Ok(())
}
fn local_point(x:i32,y:i32,ox:i32,oy:i32,scale:f64)->(f64,f64){((x as f64-ox as f64)/scale,(y as f64-oy as f64)/scale)}
fn apply(app:&tauri::AppHandle,next:Mode)->Result<Mode,String>{
 let board=app.get_webview_window("board").ok_or("Missing board")?;
 let panel=app.get_webview_window("probe-panel").ok_or("Missing panel")?;
 // Keep the recovery control visible until native hit testing has changed successfully.
 board.set_ignore_cursor_events(next.passthrough).map_err(|e|e.to_string())?;
 let panel_result=if next.passthrough&&!next.pinned {panel.hide()}else{panel.show()};
 if let Err(error)=panel_result{let previous=*app.state::<Probe>().mode.lock().map_err(|e|e.to_string())?;let _=board.set_ignore_cursor_events(previous.passthrough);let _=panel.show();return Err(error.to_string())}
 if !next.passthrough{let _=panel.set_focus();}
 *app.state::<Probe>().mode.lock().map_err(|e|e.to_string())?=next;
 app.emit("probe-mode",next).map_err(|e|e.to_string())?;Ok(next)
}
#[tauri::command]
fn probe_toggle(app:tauri::AppHandle)->Result<Mode,String>{let mut next=*app.state::<Probe>().mode.lock().map_err(|e|e.to_string())?;next.passthrough=!next.passthrough;apply(&app,next)}
#[tauri::command]
fn probe_pin(app:tauri::AppHandle)->Result<Mode,String>{let mut next=*app.state::<Probe>().mode.lock().map_err(|e|e.to_string())?;next.pinned=!next.pinned;apply(&app,next)}
#[tauri::command]
fn probe_state(app:tauri::AppHandle)->Result<Mode,String>{Ok(*app.state::<Probe>().mode.lock().map_err(|e|e.to_string())?)}
#[tauri::command]
fn probe_subscribe(window:tauri::WebviewWindow,channel:tauri::ipc::Channel<serde_json::Value>)->Result<(),String>{
 if window.label()!="board"{return Err("Board only".into())}
 *window.state::<Probe>().bridge.lock().map_err(|e|e.to_string())?=Some(channel);Ok(())
}
#[tauri::command]
fn probe_quit(app:tauri::AppHandle){app.exit(0)}
fn main(){
 let mut context=tauri::generate_context!();context.config_mut().identifier="com.quickgen.overlay-probe".into();context.config_mut().app.windows.clear();
 let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
 let app=tauri::Builder::default().manage(Probe::default()).manage(DisplayState::default()).setup(move|app|{
  let board=tauri::WebviewWindowBuilder::new(app,"board",tauri::WebviewUrl::App("overlay-probe.html".into())).title("QuickGen · B1 画板验证").transparent(true).decorations(false).shadow(false).always_on_top(true).visible(false).build()?;
  let panel=tauri::WebviewWindowBuilder::new(app,"probe-panel",tauri::WebviewUrl::App("overlay-probe.html?panel".into())).title("QuickGen · B1 控制").inner_size(240.,150.).decorations(false).resizable(false).always_on_top(true).visible(false).build()?;
  sync_display(app.handle())?;
  let handle=app.handle().clone();let (send,recv)=std::sync::mpsc::sync_channel(1);
  std::thread::spawn(move||{
   let _hotkey=match ModeHotkey::register(1){Ok(guard)=>{let _=send.send(Ok::<(),String>(()));guard},Err(error)=>{let _=send.send(Err(error));return}};
   let mut display_tick=0;
   while !worker_stop.load(Ordering::Relaxed){
    let mut msg=MSG::default();while unsafe{PeekMessageW(&mut msg,None,0,0,PM_REMOVE)}.as_bool(){if msg.message==WM_HOTKEY&&msg.wParam.0==1{let h=handle.clone();let _=handle.run_on_main_thread(move||{if let Err(error)=probe_toggle(h.clone()){let _=h.emit("probe-error",error);}});}}
    display_tick+=1;if display_tick>=30{display_tick=0;let h=handle.clone();let _=handle.run_on_main_thread(move||{if let Err(error)=sync_display(&h){let _=h.emit("probe-error",error);}});}
    let mut point=POINT::default();if unsafe{GetCursorPos(&mut point)}.is_ok(){
     if let Some(board)=handle.get_webview_window("board"){
      if let (Ok(origin),Ok(scale))=(board.inner_position(),board.scale_factor()){
       let (x,y)=local_point(point.x,point.y,origin.x,origin.y,scale);
       let state=handle.state::<Probe>();if let Ok(bridge)=state.bridge.lock(){if let Some(channel)=bridge.as_ref(){let _=channel.send(serde_json::json!({"x":x,"y":y}));}};
      }
     }
    }
    std::thread::sleep(std::time::Duration::from_millis(33));
   }
  });
  recv.recv().map_err(|e|e.to_string())??;
  board.show()?;panel.show()?;panel.set_focus()?;Ok(())
 }).invoke_handler(tauri::generate_handler![probe_toggle,probe_pin,probe_state,probe_subscribe,probe_quit]).build(context);
 let app=match app{Ok(app)=>app,Err(error)=>{stop.store(true,Ordering::Relaxed);unsafe{let _=MessageBoxW(None,&windows::core::HSTRING::from(format!("浮层验证未启动：{error}")),windows::core::w!("QuickGen 浮层验证"),MB_OK|MB_ICONERROR);}return}};
 app.run(move|_,event|{if matches!(event,tauri::RunEvent::Exit){stop.store(true,Ordering::Relaxed)}});
}
#[cfg(test)]mod tests{use super::*;#[test]fn physical_to_local_handles_negative_monitor_and_dpi(){assert_eq!(local_point(-1620,450,-1920,0,1.5),(200.,300.));assert_eq!(local_point(140,260,100,200,2.),(20.,30.));}}
