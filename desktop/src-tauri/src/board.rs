use crate::storage;
use crate::overlay_support;
use std::sync::{Mutex,Arc,atomic::{AtomicBool,Ordering}};
use tauri::Manager;
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use windows::Win32::{Foundation::POINT,Graphics::Gdi::{CreateRectRgn,CreateRoundRectRgn,CombineRgn,SetWindowRgn,DeleteObject,HGDIOBJ,RGN_OR},UI::WindowsAndMessaging::{GetCursorPos,PeekMessageW,MSG,PM_REMOVE,WM_HOTKEY,MessageBoxW,MB_OK,MB_ICONERROR}};
#[derive(Clone,Copy,Deserialize,Serialize,Default,PartialEq)]
struct Rect{x:f64,y:f64,width:f64,height:f64}
impl Rect{fn contains(&self,x:f64,y:f64)->bool{x>=self.x&&y>=self.y&&x<=self.x+self.width&&y<=self.y+self.height}}
#[derive(Clone,Deserialize,Serialize,PartialEq)]
struct HoverTarget{id:String,rect:Rect}
#[derive(Clone,Deserialize,Serialize,Default,PartialEq)]
#[serde(rename_all="camelCase")]
struct View{open:bool,passthrough:bool,pinned:bool,#[serde(default)]interacting:bool,rects:Vec<Rect>,#[serde(default)]ink_controls:Vec<Rect>,#[serde(default)]visuals:Vec<Rect>,#[serde(default)]hover_targets:Vec<HoverTarget>,panel:Rect}
#[derive(Default)]
struct State{view:Mutex<View>,bridge:Mutex<Option<tauri::ipc::Channel<Value>>>,display:Mutex<String>,manager_height:Mutex<f64>,cursor_space:Mutex<(i32,i32,f64)>,quit_requested:AtomicBool,recovery_request:Mutex<Option<(String,std::sync::mpsc::Sender<Result<Value,String>>)>>,recovery_result:Mutex<Option<Value>>}
fn send(app:&tauri::AppHandle,value:Value){if let Ok(bridge)=app.state::<State>().bridge.lock(){if let Some(channel)=bridge.as_ref(){let _=channel.send(value);}};}
pub(crate) fn annotation_exclusions(app:&tauri::AppHandle)->Result<Vec<crate::geometry::Rect>,String>{let Some(window)=app.get_webview_window("main")else{return Ok(vec![])};if !window.is_visible().map_err(|e|e.to_string())?{return Ok(vec![])}let position=window.outer_position().map_err(|e|e.to_string())?;let scale=window.scale_factor().map_err(|e|e.to_string())?;Ok(app.state::<State>().view.lock().map_err(|e|e.to_string())?.ink_controls.iter().map(|r|crate::geometry::Rect{x:position.x as f64+r.x*scale,y:position.y as f64+r.y*scale,width:r.width*scale,height:r.height*scale}).collect())}
#[tauri::command]
fn board_subscribe(app:tauri::AppHandle,channel:tauri::ipc::Channel<Value>)->Result<(),String>{*app.state::<State>().bridge.lock().map_err(|e|e.to_string())?=Some(channel);Ok(())}
#[tauri::command]
fn board_view(window:tauri::WebviewWindow,view:View)->Result<(),String>{
 if window.label()!="main"{return Err("Main only".into())}
 if view.ink_controls.len()>4||view.rects.len()>16||view.visuals.len()>16384||view.hover_targets.len()>16384||view.hover_targets.iter().any(|t|t.id.len()>256)||view.rects.iter().chain(view.ink_controls.iter()).chain(view.visuals.iter()).chain(view.hover_targets.iter().map(|t|&t.rect)).chain(std::iter::once(&view.panel)).any(|r|![r.x,r.y,r.width,r.height].iter().all(|n|n.is_finite())||r.width<0.||r.height<0.){return Err("Invalid view bounds".into())}
 let previous=window.state::<State>().view.lock().map_err(|e|e.to_string())?.clone();
 if previous==view{return Ok(())}
 let hwnd=window.hwnd().map_err(|e|e.to_string())?;let scale=window.scale_factor().map_err(|e|e.to_string())?;
 if previous.full_region()!=view.full_region()||(!view.full_region()&&(previous.rects!=view.rects||previous.visuals!=view.visuals)){unsafe{if view.full_region(){if SetWindowRgn(hwnd,None,true)==0{return Err("Cannot expand native region".into())}}
 else{let region=CreateRectRgn(0,0,0,0);if region.0.is_null(){return Err("Cannot create region".into())}
 for r in view.rects.iter().chain(view.visuals.iter()){let part=CreateRectRgn((r.x*scale).floor() as i32,(r.y*scale).floor() as i32,((r.x+r.width)*scale).ceil() as i32,((r.y+r.height)*scale).ceil() as i32);if part.0.is_null(){let _=DeleteObject(HGDIOBJ(region.0));return Err("Cannot create region".into())}CombineRgn(Some(region),Some(region),Some(part),RGN_OR);let _=DeleteObject(HGDIOBJ(part.0));}
 if SetWindowRgn(hwnd,Some(region),true)==0{let _=DeleteObject(HGDIOBJ(region.0));return Err("Cannot apply region".into())}}
 }
 }
 let was_passthrough=window.state::<State>().view.lock().map_err(|e|e.to_string())?.passthrough;
 if was_passthrough&&!view.passthrough&&!crate::resource_commands::drag_active(){let _=window.set_focus();}
 if !diagnostic_mode().is_empty(){if let Ok(path)=std::env::var("QUICKGEN_DIAGNOSTIC_STATUS"){let _=std::fs::write(path,json!({"mode":diagnostic_mode(),"open":view.open,"passthrough":view.passthrough,"visualCount":view.visuals.len()}).to_string());}}
 let panel_visible=view.panel.width>0.&&view.panel.height>0.;
 *window.state::<State>().view.lock().map_err(|e|e.to_string())?=view;
 if !crate::resource_commands::drag_active()&&!window.is_visible().map_err(|e|e.to_string())?{window.show().map_err(|e|e.to_string())?;}
 if panel_visible{if previous.panel!=window.state::<State>().view.lock().map_err(|e|e.to_string())?.panel{position_manager(window.app_handle())?}}else if let Some(child)=window.app_handle().get_webview_window("manager"){child.hide().map_err(|e|e.to_string())?}crate::ink_windows::refresh_regions(window.app_handle())?;Ok(())
}
#[tauri::command]
fn board_diagnostic()->String{diagnostic_mode().to_string()}
#[tauri::command]
fn board_smoke_finish(app:tauri::AppHandle,result:Value)->Result<(),String>{
 if !matches!(diagnostic_mode(),"generation-recovery"|"reliable-base"|"reliable-capacity"|"reliable-rehearsal"|"reliable-review"|"notes-capacity"|"notes-fault"|"notes-restart"|"notes-rich-smoke"|"notes-rich-capacity"){return Err("diagnostic only".into())}
 let root=crate::runtime_paths::isolated().ok_or("isolation required")?;
 std::fs::write(root.join("board-smoke-result.json"),serde_json::to_vec_pretty(&result).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if result["ok"]==true&&result["flushExit"]==true{request_quit(&app)}else{app.exit(if result["ok"]==true{0}else{1})}Ok(())
}
#[tauri::command]
fn board_capacity_progress(result:Value)->Result<(),String>{
 if !matches!(diagnostic_mode(),"generation-recovery"|"reliable-capacity"|"notes-capacity"|"notes-fault"|"notes-restart"|"notes-rich-smoke"|"notes-rich-capacity"){return Err("diagnostic only".into())}
 let root=crate::runtime_paths::isolated().ok_or("isolation required")?;
 if diagnostic_mode()=="generation-recovery"{use std::io::Write;let mut f=std::fs::OpenOptions::new().append(true).create(true).open(root.join("generation-progress.jsonl")).map_err(|e|e.to_string())?;writeln!(f,"{}",result).map_err(|e|e.to_string())?;}
 std::fs::write(root.join("capacity-progress.json"),serde_json::to_vec_pretty(&result).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
#[tauri::command]
fn board_probe(elapsed:f64){if elapsed.is_finite()&&elapsed>=0.{diagnostic_record(json!({"ipcRoundtripMs":elapsed}));}}
fn diagnostic_record(value:Value){if diagnostic_mode().is_empty(){return}if let Ok(path)=std::env::var("QUICKGEN_DIAGNOSTIC_STATUS"){use std::io::Write;if let Ok(mut f)=std::fs::OpenOptions::new().append(true).create(true).open(std::path::Path::new(&path).with_extension("timings.jsonl")){let _=writeln!(f,"{}",value);}}}
#[tauri::command]
fn board_quit(app:tauri::AppHandle){request_quit(&app)}
fn request_quit(app:&tauri::AppHandle){
 // A page that never subscribed cannot acknowledge a flush (e.g. a WebView error page).
 // Once subscribed, retain the full save barrier; never time out and discard edits.
 let subscribed=app.state::<State>().bridge.lock().map(|b|b.is_some()).unwrap_or(true);
 if !subscribed{app.exit(0);return}
 app.state::<State>().quit_requested.store(true,Ordering::Relaxed);send(app,json!({"type":"flush-and-quit"}));
}
#[tauri::command]
async fn board_quit_saved(window:tauri::WebviewWindow)->Result<(),String>{
 if window.label()!="main"||!window.state::<State>().quit_requested.load(Ordering::Relaxed){return Err("No pending main-board exit".into())}
 crate::resource_commands::image_shutdown(window.clone()).await?;
 window.state::<State>().quit_requested.store(false,Ordering::Relaxed);
 window.app_handle().exit(0);Ok(())
}
#[tauri::command]
fn board_drag_exports_open(window:tauri::WebviewWindow)->Result<(),String>{
 if !matches!(window.label(),"main"|"manager"){return Err("main or manager only".into())}
 let base=crate::runtime_paths::data(window.app_handle())?.join("resource-store");
 let _base_guard=quickgen_desktop::resources::guard_export_directory(&base)?;
 let path=base.join("drag-exports");let _guard=quickgen_desktop::resources::guard_export_directory(&path)?;
 std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command]
fn board_recovery_open(window:tauri::WebviewWindow,directory:String)->Result<(),String>{
 if !matches!(window.label(),"main"|"manager"){return Err("main or manager only".into())}
 let base=crate::runtime_paths::data(window.app_handle())?.join("resource-store").join("recovery-exports").canonicalize().map_err(|e|e.to_string())?;
 let path=std::path::PathBuf::from(directory).canonicalize().map_err(|e|e.to_string())?;
 if path.parent()!=Some(base.as_path())||!path.join("manifest.json").is_file(){return Err("not a completed recovery export".into())}
 std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command]
fn board_group_focused(app:tauri::AppHandle)->bool{app.webview_windows().iter().any(|(label,w)|(label=="main"||label=="manager"||label.starts_with("note-")||label.starts_with("ink-"))&&w.is_focused().unwrap_or(false))}
#[tauri::command]
async fn manager_recovery_request(window:tauri::WebviewWindow,request_id:String)->Result<Value,String>{
 if window.label()!="manager"||request_id.is_empty()||request_id.len()>128{return Err("invalid recovery request".into())}
 let (tx,rx)=std::sync::mpsc::channel();
 {
  let state=window.state::<State>();let mut pending=state.recovery_request.lock().map_err(|e|e.to_string())?;
  if pending.is_some(){return Err("恢复副本正在生成，请稍后再查看".into())}
  let bridge=state.bridge.lock().map_err(|e|e.to_string())?;let channel=bridge.as_ref().ok_or("主画板尚未就绪")?;
  *pending=Some((request_id.clone(),tx));
  if let Err(error)=channel.send(json!({"type":"recovery-export","requestId":request_id})){*pending=None;return Err(error.to_string())}
 }
 tauri::async_runtime::spawn_blocking(move||rx.recv().map_err(|_|"恢复结果通道已关闭".to_string())?).await.map_err(|e|e.to_string())?
}
#[tauri::command]
fn board_recovery_complete(window:tauri::WebviewWindow,request_id:String,result:Option<Value>,error:Option<String>)->Result<(),String>{
 if window.label()!="main"{return Err("main only".into())}
 let state=window.state::<State>();let mut pending=state.recovery_request.lock().map_err(|e|e.to_string())?;
 if pending.as_ref().map(|p|p.0.as_str())!=Some(request_id.as_str()){return Err("obsolete recovery response".into())}
 *state.recovery_result.lock().map_err(|e|e.to_string())?=Some(json!({"requestId":request_id,"result":result,"error":error}));
 complete_recovery(&mut pending,&request_id,result,error)
}
fn complete_recovery(pending:&mut Option<(String,std::sync::mpsc::Sender<Result<Value,String>>)>,request_id:&str,result:Option<Value>,error:Option<String>)->Result<(),String>{
 if pending.as_ref().map(|p|p.0.as_str())!=Some(request_id){return Err("obsolete recovery response".into())}
 if let Some((_,sender))=pending.take(){let response=match(error,result){(Some(error),_)=>Err(error),(None,Some(result))=>Ok(result),_=>Err("恢复未返回结果".into())};let _=sender.send(response);}Ok(())
}
#[tauri::command]
fn manager_recovery_status(window:tauri::WebviewWindow)->Result<Value,String>{
 if window.label()!="manager"{return Err("manager only".into())}
 let state=window.state::<State>();let pending=state.recovery_request.lock().map_err(|e|e.to_string())?;
 Ok(json!({"pending":pending.is_some(),"last":state.recovery_result.lock().map_err(|e|e.to_string())?.clone()}))
}
#[tauri::command]
fn manager_page_size(window:tauri::WebviewWindow,compact:bool)->Result<(),String>{
 if window.label()!="manager"{return Err("manager only".into())}
 *window.state::<State>().manager_height.lock().map_err(|e|e.to_string())?=if compact{344.}else{440.};position_manager(window.app_handle())
}
fn position_manager(app:&tauri::AppHandle)->Result<(),String>{
 let Some(child)=app.get_webview_window("manager")else{return Ok(())};let main=app.get_webview_window("main").ok_or("No main")?;let scale=main.scale_factor().map_err(|e|e.to_string())?;let origin=main.inner_position().map_err(|e|e.to_string())?;let size=main.inner_size().map_err(|e|e.to_string())?;let panel=app.state::<State>().view.lock().map_err(|e|e.to_string())?.panel;
 if panel.width<=0.||panel.height<=0.{return Ok(())}
 let desired=*app.state::<State>().manager_height.lock().map_err(|e|e.to_string())?;
 let placed=crate::geometry::adjacent(crate::geometry::Rect{x:0.,y:0.,width:size.width as f64/scale,height:size.height as f64/scale},crate::geometry::Rect{x:panel.x,y:panel.y,width:panel.width,height:panel.height},if desired>=440.{360.}else{308.},if desired>0.{desired}else{344.});
 child.set_size(tauri::LogicalSize::new(placed.width,placed.height)).map_err(|e|e.to_string())?;child.set_position(tauri::PhysicalPosition::new(origin.x+(placed.x*scale) as i32,origin.y+(placed.y*scale) as i32)).map_err(|e|e.to_string())?;
 unsafe{let region=CreateRoundRectRgn(0,0,(placed.width*scale).round() as i32+1,(placed.height*scale).round() as i32+1,(12.*scale).round() as i32,(12.*scale).round() as i32);if region.0.is_null(){return Err("Cannot create utility region".into())}if SetWindowRgn(child.hwnd().map_err(|e|e.to_string())?,Some(region),true)==0{let _=DeleteObject(HGDIOBJ(region.0));return Err("Cannot shape utility panel".into())}}Ok(())
}
#[tauri::command]
fn close_manager(app:tauri::AppHandle)->Result<(),String>{if let Some(w)=app.get_webview_window("manager"){w.hide().map_err(|e|e.to_string())?}if let Some(w)=app.get_webview_window("main"){let _=w.set_focus();}Ok(())}
#[tauri::command]
fn show_manager(app:tauri::AppHandle)->Result<(),String>{position_manager(&app)?;let w=app.get_webview_window("manager").ok_or("No manager")?;w.show().map_err(|e|e.to_string())?;w.set_focus().map_err(|e|e.to_string())}
#[tauri::command]
async fn open_manager(app:tauri::AppHandle,mode:String,preset_id:Option<String>)->Result<(),String>{
 if !["add","library","gateway","settings","binding"].contains(&mode.as_str()){return Err("Invalid mode".into())}
 let suffix=if mode=="binding"{let id=preset_id.as_deref().filter(|id|!id.is_empty()&&id.len()<=256).ok_or("preset ID required")?;format!("&presetId={}",url::form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>())}else{String::new()};
 *app.state::<State>().manager_height.lock().map_err(|e|e.to_string())?=if mode=="add"{312.}else if mode=="library"{344.}else{440.};
 if let Some(w)=app.get_webview_window("manager"){w.hide().map_err(|e|e.to_string())?;let url=w.url().map_err(|e|e.to_string())?.join(&format!("manager.html?mode={mode}{suffix}")).map_err(|e|e.to_string())?;w.navigate(url).map_err(|e|e.to_string())?;}
 else{let builder=tauri::WebviewWindowBuilder::new(&app,"manager",tauri::WebviewUrl::App(format!("manager.html?mode={mode}{suffix}").into())).title("QuickGen · 能力管理");let builder=if let Some(root)=crate::runtime_paths::isolated(){builder.data_directory(root.join("webview"))}else{builder};builder.inner_size(308.,344.).decorations(false).transparent(true).shadow(false).resizable(false).always_on_top(true).skip_taskbar(true).visible(false).build().map_err(|e|e.to_string())?;}
 position_manager(&app)
}
fn display(app:&tauri::AppHandle)->Result<(),String>{let monitor=app.primary_monitor().map_err(|e|e.to_string())?.or_else(||app.available_monitors().ok()?.into_iter().next()).ok_or("No monitor")?;let area=monitor.work_area();let key=format!("{:?}|{}",area,monitor.scale_factor());let state=app.state::<State>();let mut old=state.display.lock().map_err(|e|e.to_string())?;if *old!=key{let w=app.get_webview_window("main").ok_or("No main")?;w.set_position(area.position).map_err(|e|e.to_string())?;w.set_size(area.size).map_err(|e|e.to_string())?;*state.cursor_space.lock().map_err(|e|e.to_string())?=(area.position.x,area.position.y,monitor.scale_factor());state.view.lock().map_err(|e|e.to_string())?.rects.clear();position_manager(app)?;send(app,json!({"type":"display"}));*old=key;}Ok(())}
fn show_from_tray(app:&tauri::AppHandle){
 // Only board_view may reveal the initial window, after applying its native region.
 // Revealing it earlier exposes an opaque, work-area-sized browser error page.
 if let Some(w)=app.get_webview_window("main"){if w.is_visible().unwrap_or(false){let _=w.set_focus();}}
 send(app,json!({"type":"show-panel"}));
}
fn setup_tray(app:&tauri::App)->tauri::Result<()>{
 use tauri::{menu::{Menu,MenuItem},tray::{TrayIconBuilder,TrayIconEvent,MouseButton,MouseButtonState}};
 let show=MenuItem::with_id(app,"show-panel","打开面板",true,None::<&str>)?;
 let quit=MenuItem::with_id(app,"quit","退出 QuickGen",true,None::<&str>)?;
 let recovery=MenuItem::with_id(app,"recovery-export","保存恢复副本",true,None::<&str>)?;
 let menu=Menu::with_items(app,&[&show,&recovery,&quit])?;
 let mut builder=TrayIconBuilder::with_id("quickgen").tooltip("QuickGen").menu(&menu).show_menu_on_left_click(false);
 if let Some(icon)=app.default_window_icon(){builder=builder.icon(icon.clone());}
 builder.on_menu_event(|app,event|match event.id.as_ref(){"show-panel"=>show_from_tray(app),"recovery-export"=>send(app,json!({"type":"recovery-export"})),"quit"=>request_quit(app),_=>{}})
 .on_tray_icon_event(|tray,event|{if matches!(event,TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}){show_from_tray(tray.app_handle());}}).build(app)?;
 Ok(())
}
pub fn run(){let context=tauri::generate_context!();let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
 let built=tauri::Builder::default().manage(crate::ink_windows::InkHost::default()).manage(crate::note_windows::FlushState::default()).manage(State::default()).manage(crate::resource_commands::ResourceHost::default()).setup(move|app|{
 let builder=tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::App(if diagnostic_mode()=="static-webview"{"diagnostic-static.html"}else{"board.html"}.into())).title("QuickGen");let builder=if let Some(root)=crate::runtime_paths::isolated(){builder.data_directory(root.join("webview"))}else{builder};builder.transparent(true).decorations(false).shadow(false).resizable(false).always_on_top(true).skip_taskbar(true).visible(false).build()?;
 quickgen_desktop::overlay_window::install(app.get_webview_window("main").ok_or("No main")?.hwnd()?)?;
 setup_tray(app)?;
 display(app.handle())?;
 let handle=app.handle().clone();let(tx,rx)=std::sync::mpsc::sync_channel(1);
 std::thread::spawn(move||{let _key=if crate::runtime_paths::isolated().is_some(){let _=tx.send(Ok::<(),String>(()));None}else{Some(match overlay_support::ModeHotkey::register(1){Ok(k)=>{let _=tx.send(Ok::<(),String>(()));k},Err(e)=>{let _=tx.send(Err(e));return}})};let mut tick=0;let mut ignored=false;let mut last_hover=None;
 while !worker_stop.load(Ordering::Relaxed){let mut msg=MSG::default();while unsafe{PeekMessageW(&mut msg,None,0,0,PM_REMOVE)}.as_bool(){if msg.message==WM_HOTKEY&&!crate::resource_commands::drag_active(){send(&handle,json!({"type":"toggle"}));}}
 let view=handle.state::<State>().view.lock().map(|v|v.clone()).unwrap_or_default();
 if view.open&&view.passthrough||ignored {
  if let Some(w)=handle.get_webview_window("main"){let mut p=POINT::default();if unsafe{GetCursorPos(&mut p)}.is_ok(){
   let (ox,oy,scale)=*handle.state::<State>().cursor_space.lock().unwrap();
   if scale>0.{let x=(p.x-ox) as f64/scale;let y=(p.y-oy) as f64/scale;let ignore=if matches!(diagnostic_mode(),"fixed-pass"|"static-webview"){view.open&&view.passthrough}else{view.ignore_at(x,y)};
    if ignore!=ignored&&w.set_ignore_cursor_events(ignore).is_ok(){ignored=ignore;}
    if view.open&&view.passthrough&&!matches!(diagnostic_mode(),"no-hover"|"fixed-pass"|"static-webview"){let hits=view.hover_at(x,y);if hover_changed(&mut last_hover,&hits){send(&handle,json!({"type":"hover","ids":hits}));}}else{last_hover=None;}
   }
  }}
 }else{last_hover=None;}

 tick+=1;if tick%6==0&&tick<60{crate::ink_windows::queue_region_refresh(&handle);}if tick>=60{tick=0;let h=handle.clone();let queued=std::time::Instant::now();let _=handle.run_on_main_thread(move||{diagnostic_record(json!({"nativeQueueMs":queued.elapsed().as_secs_f64()*1000.}));if let Err(e)=display(&h){send(&h,json!({"type":"error","message":e}));}if let Err(e)=crate::ink_windows::refresh(&h){crate::ink_windows::emergency_stop(&h);send(&h,json!({"type":"error","message":e}));}});}
 std::thread::sleep(std::time::Duration::from_millis(16));}
 });rx.recv().map_err(|e|e.to_string())??;Ok(())
 }).on_window_event(|window,event|{if window.label().starts_with("ink-"){if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();crate::ink_windows::emergency_stop(window.app_handle());return;}if matches!(event,tauri::WindowEvent::Destroyed){crate::ink_windows::forget_window(window.app_handle(),window.label());}}if !window.label().starts_with("ink-")&&matches!(event,tauri::WindowEvent::Moved(_)|tauri::WindowEvent::Resized(_)|tauri::WindowEvent::ScaleFactorChanged{..}|tauri::WindowEvent::Focused(_)){if let Err(error)=crate::ink_windows::refresh_regions(window.app_handle()){crate::ink_windows::emergency_stop(window.app_handle());send(window.app_handle(),json!({"type":"error","message":error}));}}if matches!(event,tauri::WindowEvent::Focused(false)){send(window.app_handle(),json!({"type":"group-blur"}));}if window.label()!="main"{return}if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();request_quit(window.app_handle());return}if let tauri::WindowEvent::DragDrop(event)=event{let payload=match event{tauri::DragDropEvent::Enter{paths,position}=>json!({"type":"enter","paths":paths,"position":position}),tauri::DragDropEvent::Over{position}=>json!({"type":"over","position":position}),tauri::DragDropEvent::Drop{paths,position}=>json!({"type":"drop","paths":paths,"position":position}),tauri::DragDropEvent::Leave=>json!({"type":"leave"}),_=>return};send(window.app_handle(),payload);}})
 .invoke_handler(tauri::generate_handler![crate::ink_windows::ink_clear,crate::ink_windows::ink_subscribe,crate::ink_windows::ink_diagnostic,crate::ink_windows::ink_diagnostic_render,crate::ink_windows::ink_report_error,crate::ink_windows::ink_snapshot,crate::ink_windows::ink_ready,crate::ink_windows::ink_mutate,crate::ink_windows::ink_style,crate::ink_windows::ink_flush_ack,crate::ink_windows::ink_set_active,manager_recovery_status,manager_recovery_request,board_recovery_complete,manager_page_size,crate::resource_commands::image_budget,crate::resource_commands::image_storage,crate::resource_commands::generation_test,crate::resource_commands::gateway_image_binding,crate::resource_commands::gateway_unbind_preset,crate::resource_commands::gateway_bind_preset,crate::resource_commands::image_retry_save,crate::resource_commands::gateway_probe,crate::resource_commands::gateway_connections,crate::resource_commands::gateway_configure,crate::resource_commands::gateway_revoke,crate::resource_commands::gateway_rotate,crate::resource_commands::resource_copy_image,crate::resource_commands::resource_drag_out,crate::resource_commands::notes_test_fault,crate::note_windows::notes_test_reload,crate::note_windows::note_capacity_subscribe,crate::note_windows::notes_capacity_edit,crate::note_windows::note_capacity_ack,crate::note_windows::notes_capacity_windows,crate::note_windows::notes_rehome,crate::note_windows::note_drag,crate::note_windows::note_resize,crate::note_windows::note_set_tab_regions,crate::note_windows::notes_present,crate::note_windows::note_set_resident,crate::note_windows::note_report_error,crate::note_windows::note_delete,crate::note_windows::note_restore,crate::note_windows::note_set_top,crate::note_windows::note_save_geometry,crate::note_windows::notes_release,crate::note_windows::notes_flush,crate::note_windows::note_flush_ack,crate::note_windows::note_open,crate::note_windows::note_ready,crate::note_windows::note_hide,crate::resource_commands::notes_list,crate::resource_commands::note_create,crate::resource_commands::note_get,crate::resource_commands::note_patch,crate::resource_commands::review_archive_fault,crate::resource_commands::recovery_export,crate::resource_commands::storage_status,crate::resource_commands::migration_activate,crate::resource_commands::resource_import,crate::resource_commands::scene_snapshot,crate::resource_commands::scene_save,crate::resource_commands::scene_place_output,crate::resource_commands::task_feed,crate::resource_commands::task_control,crate::resource_commands::task_submit,crate::resource_commands::task_snapshot,crate::resource_commands::migration_verify,crate::resource_commands::migration_prepare,crate::resource_commands::resource_read,board_subscribe,board_view,board_capacity_progress,board_smoke_finish,board_diagnostic,board_probe,board_quit,board_quit_saved,board_recovery_open,board_drag_exports_open,board_group_focused,crate::resource_commands::read_image,storage::load_config,storage::save_config,storage::add_ability,open_manager,show_manager,close_manager]).build(context);
 let app=match built{Ok(a)=>a,Err(e)=>{stop.store(true,Ordering::Relaxed);unsafe{let _=MessageBoxW(None,&windows::core::HSTRING::from(format!("画板未启动：{e}")),windows::core::w!("QuickGen"),MB_OK|MB_ICONERROR);}return}};
 app.run(move|_,event|{if matches!(event,tauri::RunEvent::Exit){stop.store(true,Ordering::Relaxed)}});
}

#[cfg(feature="desktop-diagnostics")]
pub(crate) fn diagnostic_mode()->&'static str{static MODE:std::sync::OnceLock<String>=std::sync::OnceLock::new();MODE.get_or_init(||std::env::var("QUICKGEN_DIAGNOSTIC").unwrap_or_default()).as_str()}
#[cfg(not(feature="desktop-diagnostics"))]
pub(crate) fn diagnostic_mode()->&'static str{""}

fn hover_changed(last:&mut Option<Vec<String>>,hits:&[String])->bool{if last.as_deref()==Some(hits){false}else{*last=Some(hits.to_vec());true}}

impl View{fn hover_at(&self,x:f64,y:f64)->Vec<String>{if !self.open||!self.passthrough{return vec![]}self.hover_targets.iter().filter(|t|t.rect.contains(x,y)).map(|t|t.id.clone()).collect()}
fn region_for(&self,crop:bool)->bool{self.open&&(!self.passthrough||self.interacting||!crop)}
fn full_region(&self)->bool{self.region_for(diagnostic_mode()=="crop")}
fn ignore_at(&self,x:f64,y:f64)->bool{self.open&&self.passthrough&&!self.interacting&&!self.rects.iter().any(|r|r.contains(x,y))}}
#[cfg(test)]mod tests{
 use super::*;
 #[cfg(not(feature="desktop-diagnostics"))]
 #[test]fn release_has_no_diagnostic_mode(){assert_eq!(diagnostic_mode(),"");}

 #[test]fn passthrough_region_excludes_empty_desktop_but_keeps_visuals_separate_from_controls(){let mut v=View{open:true,passthrough:true,visuals:vec![Rect{x:100.,y:100.,width:180.,height:140.}],rects:vec![Rect{x:0.,y:0.,width:20.,height:80.}],..View::default()};assert!(!v.region_for(true));assert!(v.ignore_at(120.,120.));assert!(!v.ignore_at(10.,10.));v.interacting=true;assert!(v.region_for(true));assert!(!v.ignore_at(120.,120.));v.interacting=false;v.passthrough=false;assert!(v.region_for(true));v.open=false;assert!(!v.region_for(true));}

 #[test]fn movement_inside_image_emits_only_one_hover_transition(){let mut v=View{open:true,passthrough:true,hover_targets:vec![HoverTarget{id:"a".into(),rect:Rect{x:0.,y:0.,width:100.,height:100.}},HoverTarget{id:"b".into(),rect:Rect{x:50.,y:50.,width:100.,height:100.}}],..View::default()};let mut last=None;let mut count=0;for n in 0..600{if hover_changed(&mut last,&v.hover_at((n%40) as f64,20.)){count+=1}}assert_eq!(count,1);assert_eq!(v.hover_at(60.,60.),vec!["a","b"]);assert!(hover_changed(&mut last,&v.hover_at(60.,60.)));assert!(hover_changed(&mut last,&v.hover_at(500.,500.)));assert!(!hover_changed(&mut last,&v.hover_at(501.,501.)));v.passthrough=false;assert!(v.hover_at(20.,20.).is_empty());v.passthrough=true;v.hover_targets.clear();assert!(v.hover_at(20.,20.).is_empty());}

 #[test]fn view_dedup_preserves_input_mode_and_geometry_changes(){let a=View::default();let mut b=a.clone();assert!(a==b);b.interacting=true;assert!(a!=b);b=a.clone();b.rects.push(Rect{x:1.,y:2.,width:3.,height:4.});assert!(a!=b);b=a.clone();b.passthrough=true;assert!(a!=b);}

 #[test]fn captured_panel_drag_keeps_native_input_until_release(){let mut view=View{open:true,passthrough:true,rects:vec![Rect{x:0.,y:100.,width:18.,height:88.}],..View::default()};assert!(!view.ignore_at(9.,120.));assert!(view.ignore_at(500.,400.));view.interacting=true;assert!(!view.ignore_at(500.,400.));view.interacting=false;assert!(view.ignore_at(500.,400.));view.open=false;assert!(!view.ignore_at(500.,400.));}
}





#[cfg(test)]mod settings_recovery_tests{
 use super::*;
 #[test]fn stale_and_duplicate_replies_never_consume_another_request(){
  let(tx,rx)=std::sync::mpsc::channel();let mut pending=Some(("current".into(),tx));
  assert!(complete_recovery(&mut pending,"old",Some(json!({"directory":"wrong"})),None).is_err());assert!(pending.is_some());assert!(rx.try_recv().is_err());
  complete_recovery(&mut pending,"current",None,Some("save failed".into())).unwrap();assert_eq!(rx.recv().unwrap(),Err("save failed".into()));assert!(pending.is_none());
  assert!(complete_recovery(&mut pending,"current",Some(json!({})),None).is_err());
 }
}






