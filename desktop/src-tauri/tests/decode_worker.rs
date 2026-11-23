//! Exercises the actual GUI-subsystem executable through redirected pipes.
use std::{io::{Cursor,Write},process::{Command,Stdio}};
fn call(bytes:&[u8],extension:&str)->serde_json::Value {
    let mut command=Command::new(env!("CARGO_BIN_EXE_quickgen-desktop"));
    command.args(["--quickgen-decode",extension]);
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output=child.wait_with_output().unwrap();
    assert!(output.status.success(),"{:?}",output);
    let text=String::from_utf8(output.stdout).unwrap();
    serde_json::from_str(text.trim().strip_prefix("QG_DECODE:").expect("helper must return without opening desktop")).unwrap()
}
#[test] fn production_helper_decodes_three_formats_and_rejects_damage() {
    for (format,ext) in [(image::ImageFormat::Png,"png"),(image::ImageFormat::Jpeg,"jpg"),(image::ImageFormat::WebP,"webp")] {
        let mut bytes=Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4,3).write_to(&mut bytes,format).unwrap();
        let reply=call(bytes.get_ref(),ext);
        assert_eq!(reply["Ok"][0],4); assert_eq!(reply["Ok"][1],3);
    }
    assert!(call(b"damaged","png")["Err"].is_string());
}
