//! Logical desktop geometry. No browser/screen assumptions or physical-pixel mixing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge { Top, Left, Right }
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect { pub x:f64, pub y:f64, pub width:f64, pub height:f64 }
pub fn clamp_note(area:Rect,note:Rect)->Rect{
 let width=note.width.max(160.).min(area.width.max(1.));
 let height=note.height.max(120.).min(area.height.max(1.));
 Rect{x:note.x.max(area.x).min(area.x+area.width-width),y:note.y.max(area.y).min(area.y+area.height-height),width,height}
}
#[cfg(test)]mod note_bounds_tests{
 use super::*;
 #[test]fn disconnected_screen_and_oversized_notes_return_to_work_area(){
  let area=Rect{x:-1280.,y:40.,width:1280.,height:680.};
  let placed=clamp_note(area,Rect{x:9000.,y:-5000.,width:5000.,height:900.});
  assert_eq!(placed,area);
  assert_eq!(clamp_note(area,Rect{x:-1000.,y:80.,width:280.,height:240.}),Rect{x:-1000.,y:80.,width:280.,height:240.});
 }
}
#[derive(Clone, Copy, Debug)]
pub struct Placement { pub panel:Rect, pub entry:Rect }
fn bound(value:f64, low:f64, high:f64)->f64 { value.max(low).min(high.max(low)) }

pub fn place(area:Rect, edge:Edge, fraction:f64, width:f64, height:f64)->Placement {
    let width=bound(width,120.0_f64.min(area.width),area.width);
    let height=bound(height,120.0_f64.min(area.height),area.height);
    let fraction=bound(fraction,0.0,1.0);
    let (ew,eh):(f64,f64)=if edge==Edge::Top {(88.0,18.0)} else {(18.0,88.0)};
    let (ew,eh)=(ew.min(area.width),eh.min(area.height));
    let entry=match edge {
        Edge::Top=>Rect{x:area.x+bound(area.width*fraction-ew/2.0,0.0,area.width-ew),y:area.y,width:ew,height:eh},
        Edge::Left=>Rect{x:area.x,y:area.y+bound(area.height*fraction-eh/2.0,0.0,area.height-eh),width:ew,height:eh},
        Edge::Right=>Rect{x:area.x+area.width-ew,y:area.y+bound(area.height*fraction-eh/2.0,0.0,area.height-eh),width:ew,height:eh},
    };
    let panel=match edge {
        Edge::Top=>Rect{x:bound(entry.x+ew/2.0-width/2.0,area.x,area.x+area.width-width),y:area.y,width,height},
        Edge::Left=>Rect{x:area.x,y:bound(entry.y+eh/2.0-height/2.0,area.y,area.y+area.height-height),width,height},
        Edge::Right=>Rect{x:area.x+area.width-width,y:bound(entry.y+eh/2.0-height/2.0,area.y,area.y+area.height-height),width,height},
    };
    Placement{panel,entry}
}

/// Adjacent utility window: right, left, below, above; overlap only as a last resort.
pub fn adjacent(area:Rect, main:Rect, width:f64, height:f64)->Rect {
    let width=width.min(area.width);let height=height.min(area.height);let gap=12.;
    let right=area.x+area.width;let bottom=area.y+area.height;
    let aligned_x=bound(main.x,area.x,right-width);
    let aligned_y=bound(main.y,area.y,bottom-height);
    let candidates=[
        Rect{x:main.x+main.width+gap,y:aligned_y,width,height},
        Rect{x:main.x-width-gap,y:aligned_y,width,height},
        Rect{x:aligned_x,y:main.y+main.height+gap,width,height},
        Rect{x:aligned_x,y:main.y-height-gap,width,height},
    ];
    if let Some(r)=candidates.iter().find(|r|r.x>=area.x&&r.y>=area.y&&r.x+width<=right&&r.y+height<=bottom){return *r}
    // Keep the nearest side placement when no non-overlapping side fits.
    let r=candidates[0];Rect{x:bound(r.x,area.x,right-width),y:aligned_y,width,height}
}

#[cfg(test)]
pub fn interpolate(from:Rect,to:Rect,t:f64)->Rect {
    let t=bound(t,0.0,1.0);
    Rect{x:from.x+(to.x-from.x)*t,y:from.y+(to.y-from.y)*t,width:from.width+(to.width-from.width)*t,height:from.height+(to.height-from.height)*t}
}

#[cfg(test)] mod tests {
 use super::*;
 fn inside(a:Rect,b:Rect)->bool {b.x>=a.x&&b.y>=a.y&&b.x+b.width<=a.x+a.width&&b.y+b.height<=a.y+a.height}
 #[test] fn corners_and_negative_monitor_origins_stay_visible(){
  for area in [Rect{x:0.,y:0.,width:1920.,height:1040.},Rect{x:-1280.,y:-200.,width:1280.,height:720.}] {
   for edge in [Edge::Top,Edge::Left,Edge::Right] {for f in [-1.,0.,0.5,1.,2.] {
    let p=place(area,edge,f,280.,220.);assert!(inside(area,p.panel));assert!(inside(area,p.entry));
    for t in [0.,0.25,0.5,0.75,1.] {assert!(inside(area,interpolate(p.entry,p.panel,t)))}
   }}
  }
 }
 #[test] fn single_column_width_is_allowed(){let p=place(Rect{x:0.,y:0.,width:800.,height:600.},Edge::Right,0.5,120.,120.);assert_eq!(p.panel.width,120.);assert_eq!(p.panel.x,680.);}
 #[test] fn adjacent_prefers_right_then_left(){let a=Rect{x:0.,y:0.,width:1200.,height:900.};for (x,expected) in [(100.,392.),(900.,580.)]{let r=adjacent(a,Rect{x,y:100.,width:280.,height:220.},308.,400.);assert_eq!(r.x,expected);assert_eq!(r.y,100.);assert!(inside(a,r));}}
 #[test] fn adjacent_uses_below_then_above_when_sides_do_not_fit(){let a=Rect{x:0.,y:0.,width:600.,height:900.};for (y,expected) in [(0.,232.),(680.,268.)]{let r=adjacent(a,Rect{x:160.,y,width:280.,height:220.},308.,400.);assert_eq!(r.y,expected);assert_eq!(r.x,160.);assert!(inside(a,r));}}
 #[test] fn adjacent_small_negative_monitor_stays_visible(){let a=Rect{x:-300.,y:-200.,width:300.,height:250.};assert_eq!(adjacent(a,a,308.,400.),a);}
 #[test] fn huge_panel_clamps_to_monitor(){let a=Rect{x:0.,y:0.,width:800.,height:600.};assert_eq!(place(a,Edge::Top,0.5,5000.,5000.).panel,a);}
 #[test] fn reverse_morph_starts_at_current_rectangle(){let a=Rect{x:96.,y:0.,width:88.,height:18.};let b=Rect{x:0.,y:0.,width:280.,height:220.};let current=interpolate(a,b,0.4);assert_eq!(interpolate(current,a,0.),current);assert_eq!(interpolate(current,a,1.),a);}
}
