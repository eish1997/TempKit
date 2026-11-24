//! Native ink region ownership and priority above other QuickGen windows.
use windows::Win32::{Foundation::HWND,Graphics::Gdi::*,UI::WindowsAndMessaging::*};

pub struct Region(pub HRGN);
impl Drop for Region{fn drop(&mut self){unsafe{let _=DeleteObject(HGDIOBJ(self.0.0));}}}
impl Region{
 pub fn rect(left:i32,top:i32,right:i32,bottom:i32)->Result<Self,String>{let r=unsafe{CreateRectRgn(left,top,right,bottom)};if r.0.is_null(){Err("cannot allocate ink region".into())}else{Ok(Self(r))}}
 pub fn combine(&self,other:&Self,mode:RGN_COMBINE_MODE)->Result<(),String>{if unsafe{CombineRgn(Some(self.0),Some(if mode==RGN_COPY{other.0}else{self.0}),Some(other.0),mode)}.0==0{Err("cannot combine ink region".into())}else{Ok(())}}
 pub fn offset(&self,x:i32,y:i32)->Result<(),String>{if unsafe{OffsetRgn(self.0,x,y)}.0==0{Err("cannot offset ink region".into())}else{Ok(())}}
}

/// Reassert the topmost band only when another visible QuickGen window is above
/// this surface. Never activate it or continuously reorder unrelated applications.
pub fn above_owned(hwnd:HWND,owned:&[HWND])->Result<bool,String>{
 let mut current=unsafe{GetWindow(hwnd,GW_HWNDPREV)}.ok();let mut seen=std::collections::HashSet::new();
 while let Some(window)=current{if !seen.insert(window.0 as usize)||seen.len()>4096{return Err("native window order changed".into())}if owned.contains(&window)&&unsafe{IsWindowVisible(window)}.as_bool()&&!unsafe{IsIconic(window)}.as_bool(){return Ok(false)}current=unsafe{GetWindow(window,GW_HWNDPREV)}.ok();}Ok(true)
}
pub fn ensure_above(hwnd:HWND,owned:&[HWND])->Result<(),String>{if unsafe{GetWindowLongW(hwnd,GWL_EXSTYLE)}as u32&WS_EX_TOPMOST.0==0||!above_owned(hwnd,owned)?{unsafe{SetWindowPos(hwnd,Some(HWND_TOPMOST),0,0,0,0,SWP_NOACTIVATE|SWP_NOMOVE|SWP_NOSIZE)}.map_err(|e|e.to_string())?;}Ok(())}
#[cfg(test)]mod tests{use super::*;
 #[test]fn copy_region_uses_source_shape_and_offsets_negative_monitor(){let shape=Region::rect(32,0,300,240).unwrap();let copy=Region::rect(0,0,0,0).unwrap();copy.combine(&shape,RGN_COPY).unwrap();assert!(unsafe{EqualRgn(shape.0,copy.0)}.as_bool());copy.offset(-1920,40).unwrap();assert!(unsafe{PtInRegion(copy.0,-1800,100)}.as_bool());assert!(!unsafe{PtInRegion(copy.0,-1910,100)}.as_bool());let screen=Region::rect(-1920,40,0,1080).unwrap();screen.combine(&copy,RGN_DIFF).unwrap();assert!(!unsafe{PtInRegion(screen.0,-1800,100)}.as_bool());assert!(unsafe{PtInRegion(screen.0,-1910,100)}.as_bool());}
}
