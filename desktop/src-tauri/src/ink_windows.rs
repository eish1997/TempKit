//! One ephemeral annotation surface per physical monitor, owned by this host.
use std::{collections::{BTreeMap,BTreeSet},sync::{Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
use tauri::{Emitter,Manager};
use serde::Serialize;
use quickgen_desktop::ink_session::{Session,Mutation,Style};
use sha2::{Sha256,Digest};
#[derive(Clone,Debug,PartialEq,Serialize)]
pub struct Screen{pub key:String,pub label:String,pub x:i32,pub y:i32,pub width:u32,pub height:u32,pub scale:f64,pub offset_x:f64,pub offset_y:f64}
#[derive(Default)]pub struct InkHost{pub session:Mutex<Session>,screens:Mutex<BTreeMap<String,Screen>>,ready:Mutex<BTreeSet<String>>,pending:Mutex<BTreeMap<u64,BTreeMap<String,Option<Result<(),String>>>>>,next:AtomicU64,transition:AtomicBool,refresh_queued:AtomicBool,channels:Mutex<BTreeMap<String,tauri::ipc::Channel<serde_json::Value>>>,frames:Mutex<BTreeMap<String,RenderStats>>}
#[derive(Default,Serialize)]struct RenderStats{count:u64,total_ms:f64,max_ms:f64,revision:u64,objects:u32,samples:Vec<f64>}
fn diagnostics()->Result<(),String>{if crate::runtime_paths::isolated().is_none()||!matches!(crate::board::diagnostic_mode(),"notes-rich-smoke"|"notes-rich-capacity"){Err("isolated annotation diagnostic only".into())}else{Ok(())}}
#[tauri::command]pub fn ink_diagnostic_render(window:tauri::WebviewWindow,revision:u64,elapsed:f64,objects:u32)->Result<(),String>{diagnostics()?;if !window.label().starts_with("ink-")||!elapsed.is_finite()||elapsed<0.||elapsed>60000.||objects>10000{return Err("invalid annotation sample".into())}let host=window.state::<InkHost>();let mut frames=host.frames.lock().map_err(|e|e.to_string())?;let stats=frames.entry(window.label().into()).or_default();stats.count+=1;stats.total_ms+=elapsed;stats.max_ms=stats.max_ms.max(elapsed);stats.revision=revision;stats.objects=objects;if stats.samples.len()>=4096{stats.samples.remove(0);}stats.samples.push(elapsed);Ok(())}
#[tauri::command]pub fn ink_diagnostic(window:tauri::WebviewWindow,action:String,step:Option<u64>)->Result<serde_json::Value,String>{
 diagnostics()?;if window.label()!="main"{return Err("main only".into())}let app=window.app_handle();let host=window.state::<InkHost>();
 #[cfg(feature="desktop-diagnostics")]
 if action=="layer"{return layer_probe(app);}
 #[cfg(feature="desktop-diagnostics")]
 if action=="translate"{return translation_probe(app);}
 if action=="seed"||action=="step"{use quickgen_desktop::ink_session::{Object,Content,Point,COLORS};let monitor=app.primary_monitor().map_err(|e|e.to_string())?.ok_or("no monitor")?;let key=monitor.name().cloned().unwrap_or_else(||format!("monitor-{}-{}",monitor.position().x,monitor.position().y));let mut s=host.session.lock().map_err(|e|e.to_string())?;let apply=|s:&mut Session,mutation:Mutation|s.mutate(s.epoch,s.revision,mutation);
  if action=="seed"{if !s.is_empty(){return Err("seed requires empty annotations".into())}for i in 0..500{let x=12.+(i%25)as f64*36.;let y=12.+(i/25)as f64*24.;let points=(0..72).map(|p|Point{x:x+p as f64,y:y+(p as f64*0.1).sin()*9.}).collect();apply(&mut s,Mutation::Add{object:Object{id:format!("capacity-line-{i}"),monitor:key.clone(),content:Content::Stroke{points,style:Style{color:COLORS[i%COLORS.len()].into(),width:[2.,3.,6.][i%3]}}}})?;}for i in 0..50{apply(&mut s,Mutation::Add{object:Object{id:format!("capacity-text-{i}"),monitor:key.clone(),content:Content::Text{x:16.+(i%10)as f64*110.,y:500.+(i/10)as f64*40.,text:format!("标注 {i}\n中文"),style:Style{color:COLORS[i%COLORS.len()].into(),width:3.}}}})?;}}
  else{let n=step.ok_or("step required")?;let group=n/4;match n%4{0=>{apply(&mut s,Mutation::Add{object:Object{id:format!("capacity-live-{group}"),monitor:key.clone(),content:Content::Stroke{points:vec![Point{x:20.,y:20.},Point{x:150.,y:60.}],style:Style::default()}}})?;apply(&mut s,Mutation::Replace{object:Object{id:"capacity-text-0".into(),monitor:key,content:Content::Text{x:16.,y:500.,text:format!("更新 {group}\n中文"),style:Style::default()}}})?;},1=>apply(&mut s,Mutation::Erase{ids:vec![format!("capacity-line-{}",group%500)]})?,2=>apply(&mut s,Mutation::Undo)?,_=>apply(&mut s,Mutation::Erase{ids:vec![format!("capacity-live-{group}")]})?,}}
  drop(s);broadcast(app)?;
 }else if action!="status"{return Err("unknown annotation diagnostic action".into())}
 let mut windows=vec![];for(label,child)in app.webview_windows(){if !label.starts_with("ink-"){continue;}let hwnd=child.hwnd().map_err(|e|e.to_string())?;let style=unsafe{windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd,windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE)}as u32;windows.push(serde_json::json!({"label":label,"visible":child.is_visible().map_err(|e|e.to_string())?,"transparent":style&windows::Win32::UI::WindowsAndMessaging::WS_EX_TRANSPARENT.0!=0,"toolWindow":style&windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW.0!=0}));}
 let session=host.session.lock().map_err(|e|e.to_string())?;Ok(serde_json::json!({"active":session.active,"objects":session.len(),"revision":session.revision,"windows":windows,"ready":host.ready.lock().map_err(|e|e.to_string())?.len(),"frames":*host.frames.lock().map_err(|e|e.to_string())?}))
}
#[cfg(feature="desktop-diagnostics")]
fn translation_probe(app:&tauri::AppHandle)->Result<serde_json::Value,String>{
 use quickgen_desktop::ink_session::Content;let host=app.state::<InkHost>();let screen=host.screens.lock().map_err(|e|e.to_string())?.values().next().cloned().ok_or("no ink surface")?;let window=app.get_webview_window(&screen.label).ok_or("missing ink window")?;let main=app.get_webview_window("main").ok_or("missing main")?;
 let entries=host.session.lock().map_err(|e|e.to_string())?.snapshot_for(Some(&screen.key)).objects;
 for(index,is_text)in [false,true].into_iter().enumerate(){let original=entries.iter().find(|e|matches!(e.item.content,Content::Text{..})==is_text).ok_or("missing translation fixture")?.clone();let(epoch,revision)={let s=host.session.lock().map_err(|e|e.to_string())?;(s.epoch,s.revision)};
  if ink_mutate(main.clone(),format!("move-forbidden-{index}"),1,epoch,revision,Mutation::Translate{object:original.item.clone(),dx:12.,dy:8.}).is_ok(){return Err("main bypassed surface ownership".into())}
  ink_mutate(window.clone(),format!("move-probe-{index}"),1,epoch,revision,Mutation::Translate{object:original.item.clone(),dx:12.,dy:8.})?;
  let moved=host.session.lock().map_err(|e|e.to_string())?.snapshot_for(Some(&screen.key)).objects;let item=moved.iter().find(|e|e.item.id==original.item.id).ok_or("moved object lost")?;if item.order!=original.order||item.item==original.item{return Err("translation failed or changed order".into())}
  ink_mutate(window.clone(),format!("move-probe-{index}"),1,epoch,revision,Mutation::Translate{object:original.item.clone(),dx:12.,dy:8.})?;if host.session.lock().map_err(|e|e.to_string())?.snapshot_for(Some(&screen.key)).objects!=moved{return Err("move receipt retry applied twice".into())}
  let(epoch,revision)={let s=host.session.lock().map_err(|e|e.to_string())?;(s.epoch,s.revision)};ink_mutate(window.clone(),format!("move-undo-{index}"),1,epoch,revision,Mutation::Undo)?;
  if host.session.lock().map_err(|e|e.to_string())?.snapshot_for(Some(&screen.key)).objects!=entries{return Err("translation undo did not restore scene".into())}
 }Ok(serde_json::json!({"strokeAndText":true,"singleUndo":true,"lostReplyReceipt":true,"surfaceOwnership":true,"physicalMiddleButton":false}))
}
#[cfg(feature="desktop-diagnostics")]
fn layer_probe(app:&tauri::AppHandle)->Result<serde_json::Value,String>{
 use windows::Win32::{Foundation::{HWND,RECT},Graphics::Gdi::*,UI::WindowsAndMessaging::*};use crate::ink_occlusion::Region;
 struct Restore(Vec<(HWND,bool)>);impl Drop for Restore{fn drop(&mut self){for &(hwnd,top)in &self.0{if !top{unsafe{let _=SetWindowPos(hwnd,Some(HWND_NOTOPMOST),0,0,0,0,SWP_NOACTIVATE|SWP_NOMOVE|SWP_NOSIZE);}}}}}
 let foreground=unsafe{GetForegroundWindow()};let mut restore=Restore(vec![]);let mut note_rects=vec![];
 for(label,window)in app.webview_windows(){if label!="main"&&!label.starts_with("note-"){continue}let hwnd=window.hwnd().map_err(|e|e.to_string())?;let top=unsafe{GetWindowLongW(hwnd,GWL_EXSTYLE)}as u32&WS_EX_TOPMOST.0!=0;restore.0.push((hwnd,top));
  unsafe{SetWindowPos(hwnd,Some(HWND_TOPMOST),0,0,0,0,SWP_NOACTIVATE|SWP_NOMOVE|SWP_NOSIZE)}.map_err(|e|e.to_string())?;
  if label.starts_with("note-"){let mut rect=RECT::default();unsafe{GetWindowRect(hwnd,&mut rect)}.map_err(|e|e.to_string())?;note_rects.push(rect);}
 }
 refresh_regions(app)?;let exclusions=crate::board::annotation_exclusions(app)?;let screens=app.state::<InkHost>().screens.lock().map_err(|e|e.to_string())?.clone();let owned:Vec<_>=restore.0.iter().map(|r|r.0).collect();let mut note_points=0;let mut control_points=0;let mut board_points=0;
 for(label,screen)in screens{let window=app.get_webview_window(&label).ok_or("missing ink window")?;let hwnd=window.hwnd().map_err(|e|e.to_string())?;if !crate::ink_occlusion::above_owned(hwnd,&owned)?{return Err("ink is below a QuickGen window".into())}
  let region=Region::rect(0,0,0,0)?;if unsafe{GetWindowRgn(hwnd,region.0)}.0==0{return Err("missing native ink region".into())}
  let inside=|x:i32,y:i32|x>=screen.x&&y>=screen.y&&x<screen.x+screen.width as i32&&y<screen.y+screen.height as i32;
  let protected=|x:i32,y:i32|exclusions.iter().any(|r|x as f64>=r.x&&y as f64>=r.y&&(x as f64)<r.x+r.width&&(y as f64)<r.y+r.height);
  for r in &note_rects{for(x,y)in[(r.left+40,r.top+40),((r.left+r.right)/2,(r.top+r.bottom)/2),(r.right-20,r.bottom-20)]{if inside(x,y)&&!protected(x,y){note_points+=1;if !unsafe{PtInRegion(region.0,x-screen.x,y-screen.y)}.as_bool(){return Err("note still cuts a hole in ink".into())}}}}
  for y in (40..screen.height as i32).step_by(160){for x in (40..screen.width as i32).step_by(160){if !protected(x+screen.x,y+screen.y){board_points+=1;if !unsafe{PtInRegion(region.0,x,y)}.as_bool(){return Err("board area still cuts ink".into())}}}}
  for r in &exclusions{let x=(r.x+r.width/2.)as i32;let y=(r.y+r.height/2.)as i32;if inside(x,y){control_points+=1;if unsafe{PtInRegion(region.0,x-screen.x,y-screen.y)}.as_bool(){return Err("ink controls are blocked".into())}}}
 }
 if note_points==0||board_points==0||control_points==0{return Err("native layer probe did not cover all required areas".into())}
 if foreground!=unsafe{GetForegroundWindow()}{return Err("raising ink stole foreground focus".into())}
 Ok(serde_json::json!({"aboveNotesAndBoard":true,"notePoints":note_points,"boardPoints":board_points,"accessibleControlPoints":control_points,"foregroundUnchanged":true,"physicalInputTest":false}))
}
pub fn active(app:&tauri::AppHandle)->bool{app.try_state::<InkHost>().and_then(|h|h.session.lock().ok().map(|s|s.active)).unwrap_or(false)}
fn authorized(window:&tauri::WebviewWindow)->Result<(),String>{if window.label()=="main"||window.label().starts_with("ink-"){Ok(())}else{Err("annotation window only".into())}}
fn snapshot_for(app:&tauri::AppHandle,label:&str)->Result<serde_json::Value,String>{let host=app.state::<InkHost>();let screen=host.screens.lock().map_err(|e|e.to_string())?.get(label).cloned();let snapshot=host.session.lock().map_err(|e|e.to_string())?.snapshot_for(screen.as_ref().map(|s|s.key.as_str()));Ok(serde_json::json!({"session":snapshot,"screen":screen,"double_click_ms":unsafe{windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime()}}))}
// Tauri events embed payloads into eval source, even for empty target ID lists.
// High-volume snapshots use the existing IPC Channel transport: large payloads
// are fetched as data, not repeatedly compiled as multi-megabyte JavaScript.
fn broadcast(app:&tauri::AppHandle)->Result<(),String>{let channels=app.state::<InkHost>().channels.lock().map_err(|e|e.to_string())?.clone();for(label,channel)in channels{if app.get_webview_window(&label).is_some(){channel.send(snapshot_for(app,&label)?).map_err(|e|e.to_string())?;}}Ok(())}
#[tauri::command]pub fn ink_subscribe(window:tauri::WebviewWindow,channel:tauri::ipc::Channel<serde_json::Value>)->Result<(),String>{authorized(&window)?;window.state::<InkHost>().channels.lock().map_err(|e|e.to_string())?.insert(window.label().into(),channel);Ok(())}
#[tauri::command]pub fn ink_snapshot(window:tauri::WebviewWindow)->Result<serde_json::Value,String>{authorized(&window)?;snapshot_for(window.app_handle(),window.label())}
#[tauri::command]pub fn ink_report_error(window:tauri::WebviewWindow,error:Option<String>)->Result<(),String>{authorized(&window)?;if error.as_ref().is_some_and(|e|e.len()>8192){return Err("ink error limit".into())}window.app_handle().emit_to("main","ink-error",serde_json::json!({"id":window.label(),"error":error})).map_err(|e|e.to_string())}
#[tauri::command]pub fn ink_ready(window:tauri::WebviewWindow)->Result<serde_json::Value,String>{if !window.label().starts_with("ink-"){return Err("ink only".into())}window.state::<InkHost>().ready.lock().map_err(|e|e.to_string())?.insert(window.label().into());refresh_regions(window.app_handle())?;window.show().map_err(|e|e.to_string())?;snapshot_for(window.app_handle(),window.label())}
#[tauri::command]pub fn ink_mutate(window:tauri::WebviewWindow,stream:String,sequence:u64,epoch:u64,revision:u64,mutation:Mutation)->Result<serde_json::Value,String>{
 authorized(&window)?;let host=window.state::<InkHost>();let screen=host.screens.lock().map_err(|e|e.to_string())?.get(window.label()).cloned();
 {let mut session=host.session.lock().map_err(|e|e.to_string())?;
  match &mutation{
   Mutation::Add{object}|Mutation::Replace{object}|Mutation::Translate{object,..}=>{if !screen.as_ref().is_some_and(|s|s.key==object.monitor){return Err("wrong annotation monitor".into())}},
   Mutation::Erase{ids}=>{let screen=screen.as_ref().ok_or("ink surface required")?;if !session.ids_belong_to(ids,&screen.key){return Err("wrong annotation monitor".into())}},
   Mutation::Undo|Mutation::Redo=>{},
   Mutation::Clear=>return Err("clear requires main flush barrier".into()),
  }session.mutate_once(&format!("{}-{stream}",window.label()),sequence,epoch,revision,mutation)?;
 }broadcast(window.app_handle())?;snapshot_for(window.app_handle(),window.label())
}
// Clear after every renderer has committed its pending gesture. Keep the receipt
// across retries so a lost response cannot clear a later drawing a second time.
#[tauri::command]pub async fn ink_clear(window:tauri::WebviewWindow,stream:String,sequence:u64)->Result<serde_json::Value,String>{
 if window.label()!="main"{return Err("main only".into())}let app=window.app_handle().clone();let host=app.state::<InkHost>();if host.transition.swap(true,Ordering::SeqCst){return Err("画笔操作正在进行，请重试".into())}
 let result=async{release_capture(&app)?;let cloned=app.clone();tauri::async_runtime::spawn_blocking(move||flush(&cloned)).await.map_err(|e|e.to_string())??;
  {let mut s=host.session.lock().map_err(|e|e.to_string())?;let(epoch,revision)=(s.epoch,s.revision);s.mutate_once(&format!("clear-{stream}"),sequence,epoch,revision,Mutation::Clear)?;}broadcast(&app)?;snapshot_for(&app,"main")
 }.await;host.transition.store(false,Ordering::SeqCst);if let Err(error)=refresh_regions(&app){emergency_stop(&app);return Err(error)}result
}
#[tauri::command]pub fn ink_style(window:tauri::WebviewWindow,style:Style)->Result<serde_json::Value,String>{if window.label()!="main"{return Err("main only".into())}style.validate()?;crate::storage::save_config(window.app_handle().clone(),"ink".into(),serde_json::to_value(&style).map_err(|e|e.to_string())?)?;window.state::<InkHost>().session.lock().map_err(|e|e.to_string())?.set_style(style)?;broadcast(window.app_handle())?;snapshot_for(window.app_handle(),window.label())}
#[tauri::command]pub fn ink_flush_ack(window:tauri::WebviewWindow,token:u64,error:Option<String>)->Result<(),String>{if !window.label().starts_with("ink-"){return Err("ink only".into())}let host=window.state::<InkHost>();let mut pending=host.pending.lock().map_err(|e|e.to_string())?;let row=pending.get_mut(&token).and_then(|r|r.get_mut(window.label())).ok_or("no pending annotation flush")?;if row.is_some(){return Err("duplicate annotation flush".into())}*row=Some(error.map_or(Ok(()),Err));Ok(())}
fn flush(app:&tauri::AppHandle)->Result<(),String>{
 let host=app.state::<InkHost>();let ready=host.ready.lock().map_err(|e|e.to_string())?.clone();let token=host.next.fetch_add(1,Ordering::Relaxed);let labels:Vec<_>=app.webview_windows().keys().filter(|id|id.starts_with("ink-")&&ready.contains(*id)).cloned().collect();
 host.pending.lock().map_err(|e|e.to_string())?.insert(token,labels.iter().map(|id|(id.clone(),None)).collect());
 let result=(||{for label in labels{app.emit_to(&label,"ink-flush-request",token).map_err(|e|e.to_string())?;}let deadline=std::time::Instant::now()+std::time::Duration::from_secs(8);loop{{let pending=host.pending.lock().map_err(|e|e.to_string())?;let rows=pending.get(&token).ok_or("annotation flush disappeared")?;for result in rows.values(){if let Some(Err(error))=result{return Err(error.clone())}}if rows.values().all(|r|r.is_some()){return Ok(())}}if std::time::Instant::now()>=deadline{return Err("画笔编辑尚未确认，已保留草稿，请重新打开画笔重试".into())}std::thread::sleep(std::time::Duration::from_millis(15));}})();host.pending.lock().map_err(|e|e.to_string())?.remove(&token);result
}
pub fn release_capture(app:&tauri::AppHandle)->Result<(),String>{let mut failures=vec![];for(label,window)in app.webview_windows(){if label.starts_with("ink-"){if let Err(e)=window.set_ignore_cursor_events(true){let _=window.hide();failures.push(e.to_string());}}}if failures.is_empty(){Ok(())}else{Err(failures.join("; "))}}
pub fn emergency_stop(app:&tauri::AppHandle){if let Some(host)=app.try_state::<InkHost>(){if let Ok(mut session)=host.session.lock(){session.set_active(false);}let _=release_capture(app);let _=broadcast(app);}}
pub fn forget_window(app:&tauri::AppHandle,label:&str){if let Some(host)=app.try_state::<InkHost>(){if let Ok(mut ready)=host.ready.lock(){ready.remove(label);}if let Ok(mut channels)=host.channels.lock(){channels.remove(label);}}emergency_stop(app);}
async fn ui_refresh(app:tauri::AppHandle)->Result<(),String>{let(tx,rx)=std::sync::mpsc::sync_channel(1);let cloned=app.clone();app.run_on_main_thread(move||{let _=tx.send(refresh(&cloned));}).map_err(|e|e.to_string())?;tauri::async_runtime::spawn_blocking(move||rx.recv_timeout(std::time::Duration::from_secs(8)).map_err(|e|e.to_string())?).await.map_err(|e|e.to_string())?}
#[tauri::command]pub async fn ink_set_active(window:tauri::WebviewWindow,enabled:bool)->Result<serde_json::Value,String>{
 authorized(&window)?;if enabled&&window.label()!="main"{return Err("main opens annotation mode".into())}let app=window.app_handle().clone();let host=app.state::<InkHost>();if host.transition.swap(true,Ordering::SeqCst){return Err("画笔切换正在进行".into())}
 let result=async{
  if enabled{if crate::note_windows::overview_active(&app){return Err("请先退出便签总览".into())}let config=crate::storage::load_config(app.clone())?;let style=serde_json::from_value::<Style>(config["ink"].clone()).unwrap_or_default();{let mut s=host.session.lock().map_err(|e|e.to_string())?;s.set_style(style)?;s.set_active(true);}if let Err(error)=ui_refresh(app.clone()).await{host.session.lock().map_err(|e|e.to_string())?.set_active(false);let _=release_capture(&app);return Err(error)}broadcast(&app)?;
  }else{
   // Release mouse capture before waiting: even a broken renderer cannot trap the desktop.
   let release=release_capture(&app);let cloned=app.clone();let flushed=tauri::async_runtime::spawn_blocking(move||flush(&cloned)).await.map_err(|e|e.to_string())?;
   host.session.lock().map_err(|e|e.to_string())?.set_active(false);let _=release_capture(&app);broadcast(&app)?;release?;flushed?;
  }snapshot_for(&app,window.label())
 }.await;if !enabled||result.is_err(){host.session.lock().map_err(|e|e.to_string())?.set_active(false);let _=release_capture(&app);let _=broadcast(&app);}host.transition.store(false,Ordering::SeqCst);if result.is_ok(){refresh_regions(&app)?;}result
}
// External application moves do not emit Tauri events. Poll only while ink is
// visible, and never accumulate callbacks if the UI thread is occupied.
pub fn queue_region_refresh(app:&tauri::AppHandle){let Some(host)=app.try_state::<InkHost>()else{return};if !host.session.lock().map(|s|s.active||!s.is_empty()).unwrap_or(false)||host.refresh_queued.swap(true,Ordering::SeqCst){return}let cloned=app.clone();if app.run_on_main_thread(move||{if let Err(error)=refresh_regions(&cloned){emergency_stop(&cloned);let _=cloned.emit_to("main","ink-error",serde_json::json!({"id":"native-region","error":error}));}if let Some(host)=cloned.try_state::<InkHost>(){host.refresh_queued.store(false,Ordering::SeqCst);}}).is_err(){host.refresh_queued.store(false,Ordering::SeqCst);}}
pub fn refresh(app:&tauri::AppHandle)->Result<(),String>{
 let host=app.state::<InkHost>();let (active,has_objects)={let session=host.session.lock().map_err(|e|e.to_string())?;(session.active,!session.is_empty())};if !active&&!has_objects&&host.screens.lock().map_err(|e|e.to_string())?.is_empty(){return Ok(())}
 let mut connected=BTreeSet::new();let mut changed=false;
 for monitor in app.available_monitors().map_err(|e|e.to_string())?{
  let origin=monitor.position();let area=monitor.work_area();let scale=monitor.scale_factor();let key=monitor.name().cloned().unwrap_or_else(||format!("monitor-{}-{}",origin.x,origin.y));let label=format!("ink-{:x}",Sha256::digest(key.as_bytes()));let label=label[..20].to_string();connected.insert(label.clone());
  let screen=Screen{key,label:label.clone(),x:area.position.x,y:area.position.y,width:area.size.width,height:area.size.height,scale,offset_x:(area.position.x-origin.x)as f64/scale,offset_y:(area.position.y-origin.y)as f64/scale};let previous=host.screens.lock().map_err(|e|e.to_string())?.insert(label.clone(),screen.clone());
  let existing=app.get_webview_window(&label);if existing.is_none(){host.ready.lock().map_err(|e|e.to_string())?.remove(&label);let builder=tauri::WebviewWindowBuilder::new(app,&label,tauri::WebviewUrl::App(format!("ink.html?id={label}").into())).title("QuickGen · 桌面画笔").transparent(true).background_color(tauri::window::Color(0,0,0,0)).decorations(false).shadow(false).resizable(false).always_on_top(true).skip_taskbar(true).visible(false);let builder=if let Some(root)=crate::runtime_paths::isolated(){builder.data_directory(root.join("webview"))}else{builder};let window=builder.build().map_err(|e|e.to_string())?;quickgen_desktop::overlay_window::install(window.hwnd().map_err(|e|e.to_string())?)?;window.set_ignore_cursor_events(true).map_err(|e|e.to_string())?;changed=true;}
  if existing.is_none()||previous.as_ref()!=Some(&screen){changed=true;let window=app.get_webview_window(&label).ok_or("annotation window missing")?;window.set_ignore_cursor_events(true).map_err(|e|e.to_string())?;window.set_position(tauri::PhysicalPosition::new(screen.x,screen.y)).map_err(|e|e.to_string())?;window.set_size(tauri::PhysicalSize::new(screen.width,screen.height)).map_err(|e|e.to_string())?;}
 }
 for(label,window)in app.webview_windows(){if label.starts_with("ink-")&&!connected.contains(&label){window.set_ignore_cursor_events(true).map_err(|e|e.to_string())?;window.hide().map_err(|e|e.to_string())?;}}
 refresh_regions(app)?;if changed{broadcast(app)?;}Ok(())
}
/// Ink covers notes and the board. Only dedicated ink controls remain accessible.
/// Their geometry and the relative native layer are independent of canvas coordinates.
pub fn refresh_regions(app:&tauri::AppHandle)->Result<(),String>{
 let Some(host)=app.try_state::<InkHost>()else{return Ok(())};let screens=host.screens.lock().map_err(|e|e.to_string())?.clone();if screens.is_empty(){return Ok(())}let active=host.session.lock().map_err(|e|e.to_string())?.active&&!host.transition.load(Ordering::SeqCst);let ready=host.ready.lock().map_err(|e|e.to_string())?.clone();
 let connected:BTreeSet<_>=app.available_monitors().map_err(|e|e.to_string())?.iter().map(|m|m.name().cloned().unwrap_or_else(||format!("monitor-{}-{}",m.position().x,m.position().y))).collect();let exclusions=crate::board::annotation_exclusions(app)?;
 let owned=app.webview_windows().into_iter().filter(|(label,_)|!label.starts_with("ink-")).map(|(_,w)|w.hwnd().map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
 for(label,screen)in screens{let Some(window)=app.get_webview_window(&label)else{continue};if !connected.contains(&screen.key)||!ready.contains(&label){window.set_ignore_cursor_events(true).map_err(|e|e.to_string())?;continue;}
  if let Err(error)=apply_region(&window,&screen,&exclusions){let _=window.set_ignore_cursor_events(true);let _=window.hide();return Err(error)}let hwnd=window.hwnd().map_err(|e|e.to_string())?;use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW,GWL_EXSTYLE,WS_EX_TRANSPARENT};let transparent=unsafe{GetWindowLongW(hwnd,GWL_EXSTYLE)}as u32&WS_EX_TRANSPARENT.0!=0;if transparent==active{window.set_ignore_cursor_events(!active).map_err(|e|e.to_string())?;}if !window.is_visible().map_err(|e|e.to_string())?{window.show().map_err(|e|e.to_string())?;}crate::ink_occlusion::ensure_above(hwnd,&owned)?;
 }Ok(())
}
fn region_for(screen:&Screen,exclusions:&[crate::geometry::Rect])->Result<windows::Win32::Graphics::Gdi::HRGN,String>{
 use windows::Win32::Graphics::Gdi::{CreateRectRgn,CombineRgn,RGN_DIFF,DeleteObject,HGDIOBJ};
 let region=unsafe{CreateRectRgn(0,0,screen.width as i32,screen.height as i32)};if region.0.is_null(){return Err("cannot create annotation region".into())}
 for r in exclusions{let left=(r.x-screen.x as f64).floor().max(0.)as i32;let top=(r.y-screen.y as f64).floor().max(0.)as i32;let right=(r.x+r.width-screen.x as f64).ceil().min(screen.width as f64)as i32;let bottom=(r.y+r.height-screen.y as f64).ceil().min(screen.height as f64)as i32;if left>=right||top>=bottom{continue;}let cut=unsafe{CreateRectRgn(left,top,right,bottom)};let result=unsafe{CombineRgn(Some(region),Some(region),Some(cut),RGN_DIFF)};unsafe{let _=DeleteObject(HGDIOBJ(cut.0));}if result.0==0{unsafe{let _=DeleteObject(HGDIOBJ(region.0));}return Err("cannot exclude annotation controls".into())}}
 Ok(region)
}
fn apply_region(window:&tauri::WebviewWindow,screen:&Screen,exclusions:&[crate::geometry::Rect])->Result<(),String>{
 use windows::Win32::Graphics::Gdi::*;use crate::ink_occlusion::Region;
 let hwnd=window.hwnd().map_err(|e|e.to_string())?;let region=Region(region_for(screen,exclusions)?);
 let previous=Region::rect(0,0,0,0)?;if unsafe{GetWindowRgn(hwnd,previous.0)}.0!=0&&unsafe{EqualRgn(previous.0,region.0)}.as_bool(){return Ok(())}
 if unsafe{SetWindowRgn(hwnd,Some(region.0),true)}==0{return Err("cannot apply annotation region".into())}std::mem::forget(region);Ok(())
}
#[cfg(test)]mod tests{
 #[test]fn physical_regions_exclude_only_explicit_controls_on_negative_monitor(){use windows::Win32::Graphics::Gdi::{PtInRegion,DeleteObject,HGDIOBJ};use crate::geometry::Rect;let screen=super::Screen{key:"left-125".into(),label:"ink-test".into(),x:-1920,y:50,width:1920,height:1030,scale:1.25,offset_x:0.,offset_y:40.};let region=super::region_for(&screen,&[Rect{x:-1800.,y:100.,width:300.,height:260.},Rect{x:-1400.,y:200.,width:400.,height:500.},Rect{x:0.,y:0.,width:500.,height:500.}]).unwrap();unsafe{assert!(!PtInRegion(region,150,100).as_bool());assert!(!PtInRegion(region,600,200).as_bool());assert!(PtInRegion(region,1000,200).as_bool());assert!(PtInRegion(region,10,10).as_bool());assert!(DeleteObject(HGDIOBJ(region.0)).as_bool());}}
}
