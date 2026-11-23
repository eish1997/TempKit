//! Isolated CLI probe for the same production decoder protocol; never accesses the clipboard.
fn main(){
 if quickgen_desktop::decode_worker::run_if_requested(){return}
 let pixels=vec![255,0,0,255,0,255,0,128,0,0,255,0,12,34,56,78];
 let image=image::RgbaImage::from_raw(2,2,pixels.clone()).unwrap();
 let mut encoded=std::io::Cursor::new(Vec::new());
 image::DynamicImage::ImageRgba8(image).write_to(&mut encoded,image::ImageFormat::Png).unwrap();
 let dib=quickgen_desktop::decode_worker::clipboard_dib(encoded.get_ref(),"png").expect("real decoder child failed");
 assert_eq!(dib,quickgen_desktop::clipboard_pixels::rgba_to_dibv5(2,2,&pixels).unwrap());
 assert!(quickgen_desktop::decode_worker::clipboard_dib(encoded.get_ref(),"jpg").is_err());
 assert!(quickgen_desktop::decode_worker::clipboard_dib(&[0;20],"png").is_err());
 println!("{{\"ok\":true,\"actualChildRoundtrip\":true,\"rgbaAndAlphaExact\":true,\"invalidInputsRejected\":true,\"systemClipboardWritten\":false}}");
}
