#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod geometry;
mod storage;
use geometry::{Edge,Rect,place,adjacent};
use tauri::Manager;
use storage::{load_config,save_config,add_ability};
use base64::Engine;
use std::io::Write;
use tauri::{LogicalPosition,LogicalSize};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn,SetWindowRgn,DeleteObject,HGDIOBJ};

struct ManagerHeight(std::sync::Mutex<f64>);

#[derive(Default)]
struct InputBridge(std::sync::Mutex<Option<tauri::ipc::Channel<serde_json::Value>>>);
#[tauri::command]
fn subscribe_input(window:tauri::WebviewWindow,bridge:tauri::State<InputBridge>,channel:tauri::ipc::Channel<serde_json::Value>)->Result<(),String>{
 if window.label()!="main"{return Err("Input is only available to the main panel".into())}
 channel.send(serde_json::json!({"type":"ready"})).map_err(|e|e.to_string())?;
 *bridge.0.lock().map_err(|e|e.to_string())?=Some(channel);Ok(())
}
fn input_payload(event:&tauri::DragDropEvent)->Option<serde_json::Value>{
 use tauri::DragDropEvent::*;
 Some(match event{
  Enter{paths,position}=>serde_json::json!({"type":"enter","paths":paths,"position":position}),
  Over{position}=>serde_json::json!({"type":"over","position":position}),
  Drop{paths,position}=>serde_json::json!({"type":"drop","paths":paths,"position":position}),
  Leave=>serde_json::json!({"type":"leave"}),_=>return None
 })
}

#[tauri::command]
fn host_info()->String {format!("QuickGen {} · W4 config restore",env!("CARGO_PKG_VERSION"))}
#[tauri::command]
fn quit(app:tauri::AppHandle){app.exit(0)}

#[tauri::command]
fn trace_input(app:tauri::AppHandle,stage:String)->Result<(),String>{
 let allowed=["listener-ready","listener-error","bridge-ready","bridge-error","js-enter","js-drop","js-leave","read-ok","decode-ok","drop-error","native-enter","native-drop","native-leave","window-enter","window-drop","window-leave","startup"];
 if !allowed.contains(&stage.as_str()){return Err("Invalid diagnostic stage".into())}
 let dir=app.path().app_log_dir().map_err(|e|e.to_string())?;
 std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 let path=dir.join("input-diagnostics.jsonl");
 // Bounded local diagnostics: no paths, image bytes or user field contents.
 if std::fs::metadata(&path).map(|m|m.len()>256*1024).unwrap_or(false){std::fs::write(&path,[]).map_err(|e|e.to_string())?}
 let mut file=std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e|e.to_string())?;
 writeln!(file,"{}",serde_json::json!({"stage":stage,"pid":std::process::id(),"time":format!("{:?}",std::time::SystemTime::now())})).map_err(|e|e.to_string())
}

#[tauri::command]
fn display_snapshot(app:tauri::AppHandle)->Result<Vec<String>,String>{
 let mut screens:Vec<_>=app.available_monitors().map_err(|e|e.to_string())?.iter().map(|m|
  format!("{:?}|{:?}|{:?}|{:?}|{}",m.name(),m.position(),m.size(),m.work_area(),m.scale_factor())
 ).collect();screens.sort();Ok(screens)
}
#[tauri::command]
fn configure(window:tauri::WebviewWindow,edge:String,fraction:f64,width:f64,height:f64)->Result<serde_json::Value,String>{
 if ![fraction,width,height].iter().all(|n|n.is_finite()){return Err("Invalid geometry".into())}
 let monitor=window.current_monitor().map_err(|e|e.to_string())?
  .or(window.primary_monitor().map_err(|e|e.to_string())?).ok_or("No monitor")?;
 let scale=monitor.scale_factor();let area=monitor.work_area();
 let area=Rect{x:area.position.x as f64/scale,y:area.position.y as f64/scale,width:area.size.width as f64/scale,height:area.size.height as f64/scale};
 let edge=match edge.as_str(){"top"=>Edge::Top,"left"=>Edge::Left,"right"=>Edge::Right,_=>return Err("Invalid edge".into())};
 let p=place(area,edge,fraction,width,height);
 window.set_size(LogicalSize::new(p.panel.width,p.panel.height)).map_err(|e|e.to_string())?;
 window.set_position(LogicalPosition::new(p.panel.x,p.panel.y)).map_err(|e|e.to_string())?;
 position_aux(window.app_handle())?;
 Ok(serde_json::json!({"width":p.panel.width,"height":p.panel.height,"entry":{"x":p.entry.x-p.panel.x,"y":p.entry.y-p.panel.y,"width":p.entry.width,"height":p.entry.height},"area":{"x":area.x,"y":area.y,"width":area.width,"height":area.height}}))
}

#[tauri::command]
fn shape(window:tauri::WebviewWindow,x:f64,y:f64,width:f64,height:f64)->Result<(),String>{
 apply_window_region(&window,x,y,width,height)?;
 window.show().map_err(|e|e.to_string())
}

// Share the native 6px outline without showing a child before it is ready.
fn apply_window_region(window:&tauri::WebviewWindow,x:f64,y:f64,width:f64,height:f64)->Result<(),String>{
 if ![x,y,width,height].iter().all(|n|n.is_finite()) || width<=0. || height<=0. {return Err("Invalid region".into())}
 let scale=window.scale_factor().map_err(|e|e.to_string())?;
 let hwnd=window.hwnd().map_err(|e|e.to_string())?;
 // Windows owns the region on success. Unlike CSS clipping, this also removes
 // invisible portions from native hit testing and OLE drag/drop targeting.
 unsafe{
 let region=CreateRoundRectRgn((x*scale).round() as i32,(y*scale).round() as i32,((x+width)*scale).round() as i32+1,((y+height)*scale).round() as i32+1,(12.*scale).round() as i32,(12.*scale).round() as i32);
 if region.0.is_null(){return Err("CreateRoundRectRgn failed".into())}
 if SetWindowRgn(hwnd,Some(region),true)==0 {let _=DeleteObject(HGDIOBJ(region.0));return Err("SetWindowRgn failed".into())}
 }
 Ok(())
}

#[tauri::command]
fn read_image(path:String)->Result<serde_json::Value,String>{
 let path=std::path::PathBuf::from(path);
 let ext=path.extension().and_then(|v|v.to_str()).unwrap_or("").to_lowercase();
 let mime=match ext.as_str(){"png"=>"image/png","jpg"|"jpeg"=>"image/jpeg","webp"=>"image/webp",_=>return Err("请拖入 PNG、JPG 或 WebP 图片".into())};
 let info=std::fs::metadata(&path).map_err(|_|"图片不可读取")?;
 if !info.is_file()||info.len()>50*1024*1024{return Err("请选择 50 MB 以内的图片文件".into())}
 let data=std::fs::read(&path).map_err(|_|"图片读取失败")?;
 Ok(serde_json::json!({"path":path.to_string_lossy(),"name":path.file_name().unwrap_or_default().to_string_lossy(),"data":format!("data:{};base64,{}",mime,base64::engine::general_purpose::STANDARD.encode(data))}))
}
#[tauri::command]
fn record_trigger(app:tauri::AppHandle,path:String,ability:String)->Result<(),String>{
 if ability.trim().is_empty(){return Err("未选择能力".into())}
 // Simulation record only. Never claim generation or image saving.
 let dir=app.path().app_log_dir().map_err(|e|e.to_string())?;
 std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 let mut log=std::fs::OpenOptions::new().create(true).append(true).open(dir.join("triggers.jsonl")).map_err(|e|e.to_string())?;
 let line=serde_json::json!({"kind":"simulation","input":path,"ability":ability,"time":format!("{:?}",std::time::SystemTime::now())});
 writeln!(log,"{}",line).map_err(|e|e.to_string())
}

fn position_aux(app:&tauri::AppHandle)->Result<(),String>{
 let Some(aux)=app.get_webview_window("manager") else{return Ok(())};let main=app.get_webview_window("main").ok_or("No main window")?;
 let scale=main.scale_factor().map_err(|e|e.to_string())?;let p=main.inner_position().map_err(|e|e.to_string())?;let size=main.inner_size().map_err(|e|e.to_string())?;
 let monitor=main.current_monitor().map_err(|e|e.to_string())?.ok_or("No monitor")?;let area=monitor.work_area();
 let left=area.position.x as f64/scale;let top=area.position.y as f64/scale;let right=left+area.size.width as f64/scale;let bottom=top+area.size.height as f64/scale;
 let height=*app.state::<ManagerHeight>().0.lock().map_err(|e|e.to_string())?;
 let placed=adjacent(Rect{x:left,y:top,width:right-left,height:bottom-top},Rect{x:p.x as f64/scale,y:p.y as f64/scale,width:size.width as f64/scale,height:size.height as f64/scale},308.,height);
 aux.set_size(LogicalSize::new(placed.width,placed.height)).map_err(|e|e.to_string())?;
 aux.set_position(LogicalPosition::new(placed.x,placed.y)).map_err(|e|e.to_string())?;
 apply_window_region(&aux,0.,0.,placed.width,placed.height)
}
#[tauri::command]
fn close_manager(app:tauri::AppHandle)->Result<(),String>{if let Some(w)=app.get_webview_window("manager"){w.hide().map_err(|e|e.to_string())?}Ok(())}
#[tauri::command]
fn show_manager(app:tauri::AppHandle)->Result<(),String>{
 let child=app.get_webview_window("manager").ok_or("No manager window")?;
 position_aux(&app)?;child.show().map_err(|e|e.to_string())?;child.set_focus().map_err(|e|e.to_string())
}
#[tauri::command]
async fn open_manager(app:tauri::AppHandle,mode:String)->Result<(),String>{
 if !["add","library"].contains(&mode.as_str()){return Err("Invalid manager mode".into())}
 *app.state::<ManagerHeight>().0.lock().map_err(|e|e.to_string())?=if mode=="add"{312.}else{344.};
 // WebView2 creation must run outside a synchronous command (Windows deadlock).
 // Reuse the nonmodal window; reload each opening so cancelled forms are discarded.
 if let Some(child)=app.get_webview_window("manager"){
  child.hide().map_err(|e|e.to_string())?;
  let url=child.url().map_err(|e|e.to_string())?.join(&format!("manager.html?mode={mode}")).map_err(|e|e.to_string())?;
  child.navigate(url).map_err(|e|e.to_string())?;
  return position_aux(&app);
 }
 tauri::WebviewWindowBuilder::new(&app,"manager",tauri::WebviewUrl::App(format!("manager.html?mode={mode}").into())).title("QuickGen · 能力管理").inner_size(308.,344.).transparent(true).decorations(false).resizable(false).shadow(false).always_on_top(true).visible(false).build().map_err(|e|e.to_string())?;
 position_aux(&app)
}
fn main(){tauri::Builder::default().manage(InputBridge::default()).manage(ManagerHeight(std::sync::Mutex::new(344.)))
 .setup(|app|{let _=trace_input(app.handle().clone(),"startup".into());Ok(())})
 .on_window_event(|window,event|{if window.label()!="main"{return}
  if let tauri::WindowEvent::DragDrop(event)=event{
   if let Some(payload)=input_payload(event){
    let state=window.state::<InputBridge>();
    if let Ok(bridge)=state.0.lock(){if let Some(channel)=bridge.as_ref(){
     if channel.send(payload).is_err(){let _=trace_input(window.app_handle().clone(),"bridge-error".into());}
    }};
   }
  }
 let stage=match event{
  tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Enter{..})=>"window-enter",
  tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop{..})=>"window-drop",
  tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Leave)=>"window-leave",_=>return};
  let _=trace_input(window.app_handle().clone(),stage.into());
 })
 .on_webview_event(|webview,event|{if webview.label()!="main"{return}let stage=match event{
  tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Enter{..})=>"native-enter",
  tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Drop{..})=>"native-drop",
  tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Leave)=>"native-leave",_=>return};
  let _=trace_input(webview.app_handle().clone(),stage.into());
 })
 .invoke_handler(tauri::generate_handler![host_info,quit,trace_input,subscribe_input,display_snapshot,configure,shape,read_image,record_trigger,load_config,save_config,add_ability,open_manager,show_manager,close_manager]).run(tauri::generate_context!()).expect("QuickGen desktop host failed")}
