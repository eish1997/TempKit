//! OpenAI-compatible image edit framing. Pure protocol code, never sends requests.
//! A model-specific verified policy must be supplied by the capability binding.
use crate::contracts::{Parameters,ParameterValue};
use base64::Engine;
use serde::{Serialize,Deserialize};
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageEditPolicy{pub model:String,pub sizes:Vec<String>,pub qualities:Vec<String>,pub response_format:Option<String>}
pub struct EditBody{pub content_type:String,pub bytes:Vec<u8>}
pub enum ImageOutput{Inline(Vec<u8>),Remote(String)}
fn scalar(value:&str)->bool{!value.is_empty()&&value.len()<=256&&!value.chars().any(char::is_control)}
impl ImageEditPolicy{
 pub fn validate(&self)->Result<(),String>{
  if !scalar(&self.model)||self.sizes.iter().chain(self.qualities.iter()).any(|v|!scalar(v)){return Err("invalid image policy".into())}
  if !matches!(self.response_format.as_deref(),None|Some("b64_json")|Some("url")){return Err("unsupported image response format".into())}Ok(())
 }
}
pub fn validate_parameters(policy:&ImageEditPolicy,parameters:&Parameters)->Result<(),String>{edit_fields(policy,parameters).map(|_|())}
fn edit_fields<'a>(policy:&'a ImageEditPolicy,parameters:&'a Parameters)->Result<Vec<(&'static str,&'a str)>,String>{
 policy.validate()?;
 if parameters.keys().any(|key|!matches!(key.as_str(),"prompt"|"size"|"quality")){return Err("unsupported image parameter".into())}
 let prompt=match parameters.get("prompt"){Some(ParameterValue::Text(value)) if !value.trim().is_empty()&&value.len()<=32000=>value,_=>return Err("image prompt is required or exceeds limit".into())};
 let mut fields=vec![("model",policy.model.as_str()),("prompt",prompt.as_str()),("n","1")];
 for(key,allowed)in [("size",&policy.sizes),("quality",&policy.qualities)]{
  if let Some(value)=parameters.get(key){match value{ParameterValue::Text(value) if allowed.contains(value)=>fields.push((key,value)),_=>return Err(format!("unsupported {key} for this model policy"))}}
 }
 if let Some(format)=&policy.response_format{fields.push(("response_format",format))}
 Ok(fields)
}
pub fn build_edit(policy:&ImageEditPolicy,parameters:&Parameters,mime:&str,image:&[u8],boundary:&str)->Result<EditBody,String>{
 policy.validate()?;
 if boundary.len()<16||boundary.len()>70||!boundary.bytes().all(|v|v.is_ascii_alphanumeric()||v==b'-'){return Err("invalid multipart boundary".into())}
 if image.is_empty()||image.len() as u64>crate::resources::MAX_BYTES{return Err("image input exceeds limit".into())}
 let extension=match mime{"image/png"=>"png","image/jpeg"=>"jpg","image/webp"=>"webp",_=>return Err("unsupported input image MIME".into())};
 let fields=edit_fields(policy,parameters)?;
 if image.windows(boundary.len()).any(|part|part==boundary.as_bytes())||fields.iter().any(|(_,value)|value.contains(boundary)){return Err("multipart boundary collides with content".into())}
 let mut body=Vec::with_capacity(image.len()+fields.iter().map(|(_,value)|value.len()).sum::<usize>()+4096);
 for(key,value)in fields{body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}\r\n").as_bytes());}
 body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"input.{extension}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
 body.extend_from_slice(image);body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
 Ok(EditBody{content_type:format!("multipart/form-data; boundary={boundary}"),bytes:body})
}
/// Remote references are untrusted descriptors, not authorization to fetch.
/// G2 must apply download origin/redirect/size rules before requesting any URL.
pub fn parse_output(bytes:&[u8])->Result<ImageOutput,String>{
 const MAX_RESPONSE:usize=28*1024*1024;
 if bytes.len()>MAX_RESPONSE{return Err("image response exceeds limit".into())}
 let value:serde_json::Value=serde_json::from_slice(bytes).map_err(|_|"invalid image response JSON")?;
 if value.get("error").is_some(){return Err("image provider returned an error".into())}
 let data=value.get("data").and_then(|v|v.as_array()).ok_or("image response has no data")?;
 if data.len()!=1{return Err("image response must contain exactly one output".into())}
 let entry=&data[0];let inline=entry.get("b64_json").and_then(|v|v.as_str()).filter(|v|!v.is_empty());let remote=entry.get("url").and_then(|v|v.as_str()).filter(|v|!v.is_empty());
 // Gateways may provide both representations. Inline bytes take precedence;
 // an invalid nonempty inline value remains an error, never a URL fallback.
 match(inline,remote){
  (Some(encoded),_)=>{let decoded=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_|"invalid base64 image output")?;if decoded.is_empty()||decoded.len() as u64>crate::resources::MAX_BYTES{return Err("image output exceeds limit".into())}Ok(ImageOutput::Inline(decoded))},
  (None,Some(remote))=>{if remote.len()>8192{return Err("output URL exceeds limit".into())}let url=url::Url::parse(remote).map_err(|_|"invalid output URL")?;
   if url.scheme()!="https"||url.host().is_none()||!url.username().is_empty()||url.password().is_some()||url.fragment().is_some(){return Err("unsafe output URL".into())}Ok(ImageOutput::Remote(remote.into()))},
  _=>Err("image output is absent or ambiguous".into())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn gateway_dual_representation_uses_inline_without_fetching_url(){
  // Real New API/302 response shape: both representations are supplied.
  let value=br#"{"data":[{"b64_json":"AQID","url":"https://file.302.ai/result.png","revised_prompt":null}]}"#;
  assert!(matches!(parse_output(value).unwrap(),ImageOutput::Inline(v) if v==[1,2,3]));
 }
 #[test]fn optional_representations_preserve_validation(){
  for url in ["https://example.test/result.png","file:///unused",""]{
   let value=serde_json::json!({"data":[{"b64_json":"AQID","url":url}]});
   assert!(matches!(parse_output(&serde_json::to_vec(&value).unwrap()).unwrap(),ImageOutput::Inline(v) if v==[1,2,3]));
  }
  for inline in [serde_json::Value::Null,serde_json::json!("")]{
   let value=serde_json::json!({"data":[{"b64_json":inline,"url":"https://example.test/result.png"}]});
   assert!(matches!(parse_output(&serde_json::to_vec(&value).unwrap()).unwrap(),ImageOutput::Remote(_)));
  }
  for value in [r#"{"data":[{"b64_json":"","url":""}]}"#,r#"{"data":[{"b64_json":"?","url":"https://example.test/x"}]}"#,r#"{"data":[{"b64_json":"","url":"file:///secret"}]}"#]{assert!(parse_output(value.as_bytes()).is_err());}
 }
 fn policy()->ImageEditPolicy{ImageEditPolicy{model:"explicit-fixture-model".into(),sizes:vec!["1024x1024".into()],qualities:vec!["standard".into()],response_format:None}}
 #[test]fn multipart_preserves_binary_and_never_invents_model_options(){
  let mut params=Parameters::new();params.insert("prompt".into(),ParameterValue::Text("测试\r\nsecond line".into()));let bytes=[0,255,13,10,1];
  let body=build_edit(&policy(),&params,"image/png",&bytes,"quickgen-fixture-boundary").unwrap();assert!(body.bytes.windows(bytes.len()).any(|part|part==bytes));let text=String::from_utf8_lossy(&body.bytes);assert!(text.contains("explicit-fixture-model"));assert!(text.contains("测试\r\nsecond line"));assert!(!text.contains("response_format"));assert!(!text.contains("name=\"quality\""));
  params.insert("size".into(),ParameterValue::Text("unknown".into()));assert!(build_edit(&policy(),&params,"image/png",&bytes,"quickgen-fixture-boundary").is_err());params.remove("size");
  params.insert("n".into(),ParameterValue::Integer(2));assert!(build_edit(&policy(),&params,"image/png",&bytes,"quickgen-fixture-boundary").is_err());params.remove("n");
  assert!(build_edit(&policy(),&params,"image/png",b"quickgen-fixture-boundary","quickgen-fixture-boundary").is_err());
 }
 #[test]fn outputs_are_bounded_unambiguous_and_errors_do_not_echo_body(){
  assert!(matches!(parse_output(br#"{"data":[{"b64_json":"AQID"}]}"#).unwrap(),ImageOutput::Inline(v) if v==[1,2,3]));
  assert!(matches!(parse_output(br#"{"data":[{"url":"https://example.test/result.png"}]}"#).unwrap(),ImageOutput::Remote(_)));
  for value in [r#"{"data":[]}"#,r#"{"data":[{},{}]}"#,r#"{"data":[{"url":"file:///secret"}]}"#,r#"{"data":[{"url":"https://u:p@example.test/x"}]}"#,r#"{"data":[{"url":"https://example.test/x","b64_json":"?"}]}"#,r#"{"data":[{"b64_json":"?"}]}"#]{assert!(parse_output(value.as_bytes()).is_err());}
  assert_eq!(parse_output(br#"{"error":{"message":"private detail"}}"#).err().unwrap(),"image provider returned an error");
 }
}
