//! Same-executable helper: establish an OS budget before consuming image bytes.
use std::io::{Read,Write};
use std::process::{Command,Stdio};
pub const PROCESS_MEMORY_LIMIT:usize=512*1024*1024;
const MARKER:&str="QG_DECODE:";

#[cfg(windows)]
struct Job(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for Job {fn drop(&mut self){unsafe{let _=windows::Win32::Foundation::CloseHandle(self.0);}}}
#[cfg(windows)]
fn budget()->Result<Job,String> {
    use windows::Win32::System::{JobObjects::*,Threading::GetCurrentProcess};
    unsafe {
        let job=CreateJobObjectW(None,windows::core::PCWSTR::null()).map_err(|e|e.to_string())?;
        let mut info=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_PROCESS_MEMORY|JOB_OBJECT_LIMIT_PROCESS_TIME|JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        info.ProcessMemoryLimit=PROCESS_MEMORY_LIMIT;
        info.BasicLimitInformation.PerProcessUserTimeLimit=30*10_000_000;
        let result=SetInformationJobObject(job,JobObjectExtendedLimitInformation,&info as *const _ as _,std::mem::size_of_val(&info) as u32)
            .and_then(|_|AssignProcessToJobObject(job,GetCurrentProcess()));
        if let Err(e)=result {let _=windows::Win32::Foundation::CloseHandle(job);return Err(e.to_string());}
        Ok(Job(job))
    }
}
fn helper(extension:&str)->Result<(u32,u32,String),String> {
    #[cfg(windows)] let _job=budget()?;
    #[cfg(not(windows))] return Err("bounded image worker requires Windows".into());
    let mut bytes=Vec::new(); std::io::stdin().take(crate::resources::MAX_BYTES+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    let image=crate::resources::decode_local(bytes,extension)?;
    Ok((image.width,image.height,image.mime.into()))
}
pub fn run_if_requested()->bool {
    let args:Vec<_>=std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("--quickgen-clipboard-decode") {
        let result=(||->Result<(),String>{
            #[cfg(windows)]let _job=budget()?;
            #[cfg(not(windows))]return Err("bounded clipboard decoder requires Windows".into());
            let mut bytes=Vec::new();std::io::stdin().take(crate::resources::MAX_BYTES+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
            let dib=crate::resources::decode_dib_local(bytes,args.get(2).map(String::as_str).unwrap_or(""))?;
            let mut output=std::io::stdout().lock();
            output.write_all(b"QGDIBV1\0").and_then(|_|output.write_all(&(dib.len() as u64).to_le_bytes())).and_then(|_|output.write_all(&dib)).map_err(|e|e.to_string())
        })();
        if result.is_err(){std::process::exit(2)}
        return true;
    }
    if args.get(1).map(String::as_str)!=Some("--quickgen-decode") {return false;}
    let result=helper(args.get(2).map(String::as_str).unwrap_or(""));
    println!("{MARKER}{}",serde_json::to_string(&result).unwrap()); true
}
pub fn validate(bytes:&[u8],extension:&str)->Result<(u32,u32,String),String> {
    let mut command=Command::new(std::env::current_exe().map_err(|e|e.to_string())?);
    #[cfg(not(test))] command.args(["--quickgen-decode",extension]);
    #[cfg(test)] command.args(["--exact","decode_worker::tests::helper_entry","--nocapture"]).env("QUICKGEN_TEST_DECODE_EXT",extension);
    exchange(command,bytes,std::time::Duration::from_secs(45))
}
fn exchange(mut command:Command,bytes:&[u8],timeout:std::time::Duration)->Result<(u32,u32,String),String> {
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
    let mut input=child.stdin.take().unwrap();
    let deadline=std::time::Instant::now()+timeout;
    std::thread::scope(|scope| {
        let writer=scope.spawn(move|| input.write_all(bytes));
        loop {
            match child.try_wait() {
                Ok(Some(_))=>break,
                Ok(None) if std::time::Instant::now()<deadline=>std::thread::sleep(std::time::Duration::from_millis(10)),
                _=>{let _=child.kill();let _=child.wait();let _=writer.join();return Err("decode worker timed out or failed".into());}
            }
        }
        writer.join().map_err(|_|"decode writer panicked".to_string())?.map_err(|e|format!("decode worker input failed: {e}"))
    })?;
    let output=child.wait_with_output().map_err(|e|e.to_string())?;
    if !output.status.success() {return Err("image decoder exceeded budget or exited unexpectedly".into());}
    let text=String::from_utf8(output.stdout).map_err(|e|e.to_string())?;
    let line=text.lines().find_map(|s|s.strip_prefix(MARKER)).ok_or("missing decoder response")?;
    serde_json::from_str::<Result<(u32,u32,String),String>>(line).map_err(|e|e.to_string())?
}
#[cfg(test)] mod tests {
    #[test] fn stalled_input_is_killed_and_reaped() {
        if std::env::var_os("QUICKGEN_TEST_STALL").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(10));return;
        }
        let mut command=std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact","decode_worker::tests::stalled_input_is_killed_and_reaped","--nocapture"]).env("QUICKGEN_TEST_STALL","1");
        let start=std::time::Instant::now();
        let error=super::exchange(command,&vec![0;1024*1024],std::time::Duration::from_millis(100)).unwrap_err();
        assert!(error.contains("timed out")); assert!(start.elapsed()<std::time::Duration::from_secs(3));
    }
    #[cfg(windows)]
    #[test] fn os_budget_rejects_oversized_allocation() {
        if std::env::var_os("QUICKGEN_TEST_MEMORY_BUDGET").is_some() {
            let _job=super::budget().unwrap();
            let mut buffer:Vec<u8>=Vec::new();
            assert!(buffer.try_reserve_exact(super::PROCESS_MEMORY_LIMIT*2).is_err());
            return;
        }
        let status=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","decode_worker::tests::os_budget_rejects_oversized_allocation","--nocapture"])
            .env("QUICKGEN_TEST_MEMORY_BUDGET","1").status().unwrap();
        assert!(status.success());
    }
    #[test] fn helper_entry() {
        if let Ok(ext)=std::env::var("QUICKGEN_TEST_DECODE_EXT") {
            println!("{}{}",super::MARKER,serde_json::to_string(&super::helper(&ext)).unwrap());
        }
    }
}

const MAX_DIB_BYTES:u64=124+crate::resources::MAX_PIXELS*4;
fn parse_dib_frame(frame:Vec<u8>)->Result<Vec<u8>,String>{
 if frame.len()<16+124||&frame[..8]!=b"QGDIBV1\0"{return Err("invalid clipboard decoder frame".into())}
 let length=u64::from_le_bytes(frame[8..16].try_into().unwrap());
 if length>MAX_DIB_BYTES||length!=frame.len() as u64-16{return Err("clipboard decoder length mismatch".into())}
 let dib=&frame[16..];
 let word=|offset|u32::from_le_bytes(dib[offset..offset+4].try_into().unwrap());
 let width=word(4);let height=word(8) as i32;
 if word(0)!=124||width==0||width>crate::resources::MAX_DIMENSION||height>=0||height==i32::MIN{return Err("invalid clipboard dimensions".into())}
 let height=(-height) as u32;let pixels=width as u64*height as u64;
 if height>crate::resources::MAX_DIMENSION||pixels>crate::resources::MAX_PIXELS||length!=124+pixels*4||word(20) as u64!=pixels*4||word(16)!=3||dib[12..16]!=[1,0,32,0]{return Err("invalid clipboard pixel layout".into())}
 if [word(40),word(44),word(48),word(52)]!=[0xff0000,0xff00,0xff,0xff000000]{return Err("invalid clipboard color masks".into())}
 Ok(dib.to_vec())
}
pub fn clipboard_dib(bytes:&[u8],extension:&str)->Result<Vec<u8>,String>{
 if bytes.is_empty()||bytes.len() as u64>crate::resources::MAX_BYTES{return Err("clipboard input exceeds budget".into())}
 let mut command=Command::new(std::env::current_exe().map_err(|e|e.to_string())?);
 command.args(["--quickgen-clipboard-decode",extension]);
 exchange_dib(command,bytes,std::time::Duration::from_secs(45))
}
fn exchange_dib(mut command:Command,bytes:&[u8],timeout:std::time::Duration)->Result<Vec<u8>,String>{
 #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
 let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
 let mut input=child.stdin.take().unwrap();let output=child.stdout.take().unwrap();
 let deadline=std::time::Instant::now()+timeout;
 std::thread::scope(|scope|{
  let writer=scope.spawn(move||input.write_all(bytes));
  let reader=scope.spawn(move||{let mut frame=Vec::new();output.take(MAX_DIB_BYTES+17).read_to_end(&mut frame).map_err(|e|e.to_string())?;Ok::<_,String>(frame)});
  let status=loop{
   match child.try_wait(){
    Ok(Some(status))=>break Ok(status),
    Ok(None) if std::time::Instant::now()<deadline=>std::thread::sleep(std::time::Duration::from_millis(10)),
    _=>{let _=child.kill();let _=child.wait();break Err("clipboard decoder timed out or failed".to_string())}
   }
  };
  let sent=writer.join().map_err(|_|"clipboard input worker panicked".to_string())?;
  let frame=reader.join().map_err(|_|"clipboard output worker panicked".to_string())??;
  if !status?.success(){return Err("clipboard decoder exited without a valid result".into())}
  sent.map_err(|e|e.to_string())?;parse_dib_frame(frame)
 })
}
#[cfg(test)]mod clipboard_transport_tests{
 use super::*;
 fn frame()->Vec<u8>{let dib=crate::clipboard_pixels::rgba_to_dibv5(2,1,&[1,2,3,4,5,6,7,8]).unwrap();let mut out=b"QGDIBV1\0".to_vec();out.extend((dib.len() as u64).to_le_bytes());out.extend(dib);out}
 #[test]fn validates_binary_length_dimensions_and_masks(){
  assert_eq!(parse_dib_frame(frame()).unwrap().len(),132);
  for offset in [0,8,16,20,24,28,32,36,56,60,64,68]{let mut data=frame();data[offset]^=127;assert!(parse_dib_frame(data).is_err(),"offset {offset}")}
  let mut truncated=frame();truncated.pop();assert!(parse_dib_frame(truncated).is_err());let mut extra=frame();extra.push(0);assert!(parse_dib_frame(extra).is_err());
 }
 #[test]fn blocked_binary_input_is_killed_and_reaped(){
  let mut command=Command::new(std::env::current_exe().unwrap());command.args(["--exact","decode_worker::tests::stalled_input_is_killed_and_reaped","--nocapture"]).env("QUICKGEN_TEST_STALL","1");
  let start=std::time::Instant::now();assert!(exchange_dib(command,&vec![0;1024*1024],std::time::Duration::from_millis(100)).is_err());assert!(start.elapsed()<std::time::Duration::from_secs(3));
 }
}
