//! Explicit isolated runs keep configuration, managed resources and WebView data together.
use std::{path::PathBuf,sync::OnceLock};
use tauri::Manager;
static ISOLATED:OnceLock<Option<PathBuf>>=OnceLock::new();
fn parse(args:&[String])->Result<Option<PathBuf>,String> {
    let positions:Vec<_>=args.iter().enumerate().filter(|(_,s)|s.as_str()=="--isolated-data").map(|(i,_)|i).collect();
    if positions.is_empty(){return Ok(None)}
    if positions.len()!=1{return Err("--isolated-data may occur only once".into())}
    let path=PathBuf::from(args.get(positions[0]+1).ok_or("--isolated-data requires an absolute directory")?);
    if !path.is_absolute(){return Err("isolated directory must be absolute".into())} Ok(Some(path))
}
pub fn initialize()->Result<(),String> {
    let path=parse(&std::env::args().collect::<Vec<_>>())?;
    let path=path.map(prepare).transpose()?;
    ISOLATED.set(path).map_err(|_|"runtime paths already initialized".into())
}
fn prepare(path:PathBuf)->Result<PathBuf,String> {
        // A first run may use only an empty directory. Reuse requires our marker.
        std::fs::create_dir_all(&path).map_err(|e|e.to_string())?;
        let marker=path.join("quickgen-isolated-v1");
        if marker.exists() {
            if std::fs::read(&marker).map_err(|e|e.to_string())?!=b"QuickGen isolated test data v1" {return Err("invalid isolated data marker".into());}
        } else {
            if std::fs::read_dir(&path).map_err(|e|e.to_string())?.next().is_some(){return Err("isolated directory must be empty or previously initialized".into())}
            std::fs::write(&marker,b"QuickGen isolated test data v1").map_err(|e|e.to_string())?;
        }
        path.canonicalize().map_err(|e|e.to_string())
}
pub fn isolated()->Option<PathBuf>{ISOLATED.get().cloned().flatten()}
pub fn data(app:&tauri::AppHandle)->Result<PathBuf,String>{
    match isolated(){Some(path)=>Ok(path),None=>app.path().app_data_dir().map_err(|e|e.to_string())}
}
#[cfg(test)]mod tests {
    #[test]fn unrelated_data_is_never_adopted(){
        let root=std::env::temp_dir().join(format!("quickgen-isolation-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join("config.json"),b"existing").unwrap();
        assert!(super::prepare(root.clone()).is_err());assert_eq!(std::fs::read(root.join("config.json")).unwrap(),b"existing");
        let fresh=root.join("fresh");assert!(super::prepare(fresh.clone()).is_ok());
        std::fs::write(fresh.join("config.json"),b"isolated").unwrap();assert!(super::prepare(fresh).is_ok());
    }
    #[test]fn explicit_absolute_path_only(){
        assert_eq!(super::parse(&["app".into()]).unwrap(),None);
        assert!(super::parse(&["--isolated-data".into()]).is_err());
        assert!(super::parse(&["--isolated-data".into(),"relative".into()]).is_err());
        let p=std::env::temp_dir();assert_eq!(super::parse(&["--isolated-data".into(),p.to_string_lossy().into_owned()]).unwrap(),Some(p));
    }
}
