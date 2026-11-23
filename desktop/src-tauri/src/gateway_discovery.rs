//! Read-only New API model discovery. Run outside the resource worker.
use crate::{gateway_profiles::GatewayProfile,credential_vault::Secret};
use windows::{core::{w,PCWSTR},Win32::Networking::WinHttp::*};
struct Handle(*mut std::ffi::c_void);
impl Handle{fn new(value:*mut std::ffi::c_void)->Result<Self,String>{if value.is_null(){Err("network handle unavailable".into())}else{Ok(Self(value))}}}
impl Drop for Handle{fn drop(&mut self){unsafe{let _=WinHttpCloseHandle(self.0);}}}
struct Header(Vec<u16>);
impl Drop for Header{fn drop(&mut self){for unit in &mut self.0{unsafe{std::ptr::write_volatile(unit,0)}}}}
fn models(bytes:&[u8])->Result<Vec<String>,String>{
 let value:serde_json::Value=serde_json::from_slice(bytes).map_err(|_|"model response is not valid JSON")?;
 let entries=value.get("data").and_then(|v|v.as_array()).ok_or("model response has no data list")?;
 if entries.len()>4096{return Err("model list exceeds limit".into())}
 let mut names=std::collections::BTreeSet::new();
 for item in entries{let id=item.get("id").and_then(|v|v.as_str()).ok_or("invalid model entry")?;if id.is_empty()||id.len()>256||id.chars().any(char::is_control){return Err("invalid model ID".into())}names.insert(id.to_string());}
 Ok(names.into_iter().collect())
}
pub fn discover(profile:&GatewayProfile,secret:Secret)->Result<Vec<String>,String>{
 let mut sent=false;let bytes=exchange(profile,secret,None,&mut sent)?;models(&bytes)
}
#[derive(Debug)]
pub struct DeliveryFailure{pub message:String,pub acceptance_unknown:bool}
/// Caller must durably record dispatch before invoking this operation. No retries.
/// Unknown delivery must never be turned into automatic resubmission.
pub fn submit_edit(profile:&GatewayProfile,secret:Secret,body:&crate::image_protocol::EditBody)->Result<crate::image_protocol::ImageOutput,DeliveryFailure>{
 let mut sent=false;
 let bytes=exchange(profile,secret,Some(body),&mut sent).map_err(|message|DeliveryFailure{message,acceptance_unknown:sent})?;
 crate::image_protocol::parse_output(&bytes).map_err(|message|DeliveryFailure{message,acceptance_unknown:true})
}
// The selected upstream documents this exact output origin. Do not infer trust
// from an arbitrary response URL, a hostname suffix, or the gateway credential.
pub(crate) fn image_download_url(remote:&str)->Result<url::Url,String>{
 let url=url::Url::parse(remote).map_err(|_|"invalid image download address")?;
 if remote.len()>8192||url.scheme()!="https"||url.host_str()!=Some("file.302.ai")||url.port_or_known_default()!=Some(443)||!url.username().is_empty()||url.password().is_some()||url.fragment().is_some(){return Err("图片地址来源尚未核验；不会自动重新生成".into())}Ok(url)
}
pub(crate) fn download_image(remote:&str)->Result<Vec<u8>,String>{download_image_url(&image_download_url(remote)?)}
#[cfg(any(test,feature="desktop-diagnostics"))]
pub(crate) fn download_fixture(address:&str)->Result<Vec<u8>,String>{
 let url=url::Url::parse(address).map_err(|_|"invalid test URL")?;
 if url.scheme()!="http"||url.host_str()!=Some("127.0.0.1"){return Err("test server must be loopback".into())}download_image_url(&url)
}
// Private transport accepts an already validated URL; loopback is used only by
// unit tests below. Production callers must go through image_download_url.
fn download_image_url(url:&url::Url)->Result<Vec<u8>,String>{
 let host:Vec<u16>=url.host_str().ok_or("missing image host")?.encode_utf16().chain(Some(0)).collect();
 let target=format!("{}{}",url.path(),url.query().map(|q|format!("?{q}")).unwrap_or_default());
 let target:Vec<u16>=target.encode_utf16().chain(Some(0)).collect();
 let start=std::time::Instant::now();
 unsafe{
  let session=Handle::new(WinHttpOpen(w!("QuickGen/0.1"),WINHTTP_ACCESS_TYPE_NO_PROXY,PCWSTR::null(),PCWSTR::null(),0))?;
  WinHttpSetTimeouts(session.0,5000,5000,5000,5000).map_err(|_|"image timeout configuration failed")?;
  let connection=Handle::new(WinHttpConnect(session.0,PCWSTR(host.as_ptr()),url.port_or_known_default().ok_or("missing image port")?,0))?;
  let flags=if url.scheme()=="https"{WINHTTP_FLAG_SECURE}else{WINHTTP_OPEN_REQUEST_FLAGS(0)};
  let request=Handle::new(WinHttpOpenRequest(connection.0,w!("GET"),PCWSTR(target.as_ptr()),PCWSTR::null(),PCWSTR::null(),std::ptr::null(),flags))?;
  let disabled=(WINHTTP_DISABLE_REDIRECTS|WINHTTP_DISABLE_COOKIES|WINHTTP_DISABLE_AUTHENTICATION).to_le_bytes();
  WinHttpSetOption(Some(request.0),WINHTTP_OPTION_DISABLE_FEATURE,Some(&disabled)).map_err(|_|"image download isolation failed")?;
  // No bearer, referer, cookie, or implicit Windows credentials.
  WinHttpSendRequest(request.0,None,None,0,0,0).map_err(|_|"图片下载失败；不会自动重新生成")?;
  WinHttpReceiveResponse(request.0,std::ptr::null_mut()).map_err(|_|"图片下载超时或中断；不会自动重新生成")?;
  let mut status=0u32;let mut size=4;
  WinHttpQueryHeaders(request.0,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,PCWSTR::null(),Some((&mut status as *mut u32).cast()),&mut size,std::ptr::null_mut()).map_err(|_|"image download status unavailable")?;
  if status!=200{return Err(format!("图片下载返回 HTTP {status}；不会跟随重定向或重新生成"))}
  let mut bytes=Vec::new();let mut buffer=[0u8;8192];
  loop{
   if start.elapsed()>std::time::Duration::from_secs(60){return Err("image download exceeded time budget".into())}
   let mut read=0;WinHttpReadData(request.0,buffer.as_mut_ptr().cast(),buffer.len() as u32,&mut read).map_err(|_|"image download interrupted")?;
   if read==0{break}if bytes.len()+read as usize>crate::resources::MAX_BYTES as usize{return Err("image download exceeds size limit".into())}bytes.extend_from_slice(&buffer[..read as usize]);
  }
  if bytes.is_empty(){return Err("empty image download".into())}Ok(bytes)
 }
}
fn exchange(profile:&GatewayProfile,secret:Secret,edit:Option<&crate::image_protocol::EditBody>,sent:&mut bool)->Result<Vec<u8>,String>{
 profile.validate()?;
 if let Some(body)=edit{
  if body.bytes.is_empty()||body.bytes.len()>crate::resources::MAX_BYTES as usize+65536||body.content_type.len()>256||body.content_type.contains(['\r','\n'])||!body.content_type.starts_with("multipart/form-data; boundary="){return Err("invalid edit request body".into())}
 }
 if secret.bytes().is_empty()||secret.bytes().len()>4096||!secret.bytes().iter().all(|b|b.is_ascii_graphic()){return Err("invalid credential".into())}
 let url=url::Url::parse(&profile.base_url).map_err(|_|"invalid gateway address")?;
 let host:Vec<u16>=url.host_str().ok_or("missing gateway host")?.trim_matches(['[',']']).encode_utf16().chain(Some(0)).collect();
 let mut header=Header("Authorization: Bearer ".encode_utf16().collect());header.0.extend(secret.bytes().iter().map(|b|*b as u16));header.0.extend("\r\nAccept: application/json\r\n".encode_utf16());
 if let Some(body)=edit{header.0.extend(format!("Content-Type: {}\r\n",body.content_type).encode_utf16());}
 let start=std::time::Instant::now();
 unsafe{
  let session=Handle::new(WinHttpOpen(w!("QuickGen/0.1"),WINHTTP_ACCESS_TYPE_NO_PROXY,PCWSTR::null(),PCWSTR::null(),0))?;
  WinHttpSetTimeouts(session.0,5000,5000,5000,if edit.is_some(){90000}else{5000}).map_err(|_|"network timeout configuration failed")?;
  let connection=Handle::new(WinHttpConnect(session.0,PCWSTR(host.as_ptr()),url.port_or_known_default().ok_or("missing gateway port")?,0))?;
  let flags=if url.scheme()=="https"{WINHTTP_FLAG_SECURE}else{WINHTTP_OPEN_REQUEST_FLAGS(0)};
  let request=Handle::new(WinHttpOpenRequest(connection.0,if edit.is_some(){w!("POST")}else{w!("GET")},if edit.is_some(){w!("/v1/images/edits")}else{w!("/v1/models")},PCWSTR::null(),PCWSTR::null(),std::ptr::null(),flags))?;
  // No cross-origin bearer forwarding, cookies, or implicit Windows credentials.
  let disabled=(WINHTTP_DISABLE_REDIRECTS|WINHTTP_DISABLE_COOKIES|WINHTTP_DISABLE_AUTHENTICATION).to_le_bytes();
  WinHttpSetOption(Some(request.0),WINHTTP_OPTION_DISABLE_FEATURE,Some(&disabled)).map_err(|_|"network isolation configuration failed")?;
  // From this point onward even a transport error may follow server acceptance.
  *sent=true;
  let body=edit.map(|v|v.bytes.as_slice()).unwrap_or(&[]);
  WinHttpSendRequest(request.0,Some(&header.0),if body.is_empty(){None}else{Some(body.as_ptr().cast())},body.len() as u32,body.len() as u32,0).map_err(|_|"gateway request failed")?;drop(header);
  WinHttpReceiveResponse(request.0,std::ptr::null_mut()).map_err(|_|"gateway response failed or timed out")?;
  let mut status=0u32;let mut size=4;
  WinHttpQueryHeaders(request.0,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,PCWSTR::null(),Some((&mut status as *mut u32).cast()),&mut size,std::ptr::null_mut()).map_err(|_|"gateway status unavailable")?;
  if status!=200{return Err(match status{401|403=>"gateway rejected the credential".into(),300..=399=>"gateway redirected the request; redirects are not followed".into(),_=>format!("gateway returned HTTP {status}")})}
  let mut bytes=Vec::new();let mut buffer=[0u8;8192];
  loop{if start.elapsed()>std::time::Duration::from_secs(if edit.is_some(){180}else{15}){return Err("model response exceeded time budget".into())}
   let mut read=0;WinHttpReadData(request.0,buffer.as_mut_ptr().cast(),buffer.len() as u32,&mut read).map_err(|_|"model response interrupted")?;
   if read==0{break}if bytes.len()+read as usize>if edit.is_some(){28*1024*1024}else{1024*1024}{return Err(if edit.is_some(){"image response exceeds size limit"}else{"model response exceeds size limit"}.into())}bytes.extend_from_slice(&buffer[..read as usize]);
  }
  Ok(bytes)
 }
}
#[cfg(test)]mod tests{
 use super::*;use std::io::{Read,Write};
 fn response(status:&str,body:&str,extra:&str)->(GatewayProfile,std::thread::JoinHandle<String>){
  response_bytes(status,body.as_bytes(),extra)
 }
 fn response_bytes(status:&str,body:&[u8],extra:&str)->(GatewayProfile,std::thread::JoinHandle<String>){
  let socket=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=socket.local_addr().unwrap().port();let mut reply=format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",body.len()).into_bytes();reply.extend_from_slice(body);
  let worker=std::thread::spawn(move||{let(mut stream,_)=socket.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();let mut bytes=Vec::new();let mut byte=[0u8;1];while !bytes.ends_with(b"\r\n\r\n"){stream.read_exact(&mut byte).unwrap();bytes.push(byte[0]);assert!(bytes.len()<16384);}let _=stream.write_all(&reply);String::from_utf8(bytes).unwrap()});
  (GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:format!("http://127.0.0.1:{port}")},worker)
 }
 #[test]fn discovery_uses_get_and_redacts_failure_bodies(){
  for (status,body,extra,ok) in [("200 OK",r#"{"data":[{"id":"image-test"},{"id":"image-test"}]}"#,"",true),("401 Unauthorized","echo synthetic-only","",false),("302 Found","","Location: http://127.0.0.1:1/secret\r\n",false),("200 OK","invalid", "",false)]{
   let(profile,worker)=response(status,body,extra);let result=discover(&profile,Secret::from_bytes(b"synthetic-only".to_vec()));let request=worker.join().unwrap();assert!(request.starts_with("GET /v1/models HTTP/1.1"));assert!(request.contains("Authorization: Bearer synthetic-only"));
   if ok{assert_eq!(result.unwrap(),vec!["image-test"])}else{assert!(!result.unwrap_err().contains("synthetic-only"));}
  }
 }
 #[test]fn oversized_response_is_rejected_before_parsing(){
  let body="x".repeat(1024*1024+1);let(profile,worker)=response("200 OK",&body,"");
  assert_eq!(discover(&profile,Secret::from_bytes(b"synthetic-only".to_vec())).unwrap_err(),"model response exceeds size limit");worker.join().unwrap();
 }
 #[test]fn remote_image_origin_is_exact_and_never_accepts_credentials_or_local_targets(){
  assert!(image_download_url("https://file.302.ai/gpt/imgs/result.png?signature=opaque").is_ok());
  for address in ["http://file.302.ai/x","https://file.302.ai.evil.test/x","https://evilfile.302.ai/x","https://file.302.ai:444/x","https://127.0.0.1/x","https://[::1]/x","https://localhost/x","https://u:p@file.302.ai/x","https://file.302.ai/x#fragment","file:///C:/private"]{assert!(image_download_url(address).is_err(),"{address}");}
 }
 #[test]fn image_download_is_bounded_and_sends_no_gateway_credentials(){
  for(status,body,extra,ok)in [("200 OK","binary-fixture","",true),("302 Found","","Location: http://127.0.0.1:1/private\r\n",false),("403 Forbidden","private-provider-detail","",false),("200 OK","","",false)]{
   let(profile,worker)=response(status,body,extra);
   let url=url::Url::parse(&(profile.base_url+"/result.png?signature=fixture")).unwrap();
   let result=download_image_url(&url);let request=worker.join().unwrap().to_ascii_lowercase();
   assert!(request.starts_with("get /result.png?signature=fixture http/1.1"));
   for header in ["authorization:","cookie:","referer:"]{assert!(!request.contains(header));}
   if ok{assert_eq!(result.unwrap(),body.as_bytes())}else{let error=result.unwrap_err();assert!(!error.contains("private-provider-detail"));assert!(!error.contains("signature"));}
  }
  let body="x".repeat(crate::resources::MAX_BYTES as usize+1);let(profile,worker)=response("200 OK",&body,"");
  assert_eq!(download_image_url(&url::Url::parse(&profile.base_url).unwrap()).unwrap_err(),"image download exceeds size limit");worker.join().unwrap();
 }
 #[test]fn downloaded_binary_decodes_and_invalid_image_is_rejected(){
  let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(7,5).write_to(&mut png,image::ImageFormat::Png).unwrap();
  for bytes in [png.into_inner(),b"not an image".to_vec()]{
   let(profile,worker)=response_bytes("200 OK",&bytes,"Content-Type: image/png\r\n");
   let downloaded=download_image_url(&url::Url::parse(&profile.base_url).unwrap()).unwrap();assert_eq!(downloaded,bytes);worker.join().unwrap();
   let decoded=crate::resources::validate_image(downloaded,"png");
   if bytes.starts_with(b"\x89PNG"){let image=decoded.unwrap();assert_eq!((image.width,image.height),(7,5));}else{assert!(decoded.is_err());}
  }
 }
}

#[cfg(test)]mod edit_transport_tests{
 use super::*;use std::io::{Read,Write};
 fn server(reply:Option<&str>)->(GatewayProfile,std::thread::JoinHandle<(String,Vec<u8>)>){
  let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();let reply=reply.map(str::to_owned);
  let worker=std::thread::spawn(move||{
   let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
   let mut headers=Vec::new();let mut byte=[0u8;1];while !headers.ends_with(b"\r\n\r\n"){stream.read_exact(&mut byte).unwrap();headers.push(byte[0]);assert!(headers.len()<16384);}
   let headers=String::from_utf8(headers).unwrap();let size=headers.lines().find_map(|line|line.to_ascii_lowercase().strip_prefix("content-length:").map(|v|v.trim().parse::<usize>().unwrap())).unwrap();assert!(size<1024*1024);
   let mut body=vec![0;size];stream.read_exact(&mut body).unwrap();
   if let Some(reply)=reply{stream.write_all(reply.as_bytes()).unwrap();}
   (headers,body)
  });
  (GatewayProfile{schema_version:1,profile_id:"fake-profile".into(),connection_id:"fake-connection".into(),account_id:"fake-account".into(),base_url:format!("http://127.0.0.1:{port}")},worker)
 }
 #[test]fn actual_multipart_roundtrip_and_lost_response_are_not_retried(){
  let policy=crate::image_protocol::ImageEditPolicy{model:"fake-image-model".into(),sizes:vec![],qualities:vec![],response_format:None};let mut params=crate::contracts::Parameters::new();params.insert("prompt".into(),crate::contracts::ParameterValue::Text("保留原图测试".into()));
  let input=[0,255,1,13,10,2];let body=crate::image_protocol::build_edit(&policy,&params,"image/png",&input,"quickgen-fake-boundary").unwrap();
  let json=r#"{"data":[{"b64_json":"AQID"}]}"#;let reply=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",json.len());
  for success in [true,false]{
   let(profile,worker)=server(if success{Some(&reply)}else{None});let result=submit_edit(&profile,Secret::from_bytes(b"fake-token".to_vec()),&body);
   let(headers,received)=worker.join().unwrap();assert!(headers.starts_with("POST /v1/images/edits HTTP/1.1"));assert!(headers.contains("Content-Type: multipart/form-data; boundary=quickgen-fake-boundary"));assert_eq!(received,body.bytes);assert!(received.windows(input.len()).any(|v|v==input));
   if success{assert!(matches!(result.unwrap(),crate::image_protocol::ImageOutput::Inline(bytes) if bytes==[1,2,3]));}
   else{let error=result.err().unwrap();assert!(error.acceptance_unknown);assert!(!error.message.contains("fake-token"));}
  }
 }
 #[test]fn invalid_configuration_fails_before_dispatch(){
  let profile=GatewayProfile{schema_version:1,profile_id:"p".into(),connection_id:"c".into(),account_id:"a".into(),base_url:"http://not-loopback.example".into()};
  let body=crate::image_protocol::EditBody{content_type:"multipart/form-data; boundary=fixture".into(),bytes:vec![1]};
  let failure=submit_edit(&profile,Secret::from_bytes(b"fake-token".to_vec()),&body).err().unwrap();assert!(!failure.acceptance_unknown);
 }
}
