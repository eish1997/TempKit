use std::{marker::PhantomData, rc::Rc};
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_NOREPEAT, VK_OEM_3};

// Registration belongs to the creating thread. Never move this guard across threads.
pub struct ModeHotkey { id: i32, _thread: PhantomData<Rc<()>> }
impl ModeHotkey {
    pub fn register(id: i32) -> Result<Self, String> {
        unsafe { RegisterHotKey(None, id, MOD_ALT | MOD_NOREPEAT, VK_OEM_3.0 as u32) }
            .map_err(|e| format!("Alt＋· 注册失败，可能已被其他程序占用：{e}"))?;
        Ok(Self { id, _thread: PhantomData })
    }
}
impl Drop for ModeHotkey {
    fn drop(&mut self) { let _ = unsafe { UnregisterHotKey(None, self.id) }; }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayArea { pub x: i32, pub y: i32, pub width: u32, pub height: u32, pub scale: f64 }
#[derive(Debug, PartialEq)]
pub struct PanelBounds { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
pub fn panel_bounds(area: DisplayArea) -> Result<PanelBounds, String> {
    if area.width == 0 || area.height == 0 || !area.scale.is_finite() || area.scale <= 0. {
        return Err("Invalid display bounds".into());
    }
    let width = ((240. * area.scale).round() as u32).min(area.width).max(1);
    let height = ((150. * area.scale).round() as u32).min(area.height).max(1);
    let right_gap = (24. * area.scale).round() as u32;
    let top = ((100. * area.scale).round() as u32).min(area.height - height);
    Ok(PanelBounds { x: area.x.saturating_add(area.width.saturating_sub(width + right_gap) as i32), y: area.y.saturating_add(top as i32), width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_remains_inside_negative_origin_and_small_work_areas() {
        for area in [
            DisplayArea { x: -1920, y: -300, width: 1920, height: 1080, scale: 1.5 },
            DisplayArea { x: 2560, y: 0, width: 3840, height: 2160, scale: 2. },
            DisplayArea { x: 0, y: 0, width: 200, height: 120, scale: 2. },
        ] {
            let p = panel_bounds(area).unwrap();
            assert!(p.x >= area.x && p.y >= area.y);
            assert!(p.x as i64 + p.width as i64 <= area.x as i64 + area.width as i64);
            assert!(p.y as i64 + p.height as i64 <= area.y as i64 + area.height as i64);
        }
    }
    #[test]
    fn invalid_display_is_rejected() {
        for scale in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(panel_bounds(DisplayArea { x: 0, y: 0, width: 100, height: 100, scale }).is_err());
        }
    }
    #[test]
    fn native_hotkey_conflict_and_release() {
        // Real Windows registration, no key injection or system setting changes.
        let first = ModeHotkey::register(0x5147).expect("Close the probe before this test; Alt+Backquote must be available");
        assert!(std::thread::spawn(|| ModeHotkey::register(0x5148).is_err()).join().unwrap(), "another thread must not steal the hotkey");
        drop(first);
        assert!(std::thread::spawn(|| ModeHotkey::register(0x5149).is_ok()).join().unwrap(), "a released hotkey must be available again");
    }
}
