//! Diagnostic-only real WebView IPC exercise, requiring an isolated data root.
use tauri::Manager;
struct Seed(String);
struct Restart(bool);
#[tauri::command]
fn resource_smoke_paths()->serde_json::Value{
    let root=crate::runtime_paths::isolated().expect("isolated smoke");
    serde_json::json!({"valid":root.join("native-input.png"),"invalid":[root.join("native-false.jpg"),root.join("native-damaged.png"),root.join("native-oversize.png")]})
}
#[tauri::command]
fn resource_smoke_restart(window:tauri::WebviewWindow)->bool{window.state::<Restart>().0}
#[tauri::command]
fn resource_smoke_seed(window:tauri::WebviewWindow)->Result<String,String>{
    if window.label()!="main"{return Err("main only".into())} Ok(window.state::<Seed>().0.clone())
}
#[tauri::command]
fn resource_smoke_finish(app:tauri::AppHandle,result:serde_json::Value)->Result<(),String>{
    let root=crate::runtime_paths::isolated().ok_or("isolation required")?;
    std::fs::write(root.join("resource-smoke-result.json"),serde_json::to_vec_pretty(&result).unwrap()).map_err(|e|e.to_string())?;
    app.exit(if result["ok"]==true {0}else{1}); Ok(())
}
pub fn run(){
    let root=crate::runtime_paths::isolated().expect("resource smoke requires --isolated-data");
    let restarted=root.join("resource-smoke-result.json").exists();
    let mut bytes=std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(7,5).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
    std::fs::write(root.join("native-input.png"),bytes.get_ref()).unwrap();
    std::fs::write(root.join("native-false.jpg"),bytes.get_ref()).unwrap();
    std::fs::write(root.join("native-damaged.png"),b"damaged").unwrap();
    std::fs::File::create(root.join("native-oversize.png")).unwrap().set_len(quickgen_desktop::resources::MAX_BYTES+1).unwrap();
    let record={let mut service=quickgen_desktop::resources::ResourceService::open(&root.join("resource-store")).unwrap();
        if restarted {let id=std::fs::read_to_string(root.join("resource-smoke-seed.txt")).unwrap();service.read(&id).unwrap().0}else{
            let record=service.import_bytes(bytes.into_inner(),"png","isolated smoke.png".into(),quickgen_desktop::contracts::Provenance{task_id:None,parent_ids:vec![]}).unwrap();
            service.initialize_scene(serde_json::json!({"version":1,"resourceSchemaVersion":1,"items":[],"jobs":[],"selection":[],"scale":1,"tx":0,"ty":0})).unwrap();
            std::fs::write(root.join("resource-smoke-seed.txt"),&record.resource.id).unwrap();record
        }};
    tauri::Builder::default().manage(Seed(record.resource.id)).manage(Restart(restarted)).manage(crate::resource_commands::ResourceHost::default())
        .setup(move|app|{tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::App("resource-smoke.html".into()))
            .title("QuickGen isolated resource verification").data_directory(root.join("webview")).visible(false).build()?;Ok(())})
        .invoke_handler(tauri::generate_handler![crate::resource_commands::read_image,resource_smoke_paths,crate::resource_commands::recovery_export,crate::resource_commands::storage_status,crate::resource_commands::migration_activate,crate::resource_commands::resource_import,crate::resource_commands::scene_snapshot,crate::resource_commands::scene_save,crate::resource_commands::scene_place_output,crate::resource_commands::task_feed,crate::resource_commands::task_control,crate::resource_commands::task_submit,crate::resource_commands::task_snapshot,crate::resource_commands::migration_verify,crate::resource_commands::migration_prepare,crate::resource_commands::resource_read,resource_smoke_seed,resource_smoke_restart,resource_smoke_finish])
        .run(tauri::generate_context!()).expect("resource smoke startup");
}


