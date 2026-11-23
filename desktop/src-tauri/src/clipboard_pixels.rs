//! CF_DIBV5 content, without a BMP file header. Input comes from the bounded decoder.
pub fn rgba_to_dibv5(width:u32,height:u32,rgba:&[u8])->Result<Vec<u8>,&'static str>{
 let pixels=(width as u64).checked_mul(height as u64).ok_or("size overflow")?;
 if width==0||height==0||width>16384||height>16384||pixels>32_000_000{return Err("image dimensions exceed budget")}
 let size=pixels.checked_mul(4).ok_or("size overflow")?;
 if size!=rgba.len() as u64{return Err("pixel buffer size mismatch")}
 let mut out=vec![0u8;124+rgba.len()];
 fn word(out:&mut[u8],offset:usize,value:u32){out[offset..offset+4].copy_from_slice(&value.to_le_bytes())}
 word(&mut out,0,124);word(&mut out,4,width);word(&mut out,8,(-(height as i32)) as u32);
 out[12..14].copy_from_slice(&1u16.to_le_bytes());out[14..16].copy_from_slice(&32u16.to_le_bytes());
 word(&mut out,16,3);word(&mut out,20,size as u32);
 word(&mut out,40,0x00ff0000);word(&mut out,44,0x0000ff00);word(&mut out,48,0x000000ff);word(&mut out,52,0xff000000);
 word(&mut out,56,0x73524742);word(&mut out,108,4);
 for (src,dst) in rgba.chunks_exact(4).zip(out[124..].chunks_exact_mut(4)){dst.copy_from_slice(&[src[2],src[1],src[0],src[3]])}
 Ok(out)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn preserves_rgba_alpha_and_top_down_rows(){
  let pixels=[255,0,0,255,0,255,0,128,0,0,255,0,12,34,56,78];let data=rgba_to_dibv5(2,2,&pixels).unwrap();
  assert_eq!(data.len(),140);assert_eq!(&data[8..12],&(-2i32).to_le_bytes());assert_eq!(&data[124..],&[0,0,255,255,0,255,0,128,255,0,0,0,56,34,12,78]);
  assert_eq!(&data[40..44],&0x00ff0000u32.to_le_bytes());assert_eq!(&data[52..56],&0xff000000u32.to_le_bytes());
 }
 #[test]fn rejects_zero_overflow_and_mismatched_buffers(){
  for (w,h,bytes) in [(0,1,vec![]),(u32::MAX,u32::MAX,vec![]),(16384,16384,vec![]),(1,1,vec![0;3]),(1,1,vec![0;5])]{assert!(rgba_to_dibv5(w,h,&bytes).is_err())}
 }
}
