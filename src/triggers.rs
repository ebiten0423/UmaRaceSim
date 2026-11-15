use crate::{course::Course,rng::RaceRandom};
#[derive(Clone,Debug)]
pub struct Trigger {
    pub name:String, pub op:String, pub rhs:i32, pub windows:Vec<(f32,f32)>,
    pub randomized:bool, pub lot:Option<bool>, pub previous:bool,
    pub prev_count:i32, pub this_count:i32,
}
pub type Condition=Vec<Vec<Trigger>>;
fn lot_concat(r:&mut RaceRandom,cands:&[(f32,f32)])->Option<(f32,f32)> {
    if cands.is_empty() {return None;}
    let merged:Vec<(f32,f32)>;
    let cands=if crate::envc::var("JPRACE_LOT_MERGE").as_deref()==Ok("1") {
        let mut m:Vec<(f32,f32)>=vec![];
        for &(a,b) in cands {if let Some(l)=m.last_mut() {if l.1==a {l.1=b;continue;}} m.push((a,b));}
        merged=m;&merged[..]} else {cands};
    let total:f32=cands.iter().map(|(a,b)|b-a).sum();
    let mut x=r.float_range(0.0,total);
    r.float_max(1.0);
    for &(a,b) in cands {
        if x<b-a {
            if crate::envc::var_os("JPRACE_DEBUG_LOT").is_some() {eprintln!("lotc a={} b={} x={} a+x={} total={}",a,b,x,a+x,total);}
            let st=(a+x).min(b-crate::envc::var("JPRACE_LOT_TAIL").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(0.0));
            let end=if crate::envc::var("JPRACE_LOT_CLIP").as_deref()==Ok("1") {(st+10.0).min(b)} else {st+10.0};
            return Some((st,end));}
        x-=b-a;
    }
    None
}
fn lot(r:&mut RaceRandom,lo:f32,hi:f32)->(f32,f32) {
    let x=r.float_range(lo,(hi-10.0).max(lo));
    if crate::envc::var_os("JPRACE_DEBUG_LOT").is_some() {eprintln!("lot lo={} hi={} x={}",lo,hi,x);}
    (x,x+10.0)
}
pub fn build(text:&str,c:&Course,r:&mut RaceRandom)->Condition {
    if text.is_empty() {return vec![];}
    let mut cond=text.split('@').map(|g|g.split('&').map(|token| {
        let parsed=["==","!=",">=","<=",">","<"].iter().find_map(|op|token.split_once(op).map(|(n,v)|(n,*op,v)));
        let (n,op,rhs)=parsed.unwrap_or(("invalid","==","0"));
        let v=rhs.parse().unwrap_or(0);
        let before=r.calls();
        let mut t=Trigger {name:n.into(),op:op.into(),rhs:v,windows:vec![],randomized:false,lot:None,previous:false,prev_count:0,this_count:0};
        match n {
            "corner_random"=>{t.randomized=true;if let Some(s)=c.corner(v) {t.windows.push(lot(r,s.start,s.end));}},
            "is_finalcorner_random"=>{t.randomized=true;if let Some(s)=c.final_corner() {t.windows.push(lot(r,s.start,s.end));}},
            "straight_random"|"while_in_straight_random"=>{
                t.randomized=true;
                if !c.straights.is_empty() {
                    let i=r.int_max(c.straights.len() as i32) as usize;
                    let s=&c.straights[i];t.windows.push(if n=="while_in_straight_random" {(s.start,s.end)} else {lot(r,s.start,s.end)});
                }
            },
            "last_straight_random"=>{
                t.randomized=true;
                if crate::envc::var_os("JPRACE_LASTSTRAIGHT_OLD").is_none() && let Some(ls)=&c.last_straight {
                    t.windows=vec![lot(r,ls.start,ls.end)];
                } else if let Some(fc)=c.final_corner() {
                    for s in c.straights.iter().rev().filter(|s|s.start>=fc.start) {
                        t.windows=vec![lot(r,s.start,s.end)];
                    }
                }
            },
            "phase_random"|"phase_firsthalf_random"|"phase_laterhalf_random"=>{
                t.randomized=true;let (mut lo,mut hi)=c.bounds(v);let half=(hi-lo)*0.5;
                if n=="phase_firsthalf_random" {hi=lo+half;}
                if n=="phase_laterhalf_random" {lo+=half;}
                t.windows.push(lot(r,lo,hi));
            },
            "phase_corner_random"=>{
                t.randomized=true;let (lo,hi)=c.bounds(v);
                let cands=c.corners.iter().filter(|s|s.end>=lo && s.start<=hi).map(|s|(s.start.max(lo),s.end.min(hi))).collect::<Vec<_>>();
                if let Some(w)=lot_concat(r,&cands) {t.windows.push(w);}
            },
            "phase_straight_random"|"phase_first_half_straight_random"|"phase_latter_half_straight_random"=>{
                t.randomized=true;let (plo,phi)=c.bounds(v);let len=phi-plo;
                let (lo,hi)=match n {"phase_first_half_straight_random"=>(plo,plo+len*0.5),"phase_latter_half_straight_random"=>(plo+len*0.5,phi),_=>(plo,phi)};
                let cands=c.straights.iter().filter(|s|s.end>=lo && s.start<=hi).map(|s|(s.start.max(lo),s.end.min(hi))).collect::<Vec<_>>();
                if let Some(w)=lot_concat(r,&cands) {t.windows.push(w);}
            },
            "phase_firstquarter_random"=>{
                t.randomized=true;let (lo,hi)=c.bounds(v);
                t.windows.push(lot(r,lo,lo+(hi-lo)*0.25));
            },
            "run_at_full_speed_random"=>{t.randomized=true;},
            "distance_rate_after_random"=>{t.randomized=true;t.windows.push(lot(r,v as f32/100.0*c.distance,c.distance));},
            "up_slope_random"|"down_slope_random"|"up_slope_random_later_half"|"down_slope_random_later_half"=>{
                t.randomized=true;let sign=if n.starts_with("up") {1} else {-1};
                let later=n.ends_with("_later_half");
                let mut slopes=c.slopes.iter().filter(|s|s.number==sign && (!later || s.start>=c.distance*0.5)).collect::<Vec<_>>();
                slopes.sort_by(|a,b|a.start.total_cmp(&b.start));
                if !slopes.is_empty() {
                    let i=r.int_max(slopes.len() as i32) as usize;
                    t.windows.push(lot(r,slopes[i].start,slopes[i].end));
                }
            },
            "all_corner_random"=>{
                t.randomized=true;let mut candidates=c.corners.iter().map(|s|(s.start,s.end)).collect::<Vec<_>>();
                let ac_max=match crate::envc::var("JPRACE_AC_MAX").as_deref() {Ok("corners")=>c.corners.len(),Ok(x)=>x.parse::<usize>().unwrap_or(4),Err(_)=>4};
                for _ in 0..ac_max {
                    if candidates.is_empty() {break;}
                    let c0=r.calls();
                    let i=r.int_range(0,candidates.len() as i32) as usize;
                    let pick=candidates[i];let w=lot(r,pick.0,pick.1);t.windows.push(w);
                    if crate::envc::var_os("JPRACE_DEBUG_AC").is_some() {eprintln!("ac call={} cands={:?} i={} pick={:?} w={:?} rest={}",c0+1,candidates,i,pick,w,pick.1-w.1);}
                    let variant=crate::envc::var("JPRACE_AC").ok().and_then(|v|v.parse::<i32>().ok()).unwrap_or(1);
                    let threshold=if pick.1-w.1>=10.0 {candidates[i].0=w.1;w.1}
                        else {candidates.remove(i);if variant==5 {w.1} else {pick.0}};
                    match variant {
                        2=>{candidates.retain(|s|s.1>threshold);for s in &mut candidates {s.0=s.0.max(threshold);}
                            candidates.retain(|s|s.1-s.0>=10.0);},
                        3=>(),
                        4=>candidates.retain(|s|s.1>=threshold),
                        _=>candidates.retain(|s|s.0>=threshold),
                    }
                }
                t.windows.sort_by(|a,b|a.0.total_cmp(&b.0));
            },
            "random_lot"=>t.lot=Some(r.int_max(100)<v),
            _=>(),
        }
        if crate::envc::var("JPRACE_DEBUG_TOK").is_ok() && r.calls()!=before {eprintln!("tok {} {} {:?} before={}",token,r.calls()-before,t.windows,before);}
        t
    }).collect::<Vec<_>>()).collect::<Condition>();
    if crate::envc::var("JPRACE_SEPARATE_BRANCH_RANDOM").is_err() {
        for gi in 1..cond.len() {
            for ti in 0..cond[gi].len() {
                if !cond[gi][ti].randomized {continue;}
                let (name,rhs)=(cond[gi][ti].name.clone(),cond[gi][ti].rhs);
                if let Some(w)=cond[..gi].iter().flatten().find(|t|t.randomized && t.name==name && t.rhs==rhs).map(|t|t.windows.clone()) {
                    cond[gi][ti].windows=w;
                }
            }
        }
    }
    cond
}
pub fn compare(lhs:f32,op:&str,rhs:f32)->bool {
    match op {"=="=>lhs==rhs,"!="=>lhs!=rhs,">="=>lhs>=rhs,"<="=>lhs<=rhs,">"=>lhs>rhs,"<"=>lhs<rhs,_=>false}
}
pub fn set_run_at_full_speed_windows(c:&mut Condition,r:&mut RaceRandom,lo:f32,hi:f32)->usize {
    let mut n=0;
    for group in c.iter_mut() {for t in group.iter_mut() {if t.name=="run_at_full_speed_random" && t.windows.is_empty() {
        let c0=r.calls()+1;
        let w=match crate::envc::var("JPRACE_RAFSWIN").as_deref() {
            Ok("pre")=>{r.float_max(1.0);lot(r,lo,hi)},
            Ok("post")=>{let w=lot(r,lo,hi);r.float_max(1.0);w},
            Ok("concat")=>lot_concat(r,&[(lo,hi)]).unwrap_or((lo,lo+10.0)),
            Ok("concatpre")=>{r.float_max(1.0);lot_concat(r,&[(lo,hi)]).unwrap_or((lo,lo+10.0))},
            _=>lot(r,lo,hi)};
        if crate::envc::var_os("JPRACE_LOG_RAFSWIN").is_some() {eprintln!("rafswin call={} lo={:.6} hi={:.3} x={:.6}",c0,lo,hi,w.0);}
        t.windows.push(w);n+=1;}}}
    n
}
pub fn draw_run_at_full_speed_u(c:&mut Condition,r:&mut RaceRandom)->usize {
    let mut n=0;
    for group in c.iter_mut() {for t in group.iter_mut() {if t.name=="run_at_full_speed_random" && t.windows.is_empty() {
        let d=r.float_range(0.0,1.0);t.windows.push((d,f32::NAN));n+=1;}}}
    n
}
pub fn finish_run_at_full_speed_windows(c:&mut Condition,lo:f32,hi:f32)->usize {
    let mut n=0;
    for group in c.iter_mut() {for t in group.iter_mut() {if t.name=="run_at_full_speed_random" {
        for w in t.windows.iter_mut() {if w.1.is_nan() {
            let top=(hi-10.0).max(lo);let mut x=w.0*(top-lo).abs()+lo;if x<lo {x=lo;} else if top<x {x=top;}
            if crate::envc::var_os("JPRACE_LOG_RAFSWIN").is_some() {eprintln!("rafswin2 lo={:.6} hi={:.3} d={:.6} x={:.6}",lo,hi,w.0,x);}
            *w=(x,x+10.0);n+=1;}}}}}
    n
}
pub fn evaluate(c:&mut Condition,distance:f32,query:&impl Fn(&str,i32)->Option<f32>)->bool {
    let mut any=false;
    for group in c {
        let mut all=true;
        for t in group {
            let stateful=matches!(t.name.as_str(),"is_activate_any_skill"|"is_activate_heal_skill"|"is_last_straight_onetime");
            if (!all || any) && !stateful {continue;}
            let ok=if t.randomized {t.windows.iter().any(|(a,b)|*a<=distance && distance<=*b)}
                else if let Some(lot)=t.lot {lot}
                else if let Some(v)=query(&t.name,t.rhs) {
                    if t.name=="is_activate_any_skill" || t.name=="is_activate_heal_skill" {
                        let adjusted=v as i32-t.this_count;let prev=t.prev_count;t.prev_count=adjusted;
                        if prev<adjusted {t.this_count+=1;}
                        compare(i32::from(prev<adjusted) as f32,&t.op,t.rhs as f32)
                    } else if t.name=="is_last_straight_onetime" {
                        let ok=!t.previous && v!=0.0;
                        t.previous|=v!=0.0;ok
                    } else {compare(v,&t.op,t.rhs as f32)}
                }
                else {false};
            all &= ok;
        } any |= all;
    } any
}
