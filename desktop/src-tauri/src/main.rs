#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod board;
mod note_windows;
mod storage;
mod overlay_support;
mod geometry;
mod resource_commands;
mod runtime_paths;
#[cfg(feature="desktop-diagnostics")]
mod resource_smoke;
fn main(){if quickgen_desktop::decode_worker::run_if_requested(){return} if let Err(error)=runtime_paths::initialize(){eprintln!("{error}");std::process::exit(2)}
let args:Vec<String>=std::env::args().collect();
if args.iter().any(|a|a=="--install-image-cost-rule"){
 let result=(||{
  let positions:Vec<_>=args.iter().enumerate().filter(|(_,a)|a.as_str()=="--install-image-cost-rule").map(|(i,_)|i).collect();
  if positions.len()!=1{return Err("cost rule option must occur exactly once".to_string())}
  let path=args.get(positions[0]+1).ok_or("cost rule file required")?;
  let root=runtime_paths::isolated().ok_or("cost rule installation requires --isolated-data in this phase")?;
  quickgen_desktop::image_cost_rules::install_file(&root.join("resource-store"),std::path::Path::new(path))
 })();
 match result{Ok(id)=>{println!("Installed image cost rule {id}");return},Err(error)=>{eprintln!("{error}");std::process::exit(2)}}
}
#[cfg(feature="desktop-diagnostics")]
if std::env::args().any(|a|a=="--resource-smoke"){resource_smoke::run();return}
board::run()}
