//! Bounded in-memory timing only for the isolated performance diagnostic.
use std::{sync::Mutex,time::Instant};
static SAMPLES:Mutex<Vec<serde_json::Value>>=Mutex::new(Vec::new());
pub fn start()->Option<Instant>{
 (crate::runtime_paths::isolated().is_some()&&crate::board::diagnostic_mode()=="generation-recovery"&&quickgen_desktop::generation_fixture::performance_mode().is_some()).then(Instant::now)
}
pub fn record(name:&str,start:Option<Instant>){
 record_operation(name,None,start);
}
pub fn record_operation(name:&str,operation:Option<&str>,start:Option<Instant>){
 if let Some(start)=start{let ms=start.elapsed().as_secs_f64()*1000.;if let Ok(mut rows)=SAMPLES.lock(){if rows.len()<10000{rows.push(serde_json::json!({"name":name,"operation":operation,"ms":ms}));}}}
}
pub fn take()->serde_json::Value{let rows=SAMPLES.lock().map(|mut rows|std::mem::take(&mut *rows)).unwrap_or_default();serde_json::json!({"truncated":rows.len()>=10000,"samples":rows})}
