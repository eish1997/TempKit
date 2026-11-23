#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Mutex;
use tauri::Manager;
#[derive(Clone, serde::Serialize)]
struct Note { id: String, text: String, pinned: bool, visible: bool }
#[derive(Default)]
struct Notes(Mutex<Vec<Note>>);
#[tauri::command]
fn notes_list(state: tauri::State<Notes>) -> Vec<Note> { state.0.lock().unwrap().clone() }
#[tauri::command]
fn notes_create(app: tauri::AppHandle, state: tauri::State<Notes>) -> Result<(), String> {
 let id = format!("note-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
 let count = state.0.lock().unwrap().len();
 tauri::WebviewWindowBuilder::new(&app, &id, tauri::WebviewUrl::App(format!("notes-prototype.html?id={id}").into()))
 .title("便签 · 原型").inner_size(320.,300.).min_inner_size(220.,160.).position(440.+(count%6) as f64*28.,180.+(count%6) as f64*28.)
 .decorations(false).resizable(true).build().map_err(|e|e.to_string())?;
 state.0.lock().unwrap().push(Note { id, text: String::new(), pinned: false, visible: true }); Ok(())
}
#[tauri::command]
fn notes_update(window: tauri::WebviewWindow, state: tauri::State<Notes>, text: String) {
 if let Some(note)=state.0.lock().unwrap().iter_mut().find(|n|n.id==window.label()){note.text=text;}
}
#[tauri::command]
fn notes_action(app: tauri::AppHandle, state: tauri::State<Notes>, id: String, action: String) -> Result<(),String> {
 let window=app.get_webview_window(&id).ok_or("窗口不存在")?;
 let mut notes=state.0.lock().unwrap();let note=notes.iter_mut().find(|n|n.id==id).ok_or("便签不存在")?;
 match action.as_str(){
 "pin"=>{window.set_always_on_top(!note.pinned).map_err(|e|e.to_string())?;note.pinned=!note.pinned;},
 "hide"=>{window.hide().map_err(|e|e.to_string())?;note.visible=false;},
 "show"=>{window.show().map_err(|e|e.to_string())?;window.set_focus().map_err(|e|e.to_string())?;note.visible=true;},
 _=>return Err("未知操作".into())
 } Ok(())
}
#[tauri::command]
fn notes_drag(window: tauri::WebviewWindow)->Result<(),String>{window.start_dragging().map_err(|e|e.to_string())}
#[tauri::command]
fn notes_quit(app:tauri::AppHandle){app.exit(0)}
fn main(){
 let mut context=tauri::generate_context!();context.config_mut().identifier="com.quickgen.notes-prototype".into();
 tauri::Builder::default().manage(Notes::default()).setup(|app|{
 tauri::WebviewWindowBuilder::new(app,"notes-home",tauri::WebviewUrl::App("notes-prototype.html".into())).title("便签 · 交互原型").inner_size(350.,480.).min_inner_size(300.,300.).build()?;Ok(())
 }).on_window_event(|window,event|{
 if let tauri::WindowEvent::CloseRequested{api,..}=event {if window.label().starts_with("note-"){api.prevent_close();let _=window.hide();if let Some(note)=window.state::<Notes>().0.lock().unwrap().iter_mut().find(|n|n.id==window.label()){note.visible=false;}}}
 }).invoke_handler(tauri::generate_handler![notes_list,notes_create,notes_update,notes_action,notes_drag,notes_quit]).run(context).expect("便签原型启动失败");
}
