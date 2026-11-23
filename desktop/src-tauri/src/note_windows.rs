use tauri::{Manager,Emitter};
use std::{collections::{BTreeMap,BTreeSet},sync::{Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
#[derive(Default)]
pub struct FlushState { ready:Mutex<BTreeSet<String>>,pending:Mutex<BTreeMap<u64,BTreeMap<String,Option<Result<(),String>>>>>,next:AtomicU64,overview:AtomicBool }
#[tauri::command]
pub fn notes_release(window:tauri::WebviewWindow)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 for (id,child) in window.app_handle().webview_windows(){if id.starts_with("note-"){child.emit_to(child.label(),"note-flush-release",()).map_err(|e|e.to_string())?;}}
 Ok(())
}
#[tauri::command]
pub fn note_flush_ack(window:tauri::WebviewWindow,token:u64,error:Option<String>)->Result<(),String>{
 let state=window.state::<FlushState>();let mut pending=state.pending.lock().map_err(|e|e.to_string())?;
 let row=pending.get_mut(&token).and_then(|p|p.get_mut(window.label())).ok_or("no pending note flush")?;
 if row.is_some(){return Err("duplicate note acknowledgement".into())}
 *row=Some(error.map_or(Ok(()),Err));Ok(())
}
#[tauri::command]
pub async fn notes_flush(window:tauri::WebviewWindow)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 let app=window.app_handle().clone();
 tauri::async_runtime::spawn_blocking(move||{
  let windows:Vec<_>=app.webview_windows().into_iter().filter(|(label,_)|label.starts_with("note-")).collect();
  let state=app.state::<FlushState>();
  // A newly created/rehydrated WebView may not have registered flush listeners.
  // The ready acknowledgement follows initial load and listener registration.
  let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
  loop{
   if {let ready=state.ready.lock().map_err(|e|e.to_string())?;windows.iter().all(|(id,_)|ready.contains(id))}{break}
   if std::time::Instant::now()>=deadline{return Err("便签尚未就绪，已保留窗口，请重试".into())}
   std::thread::sleep(std::time::Duration::from_millis(20));
  }
  let token=state.next.fetch_add(1,Ordering::Relaxed);
  state.pending.lock().map_err(|e|e.to_string())?.insert(token,windows.iter().map(|(id,_)|(id.clone(),None)).collect());
  let result=(||{
   for (_,child) in windows{child.emit_to(child.label(),"note-flush-request",token).map_err(|e|e.to_string())?;}
   loop{
    {let guard=state.pending.lock().map_err(|e|e.to_string())?;let rows=guard.get(&token).ok_or("flush disappeared")?;
     for result in rows.values(){if let Some(Err(error))=result{return Err(error.clone())}}
     if rows.values().all(|r|r.is_some()){return Ok(())}
    }
    if std::time::Instant::now()>=deadline{return Err("便签保存未确认，已保留窗口，请重试".into())}
    std::thread::sleep(std::time::Duration::from_millis(20));
   }
  })();
  state.pending.lock().map_err(|e|e.to_string())?.remove(&token);result
 }).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub async fn note_open(window:tauri::WebviewWindow,id:String)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 let value=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(value).map_err(|e|e.to_string())?;
 if note.deleted{return Err("note_deleted".into())}
 let app=window.app_handle();
 if let Some(existing)=app.get_webview_window(&id){rehome_window(&existing)?;existing.show().map_err(|e|e.to_string())?;return existing.set_focus().map_err(|e|e.to_string())}
 let monitors=window.available_monitors().map_err(|e|e.to_string())?;
 let monitor=monitors.iter().find(|m|m.name().is_some_and(|name|name==&note.monitor)).cloned()
  .or(window.primary_monitor().map_err(|e|e.to_string())?).ok_or("No available work area")?;
 let area=monitor.work_area();let scale=monitor.scale_factor();
 let bounds=crate::geometry::clamp_note(crate::geometry::Rect{x:area.position.x as f64/scale,y:area.position.y as f64/scale,width:area.size.width as f64/scale,height:area.size.height as f64/scale},crate::geometry::Rect{x:note.x,y:note.y,width:note.width,height:note.height});
 let trace=if crate::board::diagnostic_mode()=="generation-recovery"&&crate::runtime_paths::isolated().is_some(){"&generationTrace=1"}else{""};
 let builder=tauri::WebviewWindowBuilder::new(app,&id,tauri::WebviewUrl::App(format!("note.html?id={id}{trace}").into()))
  .on_page_load(|window,payload|{if matches!(payload.event(),tauri::webview::PageLoadEvent::Started){if let Ok(mut ready)=window.state::<FlushState>().ready.lock(){ready.remove(window.label());}}})
  .title("便签").decorations(false).transparent(true).background_color(tauri::window::Color(0,0,0,0)).shadow(false).inner_size(bounds.width,bounds.height).min_inner_size(160.,120.)
  .position(bounds.x,bounds.y).always_on_top(note.always_on_top).skip_taskbar(true).visible(false);
 let builder=if let Some(root)=crate::runtime_paths::isolated(){builder.data_directory(root.join("webview"))}else{builder};
 let child=builder.build().map_err(|e|e.to_string())?;
 shape_note(&child)?;
 let handle=child.clone();
 child.on_window_event(move|event|{match event{
  tauri::WindowEvent::CloseRequested{api,..}=>{api.prevent_close();let _=handle.emit_to(handle.label(),"note-hide-request",());},
  tauri::WindowEvent::Resized(_)|tauri::WindowEvent::ScaleFactorChanged{..}=>{
   if let Err(error)=shape_note(&handle){let _=handle.app_handle().emit_to("main","note-save-error",serde_json::json!({"id":handle.label(),"error":error}));}
   let _=handle.emit_to(handle.label(),"note-geometry-changed",());
  },tauri::WindowEvent::Moved(_)=>{let _=handle.emit_to(handle.label(),"note-geometry-changed",());},_=>{}
 }});
 Ok(())
}
#[tauri::command]
pub async fn note_save_geometry(window:tauri::WebviewWindow)->Result<(),String>{
 let id=window.label().to_string();if !id.starts_with("note-"){return Err("note only".into())}
 for attempt in 0..3{
 let value=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(value).map_err(|e|e.to_string())?;
 if note.deleted{return Ok(())}
 let scale=window.scale_factor().map_err(|e|e.to_string())?;
 let position=window.outer_position().map_err(|e|e.to_string())?;let size=window.inner_size().map_err(|e|e.to_string())?;
 let (x,y,width,height)=(position.x as f64/scale,position.y as f64/scale,size.width as f64/scale,size.height as f64/scale);
 let monitor=window.current_monitor().map_err(|e|e.to_string())?.and_then(|m|m.name().cloned()).unwrap_or_default();
 if (note.x,note.y,note.width,note.height,&note.monitor)==(x,y,width,height,&monitor){return Ok(())}
 match crate::resource_commands::note_patch(window.clone(),id.clone(),quickgen_desktop::notes::NotePatch::Geometry{revision:note.geometry_revision,x,y,width,height,monitor}).await{
  Ok(_)=>return Ok(()),Err(error) if error=="note_revision_conflict"&&attempt<2=>continue,Err(error)=>return Err(error)
 }
 }
 Err("note geometry changed repeatedly; retry required".into())
}
#[tauri::command]
pub fn note_ready(window:tauri::WebviewWindow)->Result<(),String>{
 if !window.label().starts_with("note-"){return Err("note only".into())}
 shape_note(&window)?;window.state::<FlushState>().ready.lock().map_err(|e|e.to_string())?.insert(window.label().to_string());if window.state::<FlushState>().overview.load(Ordering::SeqCst){return window.hide().map_err(|e|e.to_string())}window.show().map_err(|e|e.to_string())?;window.set_focus().map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn note_hide(window:tauri::WebviewWindow)->Result<(),String>{
 let id=window.label().to_string();if !id.starts_with("note-"){return Err("note only".into())}
 let value=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(value).map_err(|e|e.to_string())?;
 crate::resource_commands::note_patch(window.clone(),id,quickgen_desktop::notes::NotePatch::Flags{revision:note.flags_revision,resident:false,always_on_top:note.always_on_top}).await?;
 window.hide().map_err(|e|e.to_string())
}

/// Changing window level never changes residency. Failed persistence restores the old level.
#[tauri::command]
pub async fn note_set_top(window:tauri::WebviewWindow,enabled:bool)->Result<serde_json::Value,String>{
 let id=window.label().to_string();if !id.starts_with("note-"){return Err("note only".into())}
 let value=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(value).map_err(|e|e.to_string())?;
 if note.deleted{return Err("note_deleted".into())}
 window.set_always_on_top(enabled).map_err(|e|e.to_string())?;
 match crate::resource_commands::note_patch(window.clone(),id,quickgen_desktop::notes::NotePatch::Flags{revision:note.flags_revision,resident:note.resident,always_on_top:enabled}).await{
  Ok(value)=>Ok(value),
  Err(error)=>{window.set_always_on_top(note.always_on_top).map_err(|rollback|format!("{error}; window rollback failed: {rollback}"))?;Err(error)}
 }
}

#[tauri::command]
pub async fn note_delete(window:tauri::WebviewWindow)->Result<serde_json::Value,String>{
 let id=window.label().to_string();if !id.starts_with("note-"){return Err("note only".into())}
 let current=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(current).map_err(|e|e.to_string())?;
 window.hide().map_err(|e|e.to_string())?;
 match crate::resource_commands::note_patch(window.clone(),id,quickgen_desktop::notes::NotePatch::Deleted{revision:note.revision,deleted:true}).await{
  Ok(value)=>Ok(value),
  Err(error)=>{window.show().map_err(|e|format!("{error}; restore window failed: {e}"))?;let _=window.set_focus();Err(error)}
 }
}
#[tauri::command]
pub async fn note_restore(window:tauri::WebviewWindow,id:String)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 let current=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(current).map_err(|e|e.to_string())?;
 if note.deleted{
  // Only discard a deleted editor; repeated restore must never discard a live draft.
  if let Some(child)=window.app_handle().get_webview_window(&id){child.destroy().map_err(|e|e.to_string())?;}
  crate::resource_commands::note_patch(window.clone(),id.clone(),quickgen_desktop::notes::NotePatch::Deleted{revision:note.revision,deleted:false}).await?;
 }else if !note.resident{
  crate::resource_commands::note_patch(window.clone(),id.clone(),quickgen_desktop::notes::NotePatch::Flags{revision:note.flags_revision,resident:true,always_on_top:note.always_on_top}).await?;
 }
 note_open(window,id).await?;Ok(())
}

#[tauri::command]
pub fn note_report_error(window:tauri::WebviewWindow,error:Option<String>)->Result<(),String>{
 if !window.label().starts_with("note-"){return Err("note only".into())}
 window.app_handle().emit_to("main","note-save-error",serde_json::json!({"id":window.label(),"error":error})).map_err(|e|e.to_string())
}

// Complete on the native thread, rather than acknowledge queued show/hide calls.
async fn apply_note_visibility(window:tauri::WebviewWindow,visible:Vec<String>)->Result<(),String>{
 let app=window.app_handle().clone();let main=window.clone();let (send,receive)=std::sync::mpsc::sync_channel(1);
 window.run_on_main_thread(move||{
  let result=(||{for (id,child) in app.webview_windows(){if id.starts_with("note-"){
   if visible.contains(&id){child.show().map_err(|e|e.to_string())?;}else{child.hide().map_err(|e|e.to_string())?;}
  }}main.set_focus().map_err(|e|e.to_string())})();let _=send.send(result);
 }).map_err(|e|e.to_string())?;
 tauri::async_runtime::spawn_blocking(move||receive.recv_timeout(std::time::Duration::from_secs(5)).map_err(|e|format!("便签窗口状态未确认：{e}"))?).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn notes_present(window:tauri::WebviewWindow,overview:bool)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 window.state::<FlushState>().overview.store(overview,Ordering::SeqCst);
 let result=async{
  let mut visible=Vec::new();
  if !overview{
   let records=crate::resource_commands::notes_list(window.clone()).await?;
   let notes:Vec<quickgen_desktop::notes::Note>=serde_json::from_value(records).map_err(|e|e.to_string())?;
   for note in notes{if note.resident&&!note.deleted{note_open(window.clone(),note.id.clone()).await?;visible.push(note.id);}}
  }
  apply_note_visibility(window.clone(),visible).await
 }.await;
 if let Err(error)=result{
  window.state::<FlushState>().overview.store(true,Ordering::SeqCst);
  if let Err(rollback)=apply_note_visibility(window,Vec::new()).await{return Err(format!("{error}; rollback: {rollback}"))}return Err(error)
 }
 Ok(())
}
#[tauri::command]
pub async fn note_set_resident(window:tauri::WebviewWindow,id:String,resident:bool)->Result<serde_json::Value,String>{
 if window.label()!="main"{return Err("main only".into())}
 let value=crate::resource_commands::note_get(window.clone(),id.clone()).await?;
 let note:quickgen_desktop::notes::Note=serde_json::from_value(value).map_err(|e|e.to_string())?;
 crate::resource_commands::note_patch(window,id,quickgen_desktop::notes::NotePatch::Flags{revision:note.flags_revision,resident,always_on_top:note.always_on_top}).await
}

#[tauri::command]
pub fn note_drag(window:tauri::WebviewWindow)->Result<(),String>{
 if !window.label().starts_with("note-"){return Err("note only".into())}
 window.start_dragging().map_err(|e|e.to_string())
}
#[tauri::command]
pub fn note_resize(window:tauri::WebviewWindow)->Result<(),String>{
 if !window.label().starts_with("note-"){return Err("note only".into())}
 let native=window.as_ref().window();
 native.start_resize_dragging(serde_json::from_value(serde_json::json!("SouthEast")).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}

fn rehome_window(window:&tauri::WebviewWindow)->Result<(),String>{
 let monitor=window.current_monitor().map_err(|e|e.to_string())?.or(window.primary_monitor().map_err(|e|e.to_string())?).ok_or("No available work area")?;
 let area=monitor.work_area();let scale=monitor.scale_factor();
 let position=window.outer_position().map_err(|e|e.to_string())?;let size=window.inner_size().map_err(|e|e.to_string())?;
 let current=crate::geometry::Rect{x:position.x as f64/scale,y:position.y as f64/scale,width:size.width as f64/scale,height:size.height as f64/scale};
 let bounds=crate::geometry::clamp_note(crate::geometry::Rect{x:area.position.x as f64/scale,y:area.position.y as f64/scale,width:area.size.width as f64/scale,height:area.size.height as f64/scale},current);
 if bounds==current{return Ok(())}
 window.set_position(tauri::PhysicalPosition::new((bounds.x*scale).round() as i32,(bounds.y*scale).round() as i32)).map_err(|e|e.to_string())?;
 window.set_size(tauri::PhysicalSize::new((bounds.width*scale).round() as u32,(bounds.height*scale).round() as u32)).map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn notes_rehome(window:tauri::WebviewWindow)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 let records=crate::resource_commands::notes_list(window.clone()).await?;
 let notes:Vec<quickgen_desktop::notes::Note>=serde_json::from_value(records).map_err(|e|e.to_string())?;
 for note in notes.into_iter().filter(|n|!n.deleted){if let Some(child)=window.app_handle().get_webview_window(&note.id){rehome_window(&child)?;note_save_geometry(child).await?;}}
 Ok(())
}

#[cfg(windows)]
fn note_region(width:u32,height:u32,scale:f64)->Result<windows::Win32::Graphics::Gdi::HRGN,String>{
 use windows::Win32::Graphics::Gdi::CreateRoundRectRgn;
 let diameter=(12.*scale).round().max(2.) as i32;
 let region=unsafe{CreateRoundRectRgn(0,0,width as i32+1,height as i32+1,diameter,diameter)};
 if region.0.is_null(){Err("Cannot create note corner region".into())}else{Ok(region)}
}
fn shape_note(window:&tauri::WebviewWindow)->Result<(),String>{
 #[cfg(windows)]{
  use windows::Win32::Graphics::Gdi::{SetWindowRgn,DeleteObject,HGDIOBJ};
  let size=window.outer_size().map_err(|e|e.to_string())?;if size.width==0||size.height==0{return Ok(())}
  let hwnd=window.hwnd().map_err(|e|e.to_string())?;
  let region=note_region(size.width,size.height,window.scale_factor().map_err(|e|e.to_string())?)?;
  // On success Windows owns the region. On failure the caller must free it.
  if unsafe{SetWindowRgn(hwnd,Some(region),true)}==0{unsafe{let _=DeleteObject(HGDIOBJ(region.0));}return Err("Cannot apply note corner region".into())}
 }
 Ok(())
}
#[cfg(all(test,windows))]mod corner_tests{
 #[test]fn physical_region_excludes_square_corners_at_each_scale(){
  use windows::Win32::Graphics::Gdi::{PtInRegion,DeleteObject,HGDIOBJ};
  for scale in [1.,1.5,2.]{let w=(280.*scale) as u32;let h=(240.*scale) as u32;let r=super::note_region(w,h,scale).unwrap();unsafe{
   assert!(!PtInRegion(r,0,0).as_bool());assert!(!PtInRegion(r,w as i32-1,h as i32-1).as_bool());assert!(PtInRegion(r,w as i32/2,h as i32/2).as_bool());assert!(DeleteObject(HGDIOBJ(r.0)).as_bool());
  }}
 }
}

fn capacity_only()->Result<(),String>{
 if !matches!(crate::board::diagnostic_mode(),"notes-capacity"|"notes-fault"|"notes-restart")||crate::runtime_paths::isolated().is_none(){return Err("isolated notes capacity diagnostic only".into())}Ok(())
}
#[tauri::command]
pub fn notes_capacity_edit(window:tauri::WebviewWindow,id:String,text:String,token:u64)->Result<(),String>{
 capacity_only()?;if window.label()!="main"||!id.starts_with("note-"){return Err("main note target only".into())}
 window.app_handle().get_webview_window(&id).ok_or("capacity note window missing")?.emit_to(id.as_str(),"note-capacity-edit",serde_json::json!({"text":text,"token":token})).map_err(|e|e.to_string())
}
#[tauri::command]
pub fn note_capacity_ack(window:tauri::WebviewWindow,token:u64,error:Option<String>,draft:String,status:String)->Result<(),String>{
 capacity_only()?;if !window.label().starts_with("note-"){return Err("note only".into())}
 window.app_handle().emit_to("main","note-capacity-ack",serde_json::json!({"id":window.label(),"token":token,"error":error,"draft":draft,"status":status})).map_err(|e|e.to_string())
}
#[tauri::command]
pub fn notes_capacity_windows(window:tauri::WebviewWindow)->Result<serde_json::Value,String>{
 capacity_only()?;if window.label()!="main"{return Err("main only".into())}
 let windows:Vec<_>=window.app_handle().webview_windows().into_iter().filter(|(id,_)|id.starts_with("note-")).collect();
 let mut visible=0;for (_,child) in &windows{if child.is_visible().map_err(|e|e.to_string())?{visible+=1;}}
 Ok(serde_json::json!({"total":windows.len(),"visible":visible}))
}

#[cfg(all(test,not(feature="desktop-diagnostics")))]mod capacity_permissions{
 #[test]fn release_rejects_capacity_hooks(){assert!(super::capacity_only().is_err());}
}

#[tauri::command]
pub fn notes_test_reload(window:tauri::WebviewWindow,id:String,token:u64)->Result<(),String>{
 if window.label()!="main"||!matches!(crate::board::diagnostic_mode(),"notes-fault"|"notes-restart")||crate::runtime_paths::isolated().is_none(){return Err("isolated note reload diagnostic only".into())}
 window.app_handle().get_webview_window(&id).ok_or("note window missing")?.emit_to(id.as_str(),"note-test-reload",token).map_err(|e|e.to_string())
}
