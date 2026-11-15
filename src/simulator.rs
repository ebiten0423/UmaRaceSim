use crate::math::PowX;
use std::collections::HashMap;
use serde_json::{Value,json};
use crate::{data::{Assets,Master,RaceIn,HorseIn,Tree,ival,sval,f32_of},course::Course,geometry::Geometry,
    math,rng::RaceRandom,modifiers::{Modifiers,Operation},triggers::{self,Condition},netsort};

pub const DT:f32=0.0666;
const PHASE_NAMES:[&str;4]=["Start","Middle","End","Last"];
const STYLE_NAMES:[&str;4]=["Nige","Senko","Sashi","Oikomi"];
fn lane_near(a:f32,b:f32,which:usize)->bool {
    let v=crate::envc::var("JPRACE_LANE_EPS").unwrap_or_else(|_|"1e-6:1e-6".into());
    match v.split(':').nth(which).and_then(|x|x.parse::<f32>().ok()) {Some(e)=>(a-b).abs()<e,None=>math::approximately(a,b)}
}
fn ro_fi_sched(hi:usize,tick:i64)->Option<bool> {
    let x=crate::envc::var("JPRACE_RO_FISCHED").ok()?;let mut listed=false;
    for e in x.split(',') {
        let Some((a,b))=e.split_once(':') else {continue};
        if a!="*" && a.parse::<usize>().ok()!=Some(hi) {continue;}
        listed=true;
        if let Some((t0,t1))=b.split_once('-') && let (Ok(t0),Ok(t1))=(t0.parse::<i64>(),t1.parse::<i64>()) && t0<=tick && tick<=t1 {return Some(true);}
    }
    if listed {Some(false)} else {None}
}
fn chain_cond(row:&HashMap<String,Value>)->bool {
    let pre=crate::envc::var("JPRACE_CHAIN_PRE").as_deref()!=Ok("0");
    (1..=2).any(|n|sval(row,&format!("condition_{n}")).contains("is_activate_other_skill_detail")
        || pre && sval(row,&format!("precondition_{n}")).contains("is_activate_other_skill_detail"))
}
#[derive(Clone)]
struct Detail {
    pre:Condition, condition:Condition, pre_passed:bool, active:bool,
    left:f32, cooldown:f32, uid:u64, number:usize, additional_seen:i32, additional_used:[i32;3], orderup_seen:i32, time_ext_used:i32,
}
#[derive(Clone)]
struct Skill { id:i32, level:i32, row:HashMap<String,Value>, enabled:bool, details:Vec<Detail>, cooldown:f32 }
#[derive(Clone,Copy)]
struct Around { i:usize, gap:f32, gap_abs:f32, lane:f32, lane_abs:f32, speed_gap:f32 }
#[derive(Clone)]
struct Horse {
    input:HorseIn, base:[f32;5], adjusted:[f32;5], mods:Modifiers, skills:Vec<Skill>,
    order_continue:[[bool;9];2],
    distance:f32, prev_distance:f32, lane:f32, prev_lane:f32, lane_max:f32, target_lane:f32, lane_speed:f32,
    position:[f32;3], pos64:[f64;3], speed:f32, cf_speed:f32, cf_hp:f32, cf_pos:[f32;2], self_speed:f32, min_speed:f32, hp:f32, max_hp:f32,
    delay:f32, original_delay:f32, phase:usize, start_dash:bool, finished:bool,
    order:usize, prev_order:usize, ou_cnt:i32, guts_order:usize, wiz_order:usize,
    front:Option<usize>, inside:Option<usize>, outside:Option<usize>, near_count:usize,
    front_time:f32, side_time:f32, near_front_time:f32, near_behind_time:f32,
    near_behind_set:Vec<f32>, surround:[bool;3], reserved:Vec<i32>,
    corner:i32, corner_end:f32, final_corner:bool, straight:bool, in_last_straight:bool, front_type:i32,
    slope:f32, slope_end:f32, called_event:i32,
    stat_max:f32, speed_max:f32, keep_prev_dbg:i32, cbs_next:f32, sl_next:f32, jp_next:[i64;2],jp_keep_next:i64,jp_keep_thr:f32,jp_sec:i32,
    force_in:f32, ex:bool, style_rate:f32, dist_speed:f32, dist_power:f32, ground_coef:f32,
    section_speed:[f32;24], section_base:[f32;24], section_base_n:[f32;24], section_base_f:[f32;24], temptation_section:i32, temptation_mode:i32, temptation_active:bool,
    temptation_count:i32, temptation_timer:f32, temptation_check:f32, temptation_forced:bool, temptation_end:u8,
    downhill:bool, downhill_check:f32, spurt_result:i32, spurt_start:f32, spurt_speed:f32, spurt_ev:bool, rc_start:f32, j_start:f32,
    spurt_calculated:bool, skill_time:f32, target_speed:f32, after_goal:f32, extra_lane:Option<f32>, force_lane:bool, raw_time:f32, scaled_time:f32,
    around:Vec<Around>, visible:Vec<usize>, in_space:f32, out_space:f32, overtake:Vec<usize>, ot_cool:f32, move_time:f32, move_dir:i32,
    order_up:[i32;8], order_up_cool:Vec<f32>, ot_target_time:f32, ot_noup_time:f32, ot_noup_order:usize, fight_count:i32, lane_move:f32,
    hp_empty:bool, block_front_acc:f32, apt_distance:i32, apt_ground:i32,
    activate_count:i32, activate_phase:[i32;4], activate_half:[i32;2], heal_count:i32, keep:crate::position_keep::Keep, cp:Conserve,
    used_skills:Vec<i32>, jp_cbs_rate:f32, jp_cbs_near:bool, jp_cbs_preset:Option<f32>,
    rafs_on:bool, rafs_t0:f32, rafs_draw:bool, speed_unc:f32, fs_j:f32, fs_x:f32, fs_x64:f64, fs_n:u32, fs_tacc:[f32;2], goal_tgt:f32, prev_mv:[f32;2], prev_side:(Option<usize>,Option<usize>), prev_front:Option<usize>, hp0_seen:bool, hp0_t:f32, fs_e:f32, fs_lj:f32, fs_r:f32, fs_pred:u32, fs_pending:bool, fs_thr:f32, sens:[f32;2], fs_cap:f32,
    slb_u:Option<[f64;2]>, hp_prev0:bool, hp_t0:bool, slb_re:bool, hp_chg:f32, slb_on:bool, slb_pending:bool, slb_v:f32, slb_thr:f32, slb_t0:f32, slb_cmp:f32, spv0:f32, mode_add:f32, tgt_nohp:f32, m6_used:f32, m6_n:usize, tgt_base:f32,
    adv_tick:i64, adv_prev:i64, keep_heal:i32, side_max_mid:f32, lane_draw_t:f32, jp_keep_next2:i64, jp_keep_next3:i64, jp_cbs_u1:f32,
    adv_kinds:Vec<(i32,i64)>,
    lane_off:f32,
    lane_min:f32,
    lane_off_ro:bool,
    fi_latch:std::cell::Cell<bool>, fi_in:bool,
    ro_base:f32,
    slope_clear_tick:i64,
}
#[derive(Clone,Default)]
struct Conserve { top:bool, pow:f32, released:bool, accel:f32, left:f32, spec:f32, ctimer:f32, last_credit:f32, s_tm:bool, s_tp:bool, lag_mode:i32, pre_mode:i32 }
fn pm_ex_nige()->bool {crate::envc::var("JPRACE_PM_EX").as_deref()!=Ok("ex")}
impl Horse {
    fn tempted(&self)->bool {self.temptation_mode!=0}
    fn section_rand_dbg(&self)->f32 {0.0}
    fn stat(&self,i:usize)->f32 {
        let mx=if self.stat_max!=2000.0 && !stat_max_idx(i) {2000.0} else {self.stat_max};
        let mx=if i==0 {mx.max(self.speed_max)} else {mx};
        self.mods.apply(i,self.adjusted[i]).clamp(1.0,mx)}
    fn ex(&self)->bool {self.ex || self.mods.apply(20,0.0)!=0.0}
    fn lane_disp(&self)->i32 {let l=if self.lane_off>0.0 {self.lane_off-(1.0/18.0+self.lane)} else {self.lane};if lane_disp_f64() {(l as f64*10000.0) as i32} else {(l*10000.0) as i32}}
    fn frame(&self)->Value {
        json!({"distance":self.distance,
        "lane":self.lane_disp(),
        "speed":(self.speed*100.0) as i32,
        "hp":self.hp as i32,"tempt":self.temptation_mode,
        "block":self.front.map_or(-1,|i|i as i32)}) }
}
pub struct Simulator<'a> {
    input:RaceIn, master:&'a Master, assets:&'a Assets, course:Course, geometry:Geometry,
    horses:Vec<Horse>, random:RaceRandom, time:f32, prev_time:f32, events:Vec<Value>,
    oracle:Vec<(usize,i64,f32,f32)>, rafs_oracle:Vec<(usize,f32)>, minor_ev:Vec<(usize,i64,f32,f32)>,
    distance_diff_max:f32, compete:crate::compete::State, pacemaker:usize, pm_ready:bool, pm_section:i32, pm_not_forward:i32, pm_handed:bool, pm_samepick:bool, cur_skill:i32, jp_entry_logged:bool, jp_spurt11:bool, jp_entry_drawn:bool, jp_entry_tick:i64, jp_pkforce_done:bool, pm_pending:Option<usize>, pm_wait:Option<(i32,i32)>, pm_fwd_from:Option<usize>, pm_follow:bool, pm_back_tick:i64, pm_hand_tick:i64, pm_trk:usize, pm_trk_tick:i64, pm_sec_tick:i64, pm_fchg:usize, pm_fchg_tick:i64, pm_back1_tick:i64, pm_hand1_tick:i64,
}
impl<'a> Simulator<'a> {
    pub fn new(mut input:RaceIn,master:&'a Master,assets:&'a Assets)->Result<Self,String> {
        if input.horses.is_empty() || input.horses.len()>18 {return Err("race requires 1..18 runners".into());}
        if input.seed<i32::MIN as i64 || input.seed>i32::MAX as i64 {return Err("seed exceeds int32".into());}
        if !matches!(input.seed_mode.as_str(),"logged"|"recovered") {return Err("unknown seed mode".into());}
        if input.seed_mode=="logged" && input.seed!=input.logged_seed {return Err("logged seed was substituted".into());}
        if !(1..=4).contains(&input.ground_condition) {return Err("ground condition outside 1..4".into());}
        input.horses.sort_by_key(|h|h.frame_order);
        for (i,h) in input.horses.iter().enumerate() {
            if h.frame_order!=i as i32+1 || !(1..=4).contains(&h.running_style) {
                return Err("runners require contiguous gates and running styles 1..4".into());
            }
        }
        let course=Course::load(assets,input.course_set)?;
        let geometry=Geometry::load(assets,&course)?;
        let mut s=Self {random:RaceRandom::new(input.seed as i32),input,master,assets,course,geometry,
            horses:vec![],time:0.0,prev_time:0.0,events:vec![],oracle:load_oracle(),rafs_oracle:load_rafs_oracle(),minor_ev:vec![],distance_diff_max:0.0,compete:crate::compete::State::new(0),pacemaker:0,pm_ready:false,pm_section:0,pm_not_forward:0,pm_handed:false,cur_skill:0,jp_entry_logged:false,jp_spurt11:false,jp_entry_drawn:false,jp_entry_tick:-1,jp_pkforce_done:false,pm_pending:None,pm_wait:None,pm_fwd_from:None,pm_follow:false,pm_back_tick:-1,pm_hand_tick:-1,pm_trk:usize::MAX,pm_trk_tick:-1,pm_sec_tick:-1,pm_fchg:usize::MAX,pm_fchg_tick:-1,pm_back1_tick:-1,pm_hand1_tick:-1,pm_samepick:false};
        s.initialize()?;Ok(s)
    }
    fn p(&self)->Tree<'_> {self.assets.params()}
    fn build_details(&mut self,hi:usize,id:i32,row:&std::collections::HashMap<String,Value>)->Vec<Detail> {
        let mut details=vec![];
        let shared_lot=(1..=2).any(|d|sval(row,&format!("condition_{d}")).contains("random_lot_shared")).then(||self.random.int_max(100));
        for d in 1..=2 {
            let text=sval(row,&format!("condition_{d}"));if text.is_empty() {continue;}
            let c0=self.random.calls();
            self.random.wide=!f32m('W');
            let pre=triggers::build(sval(row,&format!("precondition_{d}")),&self.course,&mut self.random);
            let mut condition=triggers::build(text,&self.course,&mut self.random);
            if let Some(x)=shared_lot {for g in &mut condition {for t in g.iter_mut().filter(|t|t.name=="random_lot_shared") {t.lot=Some(x<t.rhs);}}}
            self.random.wide=false;
            if crate::envc::var("JPRACE_DEBUG_DETAIL").is_ok() {eprintln!("detail {} {} {} {}",hi,id,d,self.random.calls()-c0);}
            details.push(Detail {pre_passed:pre.is_empty(),pre,condition,active:false,left:0.0,cooldown:0.0,
                uid:((hi as u64)<<48)|((id as u64)<<8)|d as u64,number:d,additional_seen:0,additional_used:[0;3],orderup_seen:0,time_ext_used:0});
        }
        if details.len()==2 && crate::envc::var("JPRACE_SEPARATE_BRANCH_RANDOM").is_err() {
            let first:Vec<_>=details[0].condition.iter().flatten().filter(|t|t.randomized).map(|t|(t.name.clone(),t.rhs,t.windows.clone())).collect();
            for t in details[1].condition.iter_mut().flatten().filter(|t|t.randomized) {
                if let Some((_,_,w))=first.iter().find(|(n,r,_)|*n==t.name && *r==t.rhs) {t.windows=w.clone();}
            }
        }
        details
    }
    fn race_base(&self)->f32 {
        let p=self.p();
        p.f("raceBaseSpeedBaseSpeed")-(self.course.distance-p.f("raceBaseSpeedBaseDistance"))/p.f("raceBaseSpeedDistanceRate")
    }
    fn rate(&self,table:&str,id:i32,column:&str)->Result<f32,String> {
        let r=self.master.find(table,"id",id as i64).ok_or_else(||format!("missing {table}:{id}"))?;
        Ok(if f32m('r') {ival(r,column) as f32/10000.0} else {ival(r,column) as f32*0.0001})
    }
    fn initialize(&mut self)->Result<(),String> {
        let p=self.assets.params();let c=&self.course;
        let track=self.master.find("race_track","id",c.master["race_track_id"].as_i64().unwrap()).ok_or("race track")?;
        let half=self.input.horses.len()<=9 && ival(track,"enable_half_gate")==1;
        let stat_max=2000.0;
        let stat_max_final=crate::envc::var("JPRACE_STAT_MAX").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(stat_max);
        let mut horses=vec![];
        for (i,h) in self.input.horses.iter().enumerate() {
            let mood=self.rate("race_motivation_rate",h.motivation,"motivation_rate")?;
            let bonus=if h.team_rank==0 {0} else {self.master.rows("race_single_mode_team_status").iter()
                .find(|r|(h.team_rank as i64)<ival(r,"team_rank_threshold")).map_or(0,|r|ival(r,"add_status"))};
            let base=[h.speed,h.stamina,h.pow,h.guts,h.wiz].map(|v|{
                let raw=if v>1200 {1200.0+(v-1200) as f32*0.5} else {v as f32};
                ((bonus as f32+raw)*mood).clamp(1.0,stat_max)
            });
            let style=h.running_style as usize-1;
            let style_apt=[h.proper_running_style_nige,h.proper_running_style_senko,h.proper_running_style_sashi,h.proper_running_style_oikomi][style];
            let distance_apt=if c.distance<=1400.0 {h.proper_distance_short} else if c.distance<=1800.0 {h.proper_distance_mile}
                else if c.distance<=2400.0 {h.proper_distance_middle} else {h.proper_distance_long};
            let ground_apt=if c.ground==1 {h.proper_ground_turf} else {h.proper_ground_dirt};
            let mut course_coef=1.0;
            let sid=c.master["course_set_status_id"].as_i64().unwrap();
            if sid!=0 {
                let row=self.master.find("race_course_set_status","course_set_status_id",sid).ok_or("course stat adjust")?;
                let targets=[ival(row,"target_status_1"),ival(row,"target_status_2")];let count=targets.iter().filter(|t|**t!=0).count() as f32;
                for t in targets.into_iter().filter(|t|*t!=0) {
                    let coef=p.arr("CourseSetAdjust.CoefAndThresholdArray").iter()
                        .find(|r|r["StatusThreshold"].as_i64().unwrap() as f32>=base[t as usize-1]).map_or(0.0,|r|f32_of(&r["Coef"]));
                    course_coef+=coef/count;
                }
            }
            let ground=Tree(&p.arr(if c.ground==1 {"_groundModifierParamTurf"} else {"_groundModifierParamDirt"})[self.input.ground_condition as usize-1]);
            let mode=matches!(self.input.race_type,6|7);
            let add=|name:&str|if mode {p.f(&format!("Global.SingleModeAdd{name}"))} else {0.0};
            let adjusted=[
                adjusted_stat(base[0],course_coef,ground.f("addSpeed"),add("Speed"),stat_max),
                adjusted_stat(base[1],1.0,0.0,add("Stamina"),stat_max),
                adjusted_stat(base[2],1.0,ground.f("addPower"),add("Pow"),stat_max),
                adjusted_stat(base[3],1.0,0.0,add("Guts"),stat_max),
                adjusted_stat(base[4],self.rate("race_proper_runningstyle_rate",style_apt,"proper_rate")?,0.0,add("Wiz"),stat_max),
            ];
            let speed_unc={let raw=if h.speed>1200 {1200.0+(h.speed-1200) as f32*0.5} else {h.speed as f32};adjusted_stat((bonus as f32+raw)*mood,course_coef,ground.f("addSpeed"),add("Speed"),1e9)};
            let lane=initial_lane(i as i32,ival(track,"initial_lane_type"),half);
            let ro=c.master["run_outside"].as_i64()==Some(1) && crate::envc::var("JPRACE_RUN_OUTSIDE").as_deref()!=Ok("0");
            let n=self.input.horses.len() as i32;let lt=ival(track,"initial_lane_type");
            let mirror=crate::envc::var("JPRACE_RUN_OUTSIDE").as_deref()==Ok("mirror");
            let ro_lane_old=crate::envc::var("JPRACE_RO_LANE").as_deref()==Ok("old");
            let abs_lane=|g:i32|if ro_lane_old {c.lane_max-(1.0/18.0+initial_lane(n-1-g,lt,half))} else {
                let k=n-1-g;(c.lane_max-(k+1) as f32*(1.0/18.0))-initial_lane_extra(k,lt,half)};
            let lane=if ro {if mirror {initial_lane(n-1-i as i32,lt,half)} else {abs_lane(i as i32)}} else {lane};
            let lane_off=if ro && mirror {c.lane_max} else {0.0f32};
            let lane_min=if ro && !mirror && crate::envc::var("JPRACE_RUN_OUTSIDE_MIN").as_deref()==Ok("1") {abs_lane(0)} else {0.0f32};
            let lane_max_h=c.lane_max;
            let ex=h.running_style==1 && h.skill_array.iter().any(|sk|self.master.find("skill_data","id",sk.skill_id as i64)
                .is_some_and(|r|(1..=2).any(|d|(1..=3).any(|a|ival(r,&format!("ability_type_{d}_{a}"))==6))));
            let raw=h.pow as f32;
            let cp_pow=if raw>1200.0 {(1200.0+(raw-1200.0)*0.5)*mood+(raw-1200.0)*0.5} else {raw*mood};
            horses.push(Horse {input:h.clone(),base,adjusted,mods:Modifiers::default(),skills:vec![],distance:0.0,prev_distance:0.0,
                order_continue:[[true;9];2],lane,prev_lane:lane,lane_max:lane_max_h,target_lane:lane,lane_speed:0.0,position:self.geometry.world(0.0,lane),pos64:self.geometry.world64(0.0,lane),speed:p.f("StartSpeed"),
                self_speed:p.f("StartSpeed"),min_speed:p.f("StartSpeed"),hp:0.0,max_hp:0.0,delay:0.0,original_delay:0.0,
                keep_prev_dbg:0,cbs_next:-1.0,sl_next:-1.0,jp_next:[-1,-1],jp_keep_next:-1,jp_keep_thr:0.0,ou_cnt:0,jp_sec:-1,phase:0,start_dash:true,finished:false,order:i,prev_order:i,ot_target_time:0.0,ot_noup_time:0.0,ot_noup_order:i,fight_count:0,cf_speed:0.0,cf_hp:0.0,cf_pos:[0.0;2],lane_move:0.0,hp_empty:false,block_front_acc:0.0,apt_distance:distance_apt,apt_ground:ground_apt,order_up:[0;8],order_up_cool:vec![0.0;self.input.horses.len()],guts_order:0,wiz_order:0,
                front:None,inside:None,outside:None,near_count:0,around:vec![],visible:vec![],in_space:0.0,out_space:0.0,overtake:vec![],ot_cool:0.0,move_time:0.0,move_dir:0,front_time:0.0,side_time:0.0,near_front_time:0.0,near_behind_time:0.0,near_behind_set:vec![],surround:[false;3],reserved:vec![],
                corner:0,corner_end:0.0,final_corner:false,straight:false,in_last_straight:false,front_type:0,
                slope:if crate::envc::var("JPRACE_SLOPE0").as_deref()!=Ok("1") {0.0} else {c.slopes.iter().find(|s|s.start<=0.0 && 0.0<s.end).map_or(0.0,|s|s.per)},
                slope_end:if crate::envc::var("JPRACE_SLOPE0").as_deref()!=Ok("1") {0.0} else {c.slopes.iter().find(|s|s.start<=0.0 && 0.0<s.end).map_or(0.0,|s|s.end)},called_event:-1,
                stat_max:if crate::envc::var("JPRACE_STAT_MAX_H").ok().is_none_or(|l|l.split(',').any(|x|x.parse::<usize>().ok()==Some(i)))
                    && !(crate::envc::var_os("JPRACE_STAT_MAX_NORAFS").is_some() && h.speed>=2001) {stat_max_final} else {stat_max},
                speed_max:if h.speed<2001 && crate::envc::var("JPRACE_SPEED_CAP").as_deref()!=Ok("all") {f32::MAX} else {stat_max},
                force_in:0.0,ex,style_rate:self.rate("race_proper_runningstyle_rate",style_apt,"proper_rate")?,dist_speed:self.rate("race_proper_distance_rate",distance_apt,"proper_rate_speed")?,
                dist_power:self.rate("race_proper_distance_rate",distance_apt,"proper_rate_power")?,
                ground_coef:self.rate("race_proper_ground_rate",ground_apt,"proper_rate")?,section_speed:[0.0;24],section_base:[0.0;24],section_base_n:[0.0;24],section_base_f:[0.0;24],
                temptation_section:0,temptation_mode:0,temptation_active:false,temptation_count:0,temptation_timer:0.0,temptation_check:0.0,temptation_forced:false,temptation_end:0,
                downhill:false,downhill_check:0.0,spurt_result:8,spurt_start:-1.0,spurt_speed:0.0,spurt_ev:false,rc_start:-1.0,j_start:-1.0,spurt_calculated:false,skill_time:-1.0,target_speed:0.0,after_goal:0.0,extra_lane:None,force_lane:false,
                raw_time:-1.0,scaled_time:-1.0,activate_count:0,activate_phase:[0;4],activate_half:[0;2],heal_count:0,used_skills:vec![],jp_cbs_rate:1.0,jp_cbs_near:false,jp_cbs_preset:None,rafs_on:false,rafs_t0:0.0,rafs_draw:false,speed_unc,fs_j:0.0,fs_x:0.0,fs_x64:0.0,fs_n:0,fs_tacc:[-1.0,-1.0],goal_tgt:-1.0,prev_mv:[-1.0,-1.0],prev_side:(None,None),prev_front:None,hp0_seen:false,hp0_t:-1.0,fs_e:-1.0,fs_lj:0.0,fs_r:0.0,fs_pred:0,fs_pending:false,fs_thr:0.0,sens:[0.0;2],fs_cap:-1.0,slb_u:None,hp_prev0:false,hp_t0:false,slb_re:false,hp_chg:0.0,slb_on:false,slb_pending:false,slb_v:0.0,slb_thr:0.0,slb_t0:-1.0,slb_cmp:-1.0,spv0:-1.0,mode_add:0.0,tgt_nohp:0.0,m6_used:0.0,m6_n:0,tgt_base:0.0,adv_tick:-100,adv_prev:-100,keep_heal:0,side_max_mid:0.0,lane_draw_t:-1.0,jp_keep_next2:-1,jp_keep_next3:-1,jp_cbs_u1:-1.0,adv_kinds:vec![],lane_off,lane_min,lane_off_ro:ro,fi_latch:std::cell::Cell::new(false),fi_in:false,ro_base:if ro {abs_lane(0)} else {0.0},slope_clear_tick:-100,keep:crate::position_keep::Keep::default(),cp:Conserve {pow:cp_pow,..Default::default()}});
        }
        self.horses=horses;
        self.compete=crate::compete::State::new(self.horses.len());
        self.pacemaker=self.horses.iter().enumerate().min_by_key(|(i,h)|
            (if h.ex() && !pm_ex_nige() {0} else {h.input.running_style},*i)).unwrap().0;
        let lot_k={let p=self.assets.params();(p.f("Skill.LotActivatePerVal1"),p.f("Skill.LotActivatePerVal2"),p.i("Skill.ActivatePerMin") as f32)};
        let lot_pass=move |r:&mut RaceRandom,row:&std::collections::HashMap<String,Value>,wiz:f32|ival(row,"activate_lot")!=1 || r.float_max(100.0)<
            (lot_k.0-lot_k.1/wiz).max(lot_k.2);
        for hi in 0..self.horses.len() {
            for sk in self.horses[hi].input.skill_array.clone() {
                let row=self.master.find("skill_data","id",sk.skill_id as i64).ok_or_else(||format!("missing skill {}",sk.skill_id))?.clone();
                let details=self.build_details(hi,sk.skill_id,&row);
                self.horses[hi].skills.push(Skill{id:sk.skill_id,level:sk.level,row,enabled:false,details,cooldown:0.0});
            }
        }
        if crate::envc::var("JPRACE_DEBUG").is_ok() {eprintln!("rng calls after build_details: {}",self.random.calls());}
        if let Ok(n)=crate::envc::var("JPRACE_ALIGN") {let n:u64=n.parse().unwrap();while self.random.calls()<n {self.random.raw();}}
        let p=self.assets.params();let range=p.f("forceInMoveTargetSpeedAddRandomRange");
        for h in self.horses.iter_mut() {
            let name=["RunningOut","Proceding","Insertion","Chasing"][h.input.running_style as usize-1];
            let v=p.f(&format!("forceInMoveTargetSpeedAdd{name}"));
            h.force_in=if math::approximately(v,0.0) {0.0} else {
                self.random.float_range(if v<=0.0 {-range} else {0.0},if v>0.0 {range} else {0.0})+v
            };
        }
        for (hi,h) in self.horses.iter_mut().enumerate() {
            for sk in &mut h.skills {
                sk.enabled=lot_pass(&mut self.random,&sk.row,h.base[4]);
                if crate::envc::var("JPRACE_DEBUG").is_ok() && ival(&sk.row,"activate_lot")==1 {eprintln!("lot {} {} {} {}",hi,sk.id,sk.enabled,h.base[4]);}
            }
        }
        if crate::envc::var("JPRACE_DEBUG").is_ok() {eprintln!("rng calls after force_in+lot: {}",self.random.calls());}
        self.skills_pass(0.0,1);self.skills_pass(0.0,0);
        if crate::envc::var("JPRACE_DEBUG").is_ok() {eprintln!("rng calls before delay: {}",self.random.calls());}
        if let Ok(n)=crate::envc::var("JPRACE_BURN_BEFORE_DELAY") {for _ in 0..n.parse::<u32>().unwrap() {self.random.raw();}}
        for hi in 0..self.horses.len() {
            let h=&self.horses[hi];
            let raw=h.input.stamina as f32;let halved=if raw>1200.0 {1200.0+(raw-1200.0)*0.5} else {raw};
            let sta=match crate::envc::var("JPRACE_SLB_STA").as_deref() {
                Ok("old")=>h.mods.apply(1,h.base[1]),
                Ok("nohalf")=>{let mood=self.rate("race_motivation_rate",h.input.motivation,"motivation_rate")?;
                    h.mods.apply(1,h.base[1]+(raw-halved)*mood)},
                _=>h.mods.apply(1,h.base[1]+(raw-halved))};
            if crate::envc::var_os("JPRACE_LOG_SLBSTA").is_some() {eprintln!("slbsta h={} raw={} mot={} base={:.2} cur={:.2} sta={:.2}",hi,h.input.stamina,h.input.motivation,h.base[1],h.mods.apply(1,h.base[1]),sta);}
            if sta>1200.0 {
                let a=self.random.raw() as f64/2147483647.0;let b=self.random.raw() as f64/2147483647.0;
                if crate::envc::var_os("JPRACE_LOG_SLB").is_some() {eprintln!("slbhorse h={} sta={:.1}",hi,h.mods.apply(1,h.base[1]));}
                self.horses[hi].slb_u=Some([a,b]);
            }
        }
        for h in self.horses.iter_mut() {
            let fixed=h.mods.consume(22,0.0);let scale=h.mods.consume(15,1.0);
            h.delay=(if fixed>0.0 {fixed} else {self.random.float_max(p.f("startDelayMax"))})*scale;
            h.original_delay=h.delay;
        }
        let race_base=self.race_base();
        for h in self.horses.iter_mut() {
            let style=if h.ex() {"OoNige"} else {STYLE_NAMES[h.input.running_style as usize-1]};
            h.max_hp=if f32m('x') {h.stat(1)*p.f("Hp.HpInitialVal1")*p.f(&format!("Hp.HpMaxCoef{style}"))+self.course.distance} else {(h.stat(1) as f64*p.f("Hp.HpInitialVal1") as f64*
                p.f(&format!("Hp.HpMaxCoef{style}")) as f64+self.course.distance as f64) as f32};
            h.hp=h.max_hp;
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()}
                else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            for i in 0..24 {
                let phase=self.course.phase(i as f32*(self.course.distance/24.0));
                let per=if phase<4 {p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[phase]))} else {0.0};
                let sbm=crate::envc::var("JPRACE_SEC_SB").ok().and_then(|x|x.parse::<i32>().ok()).unwrap_or(0);
                let mut add64=0.0f64;
                let add=if phase==2 || phase==3 {
                    let v=(h.stat(0)*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt();
                    let v64u=((h.stat(0)*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")) as f64).sqrt();
                    add64=if sbm==5 {v64u*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64} else {v as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64};
                    match sbm {3=>v*(h.dist_speed*p.f("addSpeedParamCoef")),4=>add64 as f32,_=>
                    if f32m('s') {v*h.dist_speed*p.f("addSpeedParamCoef")} else {(v as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64) as f32}}
                } else {0.0};
                let l=if f32m('L') {(h.stat(4)*0.1).log10()} else if f32m('M') {((h.stat(4)*0.1) as f64).log10() as f32} else {math::log10(h.stat(4)*0.1)};
                let (w,v2,v1,p1)=(h.stat(4) as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomMinusVal2") as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomMinusVal1") as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomPlusVal1") as f64);
                let lu=crate::envc::var("JPRACE_SEC_LU").unwrap_or_else(|_|if f32m('s') {if f32m('w') {"1".into()} else {"5".into()}} else {"0".into()});
                let (lo,up)=match lu.as_str() {
                    "1"=>{let wz=h.stat(4);(l*(wz/v2 as f32)+v1 as f32,l*(wz/p1 as f32))},
                    "2"=>{let l64=((h.stat(4)*0.1) as f64).log10();((l64*w/v2+v1) as f32,(l64*w/p1) as f32)},
                    "3"=>{let wz=h.stat(4);(l/v2 as f32*wz+v1 as f32,l/p1 as f32*wz)},
                    "4"=>{let wz=h.stat(4);(l*wz/v2 as f32+v1 as f32,l*(wz/p1 as f32))},
                    "5"=>{let wz=h.stat(4);(l*(wz/v2 as f32)+v1 as f32,l*wz/p1 as f32)},
                    "6"=>{let wz=h.stat(4);(((l*(wz/v2 as f32)) as f64+v1 as f64) as f32,l*(wz/p1 as f32))},
                    _=>if f32m('s') {(l*h.stat(4)/v2 as f32+v1 as f32,l*h.stat(4)/p1 as f32)} else {((l as f64*w/v2+v1) as f32,(l as f64*w/p1) as f32)}};
                let random=self.random.float_range_with(lo,up,if f32m('w') {crate::rng::Wide::Float} else {crate::rng::Wide::FloatDraw});
                let c=0.01f32 as f64;
                if i==0 && crate::envc::var_os("JPRACE_LOG_WIZ0").is_some() {eprintln!("wiz0 h={} wiz={:08x} l={:08x} lo={:08x} up={:08x}",h.input.frame_order-1,h.stat(4).to_bits(),l.to_bits(),lo.to_bits(),up.to_bits());}
                if crate::envc::var("JPRACE_LOG_SECRAW").ok().and_then(|x|x.parse::<i64>().ok())==Some(h.input.frame_order as i64-1) {
                    let hx=|x:f32|format!("{:08x}",x.to_bits());
                    eprintln!("secraw i={} per={} rb={} add={} random={} lo={} up={} wiz={} dd={:016x} l={}",i,hx(per),hx(race_base),hx(add),hx(random),hx(lo),hx(up),hx(h.stat(4)),self.random.last_dd.to_bits(),hx(l));
                }
                let sfma=crate::envc::var("JPRACE_SEC_FMA").ok().and_then(|x|x.parse::<i32>().ok()).unwrap_or(0);
                let ssum=crate::envc::var("JPRACE_SEC_SUM").ok();
                h.section_base_f[i]=(per*0.01).mul_add(race_base,add);
                if f32m('s') && (phase==2 || phase==3) && matches!(sbm,7|8) {
                    let plain=per*0.01*race_base+add;let fma=(per*0.01).mul_add(race_base,add);
                    h.section_base[i]=plain;
                    h.section_speed[i]=random*0.01*race_base+if sbm==7 {fma} else {plain};
                    h.section_base_n[i]=if sbm==8 {fma} else {plain};
                } else
                if f32m('s') && (phase==2 || phase==3) && matches!(sbm,1|2|5) {
                    h.section_base[i]=match sbm {1=>(per*0.01).mul_add(race_base,add),_=>(((per*0.01)*race_base) as f64+add64) as f32};
                    h.section_speed[i]=random*0.01*race_base+h.section_base[i];
                } else
                if let (true,Some(m))=(f32m('s'),ssum.as_deref()) {
                    h.section_base[i]=per*0.01*race_base+add;
                    let q=per+random;
                    h.section_speed[i]=match m {"A"=>q*0.01*race_base,"B"=>q*race_base*0.01,"C"=>q*race_base/100.0,"D"=>q/100.0*race_base,_=>q*0.01*race_base}+add;
                } else
                if f32m('s') && sfma>0 {
                    h.section_base[i]=match sfma {2|4=>(per*0.01).mul_add(race_base,add),_=>per*0.01*race_base+add};
                    h.section_speed[i]=match sfma {1|2=>(random*0.01).mul_add(race_base,h.section_base[i]),_=>(random as f64*0.01f32 as f64*race_base as f64+h.section_base[i] as f64) as f32};
                } else
                if f32m('s') {
                    h.section_base[i]=per*0.01*race_base+add;
                    h.section_speed[i]=random*0.01*race_base+h.section_base[i];
                } else {
                h.section_base[i]=(per as f64*c*race_base as f64+add as f64) as f32;
                h.section_speed[i]=(random as f64*c*race_base as f64) as f32+h.section_base[i];
                }
            }
            if let Ok(o)=crate::envc::var("JPRACE_SECSPD_ULP") {
                for e in o.split(',') {
                    let f:Vec<i64>=e.split(':').filter_map(|x|x.trim().parse().ok()).collect();
                    if f.len()==3 && f[0]==h.input.frame_order as i64-1 && (0..24).contains(&f[1]) {
                        let i=f[1] as usize;h.section_speed[i]=f32::from_bits(h.section_speed[i].to_bits().wrapping_add_signed(f[2] as i32));
                    }
                }
            }
            if !matches!(crate::envc::var("JPRACE_SEC_SB").as_deref(),Ok("7")|Ok("8")) {h.section_base_n=h.section_base;}
            if crate::envc::var("JPRACE_LOG_SECSPD").ok().and_then(|x|x.parse::<i64>().ok())==Some(h.input.frame_order as i64) {
                eprintln!("secspd frame_order={} wiz={} base={:?} speed={:?}",h.input.frame_order,h.stat(4),h.section_base,h.section_speed);
            }
            h.self_speed=h.speed;
        }
        for stat in [3,4] {
            let mut indices=(0..self.horses.len()).collect::<Vec<_>>();
            netsort::sort(&mut indices,&|a,b| {let d=self.horses[*b].stat(stat)-self.horses[*a].stat(stat);
                if math::approximately(self.horses[*a].stat(stat),self.horses[*b].stat(stat)) {*a as i32-*b as i32} else if d>0.0 {1} else {-1}});
            for (order,i) in indices.into_iter().enumerate() {if stat==3 {self.horses[i].guts_order=order;} else {self.horses[i].wiz_order=order;}}
        }
        for h in self.horses.iter_mut() {
            let chance=math::pow(p.f("Temptation.TemptationStartPerVal1")/math::log10(math::pow(h.stat(4)*0.1+1.0,2.0)),2.0);
            let chance=h.mods.apply(8,chance);
            let roll=self.random.int_max(100);
            if crate::envc::var_os("JPRACE_LOG_TEMPT").is_some() {eprintln!("temptlot wiz={:.1} chance={:.3} roll={} call={}",h.stat(4),chance,roll,self.random.calls());}
            if (roll as f32)<chance {
                h.temptation_section=self.random.int_range(p.i("Temptation.TemptationLotSectionMin"),p.i("Temptation.TemptationLotSectionMax")+1);
            }
        }
        Ok(())
    }
    fn query(&self,hi:usize,n:&str,rhs:i32)->Option<f32> {
        let h=&self.horses[hi];let c=&self.course;
        let boolean=|b:bool|if b {1.0} else {0.0};
        Some(match n {
            "always"=>1.0,"course_distance"=>c.distance,"ground_type"=>c.ground as f32,
            "track_id"=>c.master["race_track_id"].as_i64().unwrap() as f32,"rotation"=>c.turn as f32,
            "distance_type"=>if c.distance<=1400.0 {1.0} else if c.distance<=1800.0 {2.0} else if c.distance<=2400.0 {3.0} else {4.0},
            "is_basis_distance"=>boolean((c.distance as i32)%400<100),"ground_condition"=>self.input.ground_condition as f32,
            "weather"=>self.input.weather as f32,"season"=>self.input.season as f32,"motivation"=>h.input.motivation as f32,
            "running_style"=>h.input.running_style as f32,"post_number"=>post_number(hi,self.horses.len()) as f32,
            "popularity"=>((h.input.popularity-1).max(0)+1) as f32,"phase"=>if cond_phase_dist() {c.phase(h.distance) as f32} else {h.phase as f32},"corner"=>h.corner as f32,
            "is_finalcorner"=>boolean(h.final_corner),"is_finalcorner_laterhalf"=>boolean(h.final_corner && c.final_corner().is_some_and(|s|h.distance>=(s.start+s.end)*0.5)
                && (h.corner!=0 || crate::envc::var("JPRACE_FCLH").as_deref()==Ok("old"))),
            "remain_distance"=>c.distance-(h.distance as i32) as f32,"distance_rate"=>{
                let threshold=(rhs as f32/100.0*c.distance+0.5) as i32;
                return Some((h.distance as i32-threshold+rhs) as f32);
            },
            "distance_diff_top"=>{
                let gap=self.horses.iter().map(|h|h.distance).fold(0.0,f32::max)-h.distance;
                match crate::envc::var("JPRACE_DDT").as_deref() {
                    Ok("lane")=>{let top=self.horses.iter().max_by(|a,b|a.distance.total_cmp(&b.distance)).unwrap();(gap-(h.lane-top.lane).abs()).floor()},
                    Ok(x) if x.starts_with("off:")=>(gap-x[4..].parse::<f32>().unwrap_or(0.0)).floor(),
                    _=>gap.floor()}
            },
            "distance_diff_top_float"=>{
                let v=(self.horses.iter().map(|h|h.distance).fold(0.0,f32::max)-h.distance)*crate::envc::var("JPRACE_DDTF_SCALE").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(10.0);
                if crate::envc::var_os("JPRACE_DDTF_NOFLOOR").is_some() {v} else {v.floor()}
            },
            "furlong"=>match crate::envc::var("JPRACE_FURLONG").as_deref() {
                Ok("floor")=>(h.distance/200.0).floor(),
                Ok(x) if x.starts_with("off:")=>((h.distance+x[4..].parse::<f32>().unwrap_or(0.0))/200.0).floor(),
                _=>((h.distance.round() as i32)/200) as f32,
            },
            "near_infront_count"=>{let p=self.p();let (dt,lt)=(p.f("Near.DistanceThreshold"),p.f("Near.LaneDistanceThreshold"));
                let pred=crate::envc::var("JPRACE_NEAR_INFRONT").as_deref()==Ok("pred");
                self.horses.iter().filter(|o|{let g=o.distance-h.distance+if pred {(o.speed-h.speed)/15.0} else {0.0};g>0.0 && g<=dt && (o.lane-h.lane).abs()<=lt}).count() as f32},
            "is_tight_track"=>c.master["tight_track"].as_i64().unwrap_or(0) as f32,
            "is_last_straight"=>if crate::envc::var("JPRACE_LS_COND").as_deref()!=Ok("dist") {boolean(h.in_last_straight)} else {boolean(c.straights.iter().map(|s|s.start).reduce(f32::max).is_some_and(|st|h.distance>=st))},
            "phase_firsthalf"|"phase_laterhalf"=>{let ph=if cond_phase_dist() || crate::envc::var("JPRACE_COND_PHASE").as_deref()!=Ok("lag") {c.phase(h.distance)} else {h.phase};let (lo,hi)=c.bounds(ph as i32);let first=h.distance<(lo+hi)*0.5;
                if first==(n=="phase_firsthalf") {ph as f32} else {-1.0}},
            "phase_firstquarter"=>{let (lo,hi)=c.bounds(rhs);if h.distance>=lo && h.distance<=lo+(hi-lo)*0.25 {rhs as f32} else {-1.0}},
            "corner_count"=>c.corners.len() as f32,
            "succession_skill_count"=>h.input.skill_array.iter().filter(|s|if crate::envc::var_os("JPRACE_SUCCESSION_6DIGIT").is_some() {(900000..1000000).contains(&s.skill_id)}
                else {self.master.find("skill_data","id",s.skill_id as i64).is_some_and(|r|ival(r,"unique_skill_id_1")!=0)}).count() as f32,
            "is_used_skill_id"=>if h.used_skills.contains(&rhs) {rhs as f32} else {-1.0},
            "is_activate_other_skill_detail"=>boolean(h.used_skills.contains(&self.cur_skill)),
            "is_popularity_top_character_activate_advantage_skill"=>{
                let tick=(self.time/0.0666).round() as i64;
                let top=self.horses.iter().enumerate().filter(|(_,o)|o.input.popularity>0).min_by_key(|(_,o)|o.input.popularity).map(|(i,_)|i);
                let old=crate::envc::var_os("JPRACE_ADV_OLD").is_some();
                let fired=|t:usize|{let o=&self.horses[t];o.adv_tick==tick-1 || (!old && o.adv_tick==tick && o.adv_prev==tick-1)};
                match top {Some(t) if (t!=hi || !old) && fired(t)=>-1.0,Some(t) if t==hi=>1.0,_=>0.0}
            },
            "is_other_character_activate_advantage_skill"=>{
                if crate::envc::var("JPRACE_OTHER_ADV").as_deref()==Ok("0") {return None;}
                let tick=(self.time/0.0666).round() as i64;
                let team=h.input.team_id;
                if self.horses.iter().enumerate().any(|(i,o)|i!=hi && (team==0 || o.input.team_id!=team) && o.adv_kinds.iter().any(|&(k,t)|k==rhs && t==tick-1)) {rhs as f32} else {0.0}
            },
            "is_abroad"=>{
                if crate::envc::var("JPRACE_ABROAD").as_deref()==Ok("none") {return None;}
                let cs=self.master.find("race_course_set","id",self.input.course_set as i64)?;
                if ival(cs,"race_track_id")>=10200 {1.0} else {0.0}
            },
            "activate_count_param_skill"=>{
                let m=crate::envc::var("JPRACE_PARAM_SKILL").unwrap_or_default();
                if m=="none" {return None;}
                h.used_skills.iter().filter(|&&id|self.master.find("skill_data","id",id as i64).is_some_and(|r|
                    if m=="stat" {(1..=3).any(|a|(1..=5).contains(&ival(r,&format!("ability_type_1_{a}"))))} else {ival(r,"skill_category")==0})).count() as f32
            },
            "is_activate_heal_skill"=>h.heal_count as f32,
            "distance_diff_rate"=>{
                let first=self.horses.iter().find(|o|o.order==0)?.distance;
                let last=self.horses.iter().find(|o|o.order==self.horses.len()-1)?.distance;
                let spread=first-last;
                if math::approximately(spread,0.0) {0.0} else {((first-h.distance)/spread*100.0) as i32 as f32}
            },
            "accumulatetime"=>self.time.floor(),"time"=>ival(self.master.find("race_instance","id",self.input.race_instance_id as i64)?,"time") as f32,
            "grade"=>ival(self.master.find("race","id",self.input.race_id as i64)?,"grade") as f32,
            "is_dirtgrade"=>ival(self.master.find("race","id",self.input.race_id as i64)?,"is_dirtgrade") as f32,
            "order"=>if crate::envc::var("JPRACE_ORDER_TIE0").as_deref()!=Ok("0") && self.horses.iter().all(|o|o.distance==0.0) {1.0} else {h.order as f32+1.0},"order_rate"=>{
                let x=rhs as f32/100.0*self.horses.len() as f32;
                let threshold=match crate::envc::var("JPRACE_ORATE").as_deref() {Ok("floor")=>x as i32,Ok("ceil")=>x.ceil() as i32,_=>(x+0.5) as i32};
                let ord=if crate::envc::var("JPRACE_ORATE_TIE0").as_deref()!=Ok("0") && self.time==0.0 {0} else {h.order as i32};
                return Some((ord+1-threshold+rhs) as f32);
            },
            n if n.starts_with("order_rate_") && n.ends_with("_continue") => {
                let out=n.starts_with("order_rate_out");let rate:usize=n.trim_start_matches(if out {"order_rate_out"} else {"order_rate_in"}).trim_end_matches("_continue").parse().ok()?;
                boolean(h.order_continue[usize::from(out)][rate/10-1])
            },
            "is_behind_in"=>{
                match self.horses.iter().find(|o|o.order==h.order+1) {Some(b)=>boolean(b.lane<h.lane),None=>return None}
            },
            "hp_per"=>if h.max_hp>0.0 {(h.hp/h.max_hp*100.0) as i32 as f32} else {100.0},
            "is_last_straight_onetime"=>boolean(h.straight && c.last_straight_start().is_some_and(|st|h.distance>=st)),
            "base_speed"=>h.base[0],"base_stamina"=>h.base[1],"base_power"=>h.base[2],"base_guts"=>h.base[3],"base_wiz"=>h.base[4],
            "lastspurt"=>{
                let on=h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start;
                if !on {0.0} else if h.spurt_result==1 {2.0} else {1.0}
            }
            "is_lastspurt"=>boolean(h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start),
            "slope"=>if h.slope>0.0 {1.0} else if h.slope<0.0 {2.0} else {0.0},
            "is_badstart"=>boolean(h.original_delay>=0.08),
            "is_goodstart"=>{if crate::envc::var("JPRACE_GOODSTART").as_deref()==Ok("0") {return None;} boolean(self.time>0.0 && h.original_delay<self.p().f("Skill.GoodStartTime"))},
            "fan_count"=>h.input.fan_count as f32,"is_temptation"=>boolean(h.tempted()),
            "is_overtake"=>boolean(!h.overtake.is_empty()),
            "temptation_opponent_count_infront"|"temptation_opponent_count_behind"|"temptation_count_infront"|"temptation_count_behind"=>{
                let opp=n.contains("opponent");let team=h.input.team_id;let infront=n.ends_with("infront");
                self.horses.iter().filter(|o|o.tempted() && (!opp || team==0 || o.input.team_id==0 || o.input.team_id!=team) &&
                    if infront {o.order<h.order} else {o.order>h.order}).count() as f32
            },
            "is_surrounded"=>boolean(h.surround.iter().all(|b|*b)),
            "running_style_temptation_opponent_count_nige"|"running_style_temptation_opponent_count_senko"|
            "running_style_temptation_opponent_count_sashi"|"running_style_temptation_opponent_count_oikomi"=>{
                let style=if n.ends_with("nige") {1} else if n.ends_with("senko") {2} else if n.ends_with("sashi") {3} else {4};
                let team=h.input.team_id;
                self.horses.iter().enumerate().filter(|(i,o)|*i!=hi && o.input.running_style==style && o.tempted() &&
                    (team==0 || o.input.team_id==0 || o.input.team_id!=team)).count() as f32
            },
            "temptation_count"=>h.temptation_count as f32,"blocked_front"=>boolean(h.front.is_some()),
            "is_move_lane"=>if h.move_time<1.0 {0.0} else {(h.move_dir!=0) as i32 as f32+1.0},"blocked_front_continuetime"=>(if crate::envc::var_os("JPRACE_TIMER_EARLY").is_some() {if h.front.is_some() {h.front_time+DT} else {0.0}} else {h.front_time}) as i32 as f32,
            "blocked_side_continuetime"=>(if crate::envc::var_os("JPRACE_TIMER_EARLY").is_some() {if h.inside.is_some() || h.outside.is_some() {h.side_time+DT} else {0.0}} else {h.side_time}) as i32 as f32,
            "visiblehorse"=>h.visible.len() as f32,
            "is_used_skill_id_with_detail_one"=>if self.events.iter().any(|e|e["type"]==3 && e["params"][0]==hi as i64 && e["params"][1]==rhs as i64 && e["params"][3]==0) {rhs as f32} else {0.0},
            "near_count"=>h.near_count as f32,"straight_front_type"=>h.front_type as f32,
            "activate_count_all"=>h.activate_count as f32,"activate_count_heal"=>h.heal_count as f32,
            "activate_count_start"=>h.activate_phase[0] as f32,
            "activate_count_middle"=>h.activate_phase[1] as f32,
            "activate_count_end_after"=>(h.activate_phase[2]+h.activate_phase[3]) as f32,
            "activate_count_laterhalf"|"activate_count_later_half"=>h.activate_half[1] as f32,
            "infront_near_lane_time"=>h.near_front_time as i32 as f32,"behind_near_lane_time"=>h.near_behind_time as i32 as f32,
            n if n.starts_with("behind_near_lane_time_set")=>{
                let k:usize=n.trim_start_matches("behind_near_lane_time_set").parse().ok()?;
                (*h.near_behind_set.get(k.checked_sub(1)?)?).floor()
            },
            "change_order_up_middle"=>h.order_up[1] as f32,
            "change_order_up_end_after"=>h.order_up[2] as f32,
            "change_order_up_corner"=>h.order_up[3] as f32,
            "change_order_up_lastspurt"=>h.order_up[5] as f32,
            "change_order_up_finalcorner_after"=>h.order_up[7] as f32,
            "overtake_target_time"=>h.ot_target_time,
            "overtake_target_no_order_up_time"=>h.ot_noup_time,
            "lane_type"=>if h.lane<=0.2 {0.0} else if h.lane<=0.4 {1.0} else if h.lane<=0.6 {2.0} else {3.0},
            "is_activate_any_skill"=>h.activate_count as f32,
            "compete_fight_count"=>h.fight_count as f32,
            "change_order_onetime"=>(h.order as i32-h.prev_order as i32) as f32,
            "bashin_diff_infront"=>{let n=self.horses.iter().find(|n|n.order+1==h.order)?;(n.distance-h.distance).max(0.0)/2.5},
            "bashin_diff_behind"=>{let n=self.horses.iter().find(|n|n.order==h.order+1)?;(h.distance-n.distance).max(0.0)/2.5},
            "is_exist_chara_id"=>boolean(self.horses.iter().any(|n|n.input.chara_id==rhs)),
            "is_exist_skill_id"=>if self.horses.iter().any(|n|n.input.skill_array.iter().any(|s|s.skill_id==rhs)) {rhs as f32} else {0.0},
            "running_style_count_same"=>self.horses.iter().filter(|n|n.input.running_style==h.input.running_style).count() as f32,
            "running_style_count_same_rate"=>{
                let threshold=(rhs as f32/100.0*self.horses.len() as f32+0.5) as i32;
                let count=self.horses.iter().filter(|n|n.input.running_style==h.input.running_style).count() as i32;
                return Some((count-threshold+rhs) as f32);
            },
            "running_style_count_nige_otherself"|"running_style_count_senko_otherself"|"running_style_count_sashi_otherself"|"running_style_count_oikomi_otherself"=>{
                let style=if n.contains("nige") {1} else if n.contains("senko") {2} else if n.contains("sashi") {3} else {4};
                self.horses.iter().enumerate().filter(|(i,n)|*i!=hi && n.input.running_style==style).count() as f32
            },
            "running_style_equal_popularity_one"=>{
                let one=self.horses.iter().rposition(|n|(n.input.popularity-1).max(0)==0).unwrap_or(0);
                boolean(self.horses[one].input.running_style==h.input.running_style)
            },
            "same_skill_horse_count"=>self.horses.iter().enumerate().filter(|(i,n)|*i==hi || n.skills.iter().any(|k|k.id==self.cur_skill)).count() as f32,
            _=>return None,
        })
    }
    fn skills_pass(&mut self,dt:f32,priority:i64) {
        let full_dt=dt;
        for hi in 0..self.horses.len() {
            let mode=crate::envc::var("JPRACE_GATE_SKILL").unwrap_or_else(|_|"0".into());
            let delay=self.horses[hi].delay;
            let dt=match mode.as_str() {"0"=>full_dt,"1"=>if delay>0.0 {0.0} else {full_dt},_=>if delay>0.0 {(full_dt-delay).max(0.0)} else {full_dt}};
            if priority==0 {self.horses[hi].mods.update_timed(dt);}
            let mut skills=std::mem::take(&mut self.horses[hi].skills);
            if self.horses[hi].rafs_draw && priority==0 && let Ok(ord)=crate::envc::var("JPRACE_RAFS_ORDER") {
                let lo=self.horses[hi].distance;let hi_d=self.course.distance;
                let mut idx:Vec<usize>=(0..skills.len()).collect();
                if ord=="rev" {idx.reverse();} else if ord=="iddesc" {idx.sort_by_key(|&i|-(skills[i].id as i64));} else if ord=="idasc" {idx.sort_by_key(|&i|skills[i].id as i64);}
                for i in idx {for det in &mut skills[i].details {
                    let n=crate::triggers::draw_run_at_full_speed_u(&mut det.pre,&mut self.random)+crate::triggers::draw_run_at_full_speed_u(&mut det.condition,&mut self.random);
                    if n>0 {crate::triggers::finish_run_at_full_speed_windows(&mut det.pre,lo,hi_d);crate::triggers::finish_run_at_full_speed_windows(&mut det.condition,lo,hi_d);}
                }}
            }
            for skill in &mut skills {
                if ival(&skill.row,"priority")!=priority {continue;}
                if self.horses[hi].rafs_draw && (skill.enabled || crate::envc::var_os("JPRACE_RAFS_LOTALL").is_some() || (crate::envc::var("JPRACE_RAFS_LOTCHAIN").as_deref()!=Ok("0") && chain_cond(&skill.row))) {
                    let lo=self.horses[hi].distance;let hi_d=self.course.distance;
                    let static_ok:Vec<bool>=skill.details.iter().map(|det|{
                        if crate::envc::var("JPRACE_RAFS_STATIC").as_deref()!=Ok("1") {return true;}
                        const STATIC:[&str;13]=["distance_type","ground_type","track_id","rotation","course_distance","is_basis_distance","ground_condition","weather","season","running_style","motivation","post_number","popularity"];
                        let ok=|c:&crate::triggers::Condition|c.is_empty() || c.iter().any(|g|g.iter().all(|t|!STATIC.contains(&t.name.as_str()) ||
                            self.query(hi,&t.name,t.rhs).is_some_and(|v|crate::triggers::compare(v,&t.op,t.rhs as f32))));
                        ok(&det.pre) && ok(&det.condition)
                    }).collect();
                    for (di,det) in skill.details.iter_mut().enumerate() {
                        if !static_ok[di] {continue;}
                        let n=crate::triggers::draw_run_at_full_speed_u(&mut det.pre,&mut self.random)+crate::triggers::draw_run_at_full_speed_u(&mut det.condition,&mut self.random);
                        if n>0 {
                            if crate::envc::var_os("JPRACE_LOG_RAFSWIN").is_some() {eprintln!("rafsdraw h={} sk={} detail={} tick={} lo={:.6} calls={}",hi,skill.id,det.number,(self.time/0.0666).round() as i64,lo,self.random.calls());}
                            crate::triggers::finish_run_at_full_speed_windows(&mut det.pre,lo,hi_d);
                            crate::triggers::finish_run_at_full_speed_windows(&mut det.condition,lo,hi_d);
                        }
                    }
                }
                let chain_skill=crate::envc::var_os("JPRACE_CHAIN_BLOCK").is_none()
                    && chain_cond(&skill.row);
                let was_active:Vec<bool>=skill.details.iter().map(|d|d.active).collect();
                let mut handled=false;
                if skill.details.iter().any(|d|d.active) {
                    for d in &mut skill.details {
                        if d.active && d.left>=0.0 {
                            {
                                let now=self.horses[hi].activate_count;
                                let pending=(now-d.additional_seen).max(0);
                                d.additional_seen=now;
                                let ou_on=crate::envc::var("JPRACE_ORDERUP").as_deref()!=Ok("0");
                                let ou_now=self.horses[hi].ou_cnt;let ou_pending=(ou_now-d.orderup_seen).max(0);d.orderup_seen=ou_now;
                                if ou_on && ou_pending>0 && ival(&skill.row,&format!("ability_time_usage_{}",d.number))==4 {
                                    let cap=self.p().i("Skill.OrderUpAddAbilityTimeMaxCount") as i32;
                                    let n=ou_pending.min((cap-d.time_ext_used).max(0));d.time_ext_used+=n;
                                    let ext=n as f32*self.p().f("Skill.OrderUpAddAbilityTime")*(self.course.distance/self.p().i("Skill.AbilityTimeDivideDistance") as f32);
                                    d.left+=ext;
                                    let uid=d.uid;for m in self.horses[hi].mods.entries.iter_mut() {if m.owner==uid && let Some(r)=m.remaining.as_mut() && *r>=0.0 {*r+=ext;}}
                                    if crate::envc::var_os("JPRACE_LOG_ORDERUP").is_some() {eprintln!("timeext tick={} h={} sk={} n={} used={} left={:.4}",(self.time/0.0666).round() as i64,hi,skill.id,n,d.time_ext_used,d.left);}
                                }
                                for a in 1..=3 {
                                    let aty=ival(&skill.row,&format!("additional_activate_type_{}_{}",d.number,a));
                                    if !(aty==3 || (aty==2 && crate::envc::var("JPRACE_ADDL2").as_deref()!=Ok("0")) || (aty==1 && (ou_on || crate::envc::var("JPRACE_ADDL1").as_deref()==Ok("1")))) {continue;}
                                    let cap=global_additional_cap(self.p(),aty as i64);
                                    let pending=if aty==1 && crate::envc::var("JPRACE_ADDL1").as_deref()!=Ok("1") {ou_pending} else {pending};
                                    let count=take_additional_count(pending,cap,&mut d.additional_used[a-1]);
                                    for _ in 0..count {
                                        self.ability_activate(hi,skill.level,&skill.row,d,a,true);
                                    }
                                }
                            }
                            d.left-=skill_dt(dt);
                            if d.left<=0.0 {
                                if crate::envc::var_os("JPRACE_LOG_ORDERUP").is_some() {eprintln!("skillend tick={} h={} sk={}",(self.time/0.0666).round() as i64,hi,skill.id);}
                                d.active=false;
                                stop_effect(&mut self.horses,d.uid);
                                let cd=ival(&skill.row,&format!("float_cooldown_time_{}",d.number)) as f32*0.0001*
                                    (self.course.distance/self.p().i("Skill.CoolDownTimeDivideDistance") as f32);
                                let chain=crate::envc::var_os("JPRACE_SKILL_SHARED_CD").is_none()
                                    && chain_cond(&skill.row);
                                if chain {d.cooldown=cd;} else {skill.cooldown=cd;}
                            }
                        }
                    }
                    if !chain_skill {continue;}
                    handled=true;
                }
                if skill.cooldown>0.0 {skill.cooldown=(skill.cooldown-dt).max(0.0);if skill.cooldown>0.0 {continue;}}
                for (di,d) in skill.details.iter_mut().enumerate() {
                    if handled && was_active[di] {continue;}
                    if d.active && d.left>=0.0 {
                        d.left-=skill_dt(dt);
                        if d.left<=0.0 {
                            d.active=false;
                            stop_effect(&mut self.horses,d.uid);
                            d.cooldown=ival(&skill.row,&format!("float_cooldown_time_{}",d.number)) as f32*0.0001*
                                (self.course.distance/self.p().i("Skill.CoolDownTimeDivideDistance") as f32);
                        }
                    } else if d.cooldown>0.0 {d.cooldown=(d.cooldown-dt).max(0.0);}
                    let alld=crate::envc::var("JPRACE_RESERVE_ALLD").as_deref()==Ok("1");
                    let reserved=(d.number==1 || alld) && self.horses[hi].reserved.contains(&skill.id);
                    if (!skill.enabled && !reserved) || d.active || d.cooldown>0.0 || self.horses[hi].finished {continue;}
                    self.cur_skill=skill.id;
                    if !d.pre_passed {d.pre_passed=triggers::evaluate(&mut d.pre,self.horses[hi].distance,&|n,r|self.query(hi,n,r));}
                    let mut activate=reserved || d.pre_passed && triggers::evaluate(&mut d.condition,self.horses[hi].distance,&|n,r|self.query(hi,n,r));
                    if activate && let Ok(v)=crate::envc::var("JPRACE_DENY_SKILL") && v.split(',').any(|x|{let p:Vec<&str>=x.split(':').collect();p.len()==3 && p[0].parse::<usize>().ok()==Some(hi) && p[1].parse::<i32>().ok()==Some(skill.id) && p[2].parse::<usize>().ok()==Some(d.number as usize)}) {activate=false;}
                    if crate::envc::var("JPRACE_DEBUG_SKILLID").ok().and_then(|x|x.parse::<i32>().ok())==Some(skill.id) {
                        let conds:Vec<String>=d.condition.iter().flatten().map(|t|format!("{}{}{}={:?}w{:?}",t.name,t.op,t.rhs,self.query(hi,&t.name,t.rhs),t.windows)).collect();
                        let pres:Vec<String>=d.pre.iter().flatten().map(|t|format!("{}{}{}={:?}",t.name,t.op,t.rhs,self.query(hi,&t.name,t.rhs))).collect();
                        eprintln!("dbgskill t={:.4} h={} d={} pre={} act={} dist={:.3} {} | pre: {}",self.time,hi,d.number,d.pre_passed,activate,self.horses[hi].distance,conds.join(" "),pres.join(" "));
                    }
                    if activate {
                        let raw=ival(&skill.row,&format!("float_ability_time_{}",d.number));
                        d.left=if raw==-1 {-1.0} else {raw as f32*0.0001*(self.course.distance/self.p().i("Skill.AbilityTimeDivideDistance") as f32)};
                        if raw!=-1 && ival(&skill.row,&format!("ability_time_usage_{}",d.number))==8 && crate::envc::var("JPRACE_TIME_USAGE8").as_deref()!=Ok("0") {
                            let p=self.p();let hd=self.horses[hi].distance;
                            let merge=crate::envc::var("JPRACE_U8_MERGE").as_deref()!=Ok("0");
                            let n=(self.course.slopes.iter().filter(|s|s.per>0.0 && s.end<=hd && !(merge && self.course.slopes.iter().any(|t|t.per>0.0 && t.start==s.end))).count() as i64).min(p.i("Skill.ThroughUpSlopeCountTimeMax") as i64);
                            d.left=(raw+n*p.i("Skill.ThroughUpSlopeCountTimeCoef") as i64) as f32*0.0001*(self.course.distance/p.i("Skill.AbilityTimeDivideDistance") as f32);
                        }
                        if raw!=-1 && self.time==0.0 {if let Ok(x)=crate::envc::var("JPRACE_T0_EXTEND") {d.left+=x.parse::<f32>().unwrap();}}
                        if raw!=-1 && let Ok(v)=crate::envc::var("JPRACE_SKILL_DUR_ADJ") {
                            for x in v.split(',') {let p:Vec<&str>=x.split(':').collect();
                                if p.len()>=3 && p[0].parse::<usize>().ok()==Some(hi) && p[1].parse::<i32>().ok()==Some(skill.id) && (p.len()==3 || p[3].parse::<usize>().ok()==Some(d.number)) {d.left+=p[2].parse::<f32>().unwrap_or(0.0);}}
                        }
                        if raw!=-1 && ival(&skill.row,"rarity")==6 && ival(&skill.row,"unique_skill_id_1")==0 && (skill.id>=100_000_000 || crate::envc::var_os("JPRACE_TYPE42_ALL").is_some()) {
                            let m=self.horses[hi].mods.apply(27,1.0);if m!=1.0 {d.left*=m;}
                        }
                        let usage=ival(&skill.row,&format!("ability_time_usage_{}",d.number));
                        if usage==3 {
                            let mult=global_hp_duration_multiplier(self.p(),self.horses[hi].hp);
                            if raw!=-1 {d.left*=mult;}
                        }
                        if raw!=-1 && matches!(usage,2|5|7) && crate::envc::var_os("JPRACE_NO_TIME_USAGE").is_none() {
                            let p=self.p();
                            let mult=match usage {
                                2=>{let top=self.horses.iter().map(|o|o.distance).fold(0.0f32,f32::max);let diff=top-self.horses[hi].distance;
                                    (p.f("Skill.AbilityTimeDiffDistanceTopCoefMin")+diff/p.f("Skill.AbilityTimeDiffDistanceTopCoefPerDistance")).clamp(p.f("Skill.AbilityTimeDiffDistanceTopCoefMin"),p.f("Skill.AbilityTimeDiffDistanceTopCoefMax"))},
                                5=>{let v=self.horses[hi].side_max_mid;p.arr("Skill.BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray1").iter().find(|r|v<f32_of(&r["BlockedSideMaxContinueTimeThreshold"])).map_or(1.0,|r|f32_of(&r["Multiply"]))},
                                _=>{let hp=self.horses[hi].hp;p.arr("Skill.TimeRemainHpCoefAscArray2").iter().find(|r|hp<f32_of(&r["HpThreshold"])).map_or(1.0,|r|f32_of(&r["Multiply"]))},
                            };
                            d.left*=mult;
                        }
                        d.active=true;
                        let adv48=crate::envc::var("JPRACE_ADV48").as_deref()!=Ok("0");
                        let adv=(1..=3).any(|a|{let k=ival(&skill.row,&format!("ability_type_{}_{a}",d.number));let v=ival(&skill.row,&format!("float_ability_value_{}_{a}",d.number));
                            (matches!(k,21|22|27|31) || adv48 && k==48) && v>0 && ival(&skill.row,&format!("target_type_{}_{a}",d.number))==1});
                        if adv {let t=(self.time/0.0666).round() as i64;let h=&mut self.horses[hi];if h.adv_tick!=t {h.adv_prev=h.adv_tick;h.adv_tick=t;}}
                        {let t=(self.time/0.0666).round() as i64;
                         let kinds:Vec<i32>=(1..=3).filter_map(|a|{let k=ival(&skill.row,&format!("ability_type_{}_{a}",d.number)) as i32;let v=ival(&skill.row,&format!("float_ability_value_{}_{a}",d.number));
                            (k!=0 && v>0 && ival(&skill.row,&format!("target_type_{}_{a}",d.number))==1).then_some(k)}).collect();
                         let h=&mut self.horses[hi];h.adv_kinds.retain(|&(_,tt)|tt>=t-2);for k in kinds {h.adv_kinds.push((k,t));}}
                        let h=&mut self.horses[hi];h.activate_count+=1;
                        if !h.used_skills.contains(&skill.id) {h.used_skills.push(skill.id);}
                        h.activate_phase[h.phase.min(3)]+=1;
                        h.activate_half[usize::from(h.distance>=self.course.distance*0.5)]+=1;
                        d.additional_seen=h.activate_count;
                        d.orderup_seen=h.ou_cnt;d.time_ext_used=0;
                        let mut mask=0i64;
                        for a in 1..=3 {
                            let targets=self.ability(hi,skill.level,&skill.row,d,a);
                            if a==1 {
                                for t in targets {mask|=1<<t;}
                            }
                        }
                        let t0=self.time==0.0;
                        let enc=if crate::envc::var_os("JPRACE_ENC64").is_some() {(d.left as f64*10000.0) as i32} else {(d.left*10000.0) as i32};
                        self.horses[hi].skill_time=self.time;
                        self.events.push(json!({"t":self.time,"type":3,"name":"Skill","params":[hi as i32,skill.id,if raw==-1 || t0 {-1} else {enc},d.number as i32-1,if t0 {0} else {mask},0],"horse":hi,"extra":""}));
                        if reserved && alld {continue;}
                        if reserved && crate::envc::var("JPRACE_RESERVE_ENABLE").as_deref()!=Ok("0") {skill.enabled=true;}
                        if reserved && crate::envc::var("JPRACE_RESERVE_DRAW").as_deref()==Ok("act") {self.random.int_max(100);}
                        if chain_cond(&skill.row) && crate::envc::var("JPRACE_CHAIN_SAME").as_deref()!=Ok("0") {continue;}
                        break;
                    }
                }
            }
            if priority==0 {self.reserve_random(hi,&skills);}
            self.horses[hi].skills=skills;
            if priority==0 {self.horses[hi].rafs_draw=false;}
        }
    }
    fn reserve_random(&mut self,hi:usize,skills:&[Skill]) {
        self.horses[hi].reserved.clear();
        for (index,rarity) in [(25usize,2i64),(24,1)] {
            let count=self.horses[hi].mods.consume(index,0.0) as i32;
            if count<=0 {continue;}
            let r6=rarity==2 && crate::envc::var("JPRACE_RESERVE_R6").as_deref()!=Ok("0");
            let dcd=crate::envc::var("JPRACE_RESERVE_DCD").as_deref()!=Ok("0");
            let mut cand=skills.iter().filter(|k|(ival(&k.row,"rarity")==rarity || r6 && ival(&k.row,"rarity")==6) && ival(&k.row,"float_ability_time_1")!=-1 &&
                ival(&k.row,"is_general_skill")==1 && (k.cooldown<=0.0 && !k.details.iter().any(|d|d.active || dcd && d.cooldown>0.0)) &&
                !self.horses[hi].reserved.contains(&k.id)).map(|k|k.id).collect::<Vec<_>>();
            if crate::envc::var("JPRACE_RESERVE_USABLE").as_deref()==Ok("1") {
                let usable:Vec<i32>=cand.iter().copied().filter(|id|skills.iter().find(|k|k.id==*id).is_some_and(|k|self.skill_usable(hi,k))).collect();
                if !usable.is_empty() {cand=usable;}
            }
            match crate::envc::var("JPRACE_RESERVE_ORDER").as_deref() {Ok("idasc")=>cand.sort(),Ok("iddesc")=>{cand.sort();cand.reverse();},_=>{}}
            if crate::envc::var("JPRACE_RESERVE_DRAW").as_deref()==Ok("dis") && count>0 && cand.iter().any(|c|skills.iter().find(|k|k.id==*c).is_some_and(|k|!k.enabled)) {self.random.int_max(100);}
            for _ in 0..count.min(cand.len() as i32) {
                if crate::envc::var_os("JPRACE_LOG_RESERVE").is_some() {let mut r=self.random.clone();let u=r.raw() as f64/2147483647.0;eprintln!("reserve h={} t={:.4} rarity={} u={:.6} cand={:?} enabled={:?}",hi,self.time,rarity,u,cand,cand.iter().map(|c|skills.iter().find(|k|k.id==*c).map_or(false,|k|k.enabled)).collect::<Vec<_>>());}
                let i=self.random.int_range(0,cand.len() as i32) as usize;
                self.horses[hi].reserved.push(cand.remove(i));
            }
            if crate::envc::var("JPRACE_RESERVE_DRAW").as_deref()==Ok("res") {self.random.int_max(100);}
        }
    }
    fn skill_usable(&self,hi:usize,k:&Skill)->bool {
        const STATIC:[&str;13]=["distance_type","ground_type","track_id","rotation","course_distance","is_basis_distance","ground_condition","weather","season","running_style","motivation","post_number","popularity"];
        let Some(d)=k.details.iter().find(|d|d.number==1) else {return true;};
        let ok=|c:&crate::triggers::Condition|c.is_empty() || c.iter().any(|g|g.iter().all(|t|!STATIC.contains(&t.name.as_str()) ||
            self.query(hi,&t.name,t.rhs).is_some_and(|v|crate::triggers::compare(v,&t.op,t.rhs as f32))));
        ok(&d.pre) && ok(&d.condition)
    }
    fn ability(&mut self,hi:usize,level:i32,row:&HashMap<String,Value>,detail:&Detail,a:usize)->Vec<usize> {
        self.ability_activate(hi,level,row,detail,a,false)
    }
    fn ability_activate(&mut self,hi:usize,level:i32,row:&HashMap<String,Value>,detail:&Detail,a:usize,is_additional:bool)->Vec<usize> {
        let suffix=format!("{}_{}",detail.number,a);let kind=ival(row,&format!("ability_type_{suffix}"));if kind==0 {return vec![];}
        let target=ival(row,&format!("target_type_{suffix}"));let tv=ival(row,&format!("target_value_{suffix}"));
        let mut targets=match target {
            1=>vec![hi],2=>(0..self.horses.len()).collect(),3=>(0..self.horses.len()).filter(|i|*i!=hi).collect(),
            4=>{
                let pos=self.horses[hi].position;
                let mut v=self.horses[hi].visible.clone();
                let distance=|i:usize| {let n=self.horses[i].position;
                    let [dx,dy,dz]=std::array::from_fn::<_,3,_>(|k|pos[k]-n[k]);
                    ((dy*dy+dx*dx+dz*dz) as f64).sqrt() as f32};
                v.sort_by(|a,b|distance(*a).total_cmp(&distance(*b)));
                v.truncate(tv.max(0) as usize);v
            },
            5=>{
                let count=tv.min(self.horses.len() as i64-1);let mut selected=vec![];
                for _ in 0..count {
                    let start=self.random.int_max(self.horses.len() as i32) as usize;
                    for offset in 0..self.horses.len() {
                        let i=(start+offset)%self.horses.len();
                        if i!=hi && !selected.contains(&i) {selected.push(i);break;}
                    }
                } selected
            },
            6|7|8=>self.horses.iter().enumerate().filter(|(_,h)|match target {6=>h.order as i64+1==tv,7=>(h.order as i64+1)<tv,_=>tv<h.order as i64+1}).map(|(i,_)|i).collect(),
            9|10=>{
                let o=self.horses[hi].order as i64;let n=self.horses.len() as i64;
                (1..=tv).map(|k|if target==9 {o-k} else {o+k}).take_while(|x|*x>=0 && *x<n)
                    .filter_map(|x|self.horses.iter().position(|h|h.order as i64==x)).collect()
            },
            19|20=>{
                let o=self.horses[hi].order;
                let mut pool:Vec<usize>=(0..self.horses.len()).filter(|i|self.horses[*i].tempted()
                    && if target==19 {self.horses[*i].order<o} else {self.horses[*i].order>o}).collect();
                let mut v=vec![];
                for _ in 0..(tv.max(0) as usize).min(pool.len()) {
                    let k=self.random.int_max(pool.len() as i32) as usize;
                    v.push(pool.remove(k));
                } v
            },
            23=>{
                let cond=row.get(&format!("condition_{}",detail.number)).and_then(|v|v.as_str()).unwrap_or("").to_string();
                let kinds:Vec<i64>=cond.split(['&','@']).filter_map(|t|t.strip_prefix("is_other_character_activate_advantage_skill==")).filter_map(|x|x.parse().ok()).collect();
                let tick=(self.time/0.0666).round() as i64;
                let team=self.horses[hi].input.team_id;
                let v:Vec<usize>=(0..self.horses.len()).filter(|&i|i!=hi && (team==0 || self.horses[i].input.team_id!=team) && self.horses[i].adv_kinds.iter().any(|&(k,t)|kinds.contains(&(k as i64)) && t==tick-1)).collect();
                if crate::envc::var("JPRACE_T23").as_deref()==Ok("all") {v} else {v.into_iter().last().into_iter().collect()}
            },
            13=>{let mut v=vec![hi];if let Some(i)=self.horses[hi].front {v.push(i);}v},
            14=>[self.horses[hi].inside,self.horses[hi].outside].into_iter().flatten().collect(),
            17=>self.horses.iter().enumerate().filter(|(_,h)|h.input.running_style as i64==tv).map(|(i,_)|i).collect(),
            18=>self.horses.iter().enumerate().filter(|(i,h)|*i!=hi && h.input.running_style as i64==tv).map(|(i,_)|i).collect(),
            21=>{let team=if crate::envc::var("JPRACE_T21_TEAM").as_deref()==Ok("0") {0} else {self.horses[hi].input.team_id};
                self.horses.iter().enumerate().filter(|(i,h)|*i!=hi && h.input.running_style as i64==tv && h.tempted()
                    && (team==0 || h.input.team_id==0 || h.input.team_id!=team)).map(|(i,_)|i).collect()},
            22=>self.horses.iter().enumerate().filter(|(_,h)|h.input.chara_id as i64==tv).map(|(i,_)|i).collect(),
            24=>{let mode=crate::envc::var("JPRACE_T24").unwrap_or_default();
                let team=self.horses[hi].input.team_id;let mut v=vec![];
                let mut pool:Vec<usize>=if team==0 || mode=="0" {vec![]} else {(0..self.horses.len()).filter(|&i|i!=hi && self.horses[i].input.team_id==team).collect()};
                if !pool.is_empty() {
                    pool.sort_by_key(|&i|self.horses[i].input.team_member_id);
                    let k=self.random.int_max(pool.len() as i32) as usize;
                    if mode=="frontdraw" {v.push(*pool.iter().min_by_key(|&&i|self.horses[i].order).unwrap());} else {v.push(pool[k]);}
                }
                v},
            11=>{let team=self.horses[hi].input.team_id;
                if team==0 {vec![hi]} else {self.horses.iter().enumerate().filter(|(_,h)|h.input.team_id==team).map(|(i,_)|i).collect()}},
            _=>vec![],
        };
        let add_usage=if crate::envc::var_os("JPRACE_NO_ADDUSAGE").is_some() {0} else {
            let u=ival(row,&format!("ability_value_usage_{suffix}"));
            match u {
                35|36=>{
                    let c=&self.course;let dt=if c.distance<=1400.0 {1} else if c.distance<=1800.0 {2} else if c.distance<=2400.0 {3} else {4};
                    self.p().arr(if u==35 {"Skill.RaceCourseDistanceTypeValueArray"} else {"Skill.RaceCourseDistanceTypeValue2Array"}).iter()
                        .find(|r|r["DistanceType"].as_i64()==Some(dt)).and_then(|r|r["AddValue"].as_i64()).unwrap_or(0)
                },
                19|34=>{
                    let gap=self.horses.iter().map(|h|h.distance).fold(f32::MIN,f32::max)-self.horses[hi].distance;
                    self.p().arr(if u==19 {"Skill.DistanceDiffTopCoefAscArray"} else {"Skill.DistanceDiffTopCoefAscArray2"}).iter()
                        .find(|r|gap<r["DistanceDiffTopThreshold"].as_i64().unwrap_or(0) as f32).and_then(|r|r["AddValue"].as_i64()).unwrap_or(0)
                },
                _=>0,
            }
        };
        let mut value=(ival(row,&format!("float_ability_value_{suffix}"))+add_usage) as f32*0.0001;
        if let Ok(x)=crate::envc::var("JPRACE_SKILL_SCALE") && let Some((id,f))=x.split_once(':') && id.parse::<i64>().ok()==Some(ival(row,"id")) {value*=f.parse::<f32>().unwrap_or(1.0);}
        if value<0.0 {
            let team=self.horses[hi].input.team_id;
            targets.retain(|i|self.horses[*i].mods.apply(18,0.0)==0.0 && (*i==hi || team==0 || self.horses[*i].input.team_id==0 || self.horses[*i].input.team_id!=team));
        }
        let usage=ival(row,&format!("ability_value_usage_{suffix}"));
        let coef=self.master.rows("skill_level_value").iter().find(|r|ival(r,"ability_type")==kind && ival(r,"level")==level as i64)
            .map_or(1.0,|r|ival(r,"float_ability_value_coef") as f32*0.0001);
        match ival(row,&format!("ability_value_level_usage_{suffix}")) {1=>value=coef*value,3=>value=(1.0/coef)*value,_=>()};
        let lb_parts=if matches!(kind,22|27) {self.wiz_limit_break_parts(hi,row)} else {None};
        let lb_dynamic=(kind==27 || kind==22 && crate::envc::var_os("JPRACE_LB22_STATIC").is_none()) && lb_parts.is_some() && crate::envc::var_os("JPRACE_LB_STATIC").is_none();
        if usage==2 {
            let n=self.horses[hi].input.skill_array.len() as i32;
            let auto=1.0+0.01*((n-1).clamp(0,20) as f32);
            value*=crate::envc::var("JPRACE_SKILLNUM_MULT").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(auto);
        }
        if usage==10 {
            let wins=self.horses[hi].input.single_mode_win_count;
            let mult=self.p().arr("Skill.RaceWinCountCoefAscArray").iter()
                .find(|r|wins<r["WinCountThreshold"].as_i64().unwrap() as i32)
                .map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if usage==12 {
            let fans=self.horses[hi].input.fan_count;
            let mult=self.p().arr("Skill.FanCountCoefAscArray").iter()
                .find(|r|fans<r["FanCountThreshold"].as_i64().unwrap())
                .map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if usage==13 {
            let h=&self.horses[hi].input;
            let max=[h.speed,h.guts,h.pow,h.stamina,h.wiz].into_iter().max().unwrap() as f32;
            let mult=self.p().arr("Skill.MaximumRawStatusCoefAscArray").iter()
                .find(|r|max<f32_of(&r["BaseStatusThreshold"]))
                .map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if (3..=7).contains(&usage) {
            let k=(usage-3) as usize;let team=self.horses[hi].input.team_id;
            let sum=if team==0 {self.horses[hi].base[k]} else {
                self.horses.iter().filter(|h|h.input.team_id==team).fold(0.0f32,|a,h|a+h.base[k])};
            let mult=self.p().arr("Skill.TeamTotalStatusCoefAscArray").iter()
                .find(|r|sum<f32_of(&r["StatusThreshold"])).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if matches!(usage,8|9) {
            let arr=self.assets.params().arr(if usage==8 {"Skill.AbiiltyValueUsageRandom1Array"} else {"Skill.AbiiltyValueUsageRandom2Array"}).clone();
            let total=arr.iter().map(|r|r["Per"].as_i64().unwrap() as i32).sum::<i32>();
            let mut r=self.random.int_max(total);let mut mult=1.0;
            for row in &arr {r-=row["Per"].as_i64().unwrap() as i32;if r<1 {mult=f32_of(&row["AbilityValueCoef"]);break;}}
            value*=mult;
        }
        if usage==20 && crate::envc::var_os("JPRACE_NO_USAGE20").is_none() {
            let v=self.horses[hi].side_max_mid;
            let mult=self.p().arr("Skill.BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray1").iter().find(|r|v<f32_of(&r["BlockedSideMaxContinueTimeThreshold"])).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if usage==11 && crate::envc::var_os("JPRACE_NO_USAGE11").is_none() {
            let c=self.horses[hi].order_up[4] as i64;
            let mult=self.p().arr("Skill.OrderUpCornerPhaseEndAfterCoefAscArray").iter().find(|r|c<r["OrderChangeCountThreshold"].as_i64().unwrap()).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if let Some((ty,arr))=match usage {24=>Some((6001,"Skill.ArcGlobalPotentialLevelCoefArray")),26=>Some((7001,"Skill.SportFinalCompeWinCountCoefArray")),
            27=>Some((8001,"Skill.CookCookingPtCoefArray")),28=>Some((9001,"Skill.MechaResearchTotalLevelCoefArray")),31=>Some((11001,"Skill.PioneerTotalPointCoefArray")),
            32=>Some((12001,"Skill.OnsenRyokanRankCoefArray")),37=>Some((13001,"Skill.BreedersTeamRankCoefArray")),40=>Some((14001,"Skill.RamenTotalPointCoefArray")),_=>None} {
            if crate::envc::var_os("JPRACE_NO_SCENARIO_USAGE").is_none() {
                let v=self.horses[hi].input.scenario_data_array.iter().find(|x|x.kind==ty).map_or(0,|x|x.value);
                let mult=self.p().arr(arr).iter().find(|r|v<r["LevelThreshold"].as_i64().unwrap()).map_or(1.0,|r|f32_of(&r["Multiply"]));
                value*=mult;
            }
        }
        if usage==41 {
            let m=crate::envc::var("JPRACE_U41").unwrap_or_default();
            if let Ok(x)=m.parse::<f32>() {value*=x;}
            else if m!="none" {
                let v=self.horses[hi].input.scenario_data_array.iter().find(|x|x.kind==15001).map_or(0,|x|x.value);
                if v>=30 {value*=1.2;}
            }
        }
        if usage==14 {
            let m=crate::envc::var("JPRACE_U14").unwrap_or_default();
            if let Ok(x)=m.parse::<f32>() {value*=x;}
            else if m!="none" {
                let tags:Vec<i64>=self.p().arr("Skill.AbilityValueUsageTagGroup1Array").iter().filter_map(|v|v.as_i64()).collect();
                let n=self.horses[hi].used_skills.iter().filter(|&&id|self.master.find("skill_data","id",id as i64).is_some_and(|r|
                    r.get("tag_id").and_then(|v|v.as_str()).unwrap_or("").split('/').filter_map(|x|x.parse::<i64>().ok()).any(|t|tags.contains(&t)))).count() as i64;
                let mult=if m!="user" {self.p().arr("Skill.ActivateSpecificTagGroupSkillCountCoefAscArray").iter().find(|r|n<r["ActivateSpecificTagGroupSkillCountThreshold"].as_i64().unwrap_or(0)).map_or(1.0,|r|f32_of(&r["Multiply"]))}
                    else {match n {..=3=>0.0,4=>1.0,5=>2.0,_=>3.0}};
                value*=mult;
            }
        }
        if usage==30 && crate::envc::var("JPRACE_U30").as_deref()!=Ok("none") {
            let h=&self.horses[hi];let i=&h.input;
            let gs=(h.original_delay<self.p().f("Skill.GoodStartTime")) as i64;
            let v=match crate::envc::var("JPRACE_U30").as_deref() {Ok("fan")=>i.fan_count,Ok("fanwin")=>i.fan_count*i.single_mode_win_count as i64,
                _=>i.fan_count*(gs+h.order_up[1] as i64+h.activate_count as i64)};
            let mult=self.p().arr("Skill.PerformanceCountAndFansCoefArray").iter().find(|r|v<r["Threshold"].as_i64().unwrap_or(0)).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if matches!(usage,22|23) {
            let sp=self.horses[hi].stat(0);
            let mult=self.p().arr(if usage==22 {"Skill.SpeedAbilityValueCoefAscArray1"} else {"Skill.SpeedAbilityValueCoefAscArray2"}).iter()
                .find(|r|sp<r["SpeedThreshold"].as_i64().unwrap() as f32).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        let lb_base=value;
        if let Some((m,c,d))=lb_parts {value*=1.0+m*c*d;}
        if crate::envc::var("JPRACE_LOG_SKVAL").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("skval t={:.4} h={} skill={} kind={} level={} raw={} coef={:.4} usage={} lb_base={:.5} lb={:?} value={:.5}",self.time,hi,ival(row,"id"),kind,level,ival(row,&format!("float_ability_value_{suffix}")),coef,usage,lb_base,lb_parts,value);
        }
        let mut lb_rec=lb_parts.filter(|_|lb_dynamic).map(|(m,_,d)|[lb_base,m,d,self.horses[hi].input.running_style as f32,if self.horses[hi].ex() {1.0} else {0.0}]);
        if lb_rec.is_none() && kind==27 && fs_enabled() && crate::envc::var_os("JPRACE_LB_STATIC").is_none() {
            let w=Tree(self.p().get("WizLimitBreakBuff"));let rarity=ival(row,"rarity");
            let excl=match crate::envc::var("JPRACE_FS_EXCL").as_deref().unwrap_or("r6s") {"r6"=>rarity==6,"r6s"=>rarity==6 && ival(row,"id")<100_000_000,_=>false};
            if !excl && w.arr("SkillSelectionRuleArray").iter().any(|r|r["Rarity"].as_i64()==Some(rarity)) {
                let d=w.arr("CourseDistanceCoefArray").iter().find(|r|self.course.distance<=r["CourseDistanceThreshold"].as_i64().unwrap() as f32).map_or(1.0,|r|f32_of(&r["Multiply"]));
                lb_rec=Some([lb_base,0.0,d,self.horses[hi].input.running_style as f32,if self.horses[hi].ex() {1.0} else {0.0}]);
            }
        }
        if ival(row,&format!("additional_activate_type_{suffix}"))!=0 {
            if !is_additional {return targets;}
        }
        if kind==9 && value>0.0 {self.horses[hi].heal_count+=1;}
        if kind==41 && crate::envc::var_os("JPRACE_NO_TYPE41").is_none() {
            let cond=format!("{}@{}",sval(row,&format!("condition_{}",detail.number)),sval(row,&format!("precondition_{}",detail.number)));
            if let Some(x)=cond.split(['&','@']).find_map(|t|t.strip_prefix("is_exist_skill_id==").and_then(|v|v.parse::<i32>().ok())) {
                for &i in &targets {
                    if self.horses[i].skills.iter().any(|k|k.id==x) || self.horses[i].input.skill_array.iter().any(|k|k.skill_id==x) {
                        if !self.horses[i].reserved.contains(&x) {self.horses[i].reserved.push(x);}
                    }
                }
            }
        }
        for &i in &targets {
            let h=&mut self.horses[i];let uid=detail.uid;
            let effect=match kind {
                1..=5=>Some((kind as usize-1,Operation::Add,false,value)),
                6=>Some((20,Operation::Set,false,1.0)),7=>Some((13,Operation::Set,false,value)),8=>Some((19,Operation::Add,false,value)),
                9=>{h.hp_chg=(h.max_hp*value).abs();Some((12,Operation::Add,true,h.max_hp*value))},
                10=>Some((15,Operation::Set,true,value.max(0.0))),11=>Some((16,Operation::Set,false,1.0)),12=>Some((17,Operation::Set,false,1.0)),
                13=>Some((14,Operation::Set,true,value)),14=>Some((22,Operation::Set,true,value)),
                21=>Some((5,Operation::Add,false,value)),22=>{
                    if crate::envc::var_os("JPRACE_LOG_CS0").is_some() && h.hp<=0.0 {eprintln!("cs0 t={:.4} h={} value={}",self.time,hi,value);}
                    if crate::envc::var("JPRACE_CS22").as_deref()==Ok("once") && self.time>0.0 {Some((23,Operation::Add,true,value))} else
                    if h.hp<=0.0 && crate::envc::var("JPRACE_CS_HP0").as_deref()==Ok("skip") {None}
                    else if h.hp<=0.0 && self.time>0.0 && crate::envc::var("JPRACE_CS_HP0").as_deref()==Ok("merge") {Some((23,Operation::Add,true,value))} else {
                    Some((23,Operation::Add,false,value))}},
                27=>Some((6,Operation::Add,false,value)),28=>{if value>0.0 {h.mods.add(21,1.0,Operation::Set,uid,false);}Some((7,Operation::Add,false,value))},
                29=>Some((8,Operation::Add,false,value)),30=>Some((9,Operation::Add,false,value)),31=>Some((10,Operation::Add,false,value)),
                32=>{for k in 0..5 {h.mods.add(k,value,Operation::Add,uid,false);}None},
                35=>Some((11,Operation::Add,false,value)),36=>Some((24,Operation::Set,true,value)),37=>Some((25,Operation::Set,true,value)),38=>Some((18,Operation::Set,false,1.0)),
                48=>Some((26,Operation::Add,false,value)),
                42 if crate::envc::var_os("JPRACE_NO_TYPE42").is_none()=>Some((27,Operation::Set,false,value)),
                _=>None,
            };
            if let Some((index,op,once,value))=effect {
                let uid=if index==27 && crate::envc::var_os("JPRACE_TYPE42_TIMED").is_none() {u64::MAX} else {uid};
                if is_additional {
                    h.mods.add_timed(index,value,op,uid,once,detail.left);
                } else {h.mods.add(index,value,op,uid,once);}
                if (index==6 || index==23) && let Some(r)=lb_rec {h.mods.set_last_lb(r);}
            }
        }
        targets
    }
    fn ground_multi(&self)->f32 {
        let p=self.p();let a=p.arr(if self.course.ground==1 {"_groundModifierParamTurf"} else {"_groundModifierParamDirt"});
        f32_of(&a[self.input.ground_condition as usize-1]["multiHpSub"])
    }
    fn wiz_limit_break_parts(&self,hi:usize,row:&std::collections::HashMap<String,Value>)->Option<(f32,f32,f32)> {
        let h=&self.horses[hi];let p=self.p();
        let rarity=ival(row,"rarity");
        let w=Tree(p.get("WizLimitBreakBuff"));
        let Some(rule)=w.arr("SkillSelectionRuleArray").iter().find(|r|r["Rarity"].as_i64()==Some(rarity)) else {return None;};
        match crate::envc::var("JPRACE_LB_EXCL").as_deref() {
            Ok("none")=>{},
            Ok("r6s")=>if rarity==6 && ival(row,"id")<100_000_000 {return None;},
            _=>if rule["SelectionRule"]["IsExcludeSuccessionSkill"].as_i64()==Some(1) && ival(row,"unique_skill_id_1")!=0 {return None;},
        }
        let mood=self.rate("race_motivation_rate",h.input.motivation,"motivation_rate").unwrap_or(1.0);
        let raw=h.input.wiz as f32;let halved=if raw>1200.0 {1200.0+(raw-1200.0)*0.5} else {raw};
        let mut wiz=h.base[4]+(raw-halved)+h.mods.apply(4,0.0);
        if crate::envc::var("JPRACE_WIZLB_WIZ").as_deref()==Ok("raw") {wiz=raw*mood+h.mods.apply(4,0.0);}
        match crate::envc::var("JPRACE_WIZLB_WIZ").as_deref() {
            Ok("base")=>wiz=h.base[4]+h.mods.apply(4,0.0),
            Ok("adj")=>wiz=h.stat(4),
            Ok(x) if x.starts_with("add:")=>wiz+=x[4..].parse::<f32>().unwrap_or(0.0),
            Ok(x) if x.starts_with("mul:")=>wiz=h.input.wiz as f32*x[4..].parse::<f32>().unwrap_or(1.0)+h.mods.apply(4,0.0),
            _=>{}
        }
        let over=wiz-1200.0-crate::envc::var("JPRACE_WIZLB_SHIFT").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0);
        if over<=0.0 {return None;}
        let mult=if crate::envc::var_os("JPRACE_WIZLB_OLD").is_some() {w.arr("WizCoefArray").iter().rev().find(|r|over>=r["WizThreshold"].as_i64().unwrap() as f32).map_or(0.0,|r|f32_of(&r["Multiply"]))}
            else {w.arr("WizCoefArray").iter().find(|r|over<r["WizThreshold"].as_i64().unwrap() as f32).or(w.arr("WizCoefArray").last()).map_or(0.0,|r|f32_of(&r["Multiply"]))};
        let coef=self.wiz_limit_break_coef(h.phase,h.input.running_style,h.ex());
        let dist=w.arr("CourseDistanceCoefArray").iter().find(|r|self.course.distance<=r["CourseDistanceThreshold"].as_i64().unwrap() as f32).map_or(1.0,|r|f32_of(&r["Multiply"]));
        let mult=crate::envc::var("JPRACE_WIZLB_FORCE").ok().and_then(|x|x.split(',').find_map(|e|{let (a,b)=e.split_once(':')?;(a.parse::<usize>().ok()?==hi).then(||b.parse::<f32>().ok())?})).unwrap_or(mult);
        if crate::envc::var_os("JPRACE_DEBUG_WIZLB").is_some() {eprintln!("wizlb h={} t={:.4} id={} wiz={} mod={} over={} mult={} coef={} dist={} phase={}",hi,self.time,ival(row,"id"),h.input.wiz,h.mods.apply(4,0.0),over,mult,coef,dist,h.phase);}
        Some((mult,coef,dist))
    }
    fn wiz_limit_break_coef(&self,phase:usize,style:i32,ex:bool)->f32 {
        let w=Tree(self.p().get("WizLimitBreakBuff"));
        let (phase,ex)=(phase.min(3) as i64,i64::from(ex));
        w.arr("PhaseParamArray").iter().find(|r|r["Phase"].as_i64()==Some(phase))
            .and_then(|r|r["RunningStyleParamArray"].as_array().unwrap().iter().find(|x|x["RunningStyleEx"].as_i64()==Some(ex) && x["RunningStyle"].as_i64()==Some(style as i64)).map(|x|f32_of(&x["Coef"])))
            .unwrap_or(0.0)
    }
    fn jp_late_native(&mut self,hi:usize,dt:f32) {
        if dt<=0.0 || self.horses[hi].finished {return;}
        let sec=self.course.distance/24.0;let tick=(self.time/dt).round() as i64;
        if crate::envc::var("JPRACE_SL_EARLY_END").as_deref()!=Ok("0") {
            let tm=self.time;
            if self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==10 && tm>st+1e-3 && tm<en-1e-3) && !self.jp_sl_cond(hi) {
                for e in self.oracle.iter_mut() {if e.0==hi && e.1==10 && tm>e.2+1e-3 && tm<e.3-1e-3 {e.3=tm;}}
                if crate::envc::var_os("JPRACE_LOG_SLEND").is_some() {eprintln!("slend h={} tick={}",hi,tick);}
                self.horses[hi].jp_next[1]=tick+32;
            }
        }
        let d=self.horses[hi].distance;
        if !self.in_late_window(d) {return;}
        let tm=self.time;let dur=31.0*dt;
        let iv=|name:&str,def:i64|crate::envc::var(name).ok().and_then(|x|x.parse::<i64>().ok()).unwrap_or(def);
        let due=|s:&Self,k:usize|{let nx=s.horses[hi].jp_next[k];nx<0 || tick>=nx};
        let mut in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
        if in_keep && self.horses[hi].heal_count>self.horses[hi].keep_heal {
            for e in self.oracle.iter_mut() {if e.0==hi && e.1==9 && tm>=e.2-1e-3 && tm<e.3-1e-3 {e.3=tm;}}
            in_keep=false;
            if crate::envc::var_os("JPRACE_KEEP_REARM_OLD").is_none() {self.horses[hi].jp_keep_next=tick+1;}
        }
        let same10=|s:&Self|{let me=&s.horses[hi];s.horses.iter().enumerate().any(|(j,o)|j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex() && (o.distance-me.distance).abs()<=10.0)};
        let mut emit=|s:&mut Self,kind:i64,end:f32|{
            s.oracle.push((hi,kind,tm,end));
            let name=match kind {8=>"CompeteBeforeSpurt",9=>"StaminaKeep",_=>"SecureLead"};
            s.events.push(json!({"t":tm,"type":kind,"name":name,"params":[hi as i32,if end>=1e8 {0} else {(end*10000.0).round() as i32}],"horse":hi,"extra":""}));
        };
        let kn=self.horses[hi].jp_keep_next;
        if !in_keep && (kn<0 || tick>=kn) {
            if self.horses[hi].jp_keep_thr<=0.0 {let c=Tree(self.p().get("CompeteBeforeSpurt"));let (lo,up)=(c.f("CompeteBeforeSpurtStaminaKeepRandomRangeMin"),c.f("CompeteBeforeSpurtStaminaKeepRandomRangeMax"));self.horses[hi].jp_keep_thr=self.random.float_range(lo,up);}
            let need=self.jp_need_hp_mode(hi,"g");
            if crate::envc::var_os("JPRACE_LATE_V2").is_some() || crate::envc::var_os("JPRACE_LATE_OLD").is_some() {
                if self.horses[hi].hp<need*self.horses[hi].jp_keep_thr {
                    let q=crate::envc::var("JPRACE_LATE_NOTICE").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or_else(||{
                        let c=Tree(self.p().get("CompeteBeforeSpurt"));
                        c.f("CompeteBeforeSpurtStaminaKeepCheckNoticePerCoef")*(self.horses[hi].stat(4)/c.f("CompeteBeforeSpurtStaminaKeepWizDivisor")).powx(c.f("CompeteBeforeSpurtStaminaKeepWizExponent"))});
                    if self.random.float_max(100.0)<q {emit(self,9,1e9);in_keep=true;self.horses[hi].keep_heal=self.horses[hi].heal_count;}
                }
            } else {
                let old_keep=crate::envc::var("JPRACE_LATE_KEEP_MODE").as_deref()==Ok("old");
                let start=if old_keep {
                    let c=crate::envc::var("JPRACE_LATE_KEEP_C").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.97);
                    if self.horses[hi].hp<need*self.horses[hi].jp_keep_thr*c {self.random.float_max(100.0);true} else {false}
                } else {
                    let need=self.jp_need_hp_mode(hi,"nobuf");
                    let (coef,div,ex)={let c=Tree(self.p().get("CompeteBeforeSpurt"));(c.f("CompeteBeforeSpurtStaminaKeepCheckNoticePerCoef"),c.f("CompeteBeforeSpurtStaminaKeepWizDivisor"),c.f("CompeteBeforeSpurtStaminaKeepWizExponent"))};
                    let w=self.horses[hi].stat(4);
                    let lim=need*self.horses[hi].jp_keep_thr*math::pow(w/div,ex)*crate::envc::var("JPRACE_KEEP_C").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
                    let r=if self.horses[hi].hp<lim {let c0=self.random.calls()+1;let u=self.random.float_max(100.0);
                        if crate::envc::var_os("JPRACE_LOG_NATIVE").is_some() {eprintln!("nkeep h={} tick={} call={} ratio={:.5} u={:.2} p={:.2} start={}",hi,tick,c0,self.horses[hi].hp/lim,u,coef*math::log10(w),u<coef*math::log10(w));}
                        u<coef*math::log10(w)} else {
                        if crate::envc::var_os("JPRACE_LOG_NATIVE").is_some() {eprintln!("nkeep h={} tick={} ratio={:.5} short=false",hi,tick,self.horses[hi].hp/lim);}
                        false};
                    r
                };
                if start {
                    emit(self,9,1e9);in_keep=true;self.horses[hi].keep_heal=self.horses[hi].heal_count;
                    let kmax=if crate::envc::var_os("JPRACE_KEEP_CUT_SL").is_some() {2} else {1};
                    for k in 0..kmax {
                        let kind=if k==0 {8} else {10};
                        let mut cut=false;
                        for e in self.oracle.iter_mut() {if e.0==hi && e.1==kind && tm>=e.2-1e-3 && tm<e.3-1e-3 {e.3=tm;cut=true;}}
                        if cut {self.horses[hi].jp_next[k]=tick+32;}
                    }
                }
            }
            self.horses[hi].jp_keep_next=tick+iv("JPRACE_LATE_KEEP_IV",31);
        }
        if due(self,0) {
            let mut ok=false;
            if !in_keep {
                let (far,n,ns)=self.jp_cbs_state(hi);
                let (thr,addend)={let p=self.p();let c=Tree(p.get("CompeteBeforeSpurt"));let h=&self.horses[hi];let ex=i64::from(h.ex());
                    (c.arr("CompeteBeforeSpurtDistanceFarFromTopThresholdArray").iter().find(|r|r["RunningStyle"].as_i64()==Some(h.input.running_style as i64) && r["RunningStyleEx"].as_i64()==Some(ex)).map_or(0.0,|r|f32_of(&r["Threshold"])),
                     c.f("CompeteBeforeSpurtConsumeStaminaValueRateAddend"))};
                let top=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);
                let excess=(top-self.horses[hi].distance-thr).max(0.0);
                let pn_mode=crate::envc::var("JPRACE_PN_MODE").unwrap_or_default();
                let pn=if n>0 {let c=Tree(self.p().get("CompeteBeforeSpurt"));
                    if pn_mode.is_empty() || pn_mode=="guts" {
                        let x=(n as f32+c.f("CompeteBeforeSpurtSameRunnnigStyleCoefOfNearCharaNumCoef")*ns as f32)/self.horses.len() as f32;
                        ((c.f("CompeteBeforeSpurtCoefOfNearCharaNumCoef")*x).powx(c.f("CompeteBeforeSpurtNearDistancePerExponent"))*math::log10(self.horses[hi].stat(3))).min(100.0)}
                    else if pn_mode=="old" {
                    (c.f("CompeteBeforeSpurtCoefOfNearCharaNumCoef")*(n as f32+c.f("CompeteBeforeSpurtSameRunnnigStyleCoefOfNearCharaNumCoef")*ns as f32)).powx(c.f("CompeteBeforeSpurtNearDistancePerExponent")).min(100.0)}
                    else if pn_mode!="ns1" {
                        let na=crate::envc::var("JPRACE_PN_NA").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(8.7);
                        let ne=crate::envc::var("JPRACE_PN_NE").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.8);
                        let fk=if far {crate::envc::var("JPRACE_PN_FARK").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.03)} else {1.0};
                        let mut k=c.f("CompeteBeforeSpurtCoefOfNearCharaNumCoef")*math::pow(na/self.horses.len() as f32,ne)*fk;
                        if crate::envc::var("JPRACE_PN_FORM").as_deref()!=Ok("pow") {
                            let (a,b)=crate::envc::var("JPRACE_PN_LIN").ok().and_then(|x|{let (a,b)=x.split_once(',')?;Some((a.parse::<f32>().ok()?,b.parse::<f32>().ok()?))}).unwrap_or((219.0,8.0));
                            k=(a-b*self.horses.len() as f32)*fk;
                        }
                        let w=if far {Some(crate::envc::var("JPRACE_PN_FARW").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.5))} else {None}.unwrap_or(c.f("CompeteBeforeSpurtSameRunnnigStyleCoefOfNearCharaNumCoef"));
                        (k*(n as f32+w*ns as f32)).powx(c.f("CompeteBeforeSpurtNearDistancePerExponent")).min(100.0)}
                    else {let k=crate::envc::var("JPRACE_PN_C").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(c.f("CompeteBeforeSpurtCoefOfNearCharaNumCoef"));
                    let ws_on=!(far && crate::envc::var("JPRACE_PN_WS_FAR").as_deref()==Ok("0"));
                    let ws=crate::envc::var("JPRACE_PN_WS").ok().filter(|_|ws_on).and_then(|x|x.split(',').nth((self.horses[hi].input.running_style as usize).saturating_sub(1)).and_then(|v|v.parse::<f32>().ok()))
                        .unwrap_or(c.f("CompeteBeforeSpurtSameRunnnigStyleCoefOfNearCharaNumCoef"));
                    (k*(n as f32+(ws-1.0)*ns as f32)).powx(c.f("CompeteBeforeSpurtNearDistancePerExponent")).min(100.0)}} else {0.0};
                let pf=if crate::envc::var("JPRACE_LATE_PF").as_deref()==Ok("dist") {(56.0+4.0*excess.sqrt()).min(100.0)} else {self.jp_far_per(hi)};
                let old_model=crate::envc::var_os("JPRACE_LATE_OLD").is_some();
                let v2=crate::envc::var_os("JPRACE_LATE_V2").is_some();
                let mut near_hit=false;
                if old_model || v2 {
                    let u1=self.random.float_max(100.0);
                    if old_model {
                        if far {
                            if u1<pf {ok=true;} else if n>0 {let u2=self.random.float_max(100.0);if u2<pn {ok=true;near_hit=true;}}
                        } else if n>0 && u1<pn {ok=true;near_hit=true;}
                    } else {
                        let p=if far {pf+(100.0-pf)*pn/100.0} else {pn};
                        if u1<p {ok=true;near_hit=!far;}
                    }
                    if ok {self.random.float_max(100.0);if old_model && !same10(self) {self.random.float_max(100.0);}}
                } else {
                    let near_first=crate::envc::var("JPRACE_CBS2_ORDER").as_deref()==Ok("nf");
                    let c0=self.random.calls()+1;
                    let (uf,un)=if far {
                        let (uf,un)=if near_first {let a=self.random.float_max(100.0);let b=self.random.float_max(100.0);(b,a)} else {let a=self.random.float_max(100.0);let b=self.random.float_max(100.0);(a,b)};
                        if uf<pf {ok=true;} else if un<pn {ok=true;near_hit=true;}
                        (uf,un)
                    } else {
                        let un=self.random.float_max(100.0);
                        if un<pn {ok=true;near_hit=true;}
                        (-1.0,un)
                    };
                    if crate::envc::var_os("JPRACE_LOG_NATIVE").is_some() {
                        let me=&self.horses[hi];
                        let near:Vec<String>=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && (o.distance-me.distance)<=15.0 && (o.distance-me.distance)>=-10.0).map(|(j,o)|format!("{}:{:.2}:{}:{:.3}",j,o.distance-me.distance,o.input.running_style,o.lane-me.lane)).collect();
                        eprintln!("ncbs h={} tick={} call={} far={} n={} ns={} uf={:.2} un={:.2} pf={:.2} pn={:.2} ok={} wiz={:.1} style={} gap={:.3} excess={:.3} lane={:.4} order={} near={}",hi,tick,c0,far,n,ns,uf,un,pf,pn,ok,me.stat(4),me.input.running_style,top-me.distance,excess,me.lane,me.order,near.join(","));}
                }
                if ok {
                    emit(self,8,tm+dur);
                    self.horses[hi].jp_cbs_preset=Some(if near_hit {1.0+addend} else {1.0});
                }
            }
            self.horses[hi].jp_next[0]=tick+if ok {iv("JPRACE_OK_IV",63)} else {iv("JPRACE_FAIL_IV",31)};
        }
        if due(self,1) {
            let mut ok=false;
            if self.jp_sl_cond(hi) {
                let u=self.random.float_max(100.0);
                ok=u<20.0*math::log10(self.horses[hi].stat(4));
                if ok {
                    if crate::envc::var_os("JPRACE_LATE_OLD").is_some() && !in_keep && !same10(self) {self.random.float_max(100.0);}
                    emit(self,10,tm+dur);
                }
            }
            self.horses[hi].jp_next[1]=tick+if ok {iv("JPRACE_OK_IV",63)} else {iv("JPRACE_FAIL_IV",31)};
        }
    }
    fn jp_draw_after_move(&mut self,hi:usize,dt:f32) {
        if dt<=0.0 {return;}
        let spec=crate::envc::var("JPRACE_DRAW_SPEC").ok().or_else(||crate::envc::var("JPRACE_DRAW_AFTER_MOVE").ok().map(|o|format!("{o}|{o}")));
        let Some(spec)=spec else {return};
        let sec=self.course.distance/24.0;let tick=(self.time/dt).round() as i64;
        let d=self.horses[hi].distance;
        if !self.in_late_window(d) {return;}
        let entry=self.horses[hi].jp_next[0]<0 && self.horses[hi].jp_next[1]<0;
        let seci=(d/sec) as i32;
        let sec_change=!entry && self.horses[hi].jp_sec!=seci;
        self.horses[hi].jp_sec=seci;
        let mut part=spec.split('|').nth(if entry {0} else {1}).unwrap_or("").to_string();
        if sec_change {if let Some(x)=spec.split('|').nth(2) {part=format!("{x},{part}");}}
        let due=|s:&Self,k:usize|{let nx=s.horses[hi].jp_next[k];nx<0 || tick>=nx};
        let any_due=due(self,0)||due(self,1);
        let mut decided=[None,None];
        for tok in part.split(',').filter(|t|!t.is_empty()) {
            match tok {
                "T" if crate::envc::var("JPRACE_SKIP_T").is_ok_and(|v|v.split(';').any(|x|x==format!("{hi}:{tick}")))=>{},
                "T"=>{let c=self.random.calls()+1;let lo=1.035f32;let hi_=1.04f32;let u=self.random.float_range(lo,hi_);self.horses[hi].jp_keep_thr=u;eprintln!("draw kind=2 h={} tick={} ok=false call={} u={:.5}",hi,tick,c,u);},
                "Tn"|"Tf"|"Tnk"=>{
                    let (far,_,_)=self.jp_cbs_state(hi);
                    let started=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                    let go=match tok {"Tn"=>!far,"Tf"=>far,_=>!started};
                    if go {let c=self.random.calls()+1;let u=self.random.float_range(1.035,1.04);self.horses[hi].jp_keep_thr=u;eprintln!("draw kind=2 h={} tick={} ok=false call={} u={:.5}",hi,tick,c,u);}
                },
                "Ts"=>if self.horses[hi].slb_u.is_none() {let c=self.random.calls()+1;let u=self.random.float_range(1.035,1.04);self.horses[hi].jp_keep_thr=u;eprintln!("draw kind=2 h={} tick={} ok=false call={} u={:.5}",hi,tick,c,u);},
                "K"|"Kd"|"KL"=>{
                    if crate::envc::var_os("JPRACE_KEEP_REARM_OLD").is_none() && self.oracle.iter().any(|&(oh,kd,_,en)|oh==hi && kd==9 && en<1e8 && ((en/dt).round() as i64)+1==tick) {
                        self.horses[hi].jp_keep_next=tick;
                    }
                    let kn=self.horses[hi].jp_keep_next;
                    let ends_now=crate::envc::var_os("JPRACE_KEEP_ENDTICK_OLD").is_none() && self.oracle.iter().any(|&(oh,kd,_,en)|oh==hi && kd==9 && en<1e8 && ((en/dt).round() as i64)==tick);
                    if (kn<0 || tick>=kn) && !ends_now {
                        let tm=self.time;
                        let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                        let need=self.jp_need_hp_mode(hi,&crate::envc::var("JPRACE_KEEP_NEED").unwrap_or_else(|_|"nobuf".into()));let thr=if self.horses[hi].jp_keep_thr>0.0 {self.horses[hi].jp_keep_thr} else {1.0375};
                        let kc=crate::envc::var("JPRACE_KEEP_C").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
                        let wf={let w=self.horses[hi].stat(4);match crate::envc::var("JPRACE_KEEP_WIZ").as_deref() {Ok("none")=>1.0,Ok("neg")=>math::pow(1000.0/w,0.03),_=>math::pow(w/1000.0,0.03)}};
                        let short=self.horses[hi].hp<need*thr*kc*wf;
                        if crate::envc::var("JPRACE_LOG_KEEP").is_ok() || tok=="KL" {
                            let st=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                            let h=&self.horses[hi];
                            self.jp_need_hp_mode(hi,"parts");
                            let alts:Vec<String>=["all","allnb","tgt","sec","cur","nobuf"].iter().map(|m|{
                                format!("{:.1}",self.jp_need_hp_mode(hi,m))}).collect();
                            eprintln!("keepchk h={} tick={} start={} inkeep={} hp={:.1} max={:.1} need={:.1} d={:.1} style={} sta={:.0} guts={:.0} wiz={:.0} alts={}",hi,tick,st,in_keep,h.hp,h.max_hp,need,h.distance,h.input.running_style,h.stat(1),h.stat(3),h.stat(4),alts.join(","));
                        }
                        let started=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                        if crate::envc::var_os("JPRACE_LOG_KCHK").is_some() && (!in_keep || started) {
                            eprintln!("kchk h={} tick={} ratio={:.5} call={} short={} started={} wiz={:.1} thr={:.6} hp={:.3} need={:.3} wf={:.6} d={:.3}",hi,tick,self.horses[hi].hp/(need*thr*kc*wf),self.random.calls()+1,short,started,self.horses[hi].stat(4),thr,self.horses[hi].hp,need,wf,self.horses[hi].distance);
                        }
                        if tok!="KL" && (!in_keep || started) && (short || started || tok=="Kd") {let c=self.random.calls()+1;let u=self.random.float_max(100.0);
                            eprintln!("draw kind=9 h={} tick={} ok={} call={} u={:.2} hp={:.1} need={:.1}",hi,tick,started,c,u,self.horses[hi].hp,need*thr);}
                        else if started {eprintln!("keepstart_nodraw h={} tick={} hp={:.1} need={:.1}",hi,tick,self.horses[hi].hp,need*thr);}
                        let iv=crate::envc::var("JPRACE_KEEP_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(31);
                        self.horses[hi].jp_keep_next=tick+iv;
                    }
                },
                "Tk"=>{
                    let kn=self.horses[hi].jp_keep_next3;
                    if kn<0 || tick>=kn {
                        let tm=self.time;
                        let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                        let started=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                        if !in_keep || started || crate::envc::var_os("JPRACE_TK_INKEEP").is_some() {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=92 h={} tick={} ok={} call={} u={:.2}",hi,tick,started,c,u);}
                        self.horses[hi].jp_keep_next3=tick+31;
                    }
                },
                "Kb"=>{
                    let kn=self.horses[hi].jp_keep_next2;
                    if kn<0 || tick>=kn {
                        let tm=self.time;
                        let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                        let iv=crate::envc::var("JPRACE_KEEP_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(31);
                        let soon=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick+iv);
                        if !in_keep && soon {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=91 h={} tick={} ok=false call={} u={:.2}",hi,tick,c,u);}
                        self.horses[hi].jp_keep_next2=tick+iv;
                    }
                },
                "Ksr"=>{
                    let started=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                    if started {
                        let need=self.jp_need_hp_mode(hi,"g");
                        let c=crate::envc::var("JPRACE_KS_C").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.95);
                        let r=self.horses[hi].hp/need;
                        if r>=c {let cl=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=9 h={} tick={} ok=true call={} u={:.2} ratio={:.4}",hi,tick,cl,u,r);}
                        else {eprintln!("keepstart_nodraw h={} tick={} ratio={:.4}",hi,tick,r);}
                    }
                },
                "Ks"=>{
                    let skip=crate::envc::var("JPRACE_KS_SKIP").is_ok_and(|v|v.split(';').any(|x|x==format!("{hi}:{tick}")));
                    let started=!skip && self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                    if started {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=9 h={} tick={} ok=true call={} u={:.2}",hi,tick,c,u);}
                },
                "L"=>for (k,kind) in [(0usize,8i64),(1,10)] {
                    if !due(self,k) {continue;}
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick);
                    let (far,n,ns)=self.jp_cbs_state(hi);
                    let top=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);
                    let me=&self.horses[hi];
                    let ms10=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex() && (o.distance-me.distance).abs()<=10.0).count();
                    let ms10f=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex() && o.distance-me.distance>=0.0 && o.distance-me.distance<=10.0).count();
                    let (mut nf,mut nb,mut nsf,mut nsb)=(0,0,0,0);let mut near_list=vec![];
                    for (j,o) in self.horses.iter().enumerate() {if j==hi {continue;}let g=o.distance-me.distance;
                        if g<=15.0 && g>=-10.0 {let same=o.input.running_style==me.input.running_style && o.ex()==me.ex();
                            if g>0.0 {nf+=1;if same {nsf+=1;}} else {nb+=1;if same {nsb+=1;}}
                            let act=|kd:i64|self.oracle.iter().any(|&(oh,k2,st,en)|oh==j && k2==kd && self.time>=st-1e-3 && self.time<en-1e-3);
                            let flags=format!("{}{}{}",if act(8) {"c"} else {""},if act(10) {"l"} else {""},if act(9) {"k"} else {""});
                            near_list.push(format!("{}:{:.2}:{}{}:{:.4}:{}",j,g,o.input.running_style,if same {"s"} else {""},o.lane-me.lane,flags));}}
                    {let c=Tree(self.p().get("SecureLead"));let (so,sn)=(self.course.distance/1000.0,1.0+(self.course.distance+c.f("SecureLeadSafetyLeadAddend"))*c.f("SecureLeadSafetyLeadCoef"));
                    eprintln!("chk kind={} h={} tick={} ok={} far={} n={} ns={} slc={} keep={} style={} call={} slcb={} lead={:.2} gap={:.3} wiz={:.1} guts={:.1} hp={:.1} ms10={} ms10f={} lead_old={:.3} lead_new={:.3} nf={} nb={} nsf={} nsb={} near={} all={} pm={}",kind,hi,tick,ok,far,n,ns,self.jp_sl_cond(hi),in_keep,self.horses[hi].input.running_style,self.random.calls(),self.jp_sl_cond2(hi,true),self.jp_sl_margin(hi),top-self.horses[hi].distance,self.horses[hi].stat(4),self.horses[hi].stat(3),self.horses[hi].hp,ms10,ms10f,self.jp_sl_margin_s(hi,so),self.jp_sl_margin_s(hi,sn),nf,nb,nsf,nsb,near_list.join(","),
                        self.horses.iter().enumerate().filter(|(j,_)|*j!=hi).map(|(j,o)|format!("{}:{:.3}:{}{}:{}:{}",j,o.distance-me.distance,o.input.running_style,if o.ex() {"x"} else {""},o.order,
                            if self.oracle.iter().any(|&(oh,kd,st,en)|oh==j && kd==9 && self.time>=st-1e-3 && self.time<en-1e-3) {"k"} else {""})).collect::<Vec<_>>().join(","),self.pacemaker);}
                },
                "8x"=>if due(self,0) {
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==8 && ((st/dt).round() as i64)==tick);
                    if ok && !in_keep {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=88 h={} tick={} ok=true call={} u={:.2}",hi,tick,c,u);}
                },
                "8y"=>if due(self,0) {
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==8 && ((st/dt).round() as i64)==tick);
                    let (far,n,_)=self.jp_cbs_state(hi);
                    if ok && !in_keep && (far || n>=2) {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=88 h={} tick={} ok=true call={} u={:.2}",hi,tick,c,u);}
                },
                "8u"=>if due(self,0) {
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==8 && ((st/dt).round() as i64)==tick);
                    let (far,n,ns)=self.jp_cbs_state(hi);
                    let me=&self.horses[hi];
                    let alone=!self.horses.iter().enumerate().any(|(j,o)|j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex() && (o.distance-me.distance).abs()<=10.0);
                    let fm=crate::envc::var("JPRACE_8U_FAR").unwrap_or_else(|_|"always".into());
                    let nm=crate::envc::var("JPRACE_8U_NEAR").unwrap_or_else(|_|"never".into());
                    let slact=crate::envc::var("JPRACE_8U_SLACT").ok().and_then(|x|x.parse::<i64>().ok()).is_some_and(|w|self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==10 && {let t0=(st/dt).round() as i64;t0<tick && tick-t0<=w}));
                    let lanenow=crate::envc::var_os("JPRACE_8U_LANE").is_some() && (self.horses[hi].lane_draw_t-self.time).abs()<1e-4;
                    let lowrank=crate::envc::var("JPRACE_8U_ORDER").ok().and_then(|x|x.parse::<usize>().ok()).is_some_and(|k|self.horses[hi].order<k);
                    let far=far && !slact && !lanenow && !lowrank;
                    let pf_now=self.jp_far_per(hi);
                    let second=if far {match fm.as_str() {"ok"=>ok,"okn"=>ok||n>0,"u1fail"=>self.horses[hi].jp_cbs_u1>=pf_now,_=>true}}
                        else {ok && match nm.as_str() {"never"=>false,"alone"=>alone,"ns0"=>ns==0,"always"=>true,_=>n>=2}};
                    if !in_keep && second {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=88 h={} tick={} ok={} call={} u={:.2}",hi,tick,ok,c,u);
                        if ok && far && crate::envc::var("JPRACE_CBS_TYPE").as_deref()!=Ok("static") {
                            let pf=self.jp_far_per(hi);
                            let addend=Tree(self.p().get("CompeteBeforeSpurt")).f("CompeteBeforeSpurtConsumeStaminaValueRateAddend");
                            let near_hit=self.horses[hi].jp_cbs_u1>=pf;
                            self.horses[hi].jp_cbs_preset=Some(if near_hit {1.0+addend} else {1.0});
                        }
                    }
                },
                "8w"|"8v"=>if due(self,0) {
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==8 && ((st/dt).round() as i64)==tick);
                    let (far,n,_)=self.jp_cbs_state(hi);
                    let second=if tok=="8w" {far || (ok && n>=2)} else {(far && (ok || n>0)) || (!far && ok && n>=2)};
                    if !in_keep && second {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=88 h={} tick={} ok={} call={} u={:.2}",hi,tick,ok,c,u);}
                },
                "8nf"|"8nn"=>if due(self,0) {
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    if !in_keep {
                        let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==8 && ((st/dt).round() as i64)==tick);
                        let (far,n,_)=self.jp_cbs_state(hi);
                        let nearhit=tok=="8nn" && ok && crate::envc::var("JPRACE_CBS_NEARHIT_FILE").ok().and_then(|f|std::fs::read_to_string(f).ok())
                            .and_then(|t|serde_json::from_str::<serde_json::Value>(&t).ok())
                            .and_then(|v|v[self.input.seed.to_string()].as_str().map(|x|x.split(',').any(|e|e==format!("{hi}:{tick}")))).unwrap_or(false);
                        if far && n>0 && (!ok || nearhit) {
                            let c=self.random.calls()+1;let u=self.random.float_max(100.0);
                            eprintln!("draw kind=81 h={} tick={} ok={} call={} u={:.2}",hi,tick,ok,c,u);
                        }
                    }
                },
                "Kx"|"Ky"=>{
                    let k=if tok=="Kx" {0} else {1};
                    if due(self,k) {
                        let tm=self.time;
                        let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                        let started=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick);
                        if in_keep && !started {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=99 h={} tick={} ok=false call={} u={:.2}",hi,tick,c,u);}
                    }
                },
                "8m"|"10m"|"8M"|"10M"|"8a"|"10a"|"8f10"|"10f10"=>{
                    let kind:i64=if tok.starts_with('8') {8} else {10};
                    let tm=self.time;
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick);
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    if ok && !in_keep {
                        let me=&self.horses[hi];
                        let fwd_only=tok.ends_with("f10");
                        let same10=self.horses.iter().enumerate().any(|(j,o)|j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex()
                            && if fwd_only {o.distance-me.distance>=0.0 && o.distance-me.distance<=10.0} else {(o.distance-me.distance).abs()<=10.0});
                        let style_ok=if tok.ends_with('M') {
                            let p=self.p();let name=if kind==8 {"CompeteBeforeSpurt.CompeteBeforeSpurtMinorBonusRunningStyleCoefArray"} else {"SecureLead.SecureLeadMinorBonusRunningStyleCoefArray"};
                            Self::jp_style_coef(p.arr(name),me)>1.0
                        } else {true};
                        if (tok.ends_with('a') || !same10) && style_ok {
                            let c=self.random.calls()+1;let u=self.random.float_max(100.0);
                            eprintln!("draw kind={} h={} tick={} ok=true call={} u={:.2}",if kind==8 {89} else {109},hi,tick,c,u);
                        }
                    }
                },
                "r"=>{let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=0 h={} tick={} ok=false call={} u={:.2}",hi,tick,c,u);},
                "R"=>if any_due {let c=self.random.calls()+1;let u=self.random.float_max(100.0);eprintln!("draw kind=1 h={} tick={} ok=false call={} u={:.2}",hi,tick,c,u);},
                "8"|"10"|"8k"|"10o"|"8kd"|"10c"|"10k"|"10s"|"10cb"|"8c"|"8cs"|"8f"=>{
                    let kind:i64=if tok.starts_with('8') {8} else {10};let k=if kind==8 {0} else {1};
                    if !due(self,k) {continue;}
                    let tm=self.time;
                    let in_keep=self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3);
                    let logskip=crate::envc::var_os("JPRACE_LOG_SKIP").is_some();
                    if logskip {
                        let reason=if tok=="8k" && in_keep {"keep"} else if tok=="10c" && !self.jp_sl_cond(hi) {"slcond"} else {""};
                        if !reason.is_empty() {let (far,n,ns)=self.jp_cbs_state(hi);eprintln!("skip kind={} h={} tick={} reason={} far={} n={} ns={} slm={:.3} call={}",kind,hi,tick,reason,far,n,ns,self.jp_sl_margin(hi),self.random.calls()+1);}
                    }
                    if tok=="8k" && in_keep {continue;}
                    if tok=="8k" && crate::envc::var("JPRACE_CBS_N0").as_deref()==Ok("skip") {let (far,n,_)=self.jp_cbs_state(hi);if !far && n==0 {continue;}}
                    if tok=="10o" && self.horses[hi].input.running_style==4 {continue;}
                    if tok=="10c" && !self.jp_sl_cond(hi) {continue;}
                    if tok=="10k" && (!self.jp_sl_cond(hi) || in_keep) {continue;}
                    if tok=="10s" && (!self.jp_sl_cond(hi) || self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==9 && ((st/dt).round() as i64)==tick)) {continue;}
                    if tok=="10cb" && !self.jp_sl_cond2(hi,true) {continue;}
                    if tok=="8c"||tok=="8cs"||tok=="8f" {
                        if in_keep {continue;}
                        let (far,n,ns)=self.jp_cbs_state(hi);
                        if tok=="8c" && !(far||n>0) {continue;}
                        if tok=="8cs" && !(far||ns>0) {continue;}
                        if tok=="8f" && !far {continue;}
                    }
                    let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick);
                    let c=self.random.calls()+1;let u=self.random.float_max(100.0);
                    if kind==8 {self.horses[hi].jp_cbs_u1=u;}
                    if kind==8 && ok && crate::envc::var("JPRACE_CBS_TYPE").as_deref()!=Ok("static") && crate::envc::var("JPRACE_CBS_NEARPRE").as_deref()!=Ok("0") && !self.jp_cbs_state(hi).0 {
                        let addend=Tree(self.p().get("CompeteBeforeSpurt")).f("CompeteBeforeSpurtConsumeStaminaValueRateAddend");
                        self.horses[hi].jp_cbs_preset=Some(1.0+addend);
                    }
                    let top=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);
                    let near10=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && (o.distance-self.horses[hi].distance).abs()<=10.0).count();
                    let same10=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && o.input.running_style==self.horses[hi].input.running_style && (o.distance-self.horses[hi].distance).abs()<=10.0).count();
                    eprintln!("draw kind={} h={} tick={} ok={} call={} u={:.2} style={} wiz={} guts={} far={} n={} ns={} keep={} gap={:.3} order={} near10={} same10={} pow={}",kind,hi,tick,ok,c,u,self.horses[hi].input.running_style,self.horses[hi].stat(4),self.horses[hi].stat(3),
                        i32::from(self.jp_cbs_state(hi).0),self.jp_cbs_state(hi).1,self.jp_cbs_state(hi).2,i32::from(in_keep),top-self.horses[hi].distance,self.horses[hi].order,near10,same10,self.horses[hi].stat(2));
                    let field=self.horses.iter().map(|o|format!("{:.3}:{:.4}:{}:{}",o.distance,o.lane,o.input.running_style,i32::from(o.ex()))).collect::<Vec<_>>().join(",");
                    eprintln!("field kind={} h={} tick={} {}",kind,hi,tick,field);
                    decided[k]=Some(ok);
                },
                _=>(),
            }
        }
        for (k,kind) in [(0usize,8i64),(1,10)] {
            if due(self,k) {
                let ok=decided[k].unwrap_or_else(||self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick));
                self.horses[hi].jp_next[k]=tick+if ok {crate::envc::var("JPRACE_OK_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(63)} else {crate::envc::var("JPRACE_FAIL_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(31)};
                if ok && crate::envc::var("JPRACE_SCHED_END").as_deref()!=Ok("0") {
                    if let Some(&(_,_,_,en))=self.oracle.iter().find(|&&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick) {
                        if en<1e8 {self.horses[hi].jp_next[k]=(en/dt).round() as i64+crate::envc::var("JPRACE_END_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(32);}
                    }
                }
            }
        }
    }
    fn jp_far_per(&self,hi:usize)->f32 {
        if crate::envc::var("JPRACE_PF_MODE").as_deref()!=Ok("lin") {
            let c=Tree(self.p().get("CompeteBeforeSpurt"));
            return (c.f("CompeteBeforeSpurtDistanceFromTopPerCoef")*math::log10(self.horses[hi].stat(4)).powx(c.f("CompeteBeforeSpurtDistanceFromTopPerExponent"))).clamp(0.0,100.0);
        }
        let a=crate::envc::var("JPRACE_PF_A").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(46.0);
        let b=crate::envc::var("JPRACE_PF_B").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(-66.0);
        (a*math::log10(self.horses[hi].stat(4))+b).clamp(0.0,100.0)
    }
    fn in_late_window(&self,d:f32)->bool {
        match crate::envc::var("JPRACE_LATESEC").as_deref() {
            Ok("f64")=>{let idx=(d as f64/(self.course.distance as f64/24.0)).floor();(10.0..15.0).contains(&idx)},
            Ok("mul")=>{let idx=(d*24.0/self.course.distance).floor();(10.0..15.0).contains(&idx)},
            _=>{let sec=self.course.distance/24.0;d>=10.0*sec && d<15.0*sec},
        }
    }
    fn jp_sl_cond(&self,hi:usize)->bool {self.jp_sl_cond2(hi,false)}
    fn jp_sl_cond2(&self,hi:usize,behind_only:bool)->bool {
        let p=self.p();let h=&self.horses[hi];let ex=i64::from(h.ex());let st=h.input.running_style as i64;
        let arr=Tree(p.get("SecureLead")).arr("SecureLeadSafetyLeadRunningStyleDistanceArray");
        let Some(row)=arr.iter().find(|r|r["RunningStyle"].as_i64()==Some(st) && r["RunningStyleEx"].as_i64()==Some(ex)) else {return false};
        let scale=self.jp_sl_scale();
        self.horses.iter().enumerate().any(|(j,o)|j!=hi && row["OpponentRunningStyleDistanceThresholdArray"].as_array().unwrap().iter().any(|t|
            t["RunningStyle"].as_i64()==Some(o.input.running_style as i64) && t["RunningStyleEx"].as_i64()==Some(i64::from(o.ex())) &&
            f32_of(&t["Threshold"])>0.0 && (!behind_only || h.distance>=o.distance) && h.distance-o.distance<f32_of(&t["Threshold"])*scale))
    }
    fn jp_sl_scale(&self)->f32 {
        if crate::envc::var("JPRACE_SL_SCALE").as_deref()==Ok("old") {return self.course.distance/1000.0;}
        if let Some(v)=crate::envc::var("JPRACE_SL_SCALE").ok().and_then(|x|x.parse::<f32>().ok()) {return v;}
        let c=Tree(self.p().get("SecureLead"));
        let mul=crate::envc::var("JPRACE_SL_SCALE_MUL").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
        (1.0+(self.course.distance+c.f("SecureLeadSafetyLeadAddend"))*c.f("SecureLeadSafetyLeadCoef"))*mul
    }
    fn jp_sl_margin(&self,hi:usize)->f32 {self.jp_sl_margin_s(hi,self.jp_sl_scale())}
    fn jp_sl_margin_s(&self,hi:usize,scale:f32)->f32 {
        let p=self.p();let h=&self.horses[hi];let ex=i64::from(h.ex());let st=h.input.running_style as i64;
        let arr=Tree(p.get("SecureLead")).arr("SecureLeadSafetyLeadRunningStyleDistanceArray");
        let Some(row)=arr.iter().find(|r|r["RunningStyle"].as_i64()==Some(st) && r["RunningStyleEx"].as_i64()==Some(ex)) else {return f32::NAN};
        let mut m=f32::INFINITY;
        for (j,o) in self.horses.iter().enumerate() { if j==hi {continue;}
            for t in row["OpponentRunningStyleDistanceThresholdArray"].as_array().unwrap() {
                if t["RunningStyle"].as_i64()==Some(o.input.running_style as i64) && t["RunningStyleEx"].as_i64()==Some(i64::from(o.ex())) && f32_of(&t["Threshold"])>0.0 {
                    m=m.min(h.distance-o.distance-f32_of(&t["Threshold"])*scale);}}}
        m
    }
    fn jp_cbs_state(&self,hi:usize)->(bool,usize,usize) {
        let p=self.p();let c=Tree(p.get("CompeteBeforeSpurt"));let h=&self.horses[hi];let ex=i64::from(h.ex());
        let thr=c.arr("CompeteBeforeSpurtDistanceFarFromTopThresholdArray").iter().find(|r|r["RunningStyle"].as_i64()==Some(h.input.running_style as i64) && r["RunningStyleEx"].as_i64()==Some(ex)).map_or(0.0,|r|f32_of(&r["Threshold"]));
        let fp=crate::envc::var("JPRACE_FAR_POS").unwrap_or_else(|_|"order".into());
        let pos=|o:&Horse,me:bool|match fp.as_str() {"prev"=>o.prev_distance,"topprev" if !me=>o.prev_distance,"selfprev" if me=>o.prev_distance,_=>o.distance};
        let top=if fp=="order" {self.horses.iter().min_by_key(|o|o.order).map_or(0.0,|o|o.distance)}
            else if fp=="seqidx" {self.horses.iter().enumerate().map(|(j,o)|if j<=hi {o.distance} else {o.prev_distance}).fold(f32::MIN,f32::max)}
            else if fp=="seqidxr" {self.horses.iter().enumerate().map(|(j,o)|if j>=hi {o.distance} else {o.prev_distance}).fold(f32::MIN,f32::max)}
            else if fp=="seqrank" {let me=self.horses[hi].order;self.horses.iter().enumerate().map(|(j,o)|if j==hi || o.order<me {o.distance} else {o.prev_distance}).fold(f32::MIN,f32::max)}
            else if fp=="seqrankr" {let me=self.horses[hi].order;self.horses.iter().enumerate().map(|(j,o)|if j==hi || o.order>me {o.distance} else {o.prev_distance}).fold(f32::MIN,f32::max)}
            else {self.horses.iter().map(|o|pos(o,false)).fold(f32::MIN,f32::max)};
        let my=pos(h,true);
        let (fw,bw)=(c.f("CompeteBeforeSpurtNearDistanceForward"),c.f("CompeteBeforeSpurtNearDistanceBackward"));
        let mut n=0;let mut ns=0;
        let npm=crate::envc::var("JPRACE_NEAR_POS").unwrap_or_else(|_|"prev".into());let np=npm=="prev";
        for (j,o) in self.horses.iter().enumerate() {
            if j==hi {continue;}
            let od=match npm.as_str() {"seqidx" if j>hi=>o.prev_distance,"seqidxr" if j<hi=>o.prev_distance,_=>o.distance};
            let g=if np {o.prev_distance-h.prev_distance} else {od-h.distance};
            if g<=fw && g>=-bw {n+=1;if o.input.running_style==h.input.running_style && o.ex()==h.ex() {ns+=1;}}
        }
        let scale_ok=h.input.running_style as u8>=crate::envc::var("JPRACE_FAR_SCALE_MIN").ok().and_then(|x|x.parse::<u8>().ok()).unwrap_or(3);
        let thr=if !scale_ok {thr} else {match crate::envc::var("JPRACE_FAR_SCALE").as_deref() {
            Err(_)=>thr*(1.0+(self.course.distance-1000.0)*0.0008),
            Ok("old")=>thr*self.course.distance/1000.0,
            Ok("sl")=>thr*self.jp_sl_scale(),
            Ok("none")=>thr,
            Ok(x) if x.starts_with("lin")=>thr*(1.0+(self.course.distance-1000.0)*x[3..].parse::<f32>().unwrap_or(0.001)),
            Ok(x) if x.starts_with("dyn")=>thr*h.distance*x[3..].parse::<f32>().unwrap_or(0.002),
            Ok(x) if x.starts_with("top")=>thr*top*x[3..].parse::<f32>().unwrap_or(0.002),
            Ok(x)=>x.parse::<f32>().map_or(thr,|m|thr*m),
        }};
        let thr=thr+crate::envc::var("JPRACE_FAR_ADD").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0);
        (top-my>thr,n,ns)
    }
    fn jp_need_hp(&self,hi:usize)->f32 {
        let mode=crate::envc::var("JPRACE_NEED_MODE").unwrap_or_default();
        self.jp_need_hp_mode(hi,&mode)
    }
    fn jp_need_hp_mode(&self,hi:usize,mode:&str)->f32 {
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let sbn=if crate::envc::var("JPRACE_SEC_N").as_deref()==Ok("spurt") {&h.section_base} else {&h.section_base_n};
        let normal=sbn.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==2).map_or(base,|(_,v)|*v);
        let spd=if crate::envc::var_os("JPRACE_NEED_UNC").is_some() {h.mods.apply(0,h.speed_unc).max(h.stat(0))} else {h.stat(0)};
        let max={
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(spd*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let cached_add=base*p.f("lastSpurtBaseTargetSpeedAddCoef");
            let base_arg=cached_add+normal;
            let no=crate::envc::var("JPRACE_NEED_ORD").ok();
            if no.as_deref()==Some("gsb") {guts*p.f("LastSpurtTargetSpeedGutsCoef2")+(speed*h.dist_speed*p.f("addSpeedParamCoef")+base_arg*p.f("baseTargetSpeedCoef"))} else
            if (f32m('p')||f32m('P')) || no.as_deref()==Some("f32") {guts*p.f("LastSpurtTargetSpeedGutsCoef2")+speed*h.dist_speed*p.f("addSpeedParamCoef")+base_arg*p.f("baseTargetSpeedCoef")} else {
            (guts as f64*p.f("LastSpurtTargetSpeedGutsCoef2") as f64+
                speed as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64+
                base_arg as f64*p.f("baseTargetSpeedCoef") as f64) as f32}
        };
        let max=max+if crate::envc::var_os("JPRACE_NEED_SLB").is_some() && h.slb_u.is_some() {self.slb_value(hi)} else {0.0}
            +if crate::envc::var("JPRACE_NEED_FS").as_deref()!=Ok("0") && h.input.speed>=2001 && fs_enabled() {self.fs_extra_max(hi)} else {0.0};
        let phase2=self.course.distance*2.0/3.0;
        let mid=(phase2-h.distance).max(0.0);let mid_speed=sbn[((h.distance/(self.course.distance/24.0)) as usize).min(23)];
        let remain=(self.course.distance-h.distance.max(phase2)-p.i("LastSpurtDistanceGoalBuffer") as f32).max(0.0);
        let mode=mode.to_string();
        let gf=if f32m('h') {p.f("Hp.HpGutsCoef")/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt()+1.0} else {(p.f("Hp.HpGutsCoef") as f64/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt() as f64+1.0) as f32};
        let end_rate=self.hp_rate(h,max,base,p.f("Hp.HpDecRateBaseNormal"))*if h.phase>=2 {1.0} else {gf};
        if mode=="parts" {
            eprintln!("needparts h={} d={:.3} mid={:.3} midspd={:.5} midrate={:.6} remain={:.3} max={:.5} endrate={:.6} gf={:.6} phase={} normal={:.5} normrate={:.6} maxrate1={:.6} hpdec={:.6}",
                hi,h.distance,(phase2-h.distance).max(0.0),mid_speed,self.hp_rate(h,mid_speed,base,p.f("Hp.HpDecRateBaseNormal")),remain,max,end_rate,gf,h.phase,normal,
                self.hp_rate(h,normal,base,p.f("Hp.HpDecRateBaseNormal")),self.hp_rate(h,max,base,p.f("Hp.HpDecRateBaseNormal")),p.f("Hp.HpDecRateBaseNormal"));
            return 0.0;
        }
        if mode=="all" {let r=(self.course.distance-h.distance-p.i("LastSpurtDistanceGoalBuffer") as f32).max(0.0);return r/max*end_rate;}
        if mode=="g" {
            let full=(self.course.distance-h.distance.max(phase2)).max(0.0);
            return mid/normal*self.hp_rate(h,normal,base,p.f("Hp.HpDecRateBaseNormal"))+full/max*end_rate;
        }
        if mode=="allnb" {let r=(self.course.distance-h.distance).max(0.0);return r/max*end_rate;}
        let mid_speed=match mode.as_str() {"tgt"=>h.target_speed.max(1.0),"sec"=>h.section_speed[((h.distance/(self.course.distance/24.0)) as usize).min(23)],"cur"=>h.speed.max(1.0),_=>mid_speed};
        let remain=if mode=="nobuf" {(self.course.distance-h.distance.max(phase2)).max(0.0)} else {remain};
        let fk=|n:&str|crate::envc::var(n).ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
        let end_rate=if crate::envc::var("JPRACE_NEED_GF").as_deref()==Ok("0") {self.hp_rate(h,max,base,p.f("Hp.HpDecRateBaseNormal"))} else {end_rate};
        let mid_speed=match crate::envc::var("JPRACE_NEED_MIDSPD").as_deref() {Ok("sec")=>h.section_speed[((h.distance/(self.course.distance/24.0)) as usize).min(23)],Ok("normal")=>normal,Ok("tgt")=>h.target_speed.max(1.0),Ok("secbase")=>mid_speed,_=>if mode=="nobuf" {base} else {mid_speed}};
        let sc=match crate::envc::var("JPRACE_NEED_STYLE").as_deref() {Ok("cbs_end")|Ok("cbs_all")=>Self::jp_style_coef(p.arr("CompeteBeforeSpurt.CompeteBeforeSpurtConsumeStaminaRunningStyleCoefArray"),h),_=>1.0};
        let (sm,se)=match crate::envc::var("JPRACE_NEED_STYLE").as_deref() {Ok("cbs_end")=>(1.0,sc),Ok("cbs_all")=>(sc,sc),_=>(1.0,1.0)};
        if mode=="nobuf" && crate::envc::var_os("JPRACE_LOG_NEEDPARTS").is_some() {
            let sec=self.course.distance/24.0;let si=((h.distance/sec) as usize).min(23);
            eprintln!("needparts2 h={} tick={} d={:.4} mid={:.4} midspd={:.6} secspd={:.6} tgt={:.6} cur={:.6} normal={:.6} midrate={:.6} remain={:.4} max={:.6} endrate={:.6} gf={:.6} phase={} spd={:.3} guts={:.3} base={:.6} hp={:.4} maxhp={:.4} adj0={:.3} adj3={:.3} base0={:.3} base3={:.3} unc0={:.3} m0={:.3} m3={:.3} dist={:.4} pow={:.3} sta={:.3} wiz={:.3}",
                hi,(self.time/0.0666).round() as i64,h.distance,mid,mid_speed,h.section_speed[si],h.target_speed,h.speed,normal,self.hp_rate(h,mid_speed,base,p.f("Hp.HpDecRateBaseNormal")),remain,max,end_rate,gf,h.phase,h.stat(0),h.stat(3),base,h.hp,h.max_hp,
                h.adjusted[0],h.adjusted[3],h.base[0],h.base[3],h.speed_unc,h.mods.apply(0,0.0),h.mods.apply(3,0.0),h.dist_speed,h.stat(2),h.stat(1),h.stat(4));
        }
        let mid_part=if crate::envc::var("JPRACE_NEED_MIDSPD").as_deref()==Ok("secsum") {
            let sec=self.course.distance/24.0;let mut acc=0.0f32;
            let s0=((h.distance/sec) as usize).min(23);
            for s in s0..16 {let a=(s as f32*sec).max(h.distance);let b=((s+1) as f32*sec).min(phase2);if b<=a {continue;}let v=h.section_speed[s];
                acc+=(b-a)/v*self.hp_rate(h,v,base,p.f("Hp.HpDecRateBaseNormal"));}
            acc
        } else {mid/mid_speed*self.hp_rate(h,mid_speed,base,p.f("Hp.HpDecRateBaseNormal"))};
        mid_part*fk("JPRACE_NEED_MIDK")*sm+remain/max*end_rate*fk("JPRACE_NEED_ENDK")*se
    }
    fn jp_style_coef(arr:&[Value],h:&Horse)->f32 {
        let ex=i64::from(h.ex());
        arr.iter().find(|r|r["RunningStyleEx"].as_i64()==Some(ex) && r["RunningStyle"].as_i64()==Some(h.input.running_style as i64)).map_or(1.0,|r|f32_of(&r["Coef"]))
    }
    fn jp_minor_mult(&self,hi:usize,kind:i64,tick:i64)->f32 {
        let mode=crate::envc::var("JPRACE_MINOR").unwrap_or_default();
        if mode=="0" {return 1.0;}
        let me=&self.horses[hi];let p=self.p();
        let (name,nd)=if kind==8 {("CompeteBeforeSpurt.CompeteBeforeSpurtMinorBonusRunningStyleCoefArray","CompeteBeforeSpurt.CompeteBeforeSpurtMinorBonusNearDistance")}
            else {("SecureLead.SecureLeadMinorBonusRunningStyleCoefArray","SecureLead.SecureLeadMinorBonusNearDistance")};
        let nd=p.f(nd);
        let mpos=crate::envc::var("JPRACE_MINOR_POS").unwrap_or_else(|_|"prev".into());
        let pd=|o:&Horse|if mpos=="prev" {o.prev_distance} else {o.distance};
        let near:Vec<f32>=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex()).map(|(_,o)|pd(o)-pd(me)).collect();
        let rule=crate::envc::var("JPRACE_MINOR_RULE").unwrap_or_else(|_|"share".into());
        let share_rule=if rule.starts_with("share") {
            let thr=p.f(if kind==8 {"CompeteBeforeSpurt.CompeteBeforeSpurtMinorBonusPerThreshold"} else {"SecureLead.SecureLeadMinorBonusPerThreshold"});
            let near_all=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && (pd(o)-pd(me)).abs()<=nd).count() as f32;
            let same=near.iter().filter(|g|g.abs()<=nd).count() as f32;
            let (a,b)=if rule=="shareself" {(same+1.0,near_all+1.0)} else {(same,near_all)};
            if crate::envc::var_os("JPRACE_LOG_MINORCNT").is_some() {
                let ids:Vec<String>=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && (o.distance-me.distance).abs()<=nd).map(|(j,o)|format!("{}:{:.2}",j,o.distance-me.distance)).collect();
                eprintln!("minorcnt h={} tick={} kind={} same={} near={} ids={}",hi,tick,kind,same,near_all,ids.join(","));
            }
            Some(b<=0.0 || a*100.0/b<thr)
        } else {None};
        let alone=if let Some(x)=share_rule {x} else {match crate::envc::var("JPRACE_MINOR_DIR").as_deref() {
            Ok("behind")=>!near.iter().any(|&g|g<=0.0 && -g<=nd),
            Ok("ahead")=>!near.iter().any(|&g|g>=0.0 && g<=nd),
            _=>!near.iter().any(|g|g.abs()<=nd)}};
        let coef=Self::jp_style_coef(p.arr(name),me);
        let pick=!mode.starts_with("list:") || mode.strip_prefix("list:").is_some_and(|l|l.split(';').any(|x|x==format!("{hi}:{tick}:{kind}")));
        let share_ok=if crate::envc::var("JPRACE_MINOR_SHARE").as_deref()==Ok("1") {
            let thr=p.f(if kind==8 {"CompeteBeforeSpurt.CompeteBeforeSpurtMinorBonusPerThreshold"} else {"SecureLead.SecureLeadMinorBonusPerThreshold"});
            let cnt=self.horses.iter().filter(|o|o.input.running_style==me.input.running_style && o.ex()==me.ex()).count() as f32;
            cnt*100.0/(self.horses.len() as f32)<thr
        } else {true};
        let force=crate::envc::var("JPRACE_MINOR_FORCE").ok().is_some_and(|l|l.split(';').any(|x|x==format!("{hi}:{tick}:{kind}")));
        let mult=if (alone && pick && share_ok) || force {coef} else {1.0};
        if crate::envc::var_os("JPRACE_LOG_MINOR").is_some() {
            let mut nn:Vec<String>=self.horses.iter().enumerate().filter(|(j,o)|*j!=hi && o.input.running_style==me.input.running_style && o.ex()==me.ex()).map(|(j,o)|format!("{:.2}@{:.3}#{}",o.distance-me.distance,o.lane-me.lane,j)).collect();nn.sort();
            eprintln!("minor h={} tick={} kind={} style={} ex={} alone={} coef={} mult={} same={}",hi,tick,kind,me.input.running_style,me.ex(),alone,coef,mult,nn.join(","));
        }
        mult
    }
    fn jp_mode_speed(&self,hi:usize,kind:i64)->f32 {
        let h=&self.horses[hi];let p=self.p();
        match kind {
            8=>{let c=Tree(p.get("CompeteBeforeSpurt"));
                let mode=crate::envc::var("JPRACE_CBS_RATE").unwrap_or_default();
                let rate=if mode.contains('s') {h.jp_cbs_rate} else if mode.contains('h') && h.jp_cbs_near {0.5} else {1.0};
                (((h.stat(2)/c.f("CompeteBeforeSpurtSpeedUpPowDivisor")).powx(c.f("CompeteBeforeSpurtSpeedUpPowExponent"))*c.f("CompeteBeforeSpurtSpeedUpPowCoef")
                  +(h.stat(3)/c.f("CompeteBeforeSpurtSpeedUpGutsDivisor")).powx(c.f("CompeteBeforeSpurtSpeedUpGutsExponent")))
                 *c.f("CompeteBeforeSpurtSpeedUpCoef")*Self::jp_style_coef(c.arr("CompeteBeforeSpurtSpeedUpRunningStyleCoefArray"),h))*rate},
            10=>{let c=Tree(p.get("SecureLead"));
                (h.stat(3)/c.f("SecureLeadSpeedUpGutsDivisor")).powx(c.f("SecureLeadSpeedUpGutsExponent"))*c.f("SecureLeadSpeedUpCoef")
                 *Self::jp_style_coef(c.arr("SecureLeadSpeedUpRunningStyleCoefArray"),h)},
            _=>0.0,
        }
    }
    fn jp_mode_consume(&self,hi:usize,kind:i64)->f32 {
        let h=&self.horses[hi];let p=self.p();
        let (name,pre)=match kind {8=>("CompeteBeforeSpurt","CompeteBeforeSpurt"),10=>("SecureLead","SecureLead"),_=>return 0.0};
        let c=Tree(p.get(name));
        let dist=c.arr(&format!("{pre}ConsumeStaminaCourseDistanceCoefArray")).iter().find(|r|self.course.distance<f32_of(&r["Threshold"])).map_or(1.0,|r|f32_of(&r["Coef"]));
        let mut v=c.i(&format!("{pre}ConsumeStaminaValue")) as f32*Self::jp_style_coef(c.arr(&format!("{pre}ConsumeStaminaRunningStyleCoefArray")),h)*dist;
        if kind==8 && crate::envc::var("JPRACE_CBS_RATE").map(|x|x.contains('c')||x.contains('n')).unwrap_or(true) {v*=h.jp_cbs_rate;}
        v
    }
    fn hp_rate(&self,h:&Horse,speed:f32,base:f32,rate:f32)->f32 {
        let p=self.p();
        let x=math::pow(speed-base+p.f("Hp.SpeedGapParam1"),2.0);
        if f32m('h') {
            let (pw,db,g)=(p.f("Hp.SpeedGapParam1Pow"),p.f("Hp.HpDecBase"),self.ground_multi());
            let mut v=match crate::envc::var("JPRACE_HP_ORD").ok().and_then(|o|o.parse::<i32>().ok()).unwrap_or(0) {
                1=>x*db/pw*rate*g, 2=>db*x/pw*rate*g, 3=>x/pw*(db*rate*g), 4=>x/pw*db*(rate*g), 5=>rate*(x/pw*db)*g, 6=>x*(db/pw)*rate*g,
                _=>x/pw*db*rate*g};
            if h.phase>=2 {
                let gc=p.f("Hp.HpGutsCoef")/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt()+1.0;
                v=match crate::envc::var("JPRACE_HP_GUTS").as_deref() {Ok("add")=>v+v*(p.f("Hp.HpGutsCoef")/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt()),Ok("f64")=>(v as f64*(p.f("Hp.HpGutsCoef") as f64/((h.stat(3)*p.f("Hp.HpGutsCoefSqrt")) as f64).sqrt()+1.0)) as f32,_=>v*gc};
            }
            return v;
        }
        let mut v=((x/p.f("Hp.SpeedGapParam1Pow")) as f64*p.f("Hp.HpDecBase") as f64*rate as f64*self.ground_multi() as f64) as f32;
        if h.phase>=2 {
            v*=(p.f("Hp.HpGutsCoef") as f64/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt() as f64+1.0) as f32;
        }
        v
    }
    fn slb_value(&self,hi:usize)->f32 {
        let h=&self.horses[hi];let p=self.p();let x=Tree(p.get("StaminaLimitBreakBuff"));
        let mood=self.rate("race_motivation_rate",h.input.motivation,"motivation_rate").unwrap_or(1.0);
        let raw=h.input.stamina as f32;let halved=if raw>1200.0 {1200.0+(raw-1200.0)*0.5} else {raw};
        let sta=match crate::envc::var("JPRACE_SLB_S").as_deref() {Ok("stat")=>h.stat(1),Ok("old")=>raw*mood+h.mods.apply(1,0.0),_=>h.base[1]+(raw-halved)+h.mods.apply(1,0.0)};
        let over=(sta-1200.0).max(0.0);
        let dist=x.arr("TargetSpeedDistanceCoefArray").iter().find(|r|(self.course.distance as i64)<r["DistanceThreshold"].as_i64().unwrap()).map_or(0.0,|r|f32_of(&r["Multiply"]));
        let k=crate::envc::var("JPRACE_SLB_K").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(x.f("TargetSpeedForWholeCoef"));
        let base=match crate::envc::var("JPRACE_SLB_F").as_deref() {Ok("lin")=>over*x.f("TargetSpeedForStaminaCoef"),_=>(over*x.f("TargetSpeedForStaminaCoef")).sqrt()};
        let rnd=if crate::envc::var("JPRACE_SLB_RAND").as_deref()==Ok("0") {1.0} else if let Some([u1,u2])=h.slb_u {
            let tab=x.arr("TargetSpeedRandomTableArray");
            let byp=x.0["TargetSpeedRandomTableChangeProbabilityByPower"].as_i64().unwrap_or(0);
            let rawp=h.input.pow as f32;let halvedp=if rawp>1200.0 {1200.0+(rawp-1200.0)*0.5} else {rawp};
            let powe=match crate::envc::var("JPRACE_SLB_POW").as_deref() {Ok("lb")=>h.base[2]+(rawp-halvedp),Ok("stat")=>h.base[2],Ok("off")=>0.0,_=>rawp};
            let lower=tab.iter().find(|r|r["TableType"].as_i64()==Some(1)).map_or(0,|r|r["Probability"].as_i64().unwrap());
            let shift=(((powe-1200.0).max(0.0)) as i64*byp).min(lower);
            let prob=|r:&serde_json::Value|->i64 {let p=r["Probability"].as_i64().unwrap();match r["TableType"].as_i64() {Some(1)=>p-shift,Some(2)=>p+shift,_=>p}};
            let total:i64=tab.iter().map(prob).sum();
            let mut acc=0i64;let pick=(u1*total as f64) as i64;
            let row=tab.iter().find(|r|{acc+=prob(r);pick<acc}).unwrap_or(&tab[0]);
            let (lo,up)=(row["LowerLimit"].as_i64().unwrap() as f64,row["UpperLimit"].as_i64().unwrap() as f64);
            match crate::envc::var("JPRACE_SLB_RINT").as_deref() {
                Ok("0")=>((lo+(up-lo)*u2)/10000.0) as f32,
                Ok("f32")=>(lo+((up-lo)*u2).floor()) as f32/10000.0f32,
                Ok("mul")=>(lo+((up-lo)*u2).floor()) as f32*0.0001f32,
                _=>((lo+((up-lo)*u2).floor())/10000.0) as f32,
            }
        } else {1.0};
        let v=base*k*dist*rnd;
        if let Some(u)=crate::envc::var("JPRACE_SLB_ULP").ok().and_then(|o|o.split(',').find_map(|kv|{let (a,b)=kv.split_once(':')?;(a.parse::<usize>().ok()?==hi).then(||b.parse::<i32>().ok()).flatten()})) {
            return f32::from_bits(v.to_bits().wrapping_add_signed(u));
        }
        v
    }
    fn fs_foundation(&self,hi:usize)->f32 {
        let h=&self.horses[hi];let x=Tree(self.p().get("ExtraTargetSpeed"));
        let s=match crate::envc::var("JPRACE_FS_SPD").as_deref() {Ok("nomod")=>h.speed_unc,Ok("raw")=>h.input.speed as f32,_=>h.mods.apply(0,h.speed_unc)};
        let (p0,p1,f0,f1)=(x.f("ExtraTargetSpeedStartParam"),x.f("ExtraTargetSpeedMaxParam"),x.f("ExtraTargetSpeedStartFoundation"),x.f("ExtraTargetSpeedMaxFoundation"));
        let r=((s-p0)/(p1-p0)).clamp(0.0,1.0);
        f0+(f1-f0)*r
    }
    fn fs_extra_max(&self,hi:usize)->f32 {
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let calc=|spd:f32|->f32 {
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2]));
            let add=(spd*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt()*h.dist_speed*p.f("addSpeedParamCoef");
            let normal=per*0.01*base+add;
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(spd*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let (g,sp,b)=(guts*p.f("LastSpurtTargetSpeedGutsCoef2"),speed*h.dist_speed*p.f("addSpeedParamCoef"),(base*p.f("lastSpurtBaseTargetSpeedAddCoef")+normal)*p.f("baseTargetSpeedCoef"));
            if crate::envc::var("JPRACE_FS_JORD").as_deref()==Ok("old") {g+sp+b} else {g+(sp+b)}
        };
        let unc=match crate::envc::var("JPRACE_FS_SPD").as_deref() {Ok("nomod")=>h.speed_unc,Ok("raw")=>h.input.speed as f32,
            Ok("rawmod")=>h.mods.apply(0,h.input.speed as f32),Ok(x) if x.starts_with("add:")=>h.mods.apply(0,h.speed_unc)+x[4..].parse::<f32>().unwrap_or(0.0),_=>h.mods.apply(0,h.speed_unc)};
        if crate::envc::var("JPRACE_FS_JQ").as_deref()==Ok("1") && h.spurt_speed>0.0 {return calc(unc)-h.spurt_speed;}
        let s0=h.stat(0);
        match crate::envc::var("JPRACE_FS_JV").as_deref() {
            Ok("A")=>{
                let c64=|spd:f32|->f64 {
                    let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
                    let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2])) as f64;
                    let add=((spd as f64)*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef") as f64).sqrt()*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64;
                    let normal=per*0.01*base as f64+add;
                    let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow")) as f64;
                    let speed=((spd as f64)*p.f("LastSpurtTargetSpeedCoefSqrt") as f64).sqrt();
                    guts*p.f("LastSpurtTargetSpeedGutsCoef2") as f64+speed*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64+(base as f64*p.f("lastSpurtBaseTargetSpeedAddCoef") as f64+normal)*p.f("baseTargetSpeedCoef") as f64
                };
                return (c64(unc)-c64(s0)) as f32;
            }
            Ok("B")=>{
                let sp=|x:f32|(x*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
                let ad=|x:f32|(x*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt();
                return (sp(unc)-sp(s0))*h.dist_speed*p.f("addSpeedParamCoef")+(ad(unc)-ad(s0))*h.dist_speed*p.f("addSpeedParamCoef")*p.f("baseTargetSpeedCoef");
            }
            Ok("C")=>{
                let sp=|x:f32|((x as f64*p.f("LastSpurtTargetSpeedCoefSqrt") as f64).sqrt()) as f32;
                let ad=|x:f32|((x as f64*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef") as f64).sqrt()) as f32;
                let cc=|x:f32|{let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
                    let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2]));
                    let normal=per*0.01*base+ad(x)*h.dist_speed*p.f("addSpeedParamCoef");
                    let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
                    guts*p.f("LastSpurtTargetSpeedGutsCoef2")+sp(x)*h.dist_speed*p.f("addSpeedParamCoef")+(base*p.f("lastSpurtBaseTargetSpeedAddCoef")+normal)*p.f("baseTargetSpeedCoef")};
                return cc(unc)-cc(s0);
            }
            _=>{}
        }
        if crate::envc::var_os("JPRACE_LOG_J").is_some() {
            let cur=calc(unc)-calc(s0);
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2]));
            let gp=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            eprintln!("jlog tick={} h={} unc={:08x} s0={:08x} guts={:08x} ds={:08x} base={:08x} per={:08x} cur={:08x} cu={:08x} cs={:08x}",(self.time/0.0666).round() as i64,hi,unc.to_bits(),s0.to_bits(),gp.to_bits(),h.dist_speed.to_bits(),base.to_bits(),per.to_bits(),cur.to_bits(),calc(unc).to_bits(),calc(s0).to_bits());
        }
        let j=calc(unc)-calc(s0);
        if let Some(k)=crate::envc::var("JPRACE_FS_JULP").ok().and_then(|o|o.split(',').find_map(|kv|{let (a,b)=kv.split_once('=')?;(a.parse::<usize>().ok()?==hi).then(||b.parse::<i32>().ok()).flatten()})) {
            if j>0.0 {return f32::from_bits((j.to_bits() as i32+k) as u32);}
        }
        j
    }
    fn fs_extra_est_mood(&self,hi:usize)->f32 {
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let mood=self.rate("race_motivation_rate",h.input.motivation,"motivation_rate").unwrap_or(1.0);
        let calc=|spd:f32|->f32 {
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2]));
            let add=(spd*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt()*h.dist_speed*p.f("addSpeedParamCoef");
            let normal=per*0.01*base+add;
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(spd*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let (g,sp,b)=(guts*p.f("LastSpurtTargetSpeedGutsCoef2"),speed*h.dist_speed*p.f("addSpeedParamCoef"),(base*p.f("lastSpurtBaseTargetSpeedAddCoef")+normal)*p.f("baseTargetSpeedCoef"));
            g+(sp+b)
        };
        let u=h.speed_unc+h.mods.apply(0,0.0)*mood;
        (calc(u)-calc(u.min(2000.0))).max(0.0)
    }
    fn fs_extra_est(&self,hi:usize)->f32 {
        let m=crate::envc::var("JPRACE_SPURT_JSPD").unwrap_or_else(|_|"plain".into());
        if m=="plain" {return self.fs_extra_max(hi);}
        if m=="moodmax" {let a=self.fs_extra_est_mood(hi);return a.max(self.fs_extra_max(hi));}
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let mood=self.rate("race_motivation_rate",h.input.motivation,"motivation_rate").unwrap_or(1.0);
        let calc=|spd:f32|->f32 {
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()} else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            let per=p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[2]));
            let add=(spd*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt()*h.dist_speed*p.f("addSpeedParamCoef");
            let normal=per*0.01*base+add;
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(spd*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let (g,sp,b)=(guts*p.f("LastSpurtTargetSpeedGutsCoef2"),speed*h.dist_speed*p.f("addSpeedParamCoef"),(base*p.f("lastSpurtBaseTargetSpeedAddCoef")+normal)*p.f("baseTargetSpeedCoef"));
            if crate::envc::var("JPRACE_FS_JORDE").as_deref()==Ok("old") {g+sp+b} else {g+(sp+b)}
        };
        let u=match m.as_str() {"moodmod"=>h.speed_unc+h.mods.apply(0,0.0)*mood,_=>h.mods.apply(0,h.speed_unc)};
        (calc(u)-calc(u.min(2000.0))).max(0.0)
    }
    fn fs_pre_add(&self,hi:usize)->f32 {
        let h=&self.horses[hi];let p=self.p();
        if (h.input.speed as f32)<Tree(p.get("ExtraTargetSpeed")).f("ExtraTargetRawSpeedParam") {return 0.0;}
        let unc=h.mods.apply(0,h.speed_unc);let s0=h.stat(0);
        let add=|spd:f32|(spd*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt()*h.dist_speed*p.f("addSpeedParamCoef");
        (add(unc)-add(s0)).max(0.0)
    }
    fn spurt_r1_max(&self,hi:usize)->Option<f32> {
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        if h.finished || h.phase<2 || h.spurt_calculated {return None;}
        let normal=h.section_base_n.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==h.phase).map_or(base,|(_,v)|*v);
        let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
        let speed=(h.stat(0)*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
        let base_arg=base*p.f("lastSpurtBaseTargetSpeedAddCoef")+normal;
        let max=guts*p.f("LastSpurtTargetSpeedGutsCoef2")+speed*h.dist_speed*p.f("addSpeedParamCoef")+base_arg*p.f("baseTargetSpeedCoef");
        let remain=(self.course.distance-h.distance-p.i("LastSpurtDistanceGoalBuffer") as f32).max(0.0);
        let need=remain/max*self.hp_rate(h,max,base,p.f("Hp.HpDecRateBaseNormal"));
        (need<=h.hp).then_some(max)
    }
    fn spurt(&mut self,hi:usize) {
        if crate::envc::var_os("JPRACE_LOG_ADDL2").is_some() {eprintln!("addl2 {}",self.p().get("Skill.AdditionalActivateAbilityMaxCountArray"));}
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let sbn=if crate::envc::var("JPRACE_SEC_N").as_deref()==Ok("keep") {&h.section_base} else {&h.section_base_n};
        let normal=sbn.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==h.phase).map_or(base,|(_,v)|*v);
        let normal_plain=h.section_base.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==h.phase).map_or(base,|(_,v)|*v);
        let ns=crate::envc::var("JPRACE_SEC_NS").ok();
        let (normal_max,normal_hp)=match ns.as_deref() {Some("max")=>(normal,normal_plain),Some("hp")=>(normal_plain,normal),_=>(normal,normal)};
        let normal=normal_hp;
        let max={
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(h.stat(0)*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let cached_add=base*p.f("lastSpurtBaseTargetSpeedAddCoef");
            let base_arg=cached_add+normal_max;
            let (g,sp,b)=(guts*p.f("LastSpurtTargetSpeedGutsCoef2"),speed*h.dist_speed*p.f("addSpeedParamCoef"),base_arg*p.f("baseTargetSpeedCoef"));
            let (g6,s6,b6)=(guts as f64*p.f("LastSpurtTargetSpeedGutsCoef2") as f64,speed as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64,base_arg as f64*p.f("baseTargetSpeedCoef") as f64);
            let spm=crate::envc::var("JPRACE_SPMAX").ok().and_then(|x|x.parse::<i32>().ok()).unwrap_or(0);
            if crate::envc::var("JPRACE_LOG_SPMAX").as_deref()==Ok("all") || crate::envc::var("JPRACE_LOG_SPMAX").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                eprintln!("spmax t={:.4} g={:e} s={:e} b={:e} guts={:e} speed={:e} ds={:e} base_arg={:e} v64={:e} q={:e} v2={:e} v4={:e}",self.time,g,sp,b,guts,speed,h.dist_speed,base_arg,(g6+s6+b6) as f32,g+sp+b,g+(sp+b),g+b+sp);
                let hx=|x:f32|format!("{:08x}",x.to_bits());
                eprintln!("spmaxraw h={} stat0={} stat3={} base={} normal={} ds={} c1={} pw={} c2={} csq={} cadd={} cas={} cb={} v64={} q={}",hi,hx(h.stat(0)),hx(h.stat(3)),hx(base),hx(normal),hx(h.dist_speed),
                    hx(p.f("LastSpurtTargetSpeedGutsCoef1")),hx(p.f("LastSpurtTargetSpeedGutsPow")),hx(p.f("LastSpurtTargetSpeedGutsCoef2")),hx(p.f("LastSpurtTargetSpeedCoefSqrt")),
                    hx(p.f("lastSpurtBaseTargetSpeedAddCoef")),hx(p.f("addSpeedParamCoef")),hx(p.f("baseTargetSpeedCoef")),hx((g6+s6+b6) as f32),hx(g+sp+b));eprintln!("spmax64 h={} q2={:08x} q={} g={:016x} s={:016x} b={:016x} g6={:016x} s6={:016x} b6={:016x}",hi,(g+(sp+b)).to_bits(),hx(g+sp+b),(g as f64).to_bits(),(sp as f64).to_bits(),(b as f64).to_bits(),g6.to_bits(),s6.to_bits(),b6.to_bits());}
            let ovr=crate::envc::var("JPRACE_SPMAX_ULP").ok().and_then(|o|o.split(',').find_map(|kv|kv.split_once('=').filter(|(x,_)|x.trim().parse::<usize>().ok()==Some(hi)).and_then(|(_,y)|y.trim().parse::<i32>().ok())));
            if (crate::envc::var("JPRACE_LOG_SPMAX2").as_deref()==Ok("all") || crate::envc::var("JPRACE_LOG_SPMAX2").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi)) && !h.spurt_calculated {
                let hx=|x:f32|format!("{:08x}",x.to_bits());
                let nf=h.section_base_f.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==h.phase).map_or(base,|(_,v)|*v);
                eprintln!("spm2 h={} t={:.4} stat3={} c1={} pw={} phase={} guts={} speed={} ds={} base={} normal={} normal_fma={} c2={} cas={} cadd={} cb={} q={} stat0={} csq={}",hi,self.time,hx(h.stat(3)),hx(p.f("LastSpurtTargetSpeedGutsCoef1")),hx(p.f("LastSpurtTargetSpeedGutsPow")),h.phase,hx(guts),hx(speed),hx(h.dist_speed),hx(base),hx(normal_plain),hx(nf),
                    hx(p.f("LastSpurtTargetSpeedGutsCoef2")),hx(p.f("addSpeedParamCoef")),hx(p.f("lastSpurtBaseTargetSpeedAddCoef")),hx(p.f("baseTargetSpeedCoef")),hx(g+sp+b),hx(h.stat(0)),hx(p.f("LastSpurtTargetSpeedCoefSqrt")));
            }
            if let (Some(k),false)=(ovr,h.spurt_calculated) {f32::from_bits((g+sp+b).to_bits().wrapping_add_signed(k))} else
            if spm>0 {match spm {1=>g+sp+b,2=>g+(sp+b),3=>b+sp+g,4=>g+b+sp,5=>(g6 as f32+s6 as f32)+b6 as f32,
                6=>g+speed*(h.dist_speed*p.f("addSpeedParamCoef"))+b,7=>((g6+s6) as f32+b6 as f32),8=>(g6 as f32+(s6+b6) as f32),
                9=>((g6+b6) as f32+s6 as f32),10=>(b6+s6+g6) as f32,_=>g+sp+b}} else
            if (f32m('p')||f32m('q')) && crate::envc::var("JPRACE_SPMAX_ORD").as_deref()!=Ok("old") {g+(sp+b)} else
            if (f32m('p')||f32m('q')) {guts*p.f("LastSpurtTargetSpeedGutsCoef2")+speed*h.dist_speed*p.f("addSpeedParamCoef")+base_arg*p.f("baseTargetSpeedCoef")} else {
            (guts as f64*p.f("LastSpurtTargetSpeedGutsCoef2") as f64+
                speed as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64+
                base_arg as f64*p.f("baseTargetSpeedCoef") as f64) as f32}
        };
        if crate::envc::var("JPRACE_LOG_SPURTCALC").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("spurtcalc h={} t={:.4} stat0={} adj0={} base0={} mods0={} guts={} dist_speed={} normal={} base={} max={} phase={} d={:.3} speed_unc={} skills0={:?}",hi,self.time,h.stat(0),h.adjusted[0],h.base[0],h.mods.apply(0,0.0),h.stat(3),h.dist_speed,normal,base,max,h.phase,h.distance,h.speed_unc,
                h.mods.entries.iter().filter(|m|m.index==0).map(|m|(m.value,m.remaining)).collect::<Vec<_>>());
        }
        let remain=(self.course.distance-h.distance-p.i("LastSpurtDistanceGoalBuffer") as f32).max(0.0);
        let m23m=crate::envc::var("JPRACE_SPURT_M23").ok();
        let sj=match crate::envc::var("JPRACE_SPURT_J").ok() {Some(x) if x=="off"=>None,Some(x)=>Some(x),None=>Some("both".to_string())};
        let jmul=crate::envc::var("JPRACE_SPURT_JMUL").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
        let jre=crate::envc::var("JPRACE_SPURT_JRE").unwrap_or_default();
        let jest=if h.spurt_calculated && (jre.is_empty() || jre=="max" || (jre=="maxsp" && h.spurt_start>0.0 && h.distance>=h.spurt_start)) {self.fs_extra_max(hi)} else if h.spurt_calculated && jre=="fsj" {h.fs_j} else {self.fs_extra_est(hi)};
        let jadd=if sj.is_some() && h.input.speed as f32>=Tree(p.get("ExtraTargetSpeed")).f("ExtraTargetRawSpeedParam") {jest.max(0.0)*jmul} else {0.0};
        let m23m=if sj.is_some() && m23m.is_none() {Some(format!("fixed:{}",jadd))} else {m23m};
        let both_j=matches!(sj.as_deref(),Some("both")|Some("bothsort"));
        let m23=match m23m.as_deref() {Some(x) if x.starts_with("fixed:")=>x[6..].parse::<f32>().unwrap_or(0.0),
            Some("nolb")=>h.mods.entries.iter().filter(|m|m.index==23 && m.lb.is_none() && !m.once).map(|m|m.value).sum::<f32>(),
            Some(_)=>h.mods.apply(23,0.0),None=>0.0};
        let both=m23m.as_deref()==Some("both") || both_j;
        let ex=match crate::envc::var("JPRACE_SPURT_EXTRA").as_deref() {
            Ok("fsx")=>h.fs_x, Ok("slb")=>if h.slb_on {h.slb_v} else {0.0}, Ok("fsxslb")=>h.fs_x+if h.slb_on {h.slb_v} else {0.0}, Ok("all")=>h.fs_x+(if h.slb_on {h.slb_v} else {0.0})+h.mods.apply(23,0.0)+h.mods.apply(5,0.0), Ok("cur")=>h.speed-h.spurt_speed, Ok(x) if x.starts_with("fixed:")=>x[6..].parse::<f32>().unwrap_or(0.0), _=>0.0};
        let ro_gate=crate::envc::var("JPRACE_SPURT_RAFSONLY").as_deref()!=Ok("1") || h.rafs_on;
        let m23=m23+if h.spurt_calculated && ro_gate {ex} else {0.0};
        let need=|speed:f32|remain/(speed+if both {m23} else {0.0})*self.hp_rate(h,speed+m23,base,p.f("Hp.HpDecRateBaseNormal"));
        let hp_eff=h.hp*crate::envc::var("JPRACE_SPURT_HPF").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0)
            -h.max_hp*crate::envc::var("JPRACE_SPURT_HPSUB").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0);
        let hpc=crate::envc::var("JPRACE_RAFS_HPC").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(2.075);
        let hp_cand=if h.spurt_calculated && (h.rafs_on || h.fs_pending) {hp_eff-hpc*h.hp_chg} else {hp_eff};
        if let Ok(m)=crate::envc::var("JPRACE_LOG_SPURTIN") && (m!="re" || h.spurt_calculated) {
            let gc=p.f("Hp.HpGutsCoef")/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt()+1.0;
            eprintln!("spurtin h={} t={:.4} calc={} hp={} remain={} normal={} max={} m23={} both={} base={} rate={} gm={} gc={} sgp1={} sgpw={} hpdec={} d={} goal={} maxfg={} cur={} self={} tgt={} m23now={} fsx={} slbv={} slbon={} rafs={} prev={} pspd={} pst={} phase={} delta={} delta2={} thr={} spv0={} fsj={} m5={} jadd={} m6now={} fsr={} min={} slope={} down={}",
                hi,self.time,h.spurt_calculated,hp_eff,remain,normal,max,m23,both,base,p.f("Hp.HpDecRateBaseNormal"),self.ground_multi(),gc,p.f("Hp.SpeedGapParam1"),p.f("Hp.SpeedGapParam1Pow"),p.f("Hp.HpDecBase"),h.distance,
                self.course.distance-p.i("LastSpurtDistanceGoalBuffer") as f32,p.f("lastSpurtDistanceMaxFromGoal"),h.speed,h.self_speed,h.target_speed,h.mods.apply(23,0.0),h.fs_x,h.slb_v,h.slb_on,h.rafs_on,h.spurt_result,h.spurt_speed,h.spurt_start,h.phase,
                p.f("lastSpurtSpeedDelta"),p.f("lastSpurtStep2SpeedDelta"),h.stat(4)*p.f("lastSpurtStep2PerVal2")+p.f("lastSpurtStep2PerVal1"),h.spv0,h.fs_j,h.mods.apply(5,0.0),jadd,h.mods.apply(6,0.0),h.fs_r,h.min_speed,h.slope,h.downhill);
        }
        let mut now_spd:Option<f32>=None;
        let (result,start,speed)=if hp_eff<need(h.min_speed) {(4,-1.0,base)}
            else if need(max)<=hp_eff {(1,h.distance,max)}
            else {
                let normal=if h.spurt_calculated {match crate::envc::var("JPRACE_SPURT_NORMAL").as_deref() {Ok("cur")=>h.speed,Ok("self")=>h.self_speed,Ok("tgt")=>h.target_speed,Ok("spv")=>if h.distance>=h.spurt_start && h.spurt_start>0.0 {h.spurt_speed} else {normal},_=>normal}}
                    else {match crate::envc::var("JPRACE_SPURT_NORMAL0").as_deref() {Ok("tgt")=>h.target_speed,Ok("tgtns")=>h.target_speed-h.m6_used,Ok("down")=>normal+if h.downhill {h.target_speed-h.m6_used-h.tgt_base} else {0.0},_=>normal}};
                if crate::envc::var("JPRACE_LOG_SPURTCAND").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("spurtnormal h={} normal={:.5} tgt={:.5} m6={:.5} tb={:.5} down={}",hi,normal,h.target_speed,h.m6_used,h.tgt_base,h.downhill);}
                let (n_rate,n_fit,n_sort)={
                    let on=crate::envc::var("JPRACE_SPURT_NPRE").as_deref()!=Ok("0");
                    let f=crate::envc::var("JPRACE_SPURT_NPRE_F").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
                    let m=crate::envc::var("JPRACE_SPURT_NPRE_M").unwrap_or_else(|_|"rt".into());
                    let a=if on {f*self.fs_pre_add(hi)} else {0.0};
                    (normal+if m.contains('r') {a} else {0.0},normal+if m.contains('t') {a} else {0.0},normal+if m.contains('s') {a} else {0.0})
                };
                let n_sort=if h.spurt_calculated {match crate::envc::var("JPRACE_SPURT_RSORT").as_deref() {Ok("cur")=>h.speed,Ok("self")=>h.self_speed,Ok("tgt")=>h.target_speed,_=>n_sort}} else {n_sort};
                let rsort_m=crate::envc::var("JPRACE_SPURT_RSORT").unwrap_or_else(|_|"jsall".into());
                let rs_all=rsort_m=="jsall";
                let rsort_j=rsort_m=="j" || rsort_m=="js" || rs_all;
                let n_sort=if (rsort_m=="js" && h.spurt_calculated) || rs_all {normal+self.fs_pre_add(hi)} else {n_sort};
                let sadj=crate::envc::var("JPRACE_SPURT_SORTADJ").unwrap_or_default();
                let n_sort=n_sort
                    +if sadj.contains('u') && h.slope>0.0 {up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0)} else {0.0}
                    +if sadj.contains('m') {h.mods.apply(5,0.0)} else {0.0}
                    +if sadj.contains('d') && h.downhill && h.slope<0.0 {(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0)} else {0.0};
                let n_sort=n_sort-crate::envc::var("JPRACE_SPURT_SORTSUB").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0);
                let n_sort=n_sort*crate::envc::var("JPRACE_SPURT_SORTMUL").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
                if rsort_m=="js" && h.spurt_calculated && crate::envc::var("JPRACE_LOG_SPURTCAND").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("rsortjs h={} normal={} pre={} m23={}",hi,normal,self.fs_pre_add(hi),m23);}
                let candidate=|speed:f32|->Option<(f32,f32)> {
                    let spurt_rate=self.hp_rate(h,speed+m23,base,p.f("Hp.HpDecRateBaseNormal"));
                    let normal_rate=self.hp_rate(h,n_rate,base,p.f("Hp.HpDecRateBaseNormal"));
                    let goal=self.course.distance-p.i("LastSpurtDistanceGoalBuffer") as f32;
                    let mut d=remain;
                    while d>0.0 {
                        let vs=if both_j {speed+m23} else {speed};
                        let spurt_time=if sj.as_deref()==Some("bothsort") || (rsort_j && (h.spurt_calculated || rs_all)) {d/(speed+m23)} else {d/speed};let normal_time=(remain-d)/n_sort;
                        let kf=if crate::envc::var("JPRACE_SPURT_KEEP").as_deref()==Ok("1") && h.jp_keep_thr>0.0 {h.jp_keep_thr} else {1.0};
                        let fits=if (f32m('p')||f32m('k')) {(d/vs*spurt_rate+(remain-d)/n_fit*normal_rate)*kf<=hp_cand} else {(d as f64/vs as f64*spurt_rate as f64+(remain as f64-d as f64)/n_fit as f64*normal_rate as f64)*kf as f64<=hp_cand as f64};
                        if fits && goal-d<self.course.distance-p.f("lastSpurtDistanceMaxFromGoal") {
                            return Some((goal-d,normal_time+spurt_time));
                        } d-=30.0;
                    } None
                };
                let floor=normal.min(max);let mut speed=max;let mut first=None;
                for _ in 0..=100 {
                    if candidate(speed).is_some() {first=Some(speed);break;}
                    speed-=p.f("lastSpurtSpeedDelta");if speed<=floor {break;}
                }
                if let Some(mut speed)=first {
                    let mut candidates=vec![];
                    for _ in 0..=100 {
                        if let Some((start,time))=candidate(speed) {candidates.push((start,speed,time));}
                        speed-=p.f("lastSpurtStep2SpeedDelta");if speed<=floor {break;}
                    }
                    now_spd=candidates.iter().filter(|c|(c.0-h.distance).abs()<1e-3).map(|c|c.1).fold(None,|a:Option<f32>,x|Some(a.map_or(x,|y|y.max(x))));
                    if h.spurt_calculated && ro_gate && crate::envc::var("JPRACE_SPURT_RECALC_NOW").as_deref()==Ok("1") {candidates.retain(|c|(c.0-h.distance).abs()<1e-3);}
                    if !h.spurt_calculated && crate::envc::var("JPRACE_SPURT_NONOW").as_deref()==Ok("1") && candidates.iter().any(|c|(c.0-h.distance).abs()>=1e-3) {candidates.retain(|c|(c.0-h.distance).abs()>=1e-3);}
                    netsort::sort(&mut candidates,&|a,b|((a.2-b.2)*10000.0) as i32);
                    let threshold=h.stat(4)*p.f("lastSpurtStep2PerVal2")+p.f("lastSpurtStep2PerVal1");
                    let threshold=crate::envc::var("JPRACE_SPURT_THR").ok().and_then(|v|v.split(',').find_map(|x|{let (a,b)=x.split_once(':')?;(a.parse::<usize>().ok()?==hi).then(||b.parse::<f32>().ok()).flatten()})).unwrap_or(threshold);
                    let mut chosen=candidates.last().copied().unwrap_or((h.distance,first.unwrap(),0.0));
                    if crate::envc::var_os("JPRACE_LOG_R2M").is_some() {
                        let vnow=now_spd.unwrap_or(0.0);
                        let act:Vec<String>=h.skills.iter().flat_map(|sk|sk.details.iter().filter(|d|d.active).map(move |d|(sk.id,d))).filter_map(|(id,d)|{
                            let v:f32=h.mods.entries.iter().filter(|m|m.owner==d.uid && m.index==23 && !m.once).map(|m|m.value).sum();(v!=0.0).then(||format!("{}:{:.4}@{:.3}",id,v,d.left))}).collect();
                        eprintln!("r2m h={} tick={} hp={:.4} vnow={:.4} m_now={:.4} m_up={:.4} max={:.4} n={} m23={:.4} act={}",hi,(self.time/0.0666).round() as i64,hp_eff,vnow,hp_eff-need(vnow),hp_eff-need(vnow+p.f("lastSpurtStep2SpeedDelta")),max,candidates.len(),h.mods.apply(23,0.0),act.join("|"));
                        eprintln!("r2x h={} raw={} fsj={:.5} fsx={:.5} fsr={:.5} rafs={} unc={:.3} jcalc={:.5}",hi,h.input.speed,h.fs_j,h.fs_x,h.fs_r,h.rafs_on,h.speed_unc,self.fs_extra_max(hi));
                    }
                    let logc=crate::envc::var("JPRACE_LOG_SPURTCAND").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) || (h.spurt_calculated && crate::envc::var("JPRACE_LOG_SPURTCAND").as_deref()==Ok("re"));
                    if logc {eprintln!("spurtcand h={} t={:.4} d={:.3} hp={:.3} thr={:.3} n={} cands={}",hi,self.time,h.distance,h.hp,threshold,candidates.len(),
                        candidates.iter().map(|c|format!("{:.2}@{:.4}/{:.3}",c.0,c.1,c.2)).collect::<Vec<_>>().join(","));
                        {let goal=self.course.distance-p.i("LastSpurtDistanceGoalBuffer") as f32;
                        let nr=self.hp_rate(h,n_rate,base,p.f("Hp.HpDecRateBaseNormal"));
                        let ms:Vec<String>=candidates.iter().map(|c|{let sr=self.hp_rate(h,c.1+m23,base,p.f("Hp.HpDecRateBaseNormal"));let d=(goal-c.0) as f64;let vs=if both_j {c.1+m23} else {c.1};
                            format!("{:.2}@{:.4}:{:+.4}",c.0,c.1,hp_eff as f64-(d/vs as f64*sr as f64+(remain as f64-d)/n_fit as f64*nr as f64))}).collect();
                        eprintln!("spurtmargin h={} hp_eff={} {}",hi,hp_eff,ms.join(","));}
                        let mut v=max;let mut xs=vec![];
                        for _ in 0..12 {xs.push(format!("{:.4}:{:.2}",v,remain/v*self.hp_rate(h,v,base,p.f("Hp.HpDecRateBaseNormal"))));v-=p.f("lastSpurtSpeedDelta");}
                        eprintln!("spurtneed h={} max={:.4} normal={:.4} remain={:.3} {}",hi,max,normal,remain,xs.join(","));}
                    let spc=crate::envc::var_os("JPRACE_LOG_SPCALL").is_some();
                    let spc_m:Vec<String>=if spc {let goal=self.course.distance-p.i("LastSpurtDistanceGoalBuffer") as f32;
                        let nr=self.hp_rate(h,n_rate,base,p.f("Hp.HpDecRateBaseNormal"));
                        candidates.iter().take(6).map(|c|{let sr=self.hp_rate(h,c.1+m23,base,p.f("Hp.HpDecRateBaseNormal"));let d=(goal-c.0) as f64;let vs=if both_j {c.1+m23} else {c.1};
                            format!("{:.2}@{:.4}:{:+.4}",c.0,c.1,hp_eff as f64-(d/vs as f64*sr as f64+(remain as f64-d) /n_fit as f64*nr as f64))}).collect()} else {vec![]};
                    let pick=crate::envc::var(if h.spurt_calculated {"JPRACE_SPURT_PICK2"} else {"JPRACE_SPURT_PICK"}).ok().and_then(|v|v.split(',').find_map(|x|{let (a,b)=x.split_once(':')?;(a.parse::<usize>().ok()?==hi).then(||b.parse::<usize>().ok()).flatten()}));
                    let forced=pick.and_then(|k|candidates.get(k).copied());
                    if h.spurt_calculated && match crate::envc::var("JPRACE_SPURT_RECALC_DRAW").as_deref() {Ok("1")=>true,Ok("2")=>h.spurt_result==2,Ok("1r")=>h.spurt_result==1,_=>false} {self.random.int_range(0,100);}
                    let spc_n=candidates.len();let mut spc_i=-1i32;let mut spc_r=vec![];let mut spc_k=0i32;
                    for c in candidates {
                        let call=self.random.calls()+1;let r=self.random.int_range(0,100);
                        if logc {eprintln!("spurtdraw h={} call={} r={} start={:.2} speed={:.4}",hi,call,r,c.0,c.1);}
                        spc_r.push(r);
                        if (r as f32)<threshold {chosen=c;spc_i=spc_k;break;}
                        spc_k+=1;
                    }
                    if spc {eprintln!("spcall h={} t={:.4} tick={} d={:.3} calc={} hp={:.3} thr={:.3} n={} idx={} r={:?} chosen={:.2}@{:.4} m=[{}] jadd={:.5} jmax={:.5} jmood={:.5} mood={:.3} unc={:.3} mods0={:.3}",hi,self.time,(self.time/0.0666).round() as i64,h.distance,h.spurt_calculated,hp_eff,threshold,spc_n,spc_i,spc_r,chosen.0,chosen.1,spc_m.join(","),jadd,self.fs_extra_max(hi),self.fs_extra_est_mood(hi),self.rate("race_motivation_rate",h.input.motivation,"motivation_rate").unwrap_or(1.0),h.speed_unc,h.mods.apply(0,0.0));}
                    if let Some(c)=forced {chosen=c;}
                    if let Some((a,b2))=crate::envc::var(if h.spurt_calculated {"JPRACE_SPURT_SET2"} else {"JPRACE_SPURT_SET"}).ok().and_then(|v|v.split(',').find_map(|x|{let f:Vec<&str>=x.split(':').collect();(f.len()==3 && f[0].parse::<usize>().ok()==Some(hi)).then(||(f[1].parse::<f32>().ok(),f[2].parse::<f32>().ok()))})) {
                        if let (Some(a),Some(b2))=(a,b2) {chosen=(a,b2,0.0);}
                    }
                    if crate::envc::var_os("JPRACE_LOG_R2").is_some() {
                        let cm=self.compete.modes.get(hi);
                        eprintln!("r2 h={} tick={} hp={:.2} maxhp={:.1} keep={} tempt={} down={} front={:?} m23={:.3} m5={:.3} m13={:.3} spd={:.4} self={:.4} tgt={:.4} slope={} phase={} lane={:.4} cm_top={} chosen={:.2}@{:.4} first={:.2}@{:.4} ncand={}",
                            hi,(self.time/0.0666).round() as i64,h.hp,h.max_hp,h.keep.mode,h.tempted(),h.downhill,h.front,h.mods.apply(23,0.0),h.mods.apply(5,0.0),h.mods.apply(13,0.0),h.speed,h.self_speed,h.target_speed,h.slope,h.phase,h.lane,
                            cm.is_some_and(|m|m.top),chosen.0,chosen.1,0.0,first.unwrap_or(0.0),0);
                    }
                    (2,chosen.0.max(h.distance),chosen.1)
                } else {(8,-1.0,base)}
            };
        let h=&mut self.horses[hi];
        if crate::envc::var_os("JPRACE_LOG_SPURTRES").is_some() {eprintln!("spurtres h={} t={:.4} result={} speed={:.5} start={:.3} calculated={} prev={} pspeed={:.5} pstart={:.3} d={:.3} hp={:.3} rafs={} slb={} npc={}",hi,self.time,result,speed,start,h.spurt_calculated,h.spurt_result,h.spurt_speed,h.spurt_start,h.distance,h.hp,h.rafs_on,h.slb_on,h.input.npc_type);}
        let fs_elig=h.input.speed>=2001;
        if !h.spurt_calculated {h.spv0=if result==2 && (crate::envc::var("JPRACE_SLB_SPV0").as_deref()==Ok("now") || (fs_elig && crate::envc::var("JPRACE_SLB_SPV0").as_deref()==Ok("nowfs"))) {now_spd.unwrap_or(speed)} else {speed};}
        if !h.spurt_calculated || result&3!=0 {
            h.spurt_result=result;h.spurt_speed=speed;
            if !h.spurt_calculated || h.spurt_start<0.0 {h.spurt_start=start;}
            else if match crate::envc::var("JPRACE_RECALC_START").as_deref() {Ok("upd")=>h.distance<h.spurt_start,Ok("min")=>start<h.spurt_start,Ok("all")=>true,_=>false} {h.spurt_start=start;}
            else if crate::envc::var("JPRACE_RECALC_START").as_deref()==Ok("tgt") && result==2 && start<h.spurt_start {h.rc_start=start;}
            let js=crate::envc::var("JPRACE_RECALC_JSTART").unwrap_or_default();
            if h.spurt_calculated && h.spurt_start>0.0 && (js=="1" || js=="latest") {
                let cur=if h.j_start>0.0 {h.j_start.min(h.spurt_start)} else {h.spurt_start};
                if js=="latest" || start<cur {h.j_start=start;}
            }
        } h.spurt_calculated=true;
    }
    fn around(&mut self,hi:usize) {
        let p=self.p();let h=&self.horses[hi];let mut front=None;let mut inside=None;let mut outside=None;let mut nearest=f32::MAX;let mut near=0;let mut fi_in=false;
        let side_gap=crate::envc::var("JPRACE_SIDE_GAP").ok().and_then(|x|x.parse().ok()).unwrap_or(p.f("Block.SideBlockDistanceGap"));
        let mut around=Vec::with_capacity(self.horses.len());let mut visible=vec![];
        let mut in_space=h.lane-h.lane_min;let mut out_space=h.lane_max-h.lane;
        let vis=h.mods.apply(19,p.f("visibleDistance")).max(0.0);
        for (i,n) in self.horses.iter().enumerate() {
            if i==hi {continue;}
            let gap=n.distance-h.distance;let lane=n.lane-h.lane;let abs=lane.abs();
            around.push(Around{i,gap,gap_abs:gap.abs(),lane,lane_abs:abs,speed_gap:h.speed-n.speed});
            if gap.abs()<=p.f("Near.DistanceThreshold") && abs<=p.f("Near.LaneDistanceThreshold") {near+=1;}
            if crate::envc::var("JPRACE_DEBUG_BLOCK").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) && gap>0.0 && gap<p.f("Block.FrontBlockDistanceGap") && abs<0.05 {
                eprintln!("blk t={:.4} hi={} o={} gap={:.6} abs={:.7} thr={:.7}",self.time,hi,i,gap,abs,(1.0-gap/p.f("Block.FrontBlockDistanceGap")*0.6)*p.f("Block.FrontBlockLaneGap"));}
            if gap>0.0 && gap<p.f("Block.FrontBlockDistanceGap") && abs<=(1.0-gap/p.f("Block.FrontBlockDistanceGap")*0.6)*p.f("Block.FrontBlockLaneGap") && gap<nearest {
                nearest=gap;front=Some(i);
            }
            if crate::envc::var("JPRACE_DEBUG_BLOCK").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) && gap.abs()<side_gap*1.5 && abs<p.f("Block.SideBlockLaneGap")*1.3 {
                eprintln!("side t={:.4} hi={} o={} gap={:.4} lane={:.5} hit={}",self.time,hi,i,gap,lane,gap.abs()<side_gap && abs<p.f("Block.SideBlockLaneGap") && lane!=0.0);}
            if h.lane_off_ro && gap.abs()<side_gap && lane<0.0 && let Some(e)=crate::envc::var("JPRACE_RO_SIDEEPS").ok().and_then(|x|x.parse::<f32>().ok()) && abs<p.f("Block.SideBlockLaneGap")+e
                && crate::envc::var("JPRACE_RO_SIDEEPS_H").map_or(true,|v|v.split(',').any(|x|x.parse::<usize>().ok()==Some(hi))) {fi_in=true;}
            if h.lane_off_ro && lane<0.0 && abs<p.f("Block.SideBlockLaneGap") && let Some(d)=crate::envc::var("JPRACE_RO_FIGAP").ok().and_then(|x|x.parse::<f32>().ok()) && (if crate::envc::var_os("JPRACE_RO_FIGAP_BACK").is_some() {gap> -d && gap<side_gap} else {gap.abs()<d}) {fi_in=true;}
            if h.lane_off_ro && lane<0.0 && let Ok(m)=crate::envc::var("JPRACE_RO_FIMODE") {
                let v:Vec<f32>=m.split(':').filter_map(|x|x.parse().ok()).collect();
                if v.len()==3 && abs<v[0] && gap> -v[1] && gap<v[2] {fi_in=true;}
            }
            if crate::envc::var_os("JPRACE_LOG_FIPAIR").is_some() && h.lane_off_ro && lane<0.0 && abs>0.111 && abs<0.1112 && self.time<0.01 {eprintln!("fipair h={} o={} hl={:08x} ol={:08x} diff={:08x} abs={:.9}",hi,i,h.lane.to_bits(),n.lane.to_bits(),lane.to_bits(),abs);}
            if gap.abs()<side_gap && abs<p.f("Block.SideBlockLaneGap") && lane!=0.0 {
                if lane<0.0 {inside=Some(i);} else {outside=Some(i);}
            }
            if crate::envc::var("JPRACE_DEBUG_SPACE").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) && gap.abs()<side_gap*2.0 {
                eprintln!("space tick={} hi={} o={} gap={:.5} lane={:.5} in_win={} o_d={:.4} o_prev={:.4} h_d={:.4} h_prev={:.4}",(self.time/0.0666).round() as i64,hi,i,gap,lane,gap.abs()<side_gap,n.distance,n.prev_distance,h.distance,h.prev_distance);}
            if !(gap< -side_gap) {
                let sg2=crate::envc::var("JPRACE_SPACE_GAP").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(side_gap);
                let fwd=crate::envc::var("JPRACE_SPACE_GAP_FWD").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(sg2);
                let back=crate::envc::var("JPRACE_SPACE_GAP_BACK").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(sg2);
                if gap.abs()<sg2 && gap<fwd && gap> -back {
                    if lane<0.0 {if abs<in_space {in_space=abs;}} else if lane>0.0 && abs<out_space {out_space=abs;}
                }
                if vis>0.0 && gap>=0.0 && gap<=vis &&
                    abs<=((gap/vis)*f32::from_bits(0x3f238e39)+f32::from_bits(0x3de38e39))*0.5 {visible.push(i);}
            }
        }
        if let Ok(x)=crate::envc::var("JPRACE_LOG_AROUND") && let Some((a,b))=x.split_once(':') && a.parse::<usize>().ok()==Some(hi) && let Ok(o)=b.parse::<usize>() {
            if let Some(e)=around.iter().find(|e|e.i==o) {eprintln!("around tick={} h={} o={} gap={:.5} lane={:.5} h_d={:.4} o_d={:.4} h_lane={:.5} o_lane={:.5}",(self.time/0.0666).round() as i64,hi,o,e.gap,e.lane,self.horses[hi].distance,self.horses[o].distance,self.horses[hi].lane,self.horses[o].lane);}
        }
        let h=&mut self.horses[hi];h.prev_side=(h.inside,h.outside);h.prev_front=h.front;h.front=front;h.inside=inside;h.outside=outside;h.near_count=near;h.fi_in=fi_in;
        let sp=self.assets.params();
        let (g1,l1,g2,l2)=(sp.f("Surrounded.DistanceGapAbs1"),sp.f("Surrounded.LaneGap1"),sp.f("Surrounded.DistanceGap2"),sp.f("Surrounded.LaneGapAbs2"));
        let h=&mut self.horses[hi];
        h.surround[0]=false;h.surround[2]=false;h.surround[1]=false;
        for a in &around {
            if !h.surround[0] && a.gap_abs<=g1 && 0.0<=a.lane && a.lane<=l1 {h.surround[0]=true;}
            if !h.surround[1] && a.gap>0.0 && a.gap<=g2 && a.lane_abs<=l2 {h.surround[1]=true;}
            if !h.surround[2] && a.gap<0.0 && -g2<=a.gap && a.lane_abs<=l2 {h.surround[2]=true;}
        }
        h.around=around;h.visible=visible;h.in_space=in_space;h.out_space=out_space;
    }
    fn compete_runner(&self,hi:usize,stale:bool)->crate::compete::Runner {
        use crate::compete::Style;
        let h=&self.horses[hi];let c=&self.course;
        crate::compete::Runner {style:if h.ex() {Style::Oonige} else if h.input.running_style==1 {Style::Nige} else {Style::Other},
            distance:if stale {h.cf_pos[0]} else {h.distance},lane:if stale {h.cf_pos[1]} else {h.lane},speed:if stale {h.cf_speed} else {h.speed},hp:h.hp,max_hp:h.max_hp,guts:h.stat(3),strategy_proficiency:h.style_rate,
            rank:h.order+1,final_straight:if crate::envc::var_os("JPRACE_LASTSTRAIGHT_DIST").is_some() {h.straight && c.last_straight_start().is_some_and(|st|h.distance>=st)} else {h.in_last_straight},tempted:h.tempted()}
    }
    fn compete_step(&mut self,dt:f32) {
        let runners=(0..self.horses.len()).map(|i|self.compete_runner(i,false)).collect::<Vec<_>>();
        let params=crate::compete::Params{
            fight_exit_inclusive: true,
            fight_select_before_history: true,
            top_cohort:true,top_end_exit:true,top_group_end:crate::envc::var("JPRACE_TOP_GROUPEND").as_deref()!=Ok("0"),top_requires_lead:crate::envc::var("JPRACE_TOP_LEAD").ok().and_then(|x|x.parse().ok()).unwrap_or(4),..Default::default()};
        let (mut next,mut changes)=crate::compete::step(&self.compete,&runners,self.course.distance,dt,&params);
        if crate::envc::var("JPRACE_DEBUG_COMPETE").is_ok() && !changes.is_empty() {eprintln!("compete t={:.4} {:?} d={:?}",self.time,changes,changes.iter().map(|c|match c {crate::compete::Change::TopEnter(i)|crate::compete::Change::TopExit(i)|crate::compete::Change::FightEnter(i)|crate::compete::Change::FightExit(i)=>(runners[*i].distance,runners[*i].lane,runners[*i].rank)}).collect::<Vec<_>>());}
        {
            let stale=(0..self.horses.len()).map(|i|self.compete_runner(i,true)).collect::<Vec<_>>();
            let (fs,_)=crate::compete::step(&self.compete,&stale,self.course.distance,dt,&params);
            changes.retain(|c|!matches!(c,crate::compete::Change::FightEnter(_)|crate::compete::Change::FightExit(_)));
            for i in 0..runners.len() {
                if !self.compete.modes[i].fight {next.modes[i].fight=fs.modes[i].fight;next.modes[i].fight_count=fs.modes[i].fight_count;next.modes[i].fight_group=fs.modes[i].fight_group;}
                if self.compete.modes[i].fight!=next.modes[i].fight {
                    changes.push(if next.modes[i].fight {crate::compete::Change::FightEnter(i)} else {crate::compete::Change::FightExit(i)});
                }
            }
            {
                for i in 0..runners.len() {
                    if !(self.compete.modes[i].fight && !next.modes[i].fight) || runners[i].hp<runners[i].max_hp*params.fight_exit_hp_percent/100.0 {continue;}
                    let g=self.compete.modes[i].fight_group;
                    if (0..runners.len()).any(|k|k!=i && !self.compete.modes[k].fight && fs.modes[k].fight && fs.modes[k].fight_group==g
                        && (runners[k].distance-runners[i].distance).abs()<=params.fight_exit_distance) {
                        next.modes[i].fight=true;
                        changes.retain(|c|*c!=crate::compete::Change::FightExit(i));
                    }
                }
            }
            next.near=fs.near;
            changes.sort_by_key(|c|match c {crate::compete::Change::TopEnter(i)|crate::compete::Change::TopExit(i)=>(*i,0),
                crate::compete::Change::FightEnter(i)|crate::compete::Change::FightExit(i)=>(*i,1)});
            {
                let seq:Vec<usize>=fs.enter_seq.iter().copied().filter(|i|changes.contains(&crate::compete::Change::FightEnter(*i))).collect();let mut seq=seq.into_iter();
                for c in &mut changes {
                    if matches!(c,crate::compete::Change::FightEnter(_)) {*c=crate::compete::Change::FightEnter(seq.next().unwrap());}
                }
            }
        }
        {
            let params=crate::compete::Params::default();
            for i in 0..runners.len() {
                if !self.compete.modes[i].top || !next.modes[i].top {continue;}
                let peers=(0..runners.len()).filter(|&j|j!=i && self.compete.modes[j].top_group==self.compete.modes[i].top_group).collect::<Vec<_>>();
                if !peers.is_empty() && peers.iter().all(|&j|
                    ((runners[j].distance-runners[i].distance).abs())>=params.top_exit_distance ||
                    (runners[j].lane-runners[i].lane).abs()>=params.top_exit_lane) {
                    next.modes[i].top=false;
                    next.modes[i].top_distance_exit=true;
                    changes.push(crate::compete::Change::TopExit(i));
                }
            }
        }
        if crate::envc::var_os("JPRACE_LOG_TOPREM").is_some() {
            let tick=(self.time/0.0666).round() as i64;
            for i in 0..runners.len() {
                if self.compete.modes[i].top_group.is_some() {
                    eprintln!("toprem tick={} h={} top={} rem={:.6} d={:.4} lane={:.4} exitd={} guts={:.1}",tick,i,self.compete.modes[i].top,self.compete.modes[i].top_remaining,runners[i].distance,runners[i].lane,self.compete.modes[i].top_distance_exit,runners[i].guts);
                }
            }
        }
        {
            let tick=(self.time/0.0666).round() as i64;
            for (var,hold) in [("JPRACE_TOP_EXFORCE",false),("JPRACE_TOP_EXHOLD",true)] {
                if let Ok(x)=crate::envc::var(var) {
                    for e in x.split(',') {
                        let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();
                        if f.len()!=2 || f[0] as usize>=next.modes.len() {continue;}
                        let i=f[0] as usize;
                        if !hold && tick>=f[1] && next.modes[i].top {next.modes[i].top=false;next.modes[i].top_distance_exit=true;}
                        if hold && tick<f[1] && self.compete.modes[i].top && !next.modes[i].top {next.modes[i].top=true;next.modes[i].top_distance_exit=false;}
                    }
                }
            }
        }
        self.compete=next;
        for h in &mut self.horses {h.cf_speed=h.speed;h.cf_hp=h.hp;h.cf_pos=[h.distance,h.lane];}
        {
            let mut entering=changes.iter().filter_map(|c|match c {crate::compete::Change::TopEnter(i)=>Some(*i),_=>None}).collect::<Vec<_>>();
            let leader=entering.iter().copied().min_by_key(|i|(runners[*i].rank,*i));
            entering.sort_by_key(|i|(Some(*i)!=leader,*i));
            let mut ordered=entering.into_iter();
            for change in &mut changes {
                if matches!(change,crate::compete::Change::TopEnter(_)) {*change=crate::compete::Change::TopEnter(ordered.next().unwrap());}
            }
        }
        for ch in changes {
            let (ty,name,i)=match ch {crate::compete::Change::TopEnter(i)=>(4,"CompeteTop",i),crate::compete::Change::FightEnter(i)=>(5,"CompeteFight",i),_=>continue};
            if ty==5 {self.horses[i].fight_count+=1;}
            self.events.push(json!({"t":self.time,"type":ty,"name":name,"params":[i as i32],"horse":i,"extra":""}));
        }
    }
    fn compete_effects(&self,hi:usize)->crate::compete::Effects {
        if self.compete.modes.len()<=hi {return crate::compete::Effects{target_speed_add:0.0,acceleration_add:0.0,hp_base_rate:None};}
        crate::compete::effects(&self.compete_runner(hi,false),&self.compete.modes[hi],&crate::compete::Params::default())
    }
    fn near_times(&mut self,dt:f32) {
        let p=self.p();
        let (fd,fl,bd,bl)=(p.f("Skill.InfrontHorseNearDistance"),p.f("Skill.InfrontHorseNearLaneDistance"),
            p.f("Skill.BehindHorseNearDistance"),p.f("Skill.BehindHorseNearLaneDistance"));
        let n=self.horses.len();
        let mut by_order=vec![0;n];for (i,h) in self.horses.iter().enumerate() {by_order[h.order]=i;}
        for hi in 0..n {
            let (o,po,d,l)={let h=&self.horses[hi];(h.order,h.prev_order,h.distance,h.lane)};
            let (mut f,mut b)=(None,None);
            if o!=0 && o==po {
                let a=&self.horses[by_order[o-1]];let gap=a.distance-d;
                let ns=crate::envc::var("JPRACE_NEAR_STRICT").unwrap_or_else(|_|"b".into());
                let inf=if ns.contains('f') {gap<fd} else {gap<=fd};
                let inl=if ns.contains('L') {(l-a.lane).abs()<fl} else {(l-a.lane).abs()<=fl};
                f=Some(if !inf {None} else {Some(inl)});
            }
            let ns=crate::envc::var("JPRACE_NEAR_STRICT").unwrap_or_else(|_|"b".into());
            if o!=n-1 && o==po {
                let a=&self.horses[by_order[o+1]];let gap=d-a.distance;
                let inb=if ns.contains('b') {gap<bd} else {gap<=bd};
                let inl=if ns.contains('L') {(l-a.lane).abs()<bl} else {(l-a.lane).abs()<=bl};
                b=Some(if inb {Some(inl)} else {None});
            }
            let sets=self.assets.params().arr("Skill.BehindNearParamArray").iter().map(|r|(f32_of(&r["Distance"]),f32_of(&r["LaneDistance"]))).collect::<Vec<_>>();
            let behind=(o!=n-1 && o==po).then(||{let a=&self.horses[by_order[o+1]];(d-a.distance,(l-a.lane).abs())});
            if crate::envc::var("JPRACE_LOG_NEARFRONT").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                let fa=(o!=0).then(||{let a=&self.horses[by_order[o-1]];(by_order[o-1],a.distance-d,(l-a.lane).abs())});
                eprintln!("nearfront tick={} h={} o={} po={} front={:?} f={:?} t={:.3}",(self.time/0.0666).round() as i64,hi,o,po,fa,f,self.horses[hi].near_front_time);
            }
            if crate::envc::var("JPRACE_LOG_NEARBEHIND").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                let ba=(o!=n-1).then(||{let a=&self.horses[by_order[o+1]];(by_order[o+1],d-a.distance,(l-a.lane).abs(),a.prev_distance)});
                eprintln!("nearbehind tick={} h={} o={} po={} behind={:?} b={:?} t={:.4} d={:.4} pd={:.4}",(self.time/0.0666).round() as i64,hi,o,po,ba,b,self.horses[hi].near_behind_time,d,self.horses[hi].prev_distance);
            }
            let h=&mut self.horses[hi];
            h.near_front_time=match f {Some(Some(true))=>h.near_front_time+dt,_=>0.0};
            h.near_behind_time=match b {Some(Some(true))=>h.near_behind_time+dt,_=>0.0};
            h.near_behind_set.resize(sets.len(),0.0);
            for (k,(sd,sl)) in sets.into_iter().enumerate() {
                h.near_behind_set[k]=match behind {Some((g,lg)) if !(sd<g || sl<lg)=>h.near_behind_set[k]+dt,_=>0.0};
            }
        }
    }
    fn temptation(&mut self,hi:usize,dt:f32) {
        let p=self.assets.params();let h=&mut self.horses[hi];
        if h.finished {return;}
        if h.mods.has(14) {
            h.temptation_timer=h.mods.consume(14,0.0);h.temptation_forced=true;
        }
        if h.tempted() {
            h.temptation_timer=(h.temptation_timer-dt).max(0.0);
            if h.temptation_timer<=0.0 {h.temptation_mode=0;h.temptation_active=false;h.temptation_end=if h.temptation_forced {3} else {1};return;}
            if h.temptation_forced {return;}
            h.temptation_check=(h.temptation_check-dt).max(0.0);
            if h.temptation_check<=0.0 {
                h.temptation_check=p.f("Temptation.TemptationEndCheckTime");
                let c=self.random.calls()+1;let r=self.random.int_max(100);
                if crate::envc::var_os("JPRACE_LOG_TEMPT").is_some() {eprintln!("tcheck h={} t={:.4} call={} r={} per={}",hi,self.time,c,r,p.f("Temptation.TemptationEndPer"));}
                if (r as f32)<p.f("Temptation.TemptationEndPer") {h.temptation_mode=0;h.temptation_active=false;h.temptation_end=2;}
            }
        } else if h.temptation_section>0 && (h.distance/(self.course.distance/24.0)).floor() as i32+1>=h.temptation_section {
            let mode=if h.input.running_style==1 {4} else {
                let r=self.random.int_max(100) as f32;
                match h.input.running_style {
                    2=>if r<p.f("Temptation.SenkoToNigePer") {3} else {0},
                    3=>if r<p.f("Temptation.SashiToSenkoPer") {2} else {3},
                    _=>if r<p.f("Temptation.OikomiToSashiPer") {1} else if r<p.f("Temptation.OikomiToSashiPer")+p.f("Temptation.OikomiToSenkoPer") {2} else {3},
                }
            };
            h.temptation_section=0;h.temptation_mode=mode;h.temptation_active=true;h.temptation_count+=1;
            h.temptation_forced=false;
            h.temptation_timer=p.f("Temptation.TemptationForceEndTime");h.temptation_check=p.f("Temptation.TemptationEndCheckTime");
            let t=self.time;self.events.push(json!({"t":t,"type":14,"name":"Temptation","params":[hi,0],"horse":hi,"extra":format!("{}",mode)}));
        }
    }
    fn downhill(&mut self,hi:usize,dt:f32) {
        let p=self.assets.params();let h=&mut self.horses[hi];
        let late=crate::envc::var("JPRACE_DOWN_LATE").as_deref()==Ok("1") && h.slope_clear_tick==(self.time/0.0666).round() as i64-1;
        if h.slope>=0.0 && !late {if h.downhill {if crate::envc::var_os("JPRACE_LOG_DOWN").is_some() {eprintln!("downleave h={} t={:.4} check={:.4}",hi,self.time,h.downhill_check);}h.downhill=false;h.downhill_check=1.0;}return;}
        if h.downhill_check>0.0 {h.downhill_check-=dt;return;}
        h.downhill_check=1.0;
        let dk=crate::envc::var("JPRACE_DOWN_KEEP").unwrap_or_default();
        if !dk.is_empty() && !h.downhill {
            let tm=self.time;
            if self.oracle.iter().any(|&(oh,kd,st,en)|oh==hi && kd==9 && tm>=st-1e-3 && tm<en-1e-3) {
                if dk=="nodraw" {return;}
                let _=self.random.float_range(0.0,100.0);return;
            }
        }
        let h=&mut self.horses[hi];
        let wz=match crate::envc::var("JPRACE_DOWN_WIZ").as_deref() {Ok("adj")=>h.adjusted[4],Ok("base")=>h.base[4],Ok("nostyle")=>h.mods.apply(4,h.base[4]),_=>h.stat(4)};
        if crate::envc::var_os("JPRACE_LOG_DOWN").is_some() {eprintln!("downwiz h={} stat={:.3} adj={:.3} base={:.3} m4={:.3}",hi,h.stat(4),h.adjusted[4],h.base[4],h.mods.apply(4,0.0));}
        let threshold=if h.downhill {p.f("Slope.DownSlopeAddSpeedEndPer")} else {(wz*p.f("Slope.DownSlopeAddSpeedStartWizRate")).min(100.0)};
        let mut u=self.random.float_range(0.0,100.0);
        if let Ok(o)=crate::envc::var("JPRACE_DOWN_OVR") {let tk=(self.time/0.0666).round() as i64;
            for kv in o.split(',') {if let Some((k,v))=kv.split_once('=') && let Some((a,b))=k.split_once('@') && a.parse::<usize>().ok()==Some(hi) && b.parse::<i64>().ok()==Some(tk) {u=v.parse().unwrap_or(u);}}}
        if crate::envc::var_os("JPRACE_LOG_DOWN").is_some() {eprintln!("down t={:.4} h={} was={} u={:.4} thr={:.4} slope={} calls={}",self.time,hi,h.downhill,u,threshold,h.slope,self.random.calls());}
        if u<threshold {h.downhill=!h.downhill;
            if !h.downhill && crate::envc::var("JPRACE_DOWN_END_MODES").as_deref()==Ok("1") {
                let tm=self.time;
                for e in self.oracle.iter_mut() {if e.0==hi && matches!(e.1,8|10) && tm>e.2+1e-3 && tm<e.3+0.0666-1e-3 {e.3=tm-0.0666;}}
            }
        }
    }
    fn ot_check(&self,hi:usize,o:usize)->bool {
        let h=&self.horses[hi];
        let Some(a)=h.around.iter().find(|a|a.i==o).copied() else {return false};
        if a.gap< -1.0 {return false;}
        let other=&self.horses[o];
        let fstgt=|x:&Horse|->f32 {
            let m=crate::envc::var("JPRACE_OT_FSTGT").unwrap_or_default();
            if m=="0" || !x.rafs_on {return 0.0;}
            if x.hp<=0.0 && crate::envc::var("JPRACE_OT_FSHP0").as_deref()!=Ok("1") {return 0.0;}
            let e=Tree(self.p().get("ExtraTargetSpeed"));
            let raw=if m=="unc" {x.speed_unc} else {x.input.speed as f32};
            let (sp,mp,sf,mf)=(e.f("ExtraTargetSpeedStartParam"),e.f("ExtraTargetSpeedMaxParam"),e.f("ExtraTargetSpeedStartFoundation"),e.f("ExtraTargetSpeedMaxFoundation"));
            sf+(mf-sf)*((raw-sp)/(mp-sp)).clamp(0.0,1.0)-x.fs_x
        };
        let otnow=crate::envc::var("JPRACE_OT_NOW").ok().filter(|_|crate::envc::var("JPRACE_OT_NOW_H").map_or(true,|v|v.split(',').any(|x|x.parse::<usize>().ok()==Some(hi))));
        let now=|x:&Horse|{let n=x.mods.entries.iter().filter(|m|m.index==6 && !m.once).count();
            let changed=match otnow.as_deref() {Some("ownexp")|Some("allexp")=>n<x.m6_n,_=>n!=x.m6_n};
            if changed {x.target_speed-x.m6_used+x.mods.apply(6,0.0)} else {x.target_speed}};
        let own_target=h.mods.apply(5,if otnow.is_some() {now(h)} else {h.target_speed})+fstgt(h);
        let ownhp0=crate::envc::var("JPRACE_OT_OWNHP0").unwrap_or_default();
        let own_target=if h.hp<=0.0 && !h.finished {match ownhp0.as_str() {"nohp"|"both"=>h.mods.apply(5,h.tgt_nohp),"speed"=>h.speed,_=>own_target}} else {own_target};
        if crate::envc::var_os("JPRACE_LOG_OTFLIP").is_some() && h.front!=Some(o) {
            let n=h.mods.entries.iter().filter(|m|m.index==6 && !m.once).count();
            if n<h.m6_n {
                let fresh_own=h.mods.apply(5,h.target_speed-h.m6_used+h.mods.apply(6,0.0))+fstgt(h);
                let ot=if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)+fstgt(other)};
                if (own_target>ot)!=(fresh_own>ot) {
                    eprintln!("otflip tick={} h={} o={} stale={:.5} fresh={:.5} other={:.5} inlist={}",(self.time/0.0666).round() as i64,hi,o,own_target,fresh_own,ot,h.overtake.contains(&o));
                }
            }
        }
        let fresh=|x:&Horse,xi:usize|->f32 {
            if crate::envc::var("JPRACE_OT_FRESH").as_deref()==Ok("2") && xi>hi && x.keep.mode==0 {
                if let Some(m)=self.spurt_r1_max(xi) {
                    let j=if x.input.speed as f32>=Tree(self.p().get("ExtraTargetSpeed")).f("ExtraTargetRawSpeedParam") && fs_enabled() {self.fs_extra_max(xi).max(0.0)} else {0.0};
                    return x.target_speed-x.tgt_base+m+j;
                }
                return x.target_speed;
            }
            if crate::envc::var("JPRACE_OT_FRESH").as_deref()!=Ok("1") || xi<=hi {return x.target_speed;}
            let inspurt=x.spurt_start>0.0 && x.spurt_result&12==0 && x.distance>=x.spurt_start;
            if inspurt && (x.tgt_base-x.spurt_speed).abs()>1e-6 {x.target_speed-x.tgt_base+x.spurt_speed} else {x.target_speed}
        };
        let otfs=crate::envc::var("JPRACE_OT_FS").unwrap_or_default();
        if otfs=="skipall" && other.rafs_on {return false;}
        if h.front==Some(o) {return own_target>other.speed;}
        if otfs=="skip" && other.rafs_on {return false;}
        if crate::envc::var("JPRACE_DEBUG_OT").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("otchk tick={} h={} o={} gap={:.5} sgap={:.5} ratio={:.4} own_t={:.5} o_spd={:.5} o_t={:.5} o_front={:?} h_spd={:.5}",(self.time/0.0666).round() as i64,hi,o,a.gap,a.speed_gap,a.gap/a.speed_gap,own_target,other.speed,other.mods.apply(5,other.target_speed),other.front,h.speed);
        }
        if a.gap<1.0 || a.gap>20.0 || a.speed_gap<0.0 {return false;}
        match crate::envc::var("JPRACE_OT_SKIPSK").as_deref() {
            Ok("any") if other.mods.entries.iter().any(|m|m.index==6 && m.value>0.0)=>return false,
            Ok("big") if other.mods.entries.iter().any(|m|m.index==6 && m.value>=0.3)=>return false,
            Ok("lb") if other.mods.entries.iter().any(|m|m.index==6 && m.value>0.0 && m.lb.is_some())=>return false,
            _=>{}
        }
        let mut dps=crate::envc::var("JPRACE_OT_DPS").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(self.p().f("overTakeDistPerSpeed"));
        if let Ok(v)=crate::envc::var("JPRACE_OT_DPS_LATE") && let Some((ph,x))=v.split_once(':') && let (Ok(ph),Ok(x))=(ph.parse::<usize>(),x.parse::<f32>()) && h.phase>=ph {dps=x;}
        let sg=match crate::envc::var("JPRACE_OT_SGAP").as_deref() {
            Ok("lb")=>a.speed_gap-other.mods.entries.iter().filter(|m|m.index==6 && m.lb.is_some()).map(|m|m.value).sum::<f32>(),
            Ok("all")=>a.speed_gap-other.mods.entries.iter().filter(|m|m.index==6).map(|m|m.value).sum::<f32>(),
            _=>a.speed_gap};
        if crate::envc::var("JPRACE_LOG_OTC").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("otc tick={} h={} o={} gap={:.4} sg={:.4} ratio={:.3} own={:.4} oth_front={} oth_spd={:.4} oth_tgt={:.4} oth_fs={:.4}",(self.time/0.0666).round() as i64,hi,o,a.gap,sg,a.gap/sg,own_target,other.front.is_some(),other.speed,other.mods.apply(5,other.target_speed),fstgt(other));
        }
        if sg<0.0 {return false;}
        if a.gap/sg>dps {return false;}
        match crate::envc::var("JPRACE_OT_CMP").as_deref() {
            Ok("speed")=>own_target>other.speed,
            Ok("min")=>own_target>other.speed.min(other.mods.apply(5,other.target_speed)),
            Ok("tgt")=>own_target>if other.front.is_some() {other.speed} else {other.target_speed},
            Ok("selfspd")=>h.speed>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)},
            Ok("nomode")=>own_target-h.mode_add>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)-other.mode_add},
            Ok("nomodeo")=>own_target>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)-other.mode_add},
            Ok("nomodes")=>own_target-h.mode_add>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)},
            Ok("sk23")=>own_target+h.mods.apply(23,0.0)>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)+other.mods.apply(23,0.0)},
            _ if other.hp<=0.0 && !other.finished && crate::envc::var("JPRACE_OT_HP0").as_deref()==Ok("speed")=>own_target>other.speed,
            _ if other.hp<=0.0 && !other.finished && crate::envc::var("JPRACE_OT_HP0").as_deref()==Ok("nohp")=>own_target>other.mods.apply(5,other.tgt_nohp)+fstgt(other),
            _=>own_target>if other.front.is_some() {other.speed} else {other.mods.apply(5,if matches!(otnow.as_deref(),Some("all")|Some("allexp")) {now(other)} else {fresh(other,o)})+fstgt(other)},
        }
    }
    fn update_overtake(&mut self,hi:usize,dt:f32) {
        let mut list=std::mem::take(&mut self.horses[hi].overtake);let prev_nonempty=!list.is_empty();let prev_list=list.clone();
        for v in self.horses[hi].visible.clone() {if self.ot_check(hi,v) && !list.contains(&v) {list.push(v);}}
        let rv=crate::envc::var("JPRACE_OT_RETAIN").ok();
        list.retain(|&o|self.ot_check(hi,o) && match rv.as_deref() {Some("vis")=>self.horses[hi].visible.contains(&o) || self.horses[hi].front==Some(o),_=>true});
        if let Some(f)=self.horses[hi].front {if !list.contains(&f) {list.push(f);}}
        let tgt_drop=prev_list.iter().any(|&o|!list.contains(&o) && self.horses[hi].front!=Some(o) && self.horses[hi].around.iter().find(|a|a.i==o).is_some_and(|a|{
            let dps=self.p().f("overTakeDistPerSpeed");a.gap>=1.0 && a.gap<=20.0 && a.speed_gap>=0.0 && a.gap/a.speed_gap<=dps}));
        let cool=self.p().f("overTakeCoolDownTime");let tnow=self.time;let h=&mut self.horses[hi];
        let er=Some(crate::envc::var("JPRACE_OT_EMPTY_REPLAN").unwrap_or_else(|_|"hp0rafs".into()));
        if list.is_empty() && prev_nonempty && (er.as_deref()==Some("1") || (er.as_deref()==Some("hp0") && h.hp<=0.0) || (er.as_deref()==Some("hp0nf") && h.hp<=0.0 && h.prev_front.is_none()) || (er.as_deref()==Some("tgt") && tgt_drop) || (er.as_deref()==Some("hp0tgt") && tgt_drop && h.hp<=0.0) || (er.as_deref()==Some("hp0new") && tgt_drop && h.hp<=0.0 && (!h.hp0_seen || tnow-h.hp0_t<0.1)) || (er.as_deref()==Some("hp0out") && tgt_drop && h.hp<=0.0 && (!h.hp0_seen || tnow-h.hp0_t<0.1) && h.target_lane>h.lane)
            || (er.as_deref()==Some("hp0rafs") && tgt_drop && h.hp<=0.0 && (!h.hp0_seen || tnow-h.hp0_t<0.1) && h.target_lane>h.lane && (h.rafs_on || h.slb_re || h.fs_j>0.0))) {h.force_lane=true;if crate::envc::var_os("JPRACE_LOG_EREPLAN").is_some() {eprintln!("ereplan tick={} rafs={} slbre={} slbon={} fsj={:.4} npc={} spd={} h={}",(tnow/0.0666).round() as i64,h.rafs_on,h.slb_re,h.slb_on,h.fs_j,h.input.npc_type,h.input.speed,hi);}}
        let now_t=tnow;
        if h.hp<=0.0 && !h.hp0_seen {h.hp0_seen=true;h.hp0_t=now_t;if crate::envc::var("JPRACE_HP0_REPLAN").as_deref()==Ok("1") {h.force_lane=true;}}
        if !list.is_empty() {h.ot_cool=cool;}
        else if h.ot_cool>0.0 {h.ot_cool-=dt;if h.ot_cool<=0.0 {h.force_lane=true;}}
        h.overtake=list;
    }
    fn side_lag()->u8 {crate::envc::var("JPRACE_SIDE_LAG").ok().and_then(|x|x.parse().ok()).unwrap_or(0)}
    fn enable_in(&self,hi:usize)->bool {let h=&self.horses[hi];let free=match Self::side_lag() {1=>h.prev_side.0.is_none(),2=>h.inside.is_none() && h.prev_side.0.is_none(),_=>h.inside.is_none()};free && h.lane>h.lane_min+0.0001}
    fn enable_out(&self,hi:usize)->bool {let h=&self.horses[hi];let free=match Self::side_lag() {1=>h.prev_side.1.is_none(),2=>h.outside.is_none() && h.prev_side.1.is_none(),_=>h.outside.is_none()};free && h.lane<h.lane_max}
    fn ot_enable(&self,hi:usize,l:f32)->bool {
        let h=&self.horses[hi];
        let lmax=if crate::envc::var_os("JPRACE_OTEN_COURSE").is_some() {self.course.lane_max} else {h.lane_max};
        if l<0.0 || l>lmax {return false;}
        let d=l-h.lane;
        if d<0.0 {self.enable_in(hi) && -d<=h.in_space} else if d<=0.0 {true} else {self.enable_out(hi) && d<=h.out_space}
    }
    fn ot_chain(&self,mut x:usize,inward:bool)->usize {
        let range=f32::from_bits(0x3de38e39);
        let gmin=crate::envc::var("JPRACE_OTCHAIN_MIN").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(0.0);
        loop {
            let mut best=self.horses[x].lane;let mut next=None;
            for a in &self.horses[x].around {
                if a.gap>=gmin && a.gap<=3.0 && (if inward {a.lane<0.0} else {a.lane>0.0}) && a.lane_abs<range {
                    let l=self.horses[a.i].lane;
                    if if inward {l<best} else {best<l} {best=l;next=Some(a.i);}
                }
            }
            match next {Some(n)=>x=n,None=>return x}
        }
    }
    fn ot_exist(&self,hi:usize,d0:f32,d1:f32,l0:f32,l1:f32)->bool {
        self.horses[hi].visible.iter().any(|&v|{let n=&self.horses[v];d0<=n.distance && n.distance<=d1 && l0<=n.lane && n.lane<=l1})
    }
    fn ot_search(&self,hi:usize,fixed:f32)->f32 {
        let p=self.p();let h=&self.horses[hi];
        let c1=f32::from_bits(0x3d638e3a);let c2=f32::from_bits(0x3d638e39);let w=f32::from_bits(0x3d360b62);
        let mut cands=vec![];let mut min_dist=f32::MAX;
        for &t in &h.overtake {
            let td=self.horses[t].distance;if td<min_dist {min_dist=td;}
            let a=self.ot_chain(t,true);let b=self.ot_chain(t,false);
            let inl=self.horses[a].lane-c1;let outl=self.horses[b].lane+c1;
            if 0.0<=inl && !self.ot_exist(hi,h.distance,self.horses[a].distance+0.5,inl-w,inl+w) {cands.push(inl);}
            if outl<=h.lane_max && !self.ot_exist(hi,h.distance,self.horses[b].distance+0.5,outl-w,outl+w) {cands.push(outl);}
        }
        let inner_ok=h.phase<2 && self.enable_in(hi) && h.lane>c2 && !self.ot_exist(hi,h.distance,min_dist+3.0,(h.lane-w)-c2,(h.lane+w)-c2);
        if crate::envc::var("JPRACE_OT_CUR").as_deref()==Ok("fallback") {
            if inner_ok {cands.push(h.lane-c2);} else if !self.ot_exist(hi,h.distance,min_dist+3.0,h.lane-w,h.lane+w) {cands.push(h.lane);}
        } else if h.phase<2 && self.enable_in(hi) && h.lane>c2 {
            if inner_ok {cands.push(h.lane-c2);}
        } else if !self.ot_exist(hi,h.distance,min_dist+3.0,h.lane-w,h.lane+w) {cands.push(h.lane);}
        let (m_in,m_out)=(h.mods.apply(16,0.0)!=0.0,h.mods.apply(17,0.0)!=0.0);
        if crate::envc::var("JPRACE_LOG_LANEPLAN").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("otcands tick={} h={} cands={:?} en={:?}",(self.time/0.0666).round() as i64,hi,cands,cands.iter().map(|&c|self.ot_enable(hi,c)).collect::<Vec<_>>());
        }
        let mut chosen=h.lane;let mut best=f32::MAX;
        let ro_cost=if h.lane_off_ro {crate::envc::var("JPRACE_RO_OTCOST").unwrap_or_default()} else {"old".into()};
        let (ro_flip,ro_none)=(ro_cost=="flip",ro_cost!="old" && ro_cost!="flip");
        for c in cands {
            let d=h.lane-c;
            let phase=if h.phase==0 && !ro_none {if (d>0.0)!=ro_flip && d!=0.0 {100.0} else {1.0}} else if h.phase<2 {1.0}
                else if d>0.0 {p.f("overTakeOutLaneCoefEnd")} else {p.f("overTakeInLaneCoefEnd")};
            let skill=if m_in!=m_out && ((m_in && d>0.0) || (m_out && d<0.0)) {1.5} else {1.0};
            let cost=(phase*d*skill).abs();
            if cost<best && self.ot_enable(hi,c) {best=cost;chosen=c;}
        }
        if crate::envc::var("JPRACE_LOG_LANEPLAN").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            let cur_exist=self.ot_exist(hi,h.distance,min_dist+3.0,h.lane-w,h.lane+w);
            let blockers:Vec<String>=h.visible.iter().filter(|&&v|{let n=&self.horses[v];h.distance<=n.distance && n.distance<=min_dist+3.0}).map(|&v|format!("{}:{:.3}:{:.5}",v,self.horses[v].distance-h.distance,self.horses[v].lane)).collect();
            eprintln!("otsearch tick={} h={} lane={:.5} chosen={:.5} cur_exist={} min_dist_rel={:.3} extra_ok={:?} in_space={:.4} out_space={:.4} vis_in_window={:?}",(self.time/0.0666).round() as i64,hi,h.lane,chosen,cur_exist,min_dist-h.distance,h.extra_lane.map(|e|self.ot_enable(hi,e)),h.in_space,h.out_space,blockers);
        }
        if let Some(extra)=h.extra_lane {
            if chosen<h.lane || extra<=chosen || !self.ot_enable(hi,extra) {chosen} else {extra}
        } else if !math::approximately(fixed,0.0) {
            if chosen<h.lane || fixed<=chosen || !self.ot_enable(hi,fixed) {chosen} else {fixed}
        } else {chosen}
    }
    fn move_lane(&mut self,hi:usize,dt:f32,plan:bool) {
        let inside_lane=self.horses[hi].inside.map(|i|self.horses[i].lane);
        let p=self.assets.params();let h=&mut self.horses[hi];
        let enable_in=h.inside.is_none() && h.lane>h.lane_min+0.0001;let enable_out=h.outside.is_none() && h.lane<h.lane_max;
        let end=self.course.final_corner().map(|c|c.start+(c.end-c.start)*p.f("LastMoveOutStartFinalCornerRate"));
        let blocked_target=(h.target_lane<h.lane && !enable_in) ||
            (h.target_lane>h.lane && !enable_out);
        if plan && !(self.course.empty_in_move && self.course.first_move>0.0) {return;}
        let overtaking=!h.overtake.is_empty();
        if plan && h.hp<=0.0 && !h.finished && crate::envc::var("JPRACE_HP0_LANE").as_deref()==Ok("stop") {h.target_lane=h.lane;return;}
        if plan && h.hp<=0.0 && !h.finished && h.overtake.is_empty() && h.ot_cool>0.0 && crate::envc::var("JPRACE_HP0_LANE").as_deref()==Ok("cool") {h.target_lane=h.lane;return;}
        if plan && let Ok(x)=crate::envc::var("JPRACE_PLAN_STAY") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                if let Some((a,b))=e.split_once(':') && a.parse::<usize>().ok()==Some(hi) && let Some((t0,t1))=b.split_once('-')
                    && let (Ok(t0),Ok(t1))=(t0.parse::<i64>(),t1.parse::<i64>()) && t0<=tick && tick<=t1 {h.target_lane=h.lane;return;}
            }
        }
        if plan && (h.force_lane || blocked_target || overtaking || (h.target_lane-h.lane).abs()<0.02777777798473835) {
            h.force_lane=false;h.target_lane=h.lane;
            if h.extra_lane.is_none() && end.is_some_and(|d|h.distance>=d) {
                let norm=p.f("lastMoveOutLaneNormalizeVal");let norm=if math::approximately(norm,0.0) {0.2} else {norm};
                let call=self.random.calls()+1;let base=(h.lane/norm).clamp(0.0,1.0)*p.f("lastMoveOutLaneCoef");
                let r=self.random.float_range(0.0,p.f("lastMoveOutLaneAddRandom"));
                if crate::envc::var_os("JPRACE_LOG_LANERAND").is_some() {eprintln!("lanerand h={} t={:.4} call={} r={:.6}",hi,self.time,call,r);}
                h.extra_lane=Some((base+r).clamp(h.lane_min,h.lane_max));
                h.lane_draw_t=self.time;
                if crate::envc::var("JPRACE_LOG_EXTRA").is_ok() {eprintln!("extra h={} t={:.4} call={} lane={:.5} base={:.5} r={:.5} extra={:.5} lane_max={:.4}",hi,self.time,call,h.lane,base,r,base+r,h.lane_max);}
            }
            let fixed=h.mods.apply(11,0.0);
            let first=self.course.first_move;
            let use_ot=(!h.overtake.is_empty() || h.ot_cool>0.0) &&
                !(h.phase<2 && h.distance<=first && first-200.0<=h.distance);
            if use_ot {
                let t=self.ot_search(hi,fixed);
                let h=&mut self.horses[hi];h.target_lane=t.clamp(h.lane_min,h.lane_max);
                return;
            }
            let h=&mut self.horses[hi];
            if !math::approximately(fixed,0.0) {
                h.target_lane=if fixed<h.lane {if enable_in {fixed.max(h.lane-0.05)} else {h.lane}}
                    else if h.lane<fixed {if enable_out {fixed.min(h.lane+0.05)} else {h.lane}} else {h.lane};
            } else if h.hp>0.0 {
                if h.keep.mode==4 {let pd=p.f("PositionKeep.PositionKeepPaseDownTargetLane");
                    h.target_lane=if !h.lane_off_ro {pd} else {match crate::envc::var("JPRACE_RO_PDLANE").as_deref() {Ok("base")=>h.ro_base+pd,Ok("mirror")=>h.lane_max-(1.0/18.0+pd),Ok("abs")=>pd,_=>h.lane}};}
                else if let Some(extra)=h.extra_lane {if extra>h.lane && enable_out {h.target_lane=extra.min(h.lane+0.05);}}
                else if h.phase<2 && enable_in && !(h.lane_off_ro && crate::envc::var("JPRACE_RO_NOIN").as_deref()!=Ok("0")) {h.target_lane=h.lane-0.05;}
                else if h.phase==1 && !(h.lane_off_ro && crate::envc::var("JPRACE_RO_NOGAP").as_deref()!=Ok("0")) {
                    if let Some(lane)=inside_lane {
                        let gap=p.f("Block.SideBlockLaneGap");
                        let snap=h.inside.and_then(|i|h.around.iter().find(|a|a.i==i)).map_or((lane-h.lane).abs(),|a|a.lane_abs);
                        if snap<gap-f32::from_bits(0x3c638e39) {
                            h.target_lane=(lane+gap).min(h.lane+0.05);
                        }
                    }
                }
            }
            h.target_lane=h.target_lane.clamp(h.lane_min,h.lane_max);
        }
        if plan {return;}
        let h=&mut self.horses[hi];
        h.prev_lane=h.lane;
        if lane_near(h.lane,h.target_lane,0) {h.lane=h.lane.clamp(h.lane_min,h.lane_max);h.move_time=0.0;return;}
        let prev_dir=h.move_dir;h.move_dir=(h.lane<=h.target_lane) as i32;
        if h.finished && h.after_goal>=1.0 {
            let old=h.lane;let m=if h.target_lane<old {-dt} else {dt};h.lane_move=m;
            let next=if f32m('l') {dt*m+old} else {(dt as f64*m as f64+old as f64) as f32};
            let next=if h.target_lane<old {next.max(h.target_lane)} else {next.min(h.target_lane)}.clamp(h.lane_min,h.lane_max);
            if (next>old && !enable_out) || (next<old && !enable_in) {return;}
            h.lane=next;if prev_dir==h.move_dir {h.move_time+=dt;}
            if math::approximately(h.lane,h.target_lane) {h.lane_speed=0.0;}
            return;
        }
        let fl=f32m('l');
        let mut base=if fl {(h.stat(2)*p.f("laneMoveSpeedPowCoef")+p.f("laneMoveSpeedAdd"))*p.f("laneMoveSpeedBase")} else {((h.stat(2) as f64*p.f("laneMoveSpeedPowCoef") as f64+p.f("laneMoveSpeedAdd") as f64)*p.f("laneMoveSpeedBase") as f64) as f32};
        if h.phase<2 && h.distance<=self.course.first_move {
            if fl {base*=h.lane/h.lane_max*p.f("laneMoveSpeedOutCoef")+1.0;} else {
            let ratio=(h.lane/h.lane_max) as f64;
            base=(base as f64*(ratio*p.f("laneMoveSpeedOutCoef") as f64+1.0)) as f32;}
        }
        if h.phase>=2 {if fl {base*=h.order as f32*0.01+1.0;} else {base=(base as f64*(h.order as f64*0.01f32 as f64+1.0)) as f32;}}
        let accel=p.f("laneMoveAccelMoveSpeedBaseCoef")*p.f("laneMoveSpeedBase");
        if prev_dir!=h.move_dir && crate::envc::var("JPRACE_LANE_DIRRESET").as_deref()==Ok("1") {h.lane_speed=0.0;}
        h.lane_speed=if fl {if h.lane_speed<base {(h.lane_speed+accel*dt).min(base)} else {(h.lane_speed-accel*dt).max(base)}}
            else if h.lane_speed<base {((h.lane_speed as f64+accel as f64*dt as f64) as f32).min(base)} else {((h.lane_speed as f64-accel as f64*dt as f64) as f32).max(base)};
        h.lane_speed=h.lane_speed.max(0.0);
        let speed=h.mods.apply(7,h.lane_speed).clamp(0.0,0.6);
        if crate::envc::var("JPRACE_LOG_LSPD").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("lspd tick={} lane={:.9} tgt={:.9} lspd={:.6} speed={:.6} base={:.6} dir={} prev={}",(self.time/0.0666).round() as i64,h.lane,h.target_lane,h.lane_speed,speed,base,h.move_dir,prev_dir);}
        let old=h.lane;
        if h.target_lane<old && enable_in {h.lane_move=if fl {(old+1.0)*(-speed)} else {((old as f64+1.0)*(-speed as f64)) as f32};}
        else if h.target_lane>old && enable_out {h.lane_move=speed;}
        {
            let m=if h.target_lane<old {if fl {(old+1.0)*(-speed)} else {((old as f64+1.0)*(-speed as f64)) as f32}} else {speed};
            let next=if fl {dt*m+old} else {(dt as f64*m as f64+old as f64) as f32};
            if h.target_lane<old && enable_in {h.lane=next.max(h.target_lane).clamp(h.lane_min,h.lane_max);}
            else if h.target_lane>old && enable_out {h.lane=next.min(h.target_lane).clamp(h.lane_min,h.lane_max);}
        }
        if let Ok(x)=crate::envc::var("JPRACE_LANE_ULP") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();
                if f.len()==3 && f[0] as usize==hi && f[1]==tick {h.lane=f32::from_bits((h.lane.to_bits() as i64+f[2]) as u32);}
            }
        }
        let moved=if h.target_lane<old {enable_in} else if h.target_lane>old {enable_out} else {true};
        if moved && !math::approximately(speed,0.0) && prev_dir==h.move_dir {h.move_time+=dt;}
        if lane_near(h.lane,h.target_lane,1) {h.lane_speed=0.0;}
    }
    fn update_horse(&mut self,hi:usize,dt:f32) {
        let p=self.assets.params();let race_base=self.race_base();
        let default_min=race_base*p.f("Speed.MinSpeedRate");
        let front_min=self.horses[hi].front.map(|i|self.horses[i].min_speed);
        let neg_mode=crate::envc::var("JPRACE_HEAL_RECALC_NEG").unwrap_or_default();
        let need_now=if neg_mode.starts_with("short") && self.horses[hi].mods.entries.iter().any(|m|m.index==12) {self.jp_need_hp_mode(hi,"all")} else {0.0};
        let h=&mut self.horses[hi];
        h.min_speed=if h.finished {5.0} else if h.start_dash {p.f("StartSpeed")} else {
            let v=if f32m('m') {default_min+(h.stat(3)*p.f("Speed.MinSpeedGutsCoefSqrt")).sqrt()*p.f("Speed.MinSpeedGutsCoef")} else {(default_min as f64+(h.stat(3)*p.f("Speed.MinSpeedGutsCoefSqrt")).sqrt() as f64*
                p.f("Speed.MinSpeedGutsCoef") as f64) as f32};
            front_min.map_or(v,|m|v.min(m))
        };
        let heal=h.mods.consume(12,0.0);h.hp=(h.hp+heal).clamp(0.0,h.max_hp);h.hp_t0=h.hp<=0.0;
        let recalc=(heal>0.0 || (heal<0.0 && (neg_mode=="1" || (neg_mode=="short" && h.spurt_calculated && h.hp<need_now) || (neg_mode=="npc" && h.input.npc_type!=0) || (neg_mode=="shortnpc" && h.input.npc_type!=0 && h.spurt_calculated && h.hp<need_now)))) && h.phase>=2 && !(crate::envc::var("JPRACE_HEAL_RECALC").as_deref()==Ok("not1") && h.spurt_calculated && h.spurt_result==1) && crate::envc::var("JPRACE_HEAL_RECALC").as_deref()!=Ok("none");
        let recalc=recalc || crate::envc::var("JPRACE_FORCE_RECALC").ok().is_some_and(|x|x.split(',').any(|e|e.split_once(':').is_some_and(|(a,b)|a.parse::<usize>().ok()==Some(hi) && b.parse::<i64>().ok()==Some(((self.time)/dt).round() as i64))));
        let recalc=recalc || (crate::envc::var("JPRACE_RAFS_RECALC").as_deref()!=Ok("0") && h.fs_pending && !h.rafs_on && h.phase>=2);
        if h.delay>0.0 {h.delay-=dt;if h.delay>0.0 || dt==0.0 {return;}}
        if dt==0.0 {return;}
        if self.horses[hi].finished {
            if crate::envc::var("JPRACE_GOAL_LB").as_deref()!=Ok("0") && self.horses[hi].mods.entries.iter().any(|m|m.lb.is_some()) {
                let ph=self.horses[hi].phase;
                let coefs:Vec<f32>=self.horses[hi].mods.entries.iter().map(|m|m.lb.map_or(0.0,|r|if ph>=4 {0.0} else {self.wiz_limit_break_coef(ph,r[3] as i32,r[4]>0.5)})).collect();
                for (m,c) in self.horses[hi].mods.entries.iter_mut().zip(coefs) {if let Some(r)=m.lb {m.value=lb_value(r,0.0,c);}}
            }
            {
                let max=match self.horses[hi].front {
                    Some(i)=>{
                        let (h,front)=(&self.horses[hi],&self.horses[i]);let p=self.assets.params();
                        let g=h.around.iter().find(|a|a.i==i).map_or(front.distance-h.distance,|a|a.gap);
                        let t=(g/p.f("Block.FrontBlockDistanceGap")).clamp(0.0,1.0);
                        let (mx,mn)=(p.f("Block.FrontBlockMaxSpeedRateMax"),p.f("Block.FrontBlockMaxSpeedRateMin"));
                        (front.speed*((mx-mn)*t+mn)).max(h.min_speed)
                    },
                    None=>30.0,
                };
                let h=&mut self.horses[hi];let target=if h.after_goal<1.0 || crate::envc::var_os("JPRACE_GOAL_OLD").is_some() {if h.goal_tgt>0.0 {h.goal_tgt} else {h.target_speed}} else {5.0};
                if h.after_goal>=1.0 && crate::envc::var("JPRACE_GOAL_TGT").as_deref()!=Ok("old") {h.target_speed=target;}
                let mut v=h.self_speed;
                let gd64=crate::envc::var("JPRACE_GOAL_DEC").as_deref()==Ok("f64");
                if v<target {v=if gd64 {((v as f64+dt as f64) as f32).min(target)} else {(v+dt).min(target)};}
                else if v>target {v=if gd64 {((v as f64-4.0*dt as f64) as f32).max(target)}
                    else if crate::envc::var("JPRACE_GOAL_DEC").as_deref()==Ok("db") {let db=p.f("declBase");(v-(4.0/db)*db*dt).max(target)}
                    else {(v-4.0*dt).max(target)};}
                v=v.clamp(5.0,max.max(5.0));h.self_speed=v;h.speed=(h.mods.apply(23,0.0)+h.mods.apply(5,0.0)+v).clamp(5.0,max.max(5.0));
                if crate::envc::var("JPRACE_LOG_VBITS").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("vbits tick={} h={} self={:08x} spd={:08x} tgt={:08x} ag={:.4} max={:08x}",(self.time/0.0666).round() as i64,hi,h.self_speed.to_bits(),h.speed.to_bits(),target.to_bits(),h.after_goal,max.to_bits());}
            }
            let plan=self.horses[hi].after_goal<1.0;
            if plan {self.update_overtake(hi,dt);self.move_lane(hi,dt,true);}
            self.move_lane(hi,dt,false);
            if crate::envc::var("JPRACE_GOAL_LB").as_deref()!=Ok("0") {let d=self.horses[hi].distance;self.horses[hi].phase=self.course.phase(d);}
            let h=&self.horses[hi];let mut movement=h.speed*dt;
            let position=self.geometry.world(h.distance,h.lane);let dx=position[0]-h.position[0];let dz=position[2]-h.position[2];
            let old=crate::geometry::geo_old();
            let displacement=if old {((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32} else {(dx*dx+dz*dz).sqrt()};
            let cap=movement*self.geometry.distance_to_world;
            let mv0=movement;
            if cap<displacement {movement=if old {((movement as f64)*(cap as f64)/(displacement as f64)) as f32} else {movement*(cap/displacement)};}
            if crate::envc::var("JPRACE_LOG_GMOVE").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                let h=&self.horses[hi];
                eprintln!("gmv tick={} d={:08x} lane={:08x} mv0={:08x} cap={:08x} disp={:08x} mv={:08x} capped={} pos=({:08x},{:08x}) prev=({:08x},{:08x})",(self.time/0.0666).round() as i64,h.distance.to_bits(),h.lane.to_bits(),mv0.to_bits(),cap.to_bits(),displacement.to_bits(),movement.to_bits(),cap<displacement,position[0].to_bits(),position[2].to_bits(),h.position[0].to_bits(),h.position[2].to_bits());
            }
            let h=&mut self.horses[hi];
            h.position=position;h.prev_distance=h.distance;h.distance+=movement;
            return;
        }
        self.horses[hi].lane_move=0.0;
        {
            let pre=crate::envc::var("JPRACE_PK_GAP").as_deref()==Ok("pre");
            let pmcur=match crate::envc::var("JPRACE_PK_GAP").as_deref() {Ok("pmcur")=>true,Ok("excur")=>self.horses[self.pacemaker].ex(),_=>false};
            let pm=self.pacemaker;
            let runners=self.horses.iter().enumerate().map(|(j,h)|crate::position_keep::Runner {
                distance:if pre && j<hi {h.prev_distance} else if pmcur && j==pm && j>hi && h.delay<=0.0 {h.distance+h.speed*dt} else {h.distance},order:h.order,style:h.input.running_style,ex:h.ex(),phase:h.phase,
                wiz:h.stat(4),temptation:if h.tempted() && h.temptation_mode==0 {3} else {h.temptation_mode},
                speed_skill:h.mods.apply(6,0.0)>0.0 || h.mods.apply(5,0.0)>0.0 || (match crate::envc::var("JPRACE_22SKILL").as_deref() {Ok("late")=>self.time>1.0,Ok("start")|Ok("end")=>false,Ok(_)=>true,Err(_)=>false} && h.mods.apply(23,0.0)>0.0),
                speed_skill22:h.mods.apply(23,0.0)>0.0,
            }).collect::<Vec<_>>();
            let sec=self.course.distance/24.0;
            let global_end=crate::envc::var_os("JPRACE_PK_GLOBAL_END").is_some() && self.horses.iter().any(|h|h.distance>=10.0*sec);
            if global_end {if self.horses[hi].keep.mode!=0 {self.horses[hi].keep.clear(&Tree(p.get("PositionKeep")));}}
            else {
            crate::position_keep::NO_CHECK.store(crate::envc::var_os("JPRACE_PK_NOCHECK_LATE").is_some() && self.horses.iter().any(|h|h.distance>=10.0*sec),std::sync::atomic::Ordering::Relaxed);
            self.horses[hi].cp.pre_mode=self.horses[hi].keep.mode;
            self.horses[hi].keep.update(&Tree(p.get("PositionKeep")),self.course.distance as i32,24,
                &runners,hi,self.pacemaker,&mut self.random,dt,true);
            if let Ok(x)=crate::envc::var("JPRACE_PK_NOSTART") {let tk=(self.time/dt).round() as i64;
                if self.horses[hi].cp.pre_mode==0 && self.horses[hi].keep.mode!=0 && x.split(',').any(|kv|kv==format!("{}@{}",hi,tk)) {self.horses[hi].keep.mode=0;self.horses[hi].keep.multiply=1.0;}}
            if crate::envc::var("JPRACE_PK_PMBUSY").as_deref()==Ok("1") && hi!=self.pacemaker && self.horses[hi].cp.pre_mode==0 && self.horses[hi].keep.mode==3 && self.horses[self.pacemaker].keep.mode!=0 {self.horses[hi].keep.mode=0;self.horses[hi].keep.multiply=1.0;}
            if crate::envc::var_os("JPRACE_LOG_PKGAP").is_some() && matches!(self.horses[hi].cp.pre_mode,3|4) {
                let tick=(self.time/dt).round() as i64;
                eprintln!("pkgap tick={} h={} mode={} gap={:.4} tg={:.4}",tick,hi,self.horses[hi].cp.pre_mode,runners[self.pacemaker].distance-runners[hi].distance,self.horses[hi].keep.target_gap);
            }
            if crate::envc::var_os("JPRACE_LOG_PKMODE").is_some() && self.horses[hi].cp.pre_mode!=self.horses[hi].keep.mode {
                let tick=(self.time/dt).round() as i64;let h=&self.horses[hi];
                eprintln!("pkmode tick={} h={} {}->{} d={:.3} tg={:.3} pmgap={:.3} v={:.4} tgt={:.4}",tick,hi,h.cp.pre_mode,h.keep.mode,h.distance,h.keep.target_gap,self.horses[self.pacemaker].distance-h.distance,h.speed,h.target_speed);
            }
            }
        }
        if crate::envc::var("JPRACE_LOG_CHECKS").is_ok() && dt>0.0 {
            let sec=self.course.distance/24.0;let tick=(self.time/dt).round() as i64-1;
            let d=self.horses[hi].distance;
            if d>=10.0*sec && d<15.0*sec {
                for (k,kind) in [(0usize,8i64),(1,10)] {
                    let nx=self.horses[hi].jp_next[k];
                    if nx<0 || tick>=nx {
                        let ok=self.oracle.iter().any(|&(oh,kd,st,_)|oh==hi && kd==kind && ((st/dt).round() as i64)==tick);
                        let h=&self.horses[hi];
                        let top=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);
                        let (mut nf,mut nb,mut sf,mut sb)=(0,0,0,0);
                        for (j,o) in self.horses.iter().enumerate() {
                            if j==hi {continue;}
                            let g=o.distance-h.distance;let same=o.input.running_style==h.input.running_style;
                            if g>=0.0 && g<=10.0 {nf+=1;if same {sf+=1;}}
                            if g<0.0 && g>=-5.0 {nb+=1;if same {sb+=1;}}
                        }
                        let leader_style=self.horses.iter().max_by(|a,b|a.distance.total_cmp(&b.distance)).unwrap().input.running_style;
                        eprintln!("check kind={} h={} tick={} ok={} style={} ex={} order={} top_gap={:.3} nf={} nb={} sf={} sb={} pow={:.1} guts={:.1} wiz={:.1} hp={:.1} maxhp={:.1} leader_style={} lane={:.3}",
                            kind,hi,tick,ok,h.input.running_style,h.ex(),h.order,top-h.distance,nf,nb,sf,sb,h.stat(2),h.stat(3),h.stat(4),h.hp,h.max_hp,leader_style,h.lane);
                        if crate::envc::var("JPRACE_LOG_FIELD").is_ok() {
                            let field=self.horses.iter().map(|o|format!("{:.3}:{:.4}:{}:{}",o.distance,o.lane,o.input.running_style,i32::from(o.ex()))).collect::<Vec<_>>().join(",");
                            eprintln!("field kind={} h={} tick={} {}",kind,hi,tick,field);
                        }
                        self.horses[hi].jp_next[k]=tick+if ok {crate::envc::var("JPRACE_OK_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(63)} else {crate::envc::var("JPRACE_FAIL_IV").ok().and_then(|x|x.parse().ok()).unwrap_or(31)};
                    }
                }
            }
        }
        if crate::envc::var("JPRACE_CBS_PROBE").is_ok() {
            let sec=self.course.distance/24.0;let d=self.horses[hi].distance;
            if d>=10.0*sec && d<15.0*sec && dt>0.0 {
                if self.horses[hi].cbs_next<0.0 || self.time>=self.horses[hi].cbs_next-1e-4 {
                    let c=self.random.calls();let u=self.random.float_max(100.0);
                    eprintln!("probe cbs {} t={:.4} call={} u={:.2}",hi,self.time,c+1,u);
                    self.horses[hi].cbs_next=self.time+2.0;
                }
                if self.horses[hi].sl_next<0.0 || self.time>=self.horses[hi].sl_next-1e-4 {
                    let c=self.random.calls();let u=self.random.float_max(100.0);
                    eprintln!("probe sl {} t={:.4} call={} u={:.2}",hi,self.time,c+1,u);
                    self.horses[hi].sl_next=self.time+2.0;
                }
            }
        }
        if !self.horses[hi].cp.released {
            if self.compete.modes.get(hi).is_some_and(|m|m.top) {self.horses[hi].cp.top=true;}
            let cp=Tree(p.get("ConservePower"));
            {
                let h=&self.horses[hi];let style=h.input.running_style as i64;
                let sc=cp.arr("ConservePowerRunningStyleCoefArray").iter().find(|r|r["RunningStyle"].as_i64()==Some(style)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let rate=|m:i64|cp.arr("ConservePowerIncreaseCoefArray").iter().find(|r|r["Mode"].as_i64()==Some(m)).map_or(0.0,|r|f32_of(&r["Coef"]));
                let dec=|m:i64|cp.arr("ConservePowerDecreaseCoefArray").iter().find(|r|r["Mode"].as_i64()==Some(m)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let m=if crate::envc::var("JPRACE_CP_MODE").as_deref()==Ok("pre") {h.cp.pre_mode} else {h.keep.mode};
                let (tm,tp)=(h.tempted(),self.compete.modes.get(hi).is_some_and(|m|m.top));
                let (r_pd,r_n,d_tp,d_tm)=(rate(1),rate(2),dec(3),dec(4));
                let cp_cool=cp.f("ConservePowerDefaultCoolDownTime");
                let end_kind=self.horses[hi].temptation_end;
                let c=&mut self.horses[hi].cp;
                if tm && !c.s_tm && crate::envc::var("JPRACE_CP_TMSUB").as_deref()!=Ok("0") {c.spec-=c.last_credit;}
                if tm && !c.s_tm {c.spec*=d_tm;}
                c.last_credit=0.0;
                let rush_exit=!tm && c.s_tm && (matches!(end_kind,1|2) || (end_kind==3 && crate::envc::var("JPRACE_CP_RUSH3").as_deref()!=Ok("0"))) && c.lag_mode==0;
                if crate::envc::var("JPRACE_LOG_CPT").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("cpt tick={} tm={} s_tm={} tp={} m={} lag={} end_kind={} rush_exit={} ctimer={:.4} spec={:.3}",(self.time/0.0666).round() as i64,tm,c.s_tm,tp,m,c.lag_mode,end_kind,rush_exit,c.ctimer,c.spec);}
                if tp && !c.s_tp {c.spec*=d_tp;}
                c.s_tm=tm;c.s_tp=tp;
                {
                    c.lag_mode=m;
                    if tm || tp || matches!(m,1|2|3|5) {c.ctimer=-cp_cool;}
                    else if self.horses[hi].distance<(self.course.distance as i32/24*10) as f32 {
                        let c=&mut self.horses[hi].cp;c.ctimer+=dt;if rush_exit {c.ctimer+=dt;}
                        if c.ctimer>=1.0 {let g=(if m==4 {r_pd} else {r_n})*sc;c.spec+=g;c.last_credit=g;c.ctimer-=1.0;
                            if crate::envc::var("JPRACE_LOG_CPACC").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("cpacc h={} t={:.4} m={} g={:.4} spec={:.5}",hi,self.time,m,g,c.spec);}}
                    }
                }
            }
        }
        self.update_overtake(hi,dt);
        self.temptation(hi,dt);
        self.move_lane(hi,dt,true);
        if crate::envc::var("JPRACE_LOG_LANEPLAN").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            let h=&self.horses[hi];
            eprintln!("laneplan tick={} h={} d={:.3} lane={:.5} tgt={:.5} ot={:?} otcool={:.3} extra={:?} in={:?} out={:?} front={:?} ph={} fixed={} order={}",(self.time/0.0666).round() as i64,hi,h.distance,h.lane,h.target_lane,h.overtake,h.ot_cool,h.extra_lane,h.inside,h.outside,h.front,h.phase,h.mods.apply(11,0.0),h.order);
        }
        let early=crate::envc::var("JPRACE_SPURT_EARLY").as_deref()==Ok("1") && !self.horses[hi].spurt_calculated && self.course.phase(self.horses[hi].distance)>=2 && self.course.phase(self.horses[hi].distance)<4;
        if early && self.horses[hi].phase<2 {let ph=self.horses[hi].phase;self.horses[hi].phase=2;self.spurt(hi);self.horses[hi].phase=ph;}
        else if self.horses[hi].phase>=2 && (!self.horses[hi].spurt_calculated || recalc) {self.spurt(hi);}
        {
            let h=&self.horses[hi];
            if !h.spurt_ev && h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start {
                let t=self.time;let d=h.distance;
                self.horses[hi].spurt_ev=true;
                self.events.push(json!({"t":t,"type":13,"name":"LastSpurtStart","params":[hi,0],"horse":hi,"extra":format!("{:.3}",d)}));
            }
        }
        {
            let h=&self.horses[hi];
            if !h.cp.released && h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start {
                let pow=h.cp.pow+h.mods.apply(2,0.0);
                let cp=Tree(p.get("ConservePower"));
                let dt_=if self.course.distance<=1400.0 {1} else if self.course.distance<=1800.0 {2} else if self.course.distance<=2400.0 {3} else {4};
                let style=h.input.running_style as i64;
                let coef=cp.arr("ReleaseConservePowerAccelRunningStyleAndDistanceTypeCoefArray").iter()
                    .find(|r|r["RunningStyle"].as_i64()==Some(style) && r["DistanceType"].as_i64()==Some(dt_)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let dc_=cp.arr("ReleaseConservePowerActivityTimeDistanceTypeCoefArray").iter().find(|r|r["DistanceType"].as_i64()==Some(dt_)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let dur=h.cp.spec*self.course.distance.sqrt()*dc_/cp.f("ReleaseConservePowerActivityTimeCoef");
                let rushed=h.temptation_count>0;
                let dec=|m:i64|cp.arr("ConservePowerDecreaseAccelCoefArray").iter().find(|r|r["Mode"].as_i64()==Some(m)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let mode_coef=(if h.cp.top {dec(3)} else {1.0})*(if rushed {dec(4)} else {1.0});
                let accel=((pow-1200.0)*cp.f("ReleaseConservePowerDeclCoef")).sqrt()*cp.f("ReleaseConservePowerAccelCoef")*coef*mode_coef;
                let t=self.time;let h=&mut self.horses[hi];h.cp.released=true;
                if pow>1200.0 && (dur>cp.f("ConservePowerReleasableActivityTimeThreshold")) {
                    h.cp.accel=accel;h.cp.left=dur;
                    if crate::envc::var_os("JPRACE_LOG_CP").is_some() {eprintln!("cprelease h={} t={:.4} end={:.4} dur={:.5} accel={:.6} spec={:.5} pow={:.1}",hi,t,t+dur,dur,accel,h.cp.spec,pow);}
                    self.events.push(json!({"t":t,"type":6,"name":"ReleaseConservePower","params":[hi,((t+dur)*10000.0) as i32],"horse":hi,"extra":""}));
                }
            }
        }
        self.downhill(hi,dt);
        let section=({
            let length=(self.course.distance/24.0) as f64;
            if f32m('i') {(self.horses[hi].distance/(self.course.distance/24.0)) as usize} else {(self.horses[hi].distance as f64/length) as usize}
        }).min(23);
        let h=&self.horses[hi];
        let tst=if h.rc_start>0.0 {h.rc_start.min(h.spurt_start)} else {h.spurt_start};
        let pre_spurt=!(h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=tst);
        let mut target=if !pre_spurt {h.spurt_speed} else {h.section_speed[section]};
        if pre_spurt && self.course.phase(section as f32*(self.course.distance/24.0))>=2 && crate::envc::var("JPRACE_PRE_UNC").as_deref()!=Ok("0") {target+=self.fs_pre_add(hi);}
        let fx_last=crate::envc::var("JPRACE_FSX_LAST").ok();let mut fx_add=0.0f32;
        let mut tprev:Option<f32>=None;let mut tparts=(0.0f32,0.0f32);self.horses[hi].fs_cap=-1.0;
        let tb=target;self.horses[hi].tgt_base=tb;let h=&self.horses[hi];
        let lc=crate::envc::var("JPRACE_LOG_TGTCHAIN").as_deref()==Ok("all") || crate::envc::var("JPRACE_LOG_TGTCHAIN").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi);
        let mut chain:Vec<(String,u32)>=Vec::new();if lc {chain.push(("base".into(),target.to_bits()));}
        if h.keep.mode!=0 {target*=h.keep.multiply;if lc {chain.push((format!("mul{}",h.keep.multiply),target.to_bits()));}}
        let ftop=crate::envc::var("JPRACE_FS_TOP").unwrap_or_default();
        let early_f=if matches!(ftop.as_str(),"early"|"m6"|"ce") && h.rafs_on && h.input.speed>=2001 {let e=Tree(p.get("ExtraTargetSpeed"));
            let (sp,mp,sf,mf)=(e.f("ExtraTargetSpeedStartParam"),e.f("ExtraTargetSpeedMaxParam"),e.f("ExtraTargetSpeedStartFoundation"),e.f("ExtraTargetSpeedMaxFoundation"));
            sf+(mf-sf)*((h.input.speed as f32-sp)/(mp-sp)).clamp(0.0,1.0)} else {0.0};
        if ftop=="early" {target+=early_f;}
        let fi_on=self.course.empty_in_move && h.phase==0 && h.lane>=p.f("forceInMoveLaneThreshold") && ro_fi_sched(hi,(self.time/0.0666).round() as i64).unwrap_or_else(||if h.lane_off_ro && let Ok(m)=crate::envc::var("JPRACE_RO_FI2") {match m.as_str() {"out"=>h.outside.is_some(),"both"=>h.inside.is_some()||h.outside.is_some(),"none"=>false,"outrail"=>h.outside.is_some()||h.lane>=h.lane_max-0.0001,_=>h.inside.is_some()}} else {(h.inside.is_some() && !(h.lane_off_ro && crate::envc::var_os("JPRACE_RO_FIMODE").is_some())) || h.fi_in || h.lane<=h.lane_min+0.0001 || (h.lane_off_ro && (crate::envc::var("JPRACE_RO_FI").as_deref()==Ok("always") || h.fi_latch.get()))});
        if h.lane_off_ro && h.phase==0 && crate::envc::var_os("JPRACE_LOG_FI").is_some() {eprintln!("fi tick={} h={} on={} lane={:.6} d={:.4} in={:?} out={:?} fi_in={} lb={:08x} db={:08x} v={:.5}",(self.time/0.0666).round() as i64,hi,fi_on,h.lane,h.distance,h.inside,h.outside,h.fi_in,h.lane.to_bits(),h.distance.to_bits(),h.speed);}
        if fi_on {target+=h.force_in;if h.lane_off_ro && crate::envc::var("JPRACE_RO_FI").as_deref()==Ok("latch") {h.fi_latch.set(true);}if lc {chain.push((format!("fi{:08x}",h.force_in.to_bits()),target.to_bits()));}}
        if self.horses[hi].mods.entries.iter().any(|m|m.lb.is_some()) {
            let ph=if crate::envc::var("JPRACE_LB_PHASE").as_deref()==Ok("dist") {self.course.phase(self.horses[hi].distance)} else {self.horses[hi].phase};
            let coefs:Vec<f32>=self.horses[hi].mods.entries.iter().map(|m|m.lb.map_or(0.0,|r|self.wiz_limit_break_coef(ph,r[3] as i32,r[4]>0.5))).collect();
            let fs=if self.horses[hi].rafs_on {crate::envc::var("JPRACE_FS_SKILL").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0)} else {0.0};
            for (m,c) in self.horses[hi].mods.entries.iter_mut().zip(coefs) {if let Some(r)=m.lb {m.value=lb_value(r,fs,c);}}
            if crate::envc::var("JPRACE_LOG_LB").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                let tick=(self.time/0.0666).round() as i64;
                for m in self.horses[hi].mods.entries.iter().filter(|m|m.lb.is_some()) {let r=m.lb.unwrap();eprintln!("lb tick={} h={} idx={} v={:08x} r0={:08x} r1={:08x} r2={:08x} st={} ex={} ph={}",tick,hi,m.index,m.value.to_bits(),r[0].to_bits(),r[1].to_bits(),r[2].to_bits(),r[3],r[4],ph);}
            }
        }
        let h=&self.horses[hi];
        if crate::envc::var("JPRACE_LOG_M6").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            let e:Vec<String>=self.horses[hi].mods.entries.iter().filter(|m|m.index==6).map(|m|format!("{:x}:{:.5}:{:?}",m.owner>>8,m.value,m.remaining.map(|r|(r*1000.0).round()/1000.0))).collect();
            eprintln!("m6 tick={} h={} {:?}",(self.time/0.0666).round() as i64,hi,e);
        }
        let m6_before=target;
        target=if crate::envc::var("JPRACE_TGT_SUM").as_deref()==Ok("1") {h.mods.apply_sum(6,target)} else {h.mods.apply(6,target)};
        if let Some(c)=crate::envc::var("JPRACE_M6CAP").ok().and_then(|x|x.parse::<f32>().ok()) && target-m6_before>c {target=m6_before+c;}
        let m6_used=target-m6_before;self.horses[hi].m6_used=m6_used;
        if lc {for m in self.horses[hi].mods.entries.iter().filter(|m|m.index==6 && !m.once) {chain.push((format!("m6e{:08x}",m.value.to_bits()),0));}chain.push(("m6".into(),target.to_bits()));}
        if ftop=="m6" {target+=early_f;}
        self.horses[hi].m6_n=self.horses[hi].mods.entries.iter().filter(|m|m.index==6 && !m.once).count();let h=&self.horses[hi];
        let ce=self.compete_effects(hi);let h=&self.horses[hi];
        if !math::approximately(h.lane,h.prev_lane) && h.mods.apply(21,0.0)!=0.0 {
            target+=math::pow(h.stat(2)*p.f("Skill.LaneMoveAddParam1"),p.f("Skill.LaneMoveAddParam2"));
            if lc {chain.push(("lanemv".into(),target.to_bits()));}
        }
        if crate::envc::var("JPRACE_LOG_TGTBITS").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            let up=up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0);
            eprintln!("tgtbits tick={} sec={} tb={:08x} mode={} mul={:08x} before_slope={:08x} slope={} pow={:08x} up={:08x} m6={:08x} secbase={:08x} rb={:08x} forcein={:08x} fi_on={}",(self.time/0.0666).round() as i64,section,h.tgt_base.to_bits(),h.keep.mode,h.keep.multiply.to_bits(),target.to_bits(),h.slope,h.stat(2).to_bits(),up.to_bits(),h.mods.apply(6,0.0).to_bits(),h.section_base[section].to_bits(),self.race_base().to_bits(),h.force_in.to_bits(),
                self.course.empty_in_move && h.phase==0 && h.lane>=p.f("forceInMoveLaneThreshold") && (if h.lane_off_ro && let Ok(m)=crate::envc::var("JPRACE_RO_FI2") {match m.as_str() {"out"=>h.outside.is_some(),"both"=>h.inside.is_some()||h.outside.is_some(),"none"=>false,"outrail"=>h.outside.is_some()||h.lane>=h.lane_max-0.0001,_=>h.inside.is_some()}} else {(h.inside.is_some() && !(h.lane_off_ro && crate::envc::var_os("JPRACE_RO_FIMODE").is_some())) || h.fi_in || h.lane<=h.lane_min+0.0001 || (h.lane_off_ro && (crate::envc::var("JPRACE_RO_FI").as_deref()==Ok("always") || h.fi_latch.get()))}));
        }
        if h.slope>0.0 {
            target+=up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0);
        }
        else if h.downhill {target+=(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0);}
        if lc && (h.slope!=0.0 || h.downhill) {
            let term=if h.slope>0.0 {up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0)} else if h.downhill {(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0)} else {0.0};
            chain.push((format!("slope{}d{}t{:08x}",h.slope,h.downhill,term.to_bits()),target.to_bits()));}
        target+=ce.target_speed_add;if ftop=="ce" {target+=early_f;}if lc && ce.target_speed_add!=0.0 {chain.push((format!("ce{:08x}",ce.target_speed_add.to_bits()),target.to_bits()));}
        if fs_enabled() && dt>0.0 && !self.horses[hi].finished {
            let tick=(self.time/dt).round() as i64;
            let oracle=crate::envc::var_os("JPRACE_FS_ORACLE").is_some();
            let jk=crate::envc::var("JPRACE_FS_JK").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
            let start_f=Tree(self.p().get("ExtraTargetSpeed")).f("ExtraTargetSpeedStartFoundation");
            let j=(match crate::envc::var("JPRACE_FS_J").as_deref().unwrap_or("unc") {"unc"=>self.fs_extra_max(hi),x if x.parse::<f32>().is_ok()=>x.parse::<f32>().unwrap(),_=>self.fs_foundation(hi)-start_f}*jk).max(0.0);
            if !oracle {
                let elig=self.horses[hi].input.speed as f32>=Tree(self.p().get("ExtraTargetSpeed")).f("ExtraTargetRawSpeedParam");
                let in_spurt={let h=&self.horses[hi];let st=if h.j_start>0.0 {if crate::envc::var("JPRACE_RECALC_JSTART").as_deref()==Ok("latest") {h.j_start} else {h.j_start.min(h.spurt_start)}} else {h.spurt_start};h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=st};
                if elig && in_spurt {
                    self.horses[hi].fs_j=j;let eacc=crate::envc::var("JPRACE_FS_EACC").as_deref()==Ok("1") && self.horses[hi].rafs_on;if !eacc && (crate::envc::var("JPRACE_FS_JX").as_deref()!=Ok("1") || !(self.horses[hi].rafs_on || self.horses[hi].fs_pending)) {target+=j;if lc {chain.push((format!("j{:08x}",j.to_bits()),target.to_bits()));}}
                    let h=&self.horses[hi];let p=self.p();
                    let slope=if h.slope>0.0 && crate::envc::var("JPRACE_FS_UP").as_deref()!=Ok("0") {up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0)}
                        else if h.downhill && crate::envc::var_os("JPRACE_FS_DOWN").is_some() {(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0)} else {0.0};
                    self.horses[hi].fs_thr=h.spurt_speed+j+slope;
                }
                if self.horses[hi].fs_pending && !self.horses[hi].rafs_on {
                    let d=self.horses[hi].distance;
                    let h=&mut self.horses[hi];h.rafs_on=true;h.rafs_t0=self.time;
                    self.events.push(json!({"t":self.time,"type":12,"name":"RunAtFullSpeed","params":[hi as i32,0],"horse":hi,"extra":"","distance":d}));
                    if crate::envc::var("JPRACE_FS_RANDWIN").as_deref()!=Ok("0") {
                        let mut skills=std::mem::take(&mut self.horses[hi].skills);
                        for sk in &mut skills {for det in &mut sk.details {
                            if crate::envc::var_os("JPRACE_LOG_RAFSWIN").is_some() && det.pre.iter().chain(det.condition.iter()).flatten().any(|t|t.name=="run_at_full_speed_random") {
                                eprintln!("rafsctx h={} sk={} detail={} tick={} d={:.6}",hi,sk.id,det.number,(self.time/0.0666).round() as i64,d);
                            }
                            if crate::envc::var("JPRACE_RAFSWIN_LO").as_deref()==Ok("act") {
                                crate::triggers::set_run_at_full_speed_windows(&mut det.pre,&mut self.random,d,self.course.distance);
                                crate::triggers::set_run_at_full_speed_windows(&mut det.condition,&mut self.random,d,self.course.distance);
                            } else if crate::envc::var("JPRACE_RAFSWIN_DRAW").as_deref()==Ok("act") {
                                crate::triggers::draw_run_at_full_speed_u(&mut det.pre,&mut self.random);
                                crate::triggers::draw_run_at_full_speed_u(&mut det.condition,&mut self.random);
                            }
                        }}
                        self.horses[hi].skills=skills;
                        if crate::envc::var("JPRACE_RAFSWIN_LO").as_deref()!=Ok("act") && crate::envc::var("JPRACE_RAFSWIN_DRAW").as_deref()!=Ok("act") {self.horses[hi].rafs_draw=true;}
                    }
                }
            } else {
                let shift=crate::envc::var("JPRACE_FS_SHIFT").ok().and_then(|x|x.parse::<i64>().ok()).unwrap_or(0);
                if !self.horses[hi].rafs_on && self.rafs_oracle.iter().any(|&(oh,t)|oh==hi && (t/dt).round() as i64+shift==tick) {
                    let h=&mut self.horses[hi];h.rafs_on=true;h.rafs_t0=self.time;h.fs_j=j;
                }
                if self.horses[hi].rafs_on {target+=self.horses[hi].fs_j;}
            }
            let ramp_on=self.horses[hi].rafs_on || (crate::envc::var("JPRACE_FS_RAMP").as_deref()==Ok("cond") && self.horses[hi].fs_pending);
            if ramp_on {
                let g=|n:&str,d:f32|crate::envc::var(n).ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(d);
                let rmode=crate::envc::var("JPRACE_FS_R").unwrap_or_else(|_|"accup".into());
                let r=match Ok::<&str,()>(rmode.as_str()) {
                    Ok(m) if m.starts_with("acc")=>{
                        let h=&self.horses[hi];let p=self.p();let x=Tree(p.get("ExtraTargetSpeed"));
                        let array=if h.ex() {"PhaseAccelCoefExArray[0]".into()} else {format!("PhaseAccelCoefArray[{}]",h.input.running_style-1)};
                        let up=m.contains("up") && h.slope>0.0;
                        let m48=crate::envc::var("JPRACE_FS_48").unwrap_or_default();
                        let pex=if m48.is_empty() || m48=="pow" {h.mods.apply(26,0.0)*x.f("ExtraPowerAdjustCoef")} else {0.0};
                        let a=((h.stat(2)+pex)*p.f("AccelPowCoefSqrt")).sqrt()*p.f(if up {"accelPowCoefUpSlope"} else {"accelPowCoef"})*h.ground_coef*h.dist_power*p.f(&format!("Speed.{array}.{}",PHASE_NAMES[h.phase.min(3)]));
                        let a=if m.contains("sk") {h.mods.apply(10,a)} else {a};
                        if m48=="add" {
                            a*x.f("ExtraAccelSuppressCoef")*(self.course.distance/1000.0).powx(-x.f("ExtraAccelExponent"))*g("JPRACE_FS_RK",1.0)
                                +h.mods.apply(26,0.0)*crate::envc::var("JPRACE_FS_48H").ok().and_then(|o|o.split(',').find_map(|kv|kv.split_once('=').filter(|(a,_)|a.parse::<usize>().ok()==Some(hi)).and_then(|(_,v)|v.parse::<f32>().ok()))).unwrap_or(g("JPRACE_FS_48K",0.1))
                        } else if m48=="coef" {
                            a*(x.f("ExtraAccelSuppressCoef")+h.mods.apply(26,0.0))*(self.course.distance/1000.0).powx(-x.f("ExtraAccelExponent"))*g("JPRACE_FS_RK",1.0)
                        } else {
                            a*x.f("ExtraAccelSuppressCoef")*(self.course.distance/1000.0).powx(-x.f("ExtraAccelExponent"))*g("JPRACE_FS_RK",1.0)
                        }
                    },
                    _=>g("JPRACE_FS_R",0.0)};
                if crate::envc::var("JPRACE_LOG_FSR").as_deref()==Ok("all") || crate::envc::var("JPRACE_LOG_FSR").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                    let h=&self.horses[hi];let p=self.p();let x=Tree(p.get("ExtraTargetSpeed"));
                    let lp=(self.course.distance/1000.0).powx(-x.f("ExtraAccelExponent"));
                    eprintln!("fsr tick={} h={} r={:.7} rb={:08x} m26={:.4} lp={:.6} aL={:.6} pow={:.2} ph={} L={}",(self.time/dt).round() as i64,hi,r,r.to_bits(),h.mods.apply(26,0.0),lp,(r-h.mods.apply(26,0.0)*0.1)/x.f("ExtraAccelSuppressCoef"),h.stat(2),h.phase,self.course.distance);
                }
                let emax=g("JPRACE_FS_EMAX",1e9);
                let fnd_pre={let e=Tree(self.p().get("ExtraTargetSpeed"));let h=&self.horses[hi];
                    let (sp,mp,sf,mf)=(e.f("ExtraTargetSpeedStartParam"),e.f("ExtraTargetSpeedMaxParam"),e.f("ExtraTargetSpeedStartFoundation"),e.f("ExtraTargetSpeedMaxFoundation"));
                    sf+(mf-sf)*((h.input.speed as f32-sp)/(mp-sp)).clamp(0.0,1.0)};
                let r=if r!=0.0 {crate::envc::var("JPRACE_FS_RULP").ok().and_then(|o|o.split(',').find_map(|kv|kv.split_once(':').filter(|(a,_)|a.parse::<usize>().ok()==Some(hi)).and_then(|(_,v)|v.parse::<i32>().ok()))).map_or(r,|k|f32::from_bits((r.to_bits() as i32+k) as u32))} else {r};
                let h=&mut self.horses[hi];h.fs_r=r;
                let fxm=crate::envc::var("JPRACE_FSX_ACC").ok();let fxp=h.fs_x;
                if crate::envc::var("JPRACE_FS_RFIRST").as_deref()!=Ok("0") {
                    match fxm.as_deref() {
                        Some("f64")=>{h.fs_x64+=r as f64*dt as f64;h.fs_x=h.fs_x64 as f32;},
                        Some("mul")=>{h.fs_n+=1;h.fs_x=r*(h.fs_n as f32*dt);},
                        Some("mul2")=>{h.fs_n+=1;h.fs_x=r*dt*h.fs_n as f32;},
                        Some("time")=>{h.fs_x=r*(self.time-h.rafs_t0+dt);},
                        Some(m) if m.starts_with("k")=>{let k:f32=if m=="kf" {fnd_pre} else {m[1..].parse().unwrap_or(1.0)};if h.fs_n==0 {h.fs_x64=k as f64;}h.fs_x64=((h.fs_x64 as f32)+r*dt) as f64;h.fs_x=h.fs_x64 as f32-k;},
                        Some("dtt")=>{h.fs_x+=r*(self.time-self.prev_time);},
                        Some("dtt2")=>{h.fs_x+=r*(self.prev_time+dt-self.prev_time);},
                        _=>{h.fs_x+=r*dt;},
                    }
                    if h.fs_n==0 {h.fs_n=1;if let Some(k)=crate::envc::var("JPRACE_FS_XOFF").ok().and_then(|o|o.split(',').find_map(|kv|kv.split_once(':').filter(|(a,_)|a.parse::<usize>().ok()==Some(hi)).and_then(|(_,v)|v.parse::<f32>().ok()))) {h.fs_x+=k*1e-9;}}
                }
                if crate::envc::var("JPRACE_FS_JX").as_deref()==Ok("1") && h.fs_j>0.0 {target+=h.fs_j+h.fs_x.min(emax);} else {
                if let Some(c)=crate::envc::var("JPRACE_PREFS_MAX").ok().and_then(|x|x.parse::<f32>().ok()) {target=target.min(c);}
                let fs_top=crate::envc::var("JPRACE_FS_TOP").unwrap_or_default();
                if early_f>0.0 {target=target.min(30.0)-early_f;} else if matches!(fs_top.as_str(),"early"|"m6"|"ce") {target=target.min(30.0-fnd_pre);} else
                {match fs_top.as_str() {"0"|"lim"|"lim2"|"limu"|"lim2u"=>{}, "sub"=>{target=(target+fnd_pre).min(30.0)-fnd_pre;}, "subf"=>{if h.input.speed>=2001 {target=(target+fnd_pre).min(30.0)-fnd_pre;} else {target=target.min(30.0-fnd_pre);}}, "subr"=>{if h.rafs_on {target=(target+fnd_pre).min(30.0)-fnd_pre;} else {target=target.min(30.0-fnd_pre);}}, _=>{
                    let sd=crate::envc::var("JPRACE_SLB_DELAY").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
                    let sv=if slb_enabled() && h.slb_on && self.time>=h.slb_t0+sd*dt-1e-4 {h.slb_v} else {0.0};
                    match crate::envc::var("JPRACE_FS_TOPSLB").as_deref() {
                        Ok("0")=>{target=target.min(30.0-fnd_pre);},
                        Ok("b")=>{target=target.min(30.0-(fnd_pre+sv));},
                        Ok("c")=>{if sv>0.0 {target=(target+sv).min(30.0-fnd_pre)-sv;} else {target=target.min(30.0-fnd_pre);}},
                        _=>{target=target.min((30.0-fnd_pre)-sv);},
                    }}}}
                let base_now=target;
                if crate::envc::var("JPRACE_FS_TACC").as_deref()==Ok("1") && h.fs_tacc[0]==base_now && h.fs_tacc[1]>0.0 {
                    target=h.fs_tacc[1]+r*dt;
                } else if crate::envc::var("JPRACE_FS_EACC").as_deref()==Ok("1") {
                    let jn=if h.input.speed>=2001 {h.fs_j} else {0.0};
                    if h.fs_e<0.0 {h.fs_e=jn+r*dt;} else {h.fs_e+=r*dt;if jn!=h.fs_lj {h.fs_e+=jn-h.fs_lj;}}
                    h.fs_lj=jn;target+=h.fs_e;
                } else if let Some(lm)=crate::envc::var("JPRACE_FS_LIMF").ok() {
                    let fnd=fnd_pre;let fx=h.fs_x.min(emax);
                    target=if lm=="2" {(target+fnd)+(fx-fnd)} else {(target+fnd)-(fnd-fx)};
                } else if fx_last.is_some() {fx_add=h.fs_x.min(emax);}
                else if (fs_top=="lim" && target+fnd_pre>30.0) || fs_top=="limu" {let tf=(target+fnd_pre).min(30.0);tparts=(target,fxp.min(emax));tprev=Some(tf-(fnd_pre-fxp.min(emax)));target=tf-(fnd_pre-h.fs_x.min(emax));}
                else if (fs_top=="lim2" && target+fnd_pre>30.0) || fs_top=="lim2u" {let tf=(target+fnd_pre).min(30.0);tparts=(target,fxp.min(emax));tprev=Some(tf+(fxp.min(emax)-fnd_pre));target=tf+(h.fs_x.min(emax)-fnd_pre);}
                else {tparts=(target,fxp.min(emax));tprev=Some(target+fxp.min(emax));
                    if crate::envc::var("JPRACE_LOG_TPARTS").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("tparts tick={} pre={:08x} fxp={:08x} fx={:08x} cap={:08x}",(self.time/0.0666).round() as i64,target.to_bits(),fxp.min(emax).to_bits(),h.fs_x.min(emax).to_bits(),(target+fxp.min(emax)).to_bits());}
                    target+=h.fs_x.min(emax);}
                h.fs_tacc=[base_now,target];
                if lc {chain.push((format!("fsx{:08x}",h.fs_x.to_bits()),target.to_bits()));}}
                if crate::envc::var("JPRACE_FS_RFIRST").as_deref()==Ok("0") {h.fs_x+=r*dt;}
            }
        }
        if slb_enabled() && dt>0.0 && !self.horses[hi].finished && self.horses[hi].slb_u.is_some() {
            let in_spurt={let h=&self.horses[hi];h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start};
            if in_spurt {
                let h=&self.horses[hi];let p=self.p();
                let slope=if h.slope>0.0 && crate::envc::var("JPRACE_SLB_UP").as_deref()==Ok("1") {up_slope(h.slope,p.f("Slope.UpSlopeAddSpeedVal1"),h.stat(2)).min(0.0)}
                    else if h.downhill && crate::envc::var_os("JPRACE_SLB_DOWN").is_some() {(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0)} else {0.0};
                let spv=if crate::envc::var("JPRACE_SLB_SPV0").as_deref()!=Ok("0") && h.spv0>0.0 {h.spv0} else {h.spurt_speed};
                let thr=spv+h.fs_j+slope;
                self.horses[hi].slb_thr=thr;
                let h=&self.horses[hi];
                if crate::envc::var("JPRACE_SLB_TIMING").as_deref()!=Ok("old") && !h.slb_on && !h.slb_pending && h.slb_thr>0.0 && h.slb_cmp>=h.slb_thr
                    && !(h.slb_re && h.hp_prev0 && crate::envc::var("JPRACE_SLB_HP0").as_deref()!=Ok("0")) {
                    self.horses[hi].slb_pending=true;
                }
            }
            if self.horses[hi].slb_pending && !self.horses[hi].slb_on {
                let mut v=self.slb_value(hi);
                if let Ok(o)=crate::envc::var("JPRACE_SLB_OVR") {
                    for kv in o.split(',') {if let Some((x,y))=kv.split_once('=') && x.trim().parse::<usize>().ok()==Some(hi) && let Ok(y)=y.trim().parse::<f32>() {v=y;}}
                }
                let tm=self.time;
                if crate::envc::var_os("JPRACE_LOG_SLBPEEK").is_some() {
                    let mut r=self.random.clone();let c=r.calls();
                    let xs:Vec<String>=(0..8).map(|_|format!("{:.6}",r.raw() as f64/2147483647.0)).collect();
                    eprintln!("slbpeek h={} t={:.4} call={} next={}",hi,tm,c,xs.join(","));
                }
                let h=&mut self.horses[hi];h.slb_on=true;h.slb_v=v;h.slb_t0=tm;
                let (sta,u)=(self.horses[hi].stat(1),self.horses[hi].slb_u.unwrap_or([0.0,0.0]));
                if (self.course.distance>=2101.0 || crate::envc::var_os("JPRACE_SLB_EV_ALL").is_some()) && !self.horses[hi].slb_re {
                    self.events.push(json!({"t":self.time,"type":7,"name":"StaminaLimitBreakBuff","params":[hi as i32,0],"horse":hi,"extra":"","value":v,"sta":sta,"u1":u[0],"u2":u[1],"pow":self.horses[hi].stat(2),"rawsta":self.horses[hi].input.stamina,
                        "m5":self.horses[hi].mods.apply(5,0.0),"m23":self.horses[hi].mods.apply(23,0.0),"result":self.horses[hi].spurt_result,"rafs":self.horses[hi].rafs_on,"fs_elig":self.horses[hi].input.speed>=2001}));
                }
            }
            let delay=crate::envc::var("JPRACE_SLB_DELAY").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0);
            if self.horses[hi].slb_on && self.time>=self.horses[hi].slb_t0+delay*dt-1e-4 {
                match fx_last.as_deref() {
                    Some("slbsum")=>{target+=fx_add+self.horses[hi].slb_v;fx_add=0.0;},
                    Some("slbkeep")=>{target=((target as f64+fx_add as f64)+self.horses[hi].slb_v as f64) as f32;fx_add=0.0;},
                    _=>{target+=self.horses[hi].slb_v;if let Some(t)=tprev.as_mut() {
                        if crate::envc::var("JPRACE_CAP_GRP").as_deref()==Ok("slb") {*t=tparts.0+(tparts.1+self.horses[hi].slb_v);} else {*t+=self.horses[hi].slb_v;}}},
                }if lc {chain.push((format!("slb{:08x}",self.horses[hi].slb_v.to_bits()),target.to_bits()));}}
        }
        if matches!(fx_last.as_deref(),Some("slb"|"slbsum"|"slbkeep")) {target+=fx_add;}
        let h=&self.horses[hi];
        let shift=crate::envc::var("JPRACE_MODE_SHIFT").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(-1.0)*dt;
        let mut madd=0.0f32;
        let es=crate::envc::var("JPRACE_MODE_ENDSHIFT").ok();
        let mut ord:Vec<(usize,i64,f32,f32)>=self.oracle.clone();
        if crate::envc::var("JPRACE_MODE_ORDER").as_deref()!=Ok("rec") {ord.sort_by_key(|e|e.1);}
        for &(oh,kind,st,en) in &ord {
            let eshift=es.as_deref().and_then(|v|v.split(',').find_map(|x|{let (a,b)=x.split_once(':')?;(a.parse::<i64>().ok()?==kind).then(||b.parse::<f32>().ok()).flatten()})).unwrap_or(0.0)*dt;
            if oh!=hi || !(self.time>=st-shift-1e-3 && self.time<en-shift+eshift-1e-3) {continue;}
            let mult=if crate::envc::var("JPRACE_MINOR_DYN").as_deref()==Ok("1") {self.jp_minor_mult(hi,kind,(self.time/dt).round() as i64)}
                else {self.minor_ev.iter().find(|e|e.0==oh && e.1==kind && e.2==st).map_or(1.0,|e|e.3)};
            let v=self.jp_mode_speed(hi,kind)*mult;target+=v;madd+=v;if let Some(t)=tprev.as_mut() {*t+=v;}if lc {chain.push((format!("mode{}:{:08x}",kind,v.to_bits()),target.to_bits()));}
        }
        if fx_last.as_deref()==Some("mode") {target+=fx_add;}
        let h=&mut self.horses[hi];h.mode_add=madd;let h=&self.horses[hi];
        if let Ok(x)=crate::envc::var("JPRACE_TRACE_T") && x.parse::<usize>().ok()==Some(hi) {
            eprintln!("tgt t={:.4} sec={} secspd={:.5} keep={} mul={:.5} forcein={:.5} cond_in={} m6={:.5} lanemv={} slope={} down={} ce={:.5} final={:.5}",self.time,section,h.section_speed[section],h.keep.mode,h.keep.multiply,h.force_in,
                self.course.empty_in_move && h.phase==0 && h.lane>=p.f("forceInMoveLaneThreshold") && (if h.lane_off_ro && let Ok(m)=crate::envc::var("JPRACE_RO_FI2") {match m.as_str() {"out"=>h.outside.is_some(),"both"=>h.inside.is_some()||h.outside.is_some(),"none"=>false,"outrail"=>h.outside.is_some()||h.lane>=h.lane_max-0.0001,_=>h.inside.is_some()}} else {(h.inside.is_some() && !(h.lane_off_ro && crate::envc::var_os("JPRACE_RO_FIMODE").is_some())) || h.fi_in || h.lane<=h.lane_min+0.0001 || (h.lane_off_ro && (crate::envc::var("JPRACE_RO_FI").as_deref()==Ok("always") || h.fi_latch.get()))}),h.mods.apply(6,0.0),
                !math::approximately(h.lane,h.prev_lane) && h.mods.apply(21,0.0)!=0.0,h.slope,h.downhill,ce.target_speed_add,target);
        }
        let tgt_nohp=target.clamp(p.f("Speed.TargetSpeedMin"),30.0);
        if let Ok(x)=crate::envc::var("JPRACE_TGT_ADJ") {
            let f:Vec<f32>=x.split(':').filter_map(|v|v.parse().ok()).collect();
            let tick=(self.time/0.0666).round();
            if f.len()==4 && f[0] as usize==hi && tick>=f[1] && tick<=f[2] {target+=f[3];if let Some(t)=tprev.as_mut() {*t+=f[3];}}
        }
        if let Ok(x)=crate::envc::var("JPRACE_TGT_ADJL") {
            let tick=(self.time/0.0666).round();
            for w in x.split(',') {
                let f:Vec<f32>=w.split(':').filter_map(|v|v.parse().ok()).collect();
                if f.len()==4 && f[0] as usize==hi && tick>=f[1] && tick<=f[2] {target+=f[3];if let Some(t)=tprev.as_mut() {*t+=f[3];}}
            }
        }
        if h.hp<=0.0 {tprev=None;}
        if h.hp<=0.0 {
            target=h.min_speed;
            if h.rafs_on && crate::envc::var("JPRACE_HP0_FS").as_deref()==Ok("all") {
                let e=Tree(p.get("ExtraTargetSpeed"));
                let (sp,mp,sf,mf)=(e.f("ExtraTargetSpeedStartParam"),e.f("ExtraTargetSpeedMaxParam"),e.f("ExtraTargetSpeedStartFoundation"),e.f("ExtraTargetSpeedMaxFoundation"));
                let fnd=sf+(mf-sf)*((h.input.speed as f32-sp)/(mp-sp)).clamp(0.0,1.0);
                target=h.min_speed-(fnd-h.fs_x);
            }
        }
        let tmax=crate::envc::var("JPRACE_TGTMAX").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(30.0);
        target=target.clamp(p.f("Speed.TargetSpeedMin"),tmax);
        if let Some(t)=tprev {self.horses[hi].fs_cap=t.clamp(p.f("Speed.TargetSpeedMin"),tmax);}
        let h=&self.horses[hi];
        if let Ok(x)=crate::envc::var("JPRACE_TGT_ULP") {
            let f:Vec<i64>=x.split(':').filter_map(|v|v.parse().ok()).collect();
            let tick=(self.time/0.0666).round() as i64;
            if f.len()==4 && f[0] as usize==hi && tick>=f[1] && tick<=f[2] {target=f32::from_bits((target.to_bits() as i64+f[3]) as u32);}
        }
        if lc {let s:Vec<String>=chain.iter().map(|(n,b)|format!("{}={:08x}",n,b)).collect();eprintln!("tchain tick={} h={} v={:08x} final={:08x} {}{}",(self.time/0.0666).round() as i64,hi,h.self_speed.to_bits(),target.to_bits(),s.join(" "),if h.fs_cap>0.0 {format!(" cap={:08x} fsr={:08x}",h.fs_cap.to_bits(),h.fs_r.to_bits())} else {String::new()});}
        if h.finished {target=if h.after_goal<1.0 {h.target_speed} else if h.input.viewer_id!=0 {6.0} else {hi as f32*0.05+6.75};}
        let max=if let Some(i)=h.front {
            let front=&self.horses[i];
            {
                let g=h.around.iter().find(|a|a.i==i).map_or(front.distance-h.distance,|a|a.gap);
                let t=(g/p.f("Block.FrontBlockDistanceGap")).clamp(0.0,1.0);
                let (mx,mn)=(p.f("Block.FrontBlockMaxSpeedRateMax"),p.f("Block.FrontBlockMaxSpeedRateMin"));
                let v=front.speed*if f32m('b') {(mx-mn)*t+mn} else {((mx as f64-mn as f64)*t as f64+mn as f64) as f32};
                v.max(h.min_speed)
            }
        } else {30.0};
        let array=if h.ex() {"PhaseAccelCoefExArray[0]".into()} else {format!("PhaseAccelCoefArray[{}]",h.input.running_style-1)};
        let acc_dist=crate::envc::var("JPRACE_ACC_PHASE").as_deref()==Ok("dist");
        let aph=if acc_dist && !h.finished {self.course.phase(h.distance).min(3)} else {h.phase.min(3)};
        if crate::envc::var_os("JPRACE_LOG_PHLAG").is_some() && !h.finished && self.course.phase(h.distance).min(3)!=h.phase.min(3) {
            eprintln!("phlag tick={} h={} ph_var={} ph_dist={} v={:.5} target={:.5}",(self.time/0.0666).round() as i64,hi,h.phase,self.course.phase(h.distance),h.self_speed,target);
        }
        let acc_ord=crate::envc::var("JPRACE_ACC_ORD").ok();
        let mut accel=if h.finished {1.0} else if let Some(o)=acc_ord.as_deref().filter(|_|f32m('a')) {
            let fac=|c:char|match c {'s'=>(h.stat(2)*p.f("AccelPowCoefSqrt")).sqrt(),'b'=>p.f(if h.slope>0.0 {"accelPowCoefUpSlope"} else {"accelPowCoef"}),
                'g'=>h.ground_coef,'d'=>h.dist_power,_=>p.f(&format!("Speed.{array}.{}",PHASE_NAMES[aph]))};
            let mut it=o.chars();let mut a=fac(it.next().unwrap_or('s'));for c in it {a*=fac(c);}a
        } else if f32m('a') {
            (h.stat(2)*p.f("AccelPowCoefSqrt")).sqrt()*p.f(if h.slope>0.0 {"accelPowCoefUpSlope"} else {"accelPowCoef"})*
                h.ground_coef*h.dist_power*p.f(&format!("Speed.{array}.{}",PHASE_NAMES[aph]))
        } else {
            ((h.stat(2)*p.f("AccelPowCoefSqrt")).sqrt() as f64*p.f(if h.slope>0.0 {"accelPowCoefUpSlope"} else {"accelPowCoef"}) as f64*
                h.ground_coef as f64*h.dist_power as f64*p.f(&format!("Speed.{array}.{}",PHASE_NAMES[aph])) as f64) as f32
        };
        let acc_sep=crate::envc::var("JPRACE_ACC_FORM").as_deref()!=Ok("add");let mut acc_fs=0.0f32;
        if !h.finished {
            let (am,sm,aa)=if h.rafs_on {(crate::envc::var("JPRACE_FS_ACC").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0),
                crate::envc::var("JPRACE_FS_SKACC").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.0),
                crate::envc::var("JPRACE_FS_AADD").ok().map_or(Some(h.fs_r),|x|if x=="r" {Some(h.fs_r)} else {x.parse::<f32>().ok()}).unwrap_or(0.0))} else {(1.0,1.0,0.0)};
            let aa=if acc_sep {acc_fs=aa;0.0} else {aa};
            if am!=1.0 || sm!=1.0 || aa!=0.0 {accel=accel*am+(h.mods.apply(10,0.0)+ce.acceleration_add)*sm+aa;} else {accel=h.mods.apply(10,accel)+ce.acceleration_add;}
            if h.cp.left>0.0 {accel+=h.cp.accel;}if h.start_dash {accel+=p.f("StartAccelAdd");}accel=accel.max(0.0);}
        let rate=if h.finished {4.0/p.f("declBase")} else {
            let rate=p.f(if h.keep.mode==4 {"DeclRatePositionKeepPaseDown"}
                else if aph==0 {"declRateStart"} else if aph==1 {"declRateMiddle"} else {"declRateEnd"});
            let rate=if h.downhill && let Some(r)=crate::envc::var("JPRACE_DOWN_DECL").ok().and_then(|x|x.parse::<f32>().ok()) {r} else {rate};
            let rate=if let Ok(x)=crate::envc::var("JPRACE_DECL_OVR") {let f:Vec<f32>=x.split(':').filter_map(|v|v.parse().ok()).collect();let tk=(self.time/0.0666).round();
                if f.len()==4 && f[0] as usize==hi && tk>=f[1] && tk<=f[2] {f[3]} else {rate}} else {rate};
            if h.hp<=0.0 {rate.max(p.f("declRateHpZero"))} else {rate}
        };
        self.move_lane(hi,dt,false);
        let h=&mut self.horses[hi];let prev_target=h.target_speed;h.target_speed=target;h.tgt_nohp=tgt_nohp;
        h.goal_tgt=-1.0;
        if h.hp<=0.0 && h.rafs_on && !h.finished && crate::envc::var("JPRACE_HP0_FS").as_deref()!=Ok("old") {
            let e=Tree(p.get("ExtraTargetSpeed"));
            let (sp,mp,sf,mf)=(e.f("ExtraTargetSpeedStartParam"),e.f("ExtraTargetSpeedMaxParam"),e.f("ExtraTargetSpeedStartFoundation"),e.f("ExtraTargetSpeedMaxFoundation"));
            let fnd=sf+(mf-sf)*((h.input.speed as f32-sp)/(mp-sp)).clamp(0.0,1.0);
            h.goal_tgt=h.min_speed-(fnd-h.fs_x);
            if crate::envc::var("JPRACE_HP0_FS").as_deref()==Ok("all") {h.target_speed=h.min_speed;}
        }
        let mut v=h.self_speed+h.mods.consume(23,0.0);
        if h.rafs_on && v>target && crate::envc::var_os("JPRACE_FS_RRESET").is_some() {h.fs_x=0.0;}
        let dsub=if h.rafs_on && crate::envc::var("JPRACE_FS_DSUB").as_deref()!=Ok("0") {h.fs_r} else {0.0};
        let sf=crate::envc::var("JPRACE_SPD_FMA").unwrap_or_default();
        let fpin=crate::envc::var("JPRACE_FS_PIN").ok();
        let post=crate::envc::var("JPRACE_FS_POST").as_deref()==Ok("1");
        if crate::envc::var("JPRACE_LOG_ACCV").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            eprintln!("accv tick={} v={:08x} a={:08x} r={:08x} tgt={:08x} raw={:08x} dec={:08x}",(self.time/0.0666).round() as i64,v.to_bits(),accel.to_bits(),acc_fs.to_bits(),target.to_bits(),((v+accel*dt)+acc_fs*dt).to_bits(),((v-rate*p.f("declBase")*dt)+dsub*dt).to_bits());
        }
        if v<target {v=if sf.contains('a') {accel.mul_add(dt,v)}
            else if acc_sep && acc_fs!=0.0 && fpin.is_some() && prev_target>0.0 {let v1=(v+accel*dt).min(prev_target)+acc_fs*dt;if fpin.as_deref()==Some("2") {v1.min(f32::MAX)} else {v1}}
            else if acc_sep && acc_fs!=0.0 && post {(v+accel*dt).min(target)+acc_fs*dt}
            else if acc_sep && acc_fs!=0.0 && h.fs_cap>0.0 && fs_capprev() {((v+accel*dt).min(h.fs_cap)+acc_fs*dt).min(if fs_capprev_fin() {target} else {f32::MAX})}
            else if acc_sep && acc_fs!=0.0 {match crate::envc::var("JPRACE_ACC_ORDER").as_deref() {Ok("rf")=>(v+acc_fs*dt)+accel*dt,Ok("in")=>v+(accel*dt+acc_fs*dt),_=>(v+accel*dt)+acc_fs*dt}} else {v+accel*dt}.min(if ((fpin.as_deref()==Some("2") || post) && acc_fs!=0.0) || (acc_sep && acc_fs!=0.0 && h.fs_cap>0.0 && fs_capnomin()) {f32::MAX} else {target});}
        else if v>target {let dr=rate*p.f("declBase")-dsub;
            let db=p.f("declBase");
            let dn=crate::envc::var("JPRACE_DECL_NOFS").ok();
            v=if dsub==0.0 && dn.is_some() {match dn.as_deref() {Some("E")=>v-rate*(db*dt),Some("F")=>(v as f64-(rate*db) as f64*dt as f64) as f32,Some("H")=>v-(rate*(db*dt)),_=>v+(-rate*db)*dt}.max(target)} else {
            match crate::envc::var("JPRACE_DECL_FORM").as_deref() {
                Ok("B")=>v-rate*db*dt+dsub*dt, Ok("D")=>v-(rate*db*dt-dsub*dt), Ok("E")=>v-rate*(db*dt)+dsub*dt,
                Ok("A")=>if sf.contains('d') {(-dr).mul_add(dt,v)} else {v-dr*dt},
                _=>if post && dsub!=0.0 {(v-rate*db*dt).max(target)+dsub*dt} else if dsub!=0.0 && h.fs_cap>0.0 && fs_capdec() {(v-rate*db*dt).max(h.fs_cap)+dsub*dt} else {(v-rate*db*dt)+dsub*dt}}.max(if (post || (fs_capdec() && h.fs_cap>0.0)) && dsub!=0.0 {f32::MIN} else {target})};}
        if h.start_dash {v=v.min(default_min);}v=v.clamp(h.min_speed,max);
        if let Ok(x)=crate::envc::var("JPRACE_SPD_ADJ") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<&str>=e.split(':').collect();
                if f.len()==3 && f[0].parse::<usize>().ok()==Some(hi) && f[1].parse::<i64>().ok()==Some(tick) {v+=f[2].parse::<f32>().unwrap_or(0.0);}
            }
        }
        h.self_speed=v;if h.cp.left>0.0 {h.cp.left-=dt;}
        if slb_enabled() && dt>0.0 {
            let h=&self.horses[hi];
            if h.slb_u.is_some() {
                let c=match crate::envc::var("JPRACE_SLB_CMP").as_deref() {Ok("self")=>v,Ok("speed")=>v+h.mods.apply(23,0.0)+h.mods.apply(5,0.0),Ok("old")=>v+h.mods.apply(23,0.0),_=>(v+h.mods.apply(23,0.0)).min(max)};
                self.horses[hi].slb_cmp=c;
                if crate::envc::var("JPRACE_LOG_SLBCMP").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                    let h=&self.horses[hi];
                    eprintln!("slbcmp tick={} v={:.5} m23={:.4} m5={:.4} max={:.4} cmp={:.5} thr={:.5} tgt={:.5} spd={:.5} front={:?} slope={} down={}",(self.time/0.0666).round() as i64,v,h.mods.apply(23,0.0),h.mods.apply(5,0.0),max,c,h.slb_thr,target,h.speed,h.front,h.slope,h.downhill);
                }
            }
            let h=&self.horses[hi];
            if h.slb_u.is_some() && !h.slb_on && !h.slb_pending && h.slb_thr>0.0 && h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start {
                let cmp=match crate::envc::var("JPRACE_SLB_CMP").as_deref() {Ok("self")=>v,Ok("speed")=>v+h.mods.apply(23,0.0)+h.mods.apply(5,0.0),Ok("old")=>v+h.mods.apply(23,0.0),_=>(v+h.mods.apply(23,0.0)).min(max)};
                if crate::envc::var("JPRACE_LOG_SLBT").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                    eprintln!("slbt tick={} h={} v={:.5} cmp={:.5} thr={:.5} spv={:.5} fsj={:.5} rafs={} target={:.5} d={:.3}",(self.time/0.0666).round() as i64,hi,v,cmp,h.slb_thr,h.spurt_speed,h.fs_j,h.rafs_on,target,h.distance);
                }
                if crate::envc::var("JPRACE_SLB_TIMING").as_deref()==Ok("old") {if cmp>=h.slb_thr {self.horses[hi].slb_pending=true;}}
            }
        }
        if fs_enabled() && crate::envc::var_os("JPRACE_FS_ORACLE").is_none() && dt>0.0 {
            let h=&self.horses[hi];
            let spr_ok=h.spurt_result==1 || (crate::envc::var_os("JPRACE_FS_SPR2").is_some() && h.spurt_result&12==0);
            if !h.rafs_on && !h.fs_pending && h.fs_thr>0.0 && h.spurt_start>0.0 && spr_ok && h.distance>=h.spurt_start {
                let cmp=match crate::envc::var("JPRACE_FS_CMP").as_deref() {Ok("self")=>v,Ok("nocap")=>v+h.mods.apply(23,0.0),_=>(v+h.mods.apply(23,0.0)).min(max)};
                let thr=match crate::envc::var("JPRACE_FS_THR").as_deref() {Ok("min")=>target.min(h.spurt_speed+h.fs_j),_=>h.fs_thr};
                if crate::envc::var("JPRACE_LOG_FST").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                    eprintln!("fst tick={} h={} v={:.5} cmp={:.5} thr={:.5} spv={:.5} fsj={:.5} slope={} target={:.5} d={:.3} m23={:.4} m5={:.4} accel_ok={}",(self.time/0.0666).round() as i64,hi,v,cmp,thr,h.spurt_speed,h.fs_j,h.slope,target,h.distance,h.mods.apply(23,0.0),h.mods.apply(5,0.0),v<target);
                }
                if cmp>=thr {self.horses[hi].fs_pending=true;
                    if let Some(n)=crate::envc::var("JPRACE_FS_PRERAMP").ok().and_then(|x|x.parse::<f32>().ok()) {
                        let h=&self.horses[hi];let p=self.p();let x=Tree(p.get("ExtraTargetSpeed"));
                        let array=if h.ex() {"PhaseAccelCoefExArray[0]".into()} else {format!("PhaseAccelCoefArray[{}]",h.input.running_style-1)};
                        let a=(h.stat(2)*p.f("AccelPowCoefSqrt")).sqrt()*p.f(if h.slope>0.0 {"accelPowCoefUpSlope"} else {"accelPowCoef"})*h.ground_coef*h.dist_power*p.f(&format!("Speed.{array}.{}",PHASE_NAMES[h.phase.min(3)]));
                        let r=a*x.f("ExtraAccelSuppressCoef")*(self.course.distance/1000.0).powx(-x.f("ExtraAccelExponent"));
                        self.horses[hi].fs_x+=r*dt*n;
                    }
                }
            }
        }
        if crate::envc::var_os("JPRACE_FS_PRED").is_some() && self.horses[hi].input.speed>=2001 && self.horses[hi].spurt_start>0.0 && self.horses[hi].distance>=self.horses[hi].spurt_start {
            let tick=(self.time/dt).round() as i64;
            let h=&self.horses[hi];
            let x=Tree(self.p().get("ExtraTargetSpeed"));
            let lin=|s:f32|((s-x.f("ExtraTargetSpeedStartParam"))/(x.f("ExtraTargetSpeedMaxParam")-x.f("ExtraTargetSpeedStartParam"))).clamp(0.0,1.0)*(x.f("ExtraTargetSpeedMaxFoundation")-x.f("ExtraTargetSpeedStartFoundation"));
            let jm=lin(h.mods.apply(0,h.speed_unc));let jn=lin(h.speed_unc);
            let cur=h.mods.apply(23,0.0)+h.mods.apply(5,0.0)+v;
            let conds=[v>=h.spurt_speed+jm,v>=h.spurt_speed+jn,v>=h.spurt_speed.min(target)+jm,v>=h.spurt_speed.min(target)+jn,cur>=h.spurt_speed+jm,cur>=h.spurt_speed+jn,v>=h.spurt_speed,v>h.spurt_speed,v>=target,cur>h.spurt_speed];
            let mut bits=h.fs_pred;
            for (k,c) in conds.iter().enumerate() {if *c && bits&(1<<k)==0 {bits|=1<<k;eprintln!("fspred h={} tick={} cond={} v={:.5} cur={:.5} spv={:.5} tgt={:.5} jm={:.5} jn={:.5}",hi,tick,k,v,cur,h.spurt_speed,target,jm,jn);}}
            self.horses[hi].fs_pred=bits;
        }
        let h=&mut self.horses[hi];
        if let Ok(x)=crate::envc::var("JPRACE_TRACE") {if x.parse::<usize>().ok()==Some(hi) {eprintln!("trace t={:.4} d={:.3} v={:.5} target={:.5} accel={:.5} keep={} phase={} lane={:.4} sec_rand={:?} hp={:.4} spd={:.5}",self.time,h.distance,v,target,accel,h.keep.mode,h.phase,h.lane,h.section_rand_dbg(),h.hp,h.speed);}}
        let m23=h.mods.apply(23,0.0);
        let m23=if let Ok(x)=crate::envc::var("JPRACE_M23_ULP") && let Some((a,u))=x.split_once(':') && a.parse::<usize>().ok()==Some(hi) && m23!=0.0 {f32::from_bits((m23.to_bits() as i64+u.parse::<i64>().unwrap_or(0)) as u32)} else {m23};
        h.speed=if crate::envc::var("JPRACE_BLOCK_CAP").as_deref()==Ok("self") {(m23+h.mods.apply(5,0.0)+v.min(max)).max(h.min_speed)}
            else if spd_seq() {h.mods.apply(5,h.mods.apply(23,v)).clamp(h.min_speed,max)}
            else {(m23+h.mods.apply(5,0.0)+v).clamp(h.min_speed,max)};
        h.sens=if sens_on() && (v==target || (h.fs_cap>0.0 && v==h.fs_cap+h.fs_r*dt)) && v>0.0 {[-1i32,1].map(|k|(m23+h.mods.apply(5,0.0)+f32::from_bits((v.to_bits() as i32+k) as u32)).clamp(h.min_speed,max))} else {[0.0;2]};
        if crate::envc::var("JPRACE_LOG_SPD").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
            let (ph,st,ex)=(h.phase,h.input.running_style,h.ex());
            let c=self.wiz_limit_break_coef(ph,st,ex);let h=&self.horses[hi];
            let m:Vec<String>=h.mods.entries.iter().filter(|m|(m.index==23 || m.index==5) && !m.once).map(|m|format!("{}:{:08x}:{}",m.index,m.value.to_bits(),m.lb.map_or("-".into(),|r|format!("{:08x}/{:08x}/{:08x}",r[0].to_bits(),r[1].to_bits(),r[2].to_bits())))).collect();
            eprintln!("spd tick={} v={:08x} speed={:08x} min={:08x} max={:08x} phase={} lbc={:08x} mods={}",(self.time/0.0666).round() as i64,v.to_bits(),h.speed.to_bits(),h.min_speed.to_bits(),max.to_bits(),ph,c.to_bits(),m.join(","));
        }
        let h=&mut self.horses[hi];
        if let Ok(x)=crate::envc::var("JPRACE_SPEED_ADJ") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<&str>=e.split(':').collect();
                if f.len()==4 && f[0].parse::<usize>().ok()==Some(hi) && f[1].parse::<i64>().is_ok_and(|a|tick>=a) && f[2].parse::<i64>().is_ok_and(|b|tick<=b) {h.speed+=f[3].parse::<f32>().unwrap_or(0.0);}
            }
        }
        if let Ok(x)=crate::envc::var("JPRACE_SPEED_ULP") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();
                if f.len()==4 && f[0] as usize==hi && tick>=f[1] && tick<=f[2] {h.speed=f32::from_bits((h.speed.to_bits() as i64+f[3]) as u32);}
            }
        }
        let mut movement=h.speed*dt;
        if h.delay<0.0 {
            movement=crate::math::gate_movement(movement,h.delay,dt);
            h.delay=0.0;
        }
        let mut drain_rate=h.mods.apply(13,0.0);
        if drain_rate<=0.0 && let Some(r)=ce.hp_base_rate {drain_rate=r;}
        if drain_rate<=0.0 {drain_rate=p.f(if h.tempted() {"Hp.HpDecRateBaseTemptation"}
            else if h.keep.mode==4 {"Hp.HpDecRateBasePositionKeepPaseDown"} else {"Hp.HpDecRateBaseNormal"});}
        if h.downhill {drain_rate*=p.f("Hp.HpDecRateMultiplyDownSlopeAccelMode");}
        let drain_base=if h.start_dash {v} else {race_base};
        let vd=if crate::envc::var("JPRACE_HP_SPD").as_deref()==Ok("speed") {self.horses[hi].speed} else {v};
        let drain=self.hp_rate(&self.horses[hi],vd,drain_base,drain_rate);
        let mut one_time=0.0;
        for &(oh,kind,st,_) in &self.oracle {
            let cs=crate::envc::var("JPRACE_CONSUME_SHIFT").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(-1.0)*dt;
            if oh==hi && (self.time-(st-cs)).abs()<1e-3 {
                if kind==8 {
                    let (far,_,_)=self.jp_cbs_state(hi);
                    let add=Tree(self.p().get("CompeteBeforeSpurt")).f("CompeteBeforeSpurtConsumeStaminaValueRateAddend");
                    let mode=crate::envc::var("JPRACE_CBS_RATE").unwrap_or_else(|_|"n".into());
                    let r=if let Some(x)=self.horses[hi].jp_cbs_preset.take() {x}
                        else if mode.contains('a') {1.0+add} else if mode.contains('n') || mode.contains('h') {if far {1.0} else {1.0+add}} else {(if far {1.0} else {0.5})+add};
                    self.horses[hi].jp_cbs_near=!far;
                    self.horses[hi].jp_cbs_rate=r;
                    if crate::envc::var_os("JPRACE_LOG_CBSRATE").is_some() {
                        let top=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);let (_,n,ns)=self.jp_cbs_state(hi);
                        eprintln!("cbsrate h={} t={:.3} tick={} far={} rate={} gap={:.3} style={} n={} ns={}",hi,self.time,(self.time/dt).round() as i64,far,r,top-self.horses[hi].distance,self.horses[hi].input.running_style,n,ns);}
                }
                one_time+=self.jp_mode_consume(hi,kind);
            }
        }
        if crate::envc::var("JPRACE_DEBUG_KEEP").is_ok() {let h=&self.horses[hi];if h.keep.mode!=h.keep_prev_dbg {eprintln!("keep {:.3} {} {}->{} pm={}",self.time,hi,h.keep_prev_dbg,h.keep.mode,self.pacemaker);}}
        if crate::envc::var("JPRACE_DEBUG_KEEP").is_ok() {let m=self.horses[hi].keep.mode;self.horses[hi].keep_prev_dbg=m;}
        if crate::envc::var("JPRACE_DEBUG_HP").is_ok() && self.time>1.0 && self.time<1.1 {let h=&self.horses[hi];eprintln!("hp {} t={:.3} keep={} mods13={} ce={:?} tempt={} rate={} drain={} v={} base={}",hi,self.time,h.keep.mode,h.mods.apply(13,0.0),ce.hp_base_rate,h.tempted(),drain_rate,drain,v,drain_base);}
        let h=&mut self.horses[hi];
        if !h.finished && h.hp>0.0 {h.hp=(h.hp-drain*dt-one_time).clamp(0.0,h.max_hp);}
        if !h.finished && h.hp<=0.0 && !h.hp_empty && crate::envc::var("JPRACE_HP0_LANE").as_deref()==Ok("edge") {h.target_lane=h.lane;}
        if !h.finished && h.hp<=0.0 {h.hp_empty=true;}
        h.hp_prev0=h.hp_t0 && !h.finished;
        if h.hp_prev0 && h.slb_on && crate::envc::var("JPRACE_SLB_HP0").as_deref()!=Ok("0") {h.slb_on=false;h.slb_pending=false;h.slb_re=true;}
        h.phase=self.course.phase(h.distance);
        let mut position=self.geometry.world(h.distance,h.lane);
        if let Ok(x)=crate::envc::var("JPRACE_POS_ULP") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();
                if f.len()==4 && f[0] as usize==hi && f[1]==tick && (f[2]==0 || f[2]==2) {let a=f[2] as usize;position[a]=f32::from_bits((position[a].to_bits() as i64+f[3]) as u32);}
            }
        }
        let dx=position[0]-h.position[0];let dz=position[2]-h.position[2];
        let cv=crate::envc::var("JPRACE_CAP_F32").unwrap_or_default();
        let p64=self.geometry.world64(h.distance,h.lane);
        let move64=if crate::envc::var_os("JPRACE_MOVE64").is_some() {
            let (a,b)=(self.geometry.world64(0.0,0.0),self.geometry.world64(1.0,0.0));
            let dtw=((a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)+(a[2]-b[2]).powi(2)).sqrt();
            let disp=if crate::envc::var_os("JPRACE_MOVE_AHEAD").is_some() {
                let q=self.geometry.world64(h.distance+movement,h.lane);
                let y=crate::envc::var_os("JPRACE_MOVE_3D").map_or(0.0,|_|(q[1]-p64[1]).powi(2));
                ((q[0]-p64[0]).powi(2)+y+(q[2]-p64[2]).powi(2)).sqrt()
            } else {((p64[0]-h.pos64[0]).powi(2)+(p64[2]-h.pos64[2]).powi(2)).sqrt()};
            let cap=movement as f64*dtw;
            Some(if cap<disp {(movement as f64*cap/disp) as f32} else {movement})
        } else {None};
        let d64=crate::envc::var("JPRACE_DISP64").ok();
        let displacement=if crate::envc::var_os("JPRACE_POS64").is_some() {let (a,b)=(p64[0]-h.pos64[0],p64[2]-h.pos64[2]);(a*a+b*b).sqrt() as f32}
            else if d64.as_deref()==Some("1") {((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32}
            else if d64.as_deref()==Some("2") {(((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)) as f32).sqrt()}
            else if d64.as_deref()==Some("3") {dx.hypot(dz)}
            else if d64.as_deref()==Some("3d") {let dy=position[1]-h.position[1];(dx*dx+dy*dy+dz*dz).sqrt()}
            else if cv.contains('s') || !crate::geometry::geo_old() {(dx*dx+dz*dz).sqrt()} else {((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32};
        h.pos64=p64;
        let cap=if crate::envc::var_os("JPRACE_DTW64").is_some() {
            let (a,b)=(self.geometry.world64(0.0,0.0),self.geometry.world64(1.0,0.0));
            let dtw=((a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)+(a[2]-b[2]).powi(2)).sqrt();
            let c=movement as f64*dtw;
            if c<displacement as f64 && crate::envc::var_os("JPRACE_POS64").is_some() {
                let (pa,pb)=(p64[0]-h.pos64[0],p64[2]-h.pos64[2]);
                let _ = (pa,pb);
            }
            c as f32
        } else {movement*self.geometry.distance_to_world};
        let rawat={static S:std::sync::OnceLock<std::collections::HashSet<(usize,i64)>>=std::sync::OnceLock::new();
            S.get_or_init(||crate::envc::var("JPRACE_LOG_RAWAT").map(|x|x.split(',').filter_map(|e|{let (a,b)=e.split_once(':')?;Some((a.parse().ok()?,b.parse().ok()?))}).collect()).unwrap_or_default())};
        if !rawat.is_empty() && rawat.contains(&(hi,(self.time/0.0666).round() as i64)) {
            eprintln!("raw time={:e} d={:e} lane={:e} mv={:e} speed={:e} h={} {} || {} || {}",self.time,h.distance,h.lane,movement,h.speed,hi,self.geometry.raw(h.distance),self.geometry.raw(0.0),self.geometry.raw(1.0));}
        if let Ok(v)=crate::envc::var("JPRACE_DEBUG_RAW") && (v=="all" || v.parse::<usize>().ok()==Some(hi))
            && self.time<crate::envc::var("JPRACE_DEBUG_RAW_T").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(1.5) {
            eprintln!("raw time={:e} d={:e} lane={:e} mv={:e} speed={:e} h={} {} || {} || {}",self.time,h.distance,h.lane,movement,h.speed,hi,self.geometry.raw(h.distance),self.geometry.raw(0.0),self.geometry.raw(1.0));}
        if crate::envc::var("JPRACE_DEBUG_MOVE").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) && self.time<1.2 {
            eprintln!("move t={:.4} speed={:.9} mv={:.9} cap={:.9} disp={:.9} pos=({:.6},{:.6}) prev=({:.6},{:.6}) d={:.6} lane={:.5} dtw={}",self.time,h.speed,movement,cap,displacement,position[0],position[2],h.position[0],h.position[2],h.distance,h.lane,self.geometry.distance_to_world);}
        let nocap=crate::envc::var("JPRACE_NOCAP").is_ok_and(|x|{let tick=(self.time/0.0666).round() as i64;x.split(',').any(|e|{let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();f.len()==2 && f[0] as usize==hi && f[1]==tick})});
        let cap=match crate::envc::var("JPRACE_CAP_PREV").as_deref() {
            Ok("raw") if h.prev_mv[0]>0.0=>h.prev_mv[0]*self.geometry.distance_to_world,
            Ok("act") if h.prev_mv[1]>0.0=>h.prev_mv[1]*self.geometry.distance_to_world,
            _=>cap};
        let (mv_pre,capped_now)=(movement,cap<displacement && !nocap);
        if cap<displacement && !nocap {
            movement=if cv.contains('a') {movement*cap/displacement} else if cv.contains('b') {movement*(cap/displacement)}
                else if cv.contains('c') {cap/displacement*movement} else if cv.contains('d') {cap/self.geometry.distance_to_world*(cap/displacement)/(cap/self.geometry.distance_to_world)*movement}
                else if !crate::geometry::geo_old() {movement*(cap/displacement)}
                else {((movement as f64)*(cap as f64)/(displacement as f64)) as f32};
        }
        if let Some(m)=move64 {movement=m;}
        if crate::envc::var_os("JPRACE_LOG_CAPS").is_some() && !h.finished {
            eprintln!("cp {} {} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",(self.time/0.0666).round() as i64,hi,h.distance.to_bits(),h.lane.to_bits(),(h.speed*dt).to_bits(),cap.to_bits(),displacement.to_bits(),movement.to_bits());
        }
        if let Ok(x)=crate::envc::var("JPRACE_LOG_MOVE") {
            let f:Vec<f32>=x.split(':').filter_map(|v|v.parse().ok()).collect();
            let tick=(self.time/0.0666).round();
            if f.len()==3 && f[0] as usize==hi && tick>=f[1] && tick<=f[2] {
                eprintln!("mv tick={} d={:08x} lane={:08x} prevlane={:08x} speed={:08x} mv0={:08x} cap={:08x} disp={:08x} mv={:08x} capped={} pos=({:08x},{:08x}) prev=({:08x},{:08x})",tick,h.distance.to_bits(),h.lane.to_bits(),h.prev_lane.to_bits(),h.speed.to_bits(),(h.speed*dt).to_bits(),cap.to_bits(),displacement.to_bits(),movement.to_bits(),cap<displacement,position[0].to_bits(),position[2].to_bits(),h.position[0].to_bits(),h.position[2].to_bits());
            }
        }
        let fm=crate::envc::var("JPRACE_DIST_FMA").unwrap_or_default();
        let sens_all=crate::envc::var("JPRACE_LOG_SENS").as_deref()==Ok("all");
        let pinned=h.sens[0]>0.0;
        if sens_all && !pinned && !h.finished {h.sens=[-1i32,1].map(|k|f32::from_bits((h.speed.to_bits() as i32+k) as u32));}
        if sens_on() && h.sens[0]>0.0 && move64.is_none() && cv.is_empty() && crate::geometry::geo_old()==false && fm.is_empty() && !nocap {
            let nx=|sp:f32|{let mut m=sp*dt;let c=m*self.geometry.distance_to_world;if c<displacement {m=m*(c/displacement);}h.distance+m};
            let base=nx(h.speed);
            let (a,b)=(nx(h.sens[0])!=base,nx(h.sens[1])!=base);
            if base==h.distance+movement && (a||b) {eprintln!("sens tick={} h={} m={} p={}{} cap={}",(self.time/0.0666).round() as i64,hi,a as u8,b as u8,if pinned {""} else {" free=1"},u8::from(capped_now));}
        }
        let next=if move64.is_none() && !capped_now && fm.contains('u') && mv_pre==h.speed*dt {h.speed.mul_add(dt,h.distance)}
            else if move64.is_none() && capped_now && fm.contains('c') {mv_pre.mul_add(cap/displacement,h.distance)}
            else {h.distance+movement};
        h.prev_mv=[mv_pre,movement];
        h.position=position;h.prev_distance=h.distance;h.distance=next;
        if let Ok(x)=crate::envc::var("JPRACE_DIST_ULP") {
            let tick=(self.time/0.0666).round() as i64;
            for e in x.split(',') {
                let f:Vec<i64>=e.split(':').filter_map(|v|v.parse().ok()).collect();
                if f.len()==3 && f[0] as usize==hi && f[1]==tick {h.distance=f32::from_bits((h.distance.to_bits() as i64+f[2]) as u32);}
            }
        }
        if h.rafs_on && h.rafs_t0==self.time && crate::envc::var("JPRACE_RAFSWIN_LO").as_deref()!=Ok("act") && crate::envc::var("JPRACE_RAFSWIN_DRAW").as_deref()==Ok("act") {
            let lo=h.distance;let hi=self.course.distance;
            for sk in &mut h.skills {for det in &mut sk.details {
                crate::triggers::finish_run_at_full_speed_windows(&mut det.pre,lo,hi);
                crate::triggers::finish_run_at_full_speed_windows(&mut det.condition,lo,hi);
            }}
        }
        let h=&mut self.horses[hi];
        if !h.spurt_ev && h.spurt_calculated && h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start && crate::envc::var_os("JPRACE_E13_OLD").is_none() {
            h.spurt_ev=true;let (t,d)=(self.time,h.distance);
            self.events.push(json!({"t":t,"type":13,"name":"LastSpurtStart","params":[hi,0],"horse":hi,"extra":format!("{:.3}",d)}));
        }
        let h=&mut self.horses[hi];
        if crate::envc::var("JPRACE_DEBUG_SEC").is_ok() {let sec=self.course.distance/24.0;for k in [10.0f32,15.0] {if h.prev_distance<k*sec && h.distance>=k*sec {eprintln!("sec {} {} t={:.4} calls={}",k as i32+1,hi,self.time,self.random.calls());}}}
        if h.speed>=default_min {h.start_dash=false;}
        if h.slope!=0.0 && (if crate::envc::var("JPRACE_SLOPE_END").as_deref()==Ok("prev") {h.prev_distance} else {h.distance})>h.slope_end {
            if h.slope<0.0 {h.slope_clear_tick=(self.time/0.0666).round() as i64;}
            h.slope=0.0;}
        if !h.finished && h.distance>self.course.distance {
            h.finished=true;
            let fraction=((h.distance-self.course.distance)/(h.distance-h.prev_distance)).clamp(0.0,1.0);
            h.raw_time=if f32m('f') {self.time-fraction*dt} else {(self.time as f64-fraction as f64*dt as f64) as f32};h.scaled_time=h.raw_time*p.f("Global.FinishTimeCoef");
        }
    }
    fn course_events(&mut self) {
        for (ei,e) in self.course.events.iter().enumerate() {
            for h in &mut self.horses {
                if ei as i32<=h.called_event || h.distance<e.distance {continue;}
                h.called_event=ei as i32;
                match (e.kind,e.values.as_slice()) {
                    (0,[number,..])=>{
                        h.corner=*number;h.straight=false;
                        {
                            let seg=self.course.corners.iter().find(|s|s.number==*number && s.start==e.distance);
                            h.corner_end=seg.map_or(e.distance+200.0,|s|s.end);
                            h.final_corner=self.course.final_corner().is_some_and(|f|f.start==e.distance);
                        }
                    },
                    (2,[1,front,rest @ ..])=>{h.straight=true;h.front_type=*front;h.corner=0;if rest.first()==Some(&1) {h.in_last_straight=true;}},
                    (2,[1])=>(),
                    (2,_)=>{h.straight=false;h.front_type=0;h.in_last_straight=false;},
                    (11,[per,_])=>{
                        let sign=per.signum();if let Some(s)=self.course.slopes.iter().find(|s|s.number==sign && s.start<=h.distance && h.distance<=s.end) {h.slope=*per as f32/10000.0;h.slope_end=s.end;}
                    },
                    (3,[max,..]) if self.input.race_type!=3=>h.lane_max=*max as f32/10000.0,
                    _=>(),
                }
            }
        }
    }
    fn tick(&mut self,dt:f32) {
        let prof=crate::envc::var_os("JPRACE_PROFILE").is_some();let t0=std::time::Instant::now();
        for hi in 0..self.horses.len() {
            self.around(hi);let h=&mut self.horses[hi];
            if h.finished {h.after_goal+=dt;}
            if h.distance>h.corner_end {h.corner=0;}
        }
        crate::rng::CUR_TICK.store((self.time/dt).round() as i64,std::sync::atomic::Ordering::Relaxed);
        if dt>0.0 {
            let style=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
            let sec=if crate::envc::var("JPRACE_PM_SECINT").as_deref()==Ok("1") {(self.course.distance as i32/24) as f32} else {self.course.distance/24.0};
            let probe=self.horses.iter().map(|h|h.distance).fold(f32::MIN,f32::max);
            let s=(probe/sec) as i32;
            let pm_old=self.pacemaker;
            let delay=crate::envc::var("JPRACE_PM_DELAY").ok().and_then(|x|x.parse::<i32>().ok()).unwrap_or(0);
            let mut run_s=None;
            if s>self.pm_section {self.pm_section=s;self.pm_follow=false;if delay>0 {self.pm_wait=Some((s,delay));} else {run_s=Some(s);}}
            else if let Some((ws,k))=self.pm_wait {if k<=1 {self.pm_wait=None;run_s=Some(ws);} else {self.pm_wait=Some((ws,k-1));}}
            if let Some(s)=run_s {
                if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {
                    eprintln!("pmord pm={} pmd={:.4} {}",self.pacemaker,self.horses[self.pacemaker].distance,self.horses.iter().enumerate().map(|(i,h)|format!("{}:{:.3}/o{}/p{}",i,h.distance,h.order,h.prev_order)).collect::<Vec<_>>().join(" "));
                    eprintln!("pmparams fwd_range={} thr={} mfr={}",self.p().f("PositionKeep.PaseMakerChangeForwardRunningStyleRange"),self.p().f("PositionKeep.PaseMakerChangeTopHorseNotMostForwardStyleCount"),self.p().f("PositionKeep.PaseMakerChangeMostForwardStyleRange"));
                    let mut o:Vec<usize>=(0..self.horses.len()).collect();o.sort_by_key(|&i|self.horses[i].order);
                    eprintln!("pmsec t={:.3} s={} pm={} cnt={} handed={} top={:?} pmst={} pmd={:.4} tick={}",self.time,s,self.pacemaker,self.pm_not_forward,self.pm_handed,o.iter().take(3).map(|&i|(i,self.horses[i].input.running_style,self.horses[i].distance)).collect::<Vec<_>>(),style(&self.horses[self.pacemaker]),self.horses[self.pacemaker].distance,(self.time/dt).round() as i64);}
                let best=self.horses.iter().filter(|h|!h.finished).map(style).min().unwrap_or(1);
                let pmprev=crate::envc::var("JPRACE_PM_PREVORD").as_deref()!=Ok("0");
                let lead_i=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|if pmprev {h.prev_order} else {h.order}).map(|(i,_)|i);
                let keep=lead_i.is_some_and(|i|style(&self.horses[i])!=best);
                let ended=s>=self.p().f("PositionKeep.PositionKeepEndSection") as i32;
                if !keep && !ended {if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|style(h)==best).min_by_key(|(_,h)|h.prev_order) {self.pacemaker=i;}}
                let end_skip=ended && crate::envc::var("JPRACE_PM_END").as_deref()!=Ok("old");
                let forward_gap=self.horses.iter().filter(|h|!h.finished).map(|h|h.distance).fold(f32::MIN,f32::max)-self.horses[self.pacemaker].distance;
                if !end_skip && forward_gap>self.p().f("PositionKeep.PaseMakerChangeForwardRunningStyleRange") {if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|h.order) {self.pacemaker=i;}}
                let runners=self.horses.iter().map(|h|crate::pacemaker::Runner {
                    style:style(h),order:if pmprev {h.prev_order} else {h.order},distance:h.distance,finished:h.finished,
                }).collect::<Vec<_>>();
                let fwd=runners.iter().filter(|r|!r.finished).map(|r|r.style).min().unwrap_or(1);
                let fwd_top=runners.iter().enumerate().filter(|(_,r)|!r.finished && r.style==fwd).min_by_key(|(i,r)|(r.order,*i)).map(|(i,_)|i);
                let lead_st=runners.iter().enumerate().filter(|(_,r)|!r.finished).min_by_key(|(i,r)|(r.order,*i)).map(|(i,_)|runners[i].style);
                let fwdnew=!end_skip && self.pm_handed && crate::envc::var("JPRACE_PM_FWDNEW").as_deref()!=Ok("0") && lead_st!=Some(fwd) && fwd_top.is_some_and(|t|runners[t].distance>runners[self.pacemaker].distance) && self.pm_not_forward+1<2;
                if fwdnew {let i=fwd_top.unwrap();if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmfwdnew t={:.3} s={} -> {}",self.time,s,i);}
                    self.pacemaker=i;self.pm_not_forward+=1;}
                let cnt_before=self.pm_not_forward;
                let mut samepick_now=false;
                let sk=crate::envc::var("JPRACE_PM_SAMEKEEP").unwrap_or_default();
                let lead_now=runners.iter().enumerate().filter(|(_,r)|!r.finished).min_by_key(|(i,r)|(r.order,*i)).map(|(i,_)|i);
                let sk_hit=!end_skip && !fwdnew && self.pm_samepick && self.pm_handed && lead_now.is_some_and(|l|l!=self.pacemaker && runners[l].style!=fwd && (sk=="3" || sk=="2" && runners[l].style==runners[self.pacemaker].style));
                if sk_hit {let l=lead_now.unwrap();if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmsamepick t={:.3} s={} -> {}",self.time,s,l);}self.pacemaker=l;self.pm_not_forward=0;}
                if !end_skip && !fwdnew && !sk_hit && let Some(i)=crate::pacemaker::recalculate_with(&runners,&mut self.pm_not_forward,
                    crate::envc::var("JPRACE_PM_THR").ok().and_then(|x|x.parse().ok()).unwrap_or(self.assets.params().f("PositionKeep.PaseMakerChangeTopHorseNotMostForwardStyleCount") as i32),
                    self.assets.params().f("PositionKeep.PaseMakerChangeForwardRunningStyleRange"),crate::pacemaker::Rules {
                    early_forward:self.pm_handed,reset_on_forward:false,reset_on_recalc:crate::envc::var_os("JPRACE_PM_NORESET").is_none(),
                    most_forward_range:if crate::envc::var("JPRACE_PM_MFR").as_deref()!=Ok("0") {crate::envc::var("JPRACE_PM_MFR_RANGE").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(self.assets.params().f("PositionKeep.PaseMakerChangeMostForwardStyleRange"))} else {0.0},order_first:true,forward_nige_only:true,pm_style:style(&self.horses[self.pacemaker]),hand_elapsed:if self.pm_handed && self.pm_hand_tick>=0 {(self.time/dt).round() as i64-self.pm_hand_tick} else {-1}}) {
                    if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmchange t={:.3} s={} -> {}",self.time,s,i);}
                    let prev_pm=self.pacemaker;
                    self.pacemaker=i;
                    if runners[i].style!=runners.iter().filter(|r|!r.finished).map(|r|r.style).min().unwrap_or(1) {
                        if !self.pm_handed {self.pm_fwd_from=Some(prev_pm);self.pm_hand_tick=(self.time/dt).round() as i64;self.pm_hand1_tick=self.pm_hand_tick;}
                        if crate::envc::var("JPRACE_PM_SAMEKEEP").as_deref()==Ok("1") && i==prev_pm && self.pm_handed {self.pm_not_forward=cnt_before+1;}
                        else if crate::envc::var("JPRACE_PM_MFR_KEEPCNT").as_deref()!=Ok("0") && self.pm_not_forward==cnt_before {}
                        else {self.pm_not_forward=0;}
                        samepick_now=i==prev_pm && self.pm_handed;
                        self.pm_handed=true;}
                }
                self.pm_samepick=samepick_now;
            }
            if self.pacemaker!=pm_old && run_s.is_some() {self.pm_sec_tick=(self.time/dt).round() as i64;}
            if run_s.is_some() && crate::envc::var("JPRACE_PM_FOLLOW_SEC").as_deref()==Ok("1") {self.pm_fchg_tick=(self.time/dt).round() as i64;self.pm_fchg=self.pacemaker;}
            if self.pacemaker!=pm_old && crate::envc::var_os("JPRACE_PM_LATE").is_some() {self.pm_pending=Some(self.pacemaker);self.pacemaker=pm_old;}
            if self.pacemaker!=pm_old && let Ok(m)=crate::envc::var("JPRACE_PM_PKRESET") {
                for (i,h) in self.horses.iter_mut().enumerate() {
                    if m=="all" || (m=="old" && i==pm_old) || (m=="new" && i==self.pacemaker) {h.keep.interval=0.0;}
                    if (m=="old1" && i==pm_old) || m=="all1" {h.keep.force_once=true;}
                }
            }
        }
        let t1=std::time::Instant::now();
        if dt>0.0 && crate::envc::var("JPRACE_LB_PRE").as_deref()!=Ok("0") {
            for hi in 0..self.horses.len() {
                if self.horses[hi].finished || !self.horses[hi].mods.entries.iter().any(|m|m.index==23 && m.lb.is_some()) {continue;}
                let ph=self.horses[hi].phase;
                let coefs:Vec<f32>=self.horses[hi].mods.entries.iter().map(|m|if m.index==23 {m.lb.map_or(0.0,|r|self.wiz_limit_break_coef(ph,r[3] as i32,r[4]>0.5))} else {0.0}).collect();
                let fs=if self.horses[hi].rafs_on {crate::envc::var("JPRACE_FS_SKILL").ok().and_then(|x|x.parse::<f32>().ok()).unwrap_or(0.0)} else {0.0};
                for (m,c) in self.horses[hi].mods.entries.iter_mut().zip(coefs) {if m.index==23 && let Some(r)=m.lb {m.value=lb_value(r,fs,c);}}
            }
        }
        self.skills_pass(dt,1);self.skills_pass(dt,0);
        if dt>0.0 && crate::envc::var("JPRACE_SPURT_PREPASS").as_deref()==Ok("1") {
            for hi in 0..self.horses.len() {
                if !self.horses[hi].finished && self.horses[hi].phase>=2 && !self.horses[hi].spurt_calculated {self.spurt(hi);}
            }
        }
        let t2=std::time::Instant::now();
        let draw_after_all=crate::envc::var("JPRACE_DRAW_ORDER").as_deref()==Ok("after");
        if dt>0.0 && self.pm_handed && crate::envc::var("JPRACE_PM_FWDTICK").as_deref()!=Ok("0") {
            let st=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
            let best=self.horses.iter().filter(|h|!h.finished).map(st).min().unwrap_or(1);
            if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished && st(h)==best).min_by_key(|(i,h)|(h.prev_order,*i)) {
                if i!=self.pacemaker && self.horses[i].prev_order<self.horses[self.pacemaker].prev_order && st(&self.horses[self.pacemaker])!=best {
                    if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmfwdtick t={:.3} {} -> {}",self.time,self.pacemaker,i);}
                    self.pacemaker=i;
                    match crate::envc::var("JPRACE_PM_FWDRESET").as_deref() {Ok("both")=>{self.pm_handed=false;self.pm_not_forward=0;},Ok("none")=>(),Ok("hand")=>self.pm_handed=false,_=>self.pm_not_forward=0}
                }
            }
        }
        if dt>0.0 && let Ok(x)=crate::envc::var("JPRACE_PM_SET") && let Some((a,b))=x.split_once(':') && a.parse::<i64>().ok()==Some((self.time/dt).round() as i64) && let Ok(v)=b.parse::<usize>() {self.pacemaker=v;}
        for hi in 0..self.horses.len() {
            self.update_horse(hi,dt);
            if hi+1==self.horses.len() && let Some(pm)=self.pm_pending.take() {self.pacemaker=pm;}
            if dt>0.0 && self.horses[hi].jp_keep_thr<=0.0 && crate::envc::var("JPRACE_ENTRY_PERHORSE").as_deref()==Ok("1")
                && self.horses[hi].distance>=10.0*self.course.distance/24.0 && !self.horses[hi].finished {
                let c=Tree(self.p().get("CompeteBeforeSpurt"));let (lo,up)=(c.f("CompeteBeforeSpurtStaminaKeepRandomRangeMin"),c.f("CompeteBeforeSpurtStaminaKeepRandomRangeMax"));
                let call=self.random.calls()+1;
                self.horses[hi].jp_keep_thr=self.random.float_range(lo,up);
                if crate::envc::var_os("JPRACE_LOG_ENTRYDRAWS").is_some() {eprintln!("entryhorse h={} tick={} call={} thr={:.5}",hi,(self.time/dt).round() as i64,call,self.horses[hi].jp_keep_thr);}
            }
            if !draw_after_all {self.jp_draw_after_move(hi,dt);}
            if dt>0.0 {
                let o=self.horses[hi].order;
                let vis_only=crate::envc::var("JPRACE_OTT_VIS").as_deref()==Ok("1");
                let lane_lim=crate::envc::var("JPRACE_OTT_LANE").ok().and_then(|x|x.parse::<f32>().ok());
                let me_lane=self.horses[hi].lane;
                let hit=self.horses.iter().any(|n|o<n.order && n.overtake.contains(&hi) && (!vis_only || n.visible.contains(&hi) || n.front==Some(hi))
                    && lane_lim.is_none_or(|l|(n.lane-me_lane).abs()<=l));
                if crate::envc::var("JPRACE_LOG_OTT").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {
                    let me=&self.horses[hi];
                    let who:Vec<String>=self.horses.iter().enumerate().filter(|(_,n)|n.overtake.contains(&hi)).map(|(j,n)|format!("{}(o{},dd{:.2},dl{:.4},f{})",j,n.order,me.distance-n.distance,n.lane-me.lane,i32::from(n.front==Some(hi)))).collect();
                    eprintln!("ott tick={} h={} order={} hit={} t={:.4} by=[{}]",(self.time/dt).round() as i64,hi,o,hit,self.horses[hi].ot_target_time,who.join(","));
                }
                let h=&mut self.horses[hi];h.ot_target_time=if hit {h.ot_target_time+dt} else {0.0};
                if h.overtake.is_empty() || h.order<h.ot_noup_order {h.ot_noup_time=0.0;} else {h.ot_noup_time+=dt;}
                h.ot_noup_order=h.order;
            }
        }
        let t3=std::time::Instant::now();
        if prof {PROF.with(|p|{let mut p=p.borrow_mut();p[0]+=(t1-t0).as_secs_f64();p[1]+=(t2-t1).as_secs_f64();p[2]+=(t3-t2).as_secs_f64();});}
        if draw_after_all && crate::envc::var("JPRACE_ENTRY_MODE").as_deref()==Ok("precheck") && !self.jp_entry_drawn {
            if let Some(k)=crate::envc::var("JPRACE_ENTRY_DRAWS").ok().and_then(|x|x.parse::<i64>().ok()) {
                let sec=self.course.distance/24.0;
                if self.horses.iter().any(|h|h.distance>=10.0*sec) {self.jp_entry_drawn=true;for _ in 0..k {self.random.float_max(1.0);}}
            }
        }
        if (crate::envc::var_os("JPRACE_LATE").is_some() || crate::envc::var_os("JPRACE_LATE_B").is_some()) && crate::envc::var_os("JPRACE_ORACLE").is_none() && dt>0.0 {
            if !self.jp_entry_drawn && self.horses.iter().any(|h|h.distance>=10.0*self.course.distance/24.0) {
                self.jp_entry_drawn=true;
                for _ in 0..crate::envc::var("JPRACE_LATE_B").ok().and_then(|x|x.parse::<i64>().ok()).unwrap_or(0) {self.random.float_max(1.0);}
            }
            if crate::envc::var_os("JPRACE_LATE").is_some() {for hi in 0..self.horses.len() {self.jp_late_native(hi,dt);}}
        }
        if draw_after_all {
            let mut idx:Vec<usize>=(0..self.horses.len()).collect();
            match crate::envc::var("JPRACE_DRAW_HORSE_ORDER").as_deref() {
                Ok("order")=>idx.sort_by_key(|&i|self.horses[i].order),
                Ok("rorder")=>idx.sort_by_key(|&i|std::cmp::Reverse(self.horses[i].order)),
                Ok("rev")=>idx.reverse(),
                _=>(),
            }
            for hi in idx {self.jp_draw_after_move(hi,dt);}
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_LOG_KEEPEND").is_some() {
            let tick=(self.time/dt).round() as i64;
            let ends:Vec<(usize,f32)>=self.oracle.iter().filter(|e|e.1==9 && e.3<1e8 && ((e.3/dt).round() as i64)==tick).map(|e|(e.0,e.2)).collect();
            for (hi,st) in ends {
                let need=self.jp_need_hp_mode(hi,"g");let h=&self.horses[hi];
                eprintln!("keepend h={} tick={} start={} hp={:.1} needG={:.1} ratio={:.4} thr={:.5} d={:.2} heal={} healkeep={} phase={}",hi,tick,(st/dt).round() as i64,h.hp,need,h.hp/need,h.jp_keep_thr,h.distance,h.heal_count,h.keep_heal,h.phase);
            }
        }
        if dt>0.0 {
            let tick=(self.time/dt).round() as i64;
            let evs:Vec<(usize,i64,f32)>=self.oracle.iter().filter(|e|(e.1==8||e.1==10) && ((e.2/dt).round() as i64)==tick).map(|e|(e.0,e.1,e.2)).collect();
            for (hi,kind,st) in evs {
                if self.minor_ev.iter().any(|e|e.0==hi && e.1==kind && e.2==st) {continue;}
                let mult=self.jp_minor_mult(hi,kind,tick);
                self.minor_ev.push((hi,kind,st,mult));
            }
        }
        if let Ok(r)=crate::envc::var("JPRACE_DRAW_AT") && dt>0.0 {
            let tick=(self.time/dt).round() as i64;
            for e in r.split(',') {if let Some((a,b))=e.split_once(':') && a.parse::<i64>().ok()==Some(tick) {for _ in 0..b.parse::<i64>().unwrap_or(1) {self.random.float_max(1.0);}}}
        }
        if let Some(x)=crate::envc::var("JPRACE_LOG_SLM").ok().and_then(|x|x.parse::<usize>().ok()) && dt>0.0 && x<self.horses.len() {
            let tm=self.time;let act=|k:i64|self.oracle.iter().any(|&(oh,kd,st,en)|oh==x && kd==k && tm>=st-1e-3 && tm<en-1e-3);
            eprintln!("slm tick={} h={} d={:.3} margin={:.3} behind={} sl={} cbs={} sec={:.2}",(self.time/dt).round() as i64,x,self.horses[x].distance,self.jp_sl_margin(x),self.jp_sl_cond2(x,true),act(10),act(8),self.horses[x].distance/(self.course.distance/24.0));
        }
        if crate::envc::var_os("JPRACE_LOG_RAFSHP").is_some() && dt>0.0 {
            let tick=(self.time/dt).round() as i64;
            for (hi,h) in self.horses.iter().enumerate() {
                if h.finished || h.phase<2 || !(h.spurt_start>0.0 && h.distance>=h.spurt_start) {continue;}
                eprintln!("rafshp tick={} h={} rafs={} hp={:.3} need={:.3} spr={} spv={:.4} tgt={:.4} v={:.4} fsx={:.4} slb={} slbv={:.4} m23={:.4}",tick,hi,h.rafs_on,h.hp,self.jp_need_hp_mode(hi,"all"),h.spurt_result,h.spurt_speed,h.target_speed,h.speed,h.fs_x,h.slb_on,h.slb_v,h.mods.apply(23,0.0));
            }
        }
        if let Ok(r)=crate::envc::var("JPRACE_LOG_STATE") && dt>0.0 {
            let mut it=r.split(':').map(|x|x.parse::<f32>().unwrap_or(0.0));let (t0,t1)=(it.next().unwrap_or(0.0),it.next().unwrap_or(1e9));
            if self.time>=t0 && self.time<=t1 {
                let tick=(self.time/dt).round() as i64;
                for (hi,h) in self.horses.iter().enumerate() {
                    eprintln!("st tick={} h={} d={:.4} lane={:.4} v={:.5} tgt={:.5} hp={:.2} need={:.2} ord={} ph={} spst={:.2} spv={:.5} spr={} fin={} front={} sk={} rawspd={:.1} spd={:.1}",tick,hi,h.distance,h.lane,h.speed,h.target_speed,h.hp,
                        self.jp_need_hp_mode(hi,"all"),h.order,h.phase,h.spurt_start,h.spurt_speed,h.spurt_result,h.finished,h.front.map_or(-1,|i|i as i32),h.mods.apply(6,0.0),h.input.speed,h.stat(0));
                    if crate::envc::var_os("JPRACE_LOG_M23").is_some() {eprintln!("st4 tick={} h={} m23=[{}] self_speed={:.5}",tick,hi,h.mods.entries.iter().filter(|m|m.index==23).map(|m|format!("{:.4}@{:.2}",m.value,m.remaining.unwrap_or(-1.0))).collect::<Vec<_>>().join("|"),h.self_speed);}
                    eprintln!("st3 tick={} h={} spd_unc={:.3} spd_unc_mod={:.3} distspd={:.6} fs={} fsj={:.5} fsx={:.5} m5={:.4} m23={:.4} down={} slope={} m6={} m10={}",tick,hi,h.speed_unc,h.mods.apply(0,h.speed_unc),h.dist_speed,h.rafs_on,h.fs_j,h.fs_x,h.mods.apply(5,0.0),h.mods.apply(23,0.0),h.downhill,h.slope,
                        h.mods.entries.iter().filter(|m|m.index==6).map(|m|format!("{:.4}@{:.2}{}",m.value,m.remaining.unwrap_or(-1.0),m.lb.map_or(String::new(),|r|format!("[{:.3},{:.2}]",r[0],r[1])))).collect::<Vec<_>>().join("|"),
                        h.mods.entries.iter().filter(|m|m.index==10).map(|m|format!("{:.4}@{:.2}",m.value,m.remaining.unwrap_or(-1.0))).collect::<Vec<_>>().join("|"));
                    let gf=self.p().f("Hp.HpGutsCoef")/(h.stat(3)*self.p().f("Hp.HpGutsCoefSqrt")).sqrt()+1.0;
                    eprintln!("st2 tick={} h={} base={:.6} gm={:.6} gf={:.6} L={:.1} slope={} pow={:.1} guts={:.1} wiz={:.1} sta={:.1} acc={:.6} calls={}",tick,hi,self.race_base(),self.ground_multi(),gf,self.course.distance,h.slope,h.stat(2),h.stat(3),h.stat(4),h.stat(1),h.mods.apply(31,0.0),self.random.calls());
                    if crate::envc::var_os("JPRACE_LOG_OT").is_some() {eprintln!("st5 tick={} h={} ord={} ot={:?} cool={:.3} front={} vis={:?}",tick,hi,h.order,h.overtake,h.ot_cool,h.front.map_or(-1,|i|i as i32),h.visible);}
                }
            }
        }
        let tk=(self.time/dt).round() as i64;
        if dt>0.0 && let Ok(x)=crate::envc::var("JPRACE_LOG_PMTICK") && let Some((a,b))=x.split_once('-') && a.parse::<i64>().is_ok_and(|a|tk>=a) && b.parse::<i64>().is_ok_and(|b|tk<=b) {
            let mut o:Vec<usize>=(0..self.horses.len()).filter(|&i|!self.horses[i].finished).collect();o.sort_by_key(|&i|self.horses[i].order);
            eprintln!("pmtick tick={} pm={} cnt={} handed={} top={:?}",tk,self.pacemaker,self.pm_not_forward,self.pm_handed,o.iter().take(4).map(|&i|(i,self.horses[i].input.running_style,self.horses[i].order,format!("{:.3}",self.horses[i].distance))).collect::<Vec<_>>());
        }
        if dt>0.0 && self.pm_trk!=self.pacemaker {self.pm_trk=self.pacemaker;self.pm_trk_tick=tk;}
        let tbase=match crate::envc::var("JPRACE_PM_TIMER_BASE").as_deref() {Ok("chg")=>self.pm_trk_tick,Ok("sec")=>self.pm_sec_tick,_=>self.pm_hand_tick};
        let tany=crate::envc::var("JPRACE_PM_TIMER_ANY").as_deref()==Ok("1") && tbase>=0;
        if dt>0.0 && (self.pm_hand_tick>=0 || tany) && let Some(per)=crate::envc::var("JPRACE_PM_TIMER").ok().and_then(|x|x.parse::<i64>().ok()) && tk>tbase && (tk-tbase)%per==0 {
            let st=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
            let prevord=crate::envc::var("JPRACE_PM_TIMER_ORD").as_deref()==Ok("prev");
            if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(i,h)|(if prevord {h.prev_order} else {h.order},*i)) {
                if i!=self.pacemaker && st(&self.horses[i])<=st(&self.horses[self.pacemaker]) {
                    if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmtimer t={:.3} tick={} {} -> {} pmst={} topst={} handed={} cnt={} topd={:.3} pmd={:.3} pmord={} sec={} since={}",self.time,tk,self.pacemaker,i,st(&self.horses[self.pacemaker]),st(&self.horses[i]),self.pm_handed,self.pm_not_forward,self.horses[i].distance,self.horses[self.pacemaker].distance,self.horses[self.pacemaker].order,self.pm_section,tk-self.pm_hand_tick);}
                    self.pacemaker=i;self.pm_not_forward=0;
                }
            }
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_LOG_PMEPI").is_some() {
            thread_local!{static LAST:std::cell::Cell<(usize,usize,bool,i32)>=const{std::cell::Cell::new((usize::MAX,usize::MAX,false,-1))};}
            let top=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(i,h)|(h.order,*i)).map(|(i,_)|i).unwrap_or(0);
            let ptop=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(i,h)|(h.prev_order,*i)).map(|(i,_)|i).unwrap_or(0);
            let cur=(top,self.pacemaker,self.pm_handed,self.pm_not_forward);
            if LAST.with(|l|l.get())!=cur {
                LAST.with(|l|l.set(cur));
                eprintln!("pmepi tick={} top={} ptop={} topst={} pm={} pmst={} handed={} cnt={} topd={:.3} pmd={:.3} sec={}",tk,top,ptop,self.horses[top].input.running_style,self.pacemaker,self.horses[self.pacemaker].input.running_style,self.pm_handed,self.pm_not_forward,self.horses[top].distance,self.horses[self.pacemaker].distance,self.pm_section);
            }
        }
        let follow_now=self.pm_follow && match crate::envc::var("JPRACE_PM_HANDKEEP").as_deref() {Ok("sec15")=>(tk-self.pm_back_tick)%15==0,Ok("g30")=>tk%30==0,Ok("g15")=>tk%15==0,Ok(x) if x.starts_with('h')=>x[1..].parse::<i64>().is_ok_and(|n|(tk-self.pm_hand_tick)%n==0),_=>true};
        if dt>0.0 && crate::envc::var("JPRACE_PM_NOBACK").as_deref()!=Ok("1") && (self.pm_handed || follow_now || self.pm_not_forward>=self.p().f("PositionKeep.PaseMakerChangeTopHorseNotMostForwardStyleCount") as i32) {
            let st=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
            let best=self.horses.iter().filter(|h|!h.finished).map(st).min().unwrap_or(1);
            if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|h.order) {
                if st(&self.horses[i])==best {if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() && self.pacemaker!=i {eprintln!("pmback t={:.3} {} -> {} handed={} follow={}",self.time,self.pacemaker,i,self.pm_handed,self.pm_follow);}let pm_before_back=self.pacemaker;self.pacemaker=i;
                    match crate::envc::var("JPRACE_PM_BACKKEEP").as_deref() {Ok("1")=>(),Ok("old")=>self.pm_not_forward=0,_=>{if pm_before_back!=i {self.pm_not_forward=0;}}}
                    if pm_before_back!=i {self.pm_back1_tick=tk;}
                    match crate::envc::var("JPRACE_PM_HANDKEEP").as_deref() {Ok("1")=>(),Ok(x) if x.starts_with("sec") || x.starts_with('g') || x.starts_with('h')=>{self.pm_handed=false;if !self.pm_follow {self.pm_back_tick=(self.time/dt).round() as i64;}self.pm_follow=true;},_=>self.pm_handed=false}}
            }
        }
        let f1=crate::envc::var("JPRACE_PM_FOLLOW1").ok();
        let f1prev=crate::envc::var("JPRACE_PM_FOLLOW1_ORD").as_deref()==Ok("prev");
        let f1hand=crate::envc::var("JPRACE_PM_FOLLOW1_BASE").as_deref()!=Ok("back");
        let f1base=if f1hand {self.pm_hand1_tick} else {self.pm_back1_tick};
        if dt>0.0 && f1base>=0 && f1.as_deref()!=Some("off") && let Some(x)=f1.map_or(Some(if f1hand {60} else {48}),|x|x.parse::<i64>().ok()) && tk-f1base>=x {
            self.pm_back1_tick=-1;if f1hand {self.pm_hand1_tick=-1;}
            let st=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
            let best=self.horses.iter().filter(|h|!h.finished).map(st).min().unwrap_or(1);
            if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(i,h)|(if f1prev {h.prev_order} else {h.order},*i)) {
                if i!=self.pacemaker && st(&self.horses[i])==best && st(&self.horses[self.pacemaker])==best {
                    if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmfollow1 t={:.3} tick={} {} -> {}",self.time,tk,self.pacemaker,i);}
                    self.pacemaker=i;
                }
            }
        }
        if dt>0.0 {
            if self.pm_fchg!=self.pacemaker {self.pm_fchg=self.pacemaker;self.pm_fchg_tick=tk;}
            if let Some(cool)=crate::envc::var("JPRACE_PM_FOLLOW").ok().and_then(|x|x.parse::<i64>().ok()) && (self.pm_hand_tick>=0 || crate::envc::var("JPRACE_PM_FOLLOW_ANY").as_deref()==Ok("1")) && tk-self.pm_fchg_tick>=cool {
                let st=|h:&Horse|if h.ex() && !pm_ex_nige() {0} else {h.input.running_style};
                let best=self.horses.iter().filter(|h|!h.finished).map(st).min().unwrap_or(1);
                if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(i,h)|(h.order,*i)) {
                    let same_ok=crate::envc::var("JPRACE_PM_FOLLOW_SAME").as_deref()==Ok("1") && st(&self.horses[i])==st(&self.horses[self.pacemaker]);
                    if i!=self.pacemaker && (same_ok || st(&self.horses[i])==best && st(&self.horses[self.pacemaker])==best) {
                        if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {eprintln!("pmfollow t={:.3} tick={} {} -> {} since={} handed={}",self.time,tk,self.pacemaker,i,tk-self.pm_fchg_tick,self.pm_handed);}
                        self.pacemaker=i;self.pm_fchg=i;self.pm_fchg_tick=tk;
                    }
                }
            }
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_LOG_PKGAP2").is_some() {
            let tick=(self.time/dt).round() as i64;let pm=&self.horses[self.pacemaker];
            for (hi,h) in self.horses.iter().enumerate() {
                if matches!(h.cp.pre_mode,3|4) {
                    let lead=self.horses.iter().map(|o|o.prev_distance).fold(f32::MIN,f32::max);
                    let leadc=self.horses.iter().map(|o|o.distance).fold(f32::MIN,f32::max);
                    let np=self.horses.iter().filter(|o|!o.ex()).map(|o|o.prev_distance).fold(f32::MIN,f32::max);
                    let nc=self.horses.iter().filter(|o|!o.ex()).map(|o|o.distance).fold(f32::MIN,f32::max);
                    eprintln!("pkgap2 tick={} h={} mode={} gp={:.4} gc={:.4} lp={:.4} lc={:.4} np={:.4} nc={:.4} own={:.4} pm={}",tick,hi,h.cp.pre_mode,pm.prev_distance-h.prev_distance,pm.distance-h.prev_distance,lead-h.prev_distance,leadc-h.prev_distance,np-h.prev_distance,nc-h.prev_distance,h.prev_distance,self.pacemaker);
                }
            }
        }
        if dt>0.0 {for h in &mut self.horses {if h.front.is_some() {h.block_front_acc+=dt;}}}
        if dt>0.0 && !self.jp_entry_drawn {
            if let Some(k)=crate::envc::var("JPRACE_ENTRY_DRAWS").ok().and_then(|x|x.parse::<i64>().ok()) {
                let sec=self.course.distance/24.0;
                if self.jp_entry_tick<0 && self.horses.iter().any(|h|h.distance>=10.0*sec) {self.jp_entry_tick=(self.time/dt).round() as i64;}
                let off=crate::envc::var("JPRACE_ENTRY_OFFSET").ok().and_then(|x|x.parse::<i64>().ok()).unwrap_or(0);
                if self.jp_entry_tick>=0 && (self.time/dt).round() as i64>=self.jp_entry_tick+off {
                    self.jp_entry_drawn=true;
                    if crate::envc::var_os("JPRACE_LOG_ENTRYDRAWS").is_some() {eprintln!("entrydraws t={:.4} tick={} start_call={} k={}",self.time,(self.time/dt).round() as i64,self.random.calls()+1,k);}
                    for _ in 0..k {self.random.float_max(1.0);}
                }
            }
        }
        if dt>0.0 && !self.jp_pkforce_done {
            if let Ok(m)=crate::envc::var("JPRACE_PK_FORCE") {
                let sec=self.course.distance/24.0;
                if self.horses.iter().any(|h|h.distance>=10.0*sec) {
                    self.jp_pkforce_done=true;
                    let p=self.assets.params();
                    let runners=self.horses.iter().map(|h|crate::position_keep::Runner {
                        distance:h.distance,order:h.order,style:h.input.running_style,ex:h.ex(),phase:h.phase,
                        wiz:h.stat(4),temptation:if h.tempted() && h.temptation_mode==0 {3} else {h.temptation_mode},
                        speed_skill:h.mods.apply(6,0.0)>0.0 || h.mods.apply(5,0.0)>0.0,speed_skill22:h.mods.apply(23,0.0)>0.0,
                    }).collect::<Vec<_>>();
                    let c0=self.random.calls();
                    let far=crate::envc::var_os("JPRACE_PK_FORCE_ALLSEC").is_some();
                    for hi in 0..self.horses.len() {
                        let mut kp=self.horses[hi].keep.clone();kp.force_ready(m=="reset");
                        let c=self.random.calls();
                        let mut rr=runners.clone();
                        if far {let s0=(p.i("PositionKeep.PositionKeepEndSection") as f32-0.5)*sec;rr[hi].distance=s0.min(rr[hi].distance);}
                        kp.update(&Tree(p.get("PositionKeep")),self.course.distance as i32,24,&rr,hi,self.pacemaker,&mut self.random,dt,true);
                        if crate::envc::var_os("JPRACE_LOG_PKFORCE").is_some() {eprintln!("pkforce h={} draws={} wanted={} mode={}",hi,self.random.calls()-c,kp.wanted,kp.mode);}
                    }
                    if crate::envc::var_os("JPRACE_LOG_PKFORCE").is_some() {eprintln!("pkforce total={}",self.random.calls()-c0);}
                }
            }
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_SPURT_AT11").is_some() && !self.jp_spurt11 {
            let sec=self.course.distance/24.0;
            if self.horses.iter().any(|h|h.distance>=10.0*sec) {
                self.jp_spurt11=true;
                let c0=self.random.calls();
                for hi in 0..self.horses.len() {
                    let h=&self.horses[hi];let save=(h.spurt_result,h.spurt_speed,h.spurt_start,h.spurt_calculated);
                    let c=self.random.calls();
                    self.spurt(hi);
                    if crate::envc::var_os("JPRACE_LOG_SPURT11").is_some() {eprintln!("spurt11 h={} draws={} result={}",hi,self.random.calls()-c,self.horses[hi].spurt_result);}
                    let h=&mut self.horses[hi];(h.spurt_result,h.spurt_speed,h.spurt_start,h.spurt_calculated)=save;
                }
                if crate::envc::var_os("JPRACE_LOG_SPURT11").is_some() {eprintln!("spurt11 total={}",self.random.calls()-c0);}
            }
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_LOG_ENTRYSTATE").is_some() && !self.jp_entry_logged {
            let sec=self.course.distance/24.0;
            if self.horses.iter().any(|h|h.distance>=10.0*sec) {
                self.jp_entry_logged=true;
                let top=self.horses.iter().map(|h|h.distance).fold(f32::MIN,f32::max);
                for hi in 0..self.horses.len() {
                    let need=self.jp_need_hp(hi);let slc=self.jp_sl_cond(hi);let (far,n,ns)=self.jp_cbs_state(hi);let h=&self.horses[hi];
                    eprintln!("entrystate t={:.4} h={} style={} ex={} wiz={:.0} guts={:.0} sta={:.0} hp={:.1} max={:.1} need={:.1} gap={:.2} lane={:.4} order={} keep={} tempt={} insec11={} slc={} far={} n={} ns={} down={} slope={} calls={}",
                        self.time,hi,h.input.running_style,i32::from(h.ex()),h.stat(4),h.stat(3),h.stat(1),h.hp,h.max_hp,need,top-h.distance,h.lane,h.order,h.keep.mode,i32::from(h.tempted()),i32::from(h.distance>=10.0*sec),
                        i32::from(slc),i32::from(far),n,ns,i32::from(h.downhill),h.slope,self.random.calls());
                }
            }
        }
        if dt>0.0 && crate::envc::var_os("JPRACE_LOG_STATE").is_some() {
            let sec=self.course.distance/24.0;
            let st:Vec<String>=self.horses.iter().map(|h|format!("{}{}{}",if h.distance>=10.0*sec {1} else {0},if h.front.is_some() {1} else {0},if h.side_time>0.0 {1} else {0})).collect();
            eprintln!("state t={:.4} {}",self.time,st.join(","));
        }
        for h in &mut self.horses {
            h.front_time=if h.front.is_some() {h.front_time+dt} else {0.0};
            h.side_time=if h.inside.is_some() || h.outside.is_some() {h.side_time+dt} else {0.0};
            if h.phase==1 && h.side_time>h.side_max_mid {h.side_max_mid=h.side_time;}
        }
        let mut overlap=(0..self.horses.len()).collect::<Vec<_>>();
        netsort::sort(&mut overlap,&|a,b|(self.horses[*a].lane as i32-self.horses[*b].lane as i32)*10000);
        for pair in overlap.windows(2) {
            let a=pair[0];let b=pair[1];
            if (self.horses[b].distance-self.horses[a].distance).abs()<0.4 && (self.horses[b].lane-self.horses[a].lane).abs()<1.0/45.0 {
                self.horses[b].lane=self.horses[a].lane+1.0/45.0;self.horses[b].force_lane=true;
            }
        }
        let mut order=(0..self.horses.len()).collect::<Vec<_>>();
        order.sort_by(|a,b|self.horses[*b].distance.total_cmp(&self.horses[*a].distance).then(a.cmp(b)));
        if dt>0.0 {self.compete_step(dt);}
        if dt>0.0 {self.near_times(dt);}
        for (i,hi) in order.iter().copied().enumerate() {let h=&mut self.horses[hi];h.prev_order=h.order;h.order=i;
            if i!=h.prev_order && crate::envc::var("JPRACE_LOG_ORDERUP").ok().and_then(|x|x.parse::<usize>().ok())==Some(hi) {eprintln!("orderchg tick={} h={} {} -> {} d={:.3}",(self.time/0.0666).round() as i64,hi,h.prev_order,i,h.distance);}
            if let Ok(x)=crate::envc::var("JPRACE_OU_ADD") && let Some((a,b))=x.split_once(':') && a.parse::<usize>().ok()==Some(hi) && b.parse::<i64>().ok()==Some((self.time/0.0666).round() as i64) {h.ou_cnt+=1;}
            if i<h.prev_order {h.ou_cnt+=if crate::envc::var("JPRACE_ORDERUP").as_deref()==Ok("pos") {(h.prev_order-i) as i32} else {1};}}
        {
            let cool=self.p().f("overTakeCountCoolDownTime");
            for hi in 0..self.horses.len() {
                let h=&self.horses[hi];if h.finished {continue;}
                let (cur,prev)=(h.order,h.prev_order);
                let h=&mut self.horses[hi];
                for c in &mut h.order_up_cool {*c-=dt;if *c<=0.0 && *c!=0.0 {*c=0.0;}}
                if cur>=prev {continue;}
                let Some(&passed)=order.get(cur+1) else {continue};
                if h.order_up_cool[passed]>0.0 {continue;}
                h.order_up_cool[passed]=cool;
                let lastspurt=h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start;
                match h.phase {0=>h.order_up[0]+=1,1=>h.order_up[1]+=1,2|3=>h.order_up[2]+=1,_=>()}
                if h.corner!=0 {h.order_up[3]+=1;if matches!(h.phase,2|3) {h.order_up[4]+=1;}}
                if lastspurt {h.order_up[5]+=1;}
                if h.distance>=200.0 {h.order_up[6]+=1;}
                if h.final_corner {h.order_up[7]+=1;}
            }
        }
        let unfinished=self.horses.iter().filter(|h|!h.finished).map(|h|h.distance).collect::<Vec<_>>();
        if let (Some(max),Some(min))=(unfinished.iter().copied().reduce(f32::max),unfinished.iter().copied().reduce(f32::min)) {
            self.distance_diff_max=self.distance_diff_max.max(max-min);
        }
        if dt>0.0 && self.time>=5.0 {
            let n=self.horses.len() as f32;
            for h in &mut self.horses {
                for i in 0..9 {
                    let threshold=(((i+1) as f32*10.0)/100.0*n+0.5) as i32;let o=h.order as i32+1;
                    if threshold<o {h.order_continue[0][i]=false;}
                    if o<threshold {h.order_continue[1][i]=false;}
                }
            }
        }
        if dt>0.0 {self.course_events();self.pm_ready=true;}
    }
    fn record(&self)->Value {
        json!({"t":self.time,"horses":self.horses.iter().map(|h|h.frame()).collect::<Vec<_>>()})
    }
    fn defeats(&self,finish:&[usize])->Vec<i32> {
        let d=Tree(self.assets.params().get("Defeat"));let n=self.horses.len();
        let mut finish_order=vec![0usize;n];for (o,&i) in finish.iter().enumerate() {finish_order[i]=o;}
        let per=|p:&str|((d.i(p) as f32/100.0)*n as f32+0.5) as i32;
        let top=|hi:usize|{
            let pick=|k:usize|finish[k];
            let (mut sum,mut cnt)=(0,0);
            for k in 0..3.min(n) {let i=pick(k);if i!=hi {sum+=self.horses[i].skills.len() as i32;cnt+=1;}}
            if cnt==0 {0} else {(sum as f32/cnt as f32+0.5) as i32}
        };
        (0..n).map(|hi|{
            let h=&self.horses[hi];let fo=finish_order[hi];
            if fo<1 {return 1;}
            if fo as i32<=d.i("DefeatLoseFinishOrder") as i32-1 {
                let prev=&self.horses[finish[fo-1]];
                let diff=h.scaled_time-prev.scaled_time;let x=diff*15.0;
                let b=if 0.0>x {0.0} else {x/2.5} as f64;
                let b=b.floor() as i32;
                if b<=d.i("DefeatLoseBashinDiff") as i32 {return 2;}
            }
            if h.apt_distance<d.i("DefeatProperDistance") as i32 {return 12;}
            if h.apt_ground<d.i("DefeatProperGround") as i32 {return 13;}
            let same=self.horses.iter().filter(|o|o.input.running_style==h.input.running_style).count() as i32;
            if same>per("DefeatSameRunningStylePer") {return 3;}
            if h.hp_empty {
                if h.temptation_count>=d.i("DefeatTemptaionCount") as i32 {return 4;}
                return if (h.guts_order as i32+1)<per("DefeatGutsOrderPer") {6} else {5};
            }
            if h.spurt_result&0xc!=0 {return 7;}
            if h.spurt_result==2 {return 8;}
            if h.input.motivation<=d.i("DefeatMotivation") as i32 {return 14;}
            if (h.skills.len() as i32)<top(hi) {return 9;}
            if h.block_front_acc<d.f("DefeatFrontBlockAccumulateTime") {11} else {10}
        }).collect()
    }
    pub fn run(mut self,max_ticks:usize)->Result<Value,String> {
        let start=std::time::Instant::now();
        let r=self.run_inner(max_ticks);
        if crate::envc::var_os("JPRACE_PROFILE").is_some() {PROF.with(|p|{let p=p.borrow();eprintln!("profile total={:.3} around+pm={:.3} skills={:.3} update={:.3}",start.elapsed().as_secs_f64(),p[0],p[1],p[2]);});}
        r
    }
    fn run_inner(mut self,max_ticks:usize)->Result<Value,String> {
        self.tick(0.0);let mut frames=vec![self.record()];let mut interval=0.0;let mut all=false;
        let mut rush_start=self.horses.iter().map(|h|if h.tempted() {Some(self.time)} else {None}).collect::<Vec<_>>();let mut rushing_events=Vec::new();
        for _ in 0..max_ticks {
            self.prev_time=self.time;self.time+=DT;if crate::envc::var_os("JPRACE_LOG_RAND").is_some() {eprintln!("tickmark t={:.4} call={}",self.time,self.random.calls());}self.tick(DT);interval-=DT;all=self.horses.iter().all(|h|h.finished);
            for (hi,h) in self.horses.iter().enumerate() {
                if h.tempted() && !h.finished {
                    if rush_start[hi].is_none() {rush_start[hi]=Some(self.time);}
                } else if let Some(start)=rush_start[hi].take() {
                    let end=if h.finished {h.raw_time} else {self.time};
                    rushing_events.push(json!({"horse":hi,"t":start,"duration":(end-start).max(0.0)}));
                }
            }
            if interval<=0.0 || all {
                frames.push(self.record());
                interval=if self.time<1.0 || self.horses.iter().any(|h|h.input.viewer_id!=0 && (self.course.distance-h.distance).abs()<=25.0) {DT} else {1.0};
            }
            if all {break;}
            if let Some(st)=crate::envc::var("JPRACE_STOP_T").ok().and_then(|x|x.parse::<f32>().ok()) && self.time>=st {
                frames.push(self.record());
                return Ok(json!({"frames":frames,"partial":true}));
            }
        }
        if !all {return Err(format!("race did not finish within {max_ticks} ticks"));}
        let mut finish=(0..self.horses.len()).collect::<Vec<_>>();
        finish.sort_by(|a,b|self.horses[*a].scaled_time.total_cmp(&self.horses[*b].scaled_time).then(a.cmp(b)));
        let original_finish_times=self.horses.iter().map(|h|h.scaled_time).collect::<Vec<_>>();
        let winner=finish[0];let p=Tree(&self.course.master);
        let lo=p.i("finish_time_min") as f32*0.0001;let hi=p.i("finish_time_max") as f32*0.0001;
        let time=self.horses[winner].scaled_time;
        if time<lo || time>hi {
            let mut clamp=RaceRandom::new(self.input.seed as i32);
            let (bound,range)=if time<lo {(lo,p.i("finish_time_min_random_range") as f32*0.0001)} else {(hi,p.i("finish_time_max_random_range") as f32*0.0001)};
            let shift=clamp.float_range(-range,range)+bound-time;
            for h in &mut self.horses {h.scaled_time+=shift;}
        }
        let defeat=self.defeats(&finish);
        let mut results=vec![Value::Null;self.horses.len()];
        for (order,hi) in finish.iter().copied().enumerate() {
            let h=&self.horses[hi];let diff=if order==0 {0.0} else {
                original_finish_times[hi]-original_finish_times[finish[order-1]]
            };
            results[hi]=json!({"order":order,"time":h.scaled_time,"diff":diff,"delay":h.original_delay,
                "guts_order":h.guts_order,"wiz_order":h.wiz_order,"last_spurt":h.spurt_start,
                "style":h.input.running_style,"defeat":defeat[hi],"raw_time":h.raw_time});
        }
        Ok(json!({"schema":"race-wire-snapshot-v1","mode":"independent","version":100000002,
            "runner_count":self.horses.len(),"distance_diff_max":self.distance_diff_max,
            "frames":frames,"events":self.events,"results":results,"rushing_events":rushing_events,
            "provenance":{"replay_sha":self.input.replay_sha,"logged_seed":self.input.logged_seed,
                "seed":self.input.seed,"seed_mode":self.input.seed_mode,"input_sha256":self.input.input_sha256}}))
    }
}

pub fn initial_lane(gate:i32,lane_type:i64,half:bool)->f32 {
    let mut lane=gate as f32*(1.0/18.0);
    let extra=initial_lane_extra(gate,lane_type,half);
    if extra!=0.0 {lane+=extra;}
    lane
}
fn initial_lane_extra(gate:i32,lane_type:i64,half:bool)->f32 {
    match lane_type {
        1 if gate>8=>0.03333333507180214,
        3 if gate>13=>0.10333333164453506,
        4 if gate>7 && !half=>0.03333333507180214,
        _=>0.0,
    }
}

fn stop_effect(horses:&mut [Horse],uid:u64) {
    for h in horses {
        h.self_speed+=h.mods.entries.iter().filter(|m|m.owner==uid && m.index==23 && !m.once).map(|m|m.value).sum::<f32>();
        h.mods.stop(uid);
    }
}

pub fn post_number(index:usize,count:usize)->usize {
    if count<=8 {return index+1;}
    let (q,r)=(count/8,count%8);let mut end=0;
    for post in 1..=8 {end+=q+usize::from(post>8-r);if index<end {return post;}}
    1
}

fn global_hp_duration_multiplier(params:Tree<'_>,hp:f32)->f32 {
    params.arr("Skill.TimeRemainHpCoefAscArray").iter()
        .find(|r|hp<f32_of(&r["HpThreshold"]))
        .map_or(1.0,|r|f32_of(&r["Multiply"]))
}

fn take_additional_count(pending:i32,cap:i32,used:&mut i32)->i32 {
    let count=pending.max(0).min((cap-*used).max(0));
    *used+=count;
    count
}
fn global_additional_cap(params:Tree<'_>,kind:i64)->i32 {
    if crate::envc::var_os("JPRACE_LOG_ADDL").is_some() {eprintln!("addl {}",params.get("Skill.AdditionalActivateAbilityMaxCountArray"));}
    params.arr("Skill.AdditionalActivateAbilityMaxCountArray").iter()
        .find(|r|r["AdditionalActivateType"].as_i64()==Some(kind))
        .map_or(0,|r|r["AdditionalActivateMaxCount"].as_i64().unwrap_or(0) as i32)
}

fn adjusted_stat(base:f32,coef:f32,ground:f32,career:f32,max:f32)->f32 {
    if f32m('g') {return (base*coef+ground+career).clamp(1.0,max);}
    (base as f64*coef as f64+ground as f64+career as f64).clamp(1.0,max as f64) as f32
}
thread_local!(static PROF:std::cell::RefCell<[f64;4]>=const {std::cell::RefCell::new([0.0;4])});
fn skill_dt(dt:f32)->f32 {
    static V:std::sync::OnceLock<Option<f32>>=std::sync::OnceLock::new();
    match V.get_or_init(||crate::envc::var("JPRACE_SKILL_DT").ok().and_then(|x|if x=="15" {Some(1.0/15.0)} else {x.parse::<f32>().ok()})) {Some(v) if dt>0.0=>*v,_=>dt}
}
fn lane_disp_f64()->bool {static V:std::sync::OnceLock<bool>=std::sync::OnceLock::new();*V.get_or_init(||crate::envc::var("JPRACE_LANE_DISP").as_deref()==Ok("f64"))}
pub fn slb_enabled()->bool {crate::envc::var("JPRACE_SLB").as_deref()!=Ok("0")}
pub fn fs_enabled()->bool {crate::envc::var("JPRACE_FS").as_deref()!=Ok("0")}
pub fn up_slope(slope:f32,v1:f32,pow:f32)->f32 {
    static M:std::sync::OnceLock<u8>=std::sync::OnceLock::new();
    match *M.get_or_init(||match crate::envc::var("JPRACE_UPSLOPE").as_deref() {Ok("mul")=>1,_=>0}) {1=>-slope.abs()*v1/pow,_=>-slope.abs()*(v1/pow)}
}
pub fn f32m(c:char)->bool {static S:std::sync::OnceLock<String>=std::sync::OnceLock::new();S.get_or_init(||crate::envc::var("JPRACE_F32").unwrap_or_else(|_|"mslMWqawhxib".into())).contains(c)}

fn load_rafs_oracle()->Vec<(usize,f32)> {
    let Ok(path)=crate::envc::var("JPRACE_FS_FILE").or_else(|_|crate::envc::var("JPRACE_ORACLE")) else {return vec![]};
    let v:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    v["events"].as_array().unwrap().iter().filter(|e|e["type"].as_i64()==Some(12)).map(|e|(e["params"][0].as_i64().unwrap() as usize,e["t"].as_f64().unwrap() as f32)).collect()
}
fn load_oracle()->Vec<(usize,i64,f32,f32)> {
    let Ok(path)=crate::envc::var("JPRACE_ORACLE") else {return vec![]};
    let v:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    v["events"].as_array().unwrap().iter().filter(|e|matches!(e["type"].as_i64(),Some(8|9|10))).map(|e|{
        let p=e["params"].as_array().unwrap();
        (p[0].as_i64().unwrap() as usize,e["type"].as_i64().unwrap(),e["t"].as_f64().unwrap() as f32,{let e=p[1].as_i64().unwrap();if e==0 {f32::MAX} else {e as f32/10000.0}})
    }).collect()
}

fn cond_phase_dist()->bool {crate::envc::var("JPRACE_COND_PHASE").as_deref()==Ok("dist")}
fn stat_max_idx(i:usize)->bool {
    static S:std::sync::OnceLock<Option<Vec<usize>>>=std::sync::OnceLock::new();
    S.get_or_init(||crate::envc::var("JPRACE_STAT_MAX_IDX").ok().map(|x|x.split(',').filter_map(|v|v.parse().ok()).collect())).as_ref().is_none_or(|v|v.contains(&i))
}

fn sens_on()->bool {static V:std::sync::OnceLock<bool>=std::sync::OnceLock::new();*V.get_or_init(||crate::envc::var_os("JPRACE_LOG_SENS").is_some())}

fn fs_capprev_mode()->u8 {static V:std::sync::OnceLock<u8>=std::sync::OnceLock::new();*V.get_or_init(||match crate::envc::var("JPRACE_FS_CAPPREV").as_deref() {Ok("0")=>0,Ok("1")=>1,Ok("2")=>2,Ok("3")=>3,Ok("5")=>5,_=>4})}
fn fs_capprev()->bool {fs_capprev_mode()!=0}
fn fs_capprev_fin()->bool {fs_capprev_mode()==2}
fn fs_capdec()->bool {matches!(fs_capprev_mode(),3|4)}
fn fs_capnomin()->bool {matches!(fs_capprev_mode(),4|5)}

fn spd_seq()->bool {static V:std::sync::OnceLock<bool>=std::sync::OnceLock::new();*V.get_or_init(||crate::envc::var("JPRACE_SPD_SEQ").as_deref()==Ok("1"))}

fn lb_value(r:[f32;5],fs:f32,c:f32)->f32 {
    static V:std::sync::OnceLock<u8>=std::sync::OnceLock::new();
    match *V.get_or_init(||match crate::envc::var("JPRACE_LB_FORM").as_deref() {Ok("old")=>0,_=>1}) {
        1=>r[0]+r[0]*(r[1]+fs)*c*r[2], _=>r[0]*(1.0+(r[1]+fs)*c*r[2])}
}
