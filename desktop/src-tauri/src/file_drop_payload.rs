//! CF_HDROP wire payload for an existing, protected export copy.
//! This builds bytes only: the OLE owner must allocate HGLOBAL and retain files.
use std::path::Path;

pub fn file_drop_payload(paths: &[&Path]) -> Result<Vec<u8>, &'static str> {
    if paths.is_empty() || paths.len() > 32 { return Err("invalid export count"); }
    let mut names = Vec::<u16>::new();
    for path in paths {
        if !path.is_absolute() { return Err("export path must be absolute"); }
        #[cfg(windows)]
        let units: Vec<u16> = {
            use std::os::windows::ffi::OsStrExt;
            path.as_os_str().encode_wide().collect()
        };
        #[cfg(not(windows))]
        let units: Vec<u16> = path.to_str().ok_or("invalid export path")?.encode_utf16().collect();
        if units.is_empty() || units.contains(&0) || units.len() > 32767 { return Err("invalid export path"); }
        names.extend(units); names.push(0);
    }
    names.push(0);
    // DROPFILES: offset DWORD, POINT (two LONGs), fNC BOOL, fWide BOOL.
    let mut bytes = Vec::with_capacity(20 + names.len() * 2);
    for field in [20u32, 0, 0, 0, 1] { bytes.extend(field.to_le_bytes()); }
    for unit in names { bytes.extend(unit.to_le_bytes()); }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_absolute_files_and_double_terminator() {
        let first = std::env::temp_dir().join("测试图.png");
        let second = std::env::temp_dir().join("output.webp");
        let bytes = file_drop_payload(&[&first,&second]).unwrap();
        assert_eq!(&bytes[..4], &20u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        let units: Vec<u16> = bytes[20..].chunks_exact(2).map(|v|u16::from_le_bytes([v[0],v[1]])).collect();
        let text=String::from_utf16(&units).unwrap();
        assert_eq!(text,format!("{}\0{}\0\0",first.display(),second.display()));
    }
    #[test]
    fn reject_empty_relative_nul_and_excess_count() {
        assert!(file_drop_payload(&[]).is_err());
        assert!(file_drop_payload(&[Path::new("relative.png")]).is_err());
        let invalid=std::env::temp_dir().join("bad\0.png");
        assert!(file_drop_payload(&[&invalid]).is_err());
        let path=std::env::temp_dir().join("one.png");
        assert!(file_drop_payload(&vec![path.as_path();33]).is_err());
    }
}
