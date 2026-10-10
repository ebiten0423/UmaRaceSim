use std::collections::HashMap;
use serde_json::{Value,json};
use crate::{data::{Assets,Master,RaceIn,HorseIn,Tree,ival,sval,f32_of},course::Course,geometry::Geometry,
    math,rng::RaceRandom,modifiers::{Modifiers,Operation},triggers::{self,Condition},netsort};

pub const DT:f32=0.0666;
const PHASE_NAMES:[&str;4]=["Start","Middle","End","Last"];
const STYLE_NAMES:[&str;4]=["Nige","Senko","Sashi","Oikomi"];
#[derive(Clone)]
struct Detail {
    pre:Condition, condition:Condition, pre_passed:bool, active:bool,
    left:f32, cooldown:f32, uid:u64, number:usize, additional_seen:i32, additional_used:[i32;3],
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
    position:[f32;3], speed:f32, cf_speed:f32, cf_hp:f32, cf_pos:[f32;2], self_speed:f32, min_speed:f32, hp:f32, max_hp:f32,
    delay:f32, original_delay:f32, phase:usize, start_dash:bool, finished:bool,
    order:usize, prev_order:usize, guts_order:usize, wiz_order:usize,
    front:Option<usize>, inside:Option<usize>, outside:Option<usize>, near_count:usize,
    front_time:f32, side_time:f32, near_front_time:f32, near_behind_time:f32,
    near_behind_set:Vec<f32>, surround:[bool;3], reserved:Vec<i32>,
    corner:i32, corner_end:f32, final_corner:bool, straight:bool, front_type:i32,
    slope:f32, slope_end:f32, called_event:i32,
    stat_max:f32,
    force_in:f32, ex:bool, style_rate:f32, dist_speed:f32, dist_power:f32, ground_coef:f32,
    section_speed:[f32;24], section_base:[f32;24], temptation_section:i32, temptation_mode:i32, temptation_active:bool,
    temptation_count:i32, temptation_timer:f32, temptation_check:f32, temptation_forced:bool, temptation_end:u8,
    downhill:bool, downhill_check:f32, spurt_result:i32, spurt_start:f32, spurt_speed:f32,
    spurt_calculated:bool, skill_time:f32, target_speed:f32, after_goal:f32, extra_lane:Option<f32>, force_lane:bool, raw_time:f32, scaled_time:f32,
    around:Vec<Around>, visible:Vec<usize>, in_space:f32, out_space:f32, overtake:Vec<usize>, ot_cool:f32, move_time:f32, move_dir:i32,
    order_up:[i32;8], order_up_cool:Vec<f32>, ot_target_time:f32, fight_count:i32, lane_move:f32,
    hp_empty:bool, block_front_acc:f32, apt_distance:i32, apt_ground:i32,
    activate_count:i32, activate_phase:[i32;4], activate_half:[i32;2], heal_count:i32, keep:crate::position_keep::Keep, cp:Conserve,
}
#[derive(Clone,Default)]
struct Conserve { top:bool, pow:f32, released:bool, accel:f32, left:f32, spec:f32, ctimer:f32, last_credit:f32, s_tm:bool, s_tp:bool, lag_mode:i32 }
impl Horse {
    fn tempted(&self)->bool {self.temptation_mode!=0}
    fn stat(&self,i:usize)->f32 {self.mods.apply(i,self.adjusted[i]).clamp(1.0,self.stat_max)}
    fn ex(&self)->bool {self.ex || self.mods.apply(20,0.0)!=0.0}
    fn frame(&self)->Value {
        json!({"distance":self.distance,
        "lane":(self.lane as f64*10000.0) as i32,
        "speed":(self.speed as f64*100.0) as i32,"hp":self.hp as i32,"tempt":self.temptation_mode,
        "block":self.front.map_or(-1,|i|i as i32)}) }
}
pub struct Simulator<'a> {
    input:RaceIn, master:&'a Master, assets:&'a Assets, course:Course, geometry:Geometry,
    horses:Vec<Horse>, random:RaceRandom, time:f32, events:Vec<Value>,
    distance_diff_max:f32, compete:crate::compete::State, pacemaker:usize, pm_ready:bool, pm_section:i32, pm_not_forward:i32, pm_handed:bool, cur_skill:i32,
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
            horses:vec![],time:0.0,events:vec![],distance_diff_max:0.0,compete:crate::compete::State::new(0),pacemaker:0,pm_ready:false,pm_section:0,pm_not_forward:0,pm_handed:false,cur_skill:0};
        s.initialize()?;Ok(s)
    }
    fn p(&self)->Tree<'_> {self.assets.params()}
    fn build_details(&mut self,hi:usize,id:i32,row:&std::collections::HashMap<String,Value>)->Vec<Detail> {
        let mut details=vec![];
        for d in 1..=2 {
            let text=sval(row,&format!("condition_{d}"));if text.is_empty() {continue;}
            self.random.wide=true;
            let pre=triggers::build(sval(row,&format!("precondition_{d}")),&self.course,&mut self.random);
            let condition=triggers::build(text,&self.course,&mut self.random);
            self.random.wide=false;
            details.push(Detail {pre_passed:pre.is_empty(),pre,condition,active:false,left:0.0,cooldown:0.0,
                uid:((hi as u64)<<48)|((id as u64)<<8)|d as u64,number:d,additional_seen:0,additional_used:[0;3]});
        }
        details
    }
    fn race_base(&self)->f32 {
        let p=self.p();
        p.f("raceBaseSpeedBaseSpeed")-(self.course.distance-p.f("raceBaseSpeedBaseDistance"))/p.f("raceBaseSpeedDistanceRate")
    }
    fn rate(&self,table:&str,id:i32,column:&str)->Result<f32,String> {
        let r=self.master.find(table,"id",id as i64).ok_or_else(||format!("missing {table}:{id}"))?;
        Ok(ival(r,column) as f32*0.0001)
    }
    fn initialize(&mut self)->Result<(),String> {
        let p=self.assets.params();let c=&self.course;
        let track=self.master.find("race_track","id",c.master["race_track_id"].as_i64().unwrap()).ok_or("race track")?;
        let half=self.input.horses.len()<=9 && ival(track,"enable_half_gate")==1;
        let stat_max=2000.0;
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
            let lane=initial_lane(i as i32,ival(track,"initial_lane_type"),half);
            let ex=h.running_style==1 && h.skill_array.iter().any(|sk|self.master.find("skill_data","id",sk.skill_id as i64)
                .is_some_and(|r|(1..=2).any(|d|(1..=3).any(|a|ival(r,&format!("ability_type_{d}_{a}"))==6))));
            let raw=h.pow as f32;
            let cp_pow=if raw>1200.0 {(1200.0+(raw-1200.0)*0.5)*mood+(raw-1200.0)*0.5} else {raw*mood};
            horses.push(Horse {input:h.clone(),base,adjusted,mods:Modifiers::default(),skills:vec![],distance:0.0,prev_distance:0.0,
                order_continue:[[true;9];2],lane,prev_lane:lane,lane_max:c.lane_max,target_lane:lane,lane_speed:0.0,position:self.geometry.world(0.0,lane),speed:p.f("StartSpeed"),
                self_speed:p.f("StartSpeed"),min_speed:p.f("StartSpeed"),hp:0.0,max_hp:0.0,delay:0.0,original_delay:0.0,
                phase:0,start_dash:true,finished:false,order:i,prev_order:i,ot_target_time:0.0,fight_count:0,cf_speed:0.0,cf_hp:0.0,cf_pos:[0.0;2],lane_move:0.0,hp_empty:false,block_front_acc:0.0,apt_distance:distance_apt,apt_ground:ground_apt,order_up:[0;8],order_up_cool:vec![0.0;self.input.horses.len()],guts_order:0,wiz_order:0,
                front:None,inside:None,outside:None,near_count:0,around:vec![],visible:vec![],in_space:0.0,out_space:0.0,overtake:vec![],ot_cool:0.0,move_time:0.0,move_dir:0,front_time:0.0,side_time:0.0,near_front_time:0.0,near_behind_time:0.0,near_behind_set:vec![],surround:[false;3],reserved:vec![],
                corner:0,corner_end:0.0,final_corner:false,straight:false,front_type:0,slope:0.0,slope_end:0.0,called_event:-1,
                stat_max,force_in:0.0,ex,style_rate:self.rate("race_proper_runningstyle_rate",style_apt,"proper_rate")?,dist_speed:self.rate("race_proper_distance_rate",distance_apt,"proper_rate_speed")?,
                dist_power:self.rate("race_proper_distance_rate",distance_apt,"proper_rate_power")?,
                ground_coef:self.rate("race_proper_ground_rate",ground_apt,"proper_rate")?,section_speed:[0.0;24],section_base:[0.0;24],
                temptation_section:0,temptation_mode:0,temptation_active:false,temptation_count:0,temptation_timer:0.0,temptation_check:0.0,temptation_forced:false,temptation_end:0,
                downhill:false,downhill_check:0.0,spurt_result:8,spurt_start:-1.0,spurt_speed:0.0,spurt_calculated:false,skill_time:-1.0,target_speed:0.0,after_goal:0.0,extra_lane:None,force_lane:false,
                raw_time:-1.0,scaled_time:-1.0,activate_count:0,activate_phase:[0;4],activate_half:[0;2],heal_count:0,keep:crate::position_keep::Keep::default(),cp:Conserve {pow:cp_pow,..Default::default()}});
        }
        self.horses=horses;
        self.compete=crate::compete::State::new(self.horses.len());
        self.pacemaker=self.horses.iter().enumerate().min_by_key(|(i,h)|
            (if h.ex() {0} else {h.input.running_style},*i)).unwrap().0;
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
        let p=self.assets.params();let range=p.f("forceInMoveTargetSpeedAddRandomRange");
        for h in self.horses.iter_mut() {
            let name=["RunningOut","Proceding","Insertion","Chasing"][h.input.running_style as usize-1];
            let v=p.f(&format!("forceInMoveTargetSpeedAdd{name}"));
            h.force_in=if math::approximately(v,0.0) {0.0} else {
                self.random.float_range(if v<=0.0 {-range} else {0.0},if v>0.0 {range} else {0.0})+v
            };
        }
        for h in self.horses.iter_mut() {
            for sk in &mut h.skills {
                sk.enabled=lot_pass(&mut self.random,&sk.row,h.base[4]);
            }
        }
        self.skills_pass(0.0,1);self.skills_pass(0.0,0);
        for h in self.horses.iter_mut() {
            let fixed=h.mods.consume(22,0.0);let scale=h.mods.consume(15,1.0);
            h.delay=(if fixed>0.0 {fixed} else {self.random.float_max(p.f("startDelayMax"))})*scale;
            h.original_delay=h.delay;
        }
        let race_base=self.race_base();
        for h in self.horses.iter_mut() {
            let style=if h.ex() {"OoNige"} else {STYLE_NAMES[h.input.running_style as usize-1]};
            h.max_hp=(h.stat(1) as f64*p.f("Hp.HpInitialVal1") as f64*
                p.f(&format!("Hp.HpMaxCoef{style}")) as f64+self.course.distance as f64) as f32;
            h.hp=h.max_hp;
            let array=if h.ex() {"PhaseBaseTargetSpeedPerExArray[0]".to_owned()}
                else {format!("PhaseBaseTargetSpeedPerArray[{}]",h.input.running_style-1)};
            for i in 0..24 {
                let phase=self.course.phase(i as f32*(self.course.distance/24.0));
                let per=if phase<4 {p.f(&format!("BasetTargetSpeed.{array}.{}",PHASE_NAMES[phase]))} else {0.0};
                let add=if phase==2 || phase==3 {
                    let v=(h.stat(0)*p.f("BasetTargetSpeed.PhaseEndBaseTargetSpeedCoef")).sqrt();
                    (v as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64) as f32
                } else {0.0};
                let l=math::log10(h.stat(4)*0.1);
                let (w,v2,v1,p1)=(h.stat(4) as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomMinusVal2") as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomMinusVal1") as f64,p.f("BasetTargetSpeed.BaseTargetSpeedRandomPlusVal1") as f64);
                let (lo,up)=((l as f64*w/v2+v1) as f32,(l as f64*w/p1) as f32);
                let random=self.random.float_range_with(lo,up,crate::rng::Wide::FloatDraw);
                let c=0.01f32 as f64;
                h.section_base[i]=(per as f64*c*race_base as f64+add as f64) as f32;
                h.section_speed[i]=(random as f64*c*race_base as f64) as f32+h.section_base[i];
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
            if (self.random.int_max(100) as f32)<chance {
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
            "popularity"=>((h.input.popularity-1).max(0)+1) as f32,"phase"=>h.phase as f32,"corner"=>h.corner as f32,
            "is_finalcorner"=>boolean(h.final_corner),"is_finalcorner_laterhalf"=>boolean(h.final_corner && c.final_corner().is_some_and(|s|h.distance>=(s.start+s.end)*0.5)),
            "remain_distance"=>c.distance-(h.distance as i32) as f32,"distance_rate"=>{
                let threshold=(rhs as f32/100.0*c.distance+0.5) as i32;
                return Some((h.distance as i32-threshold+rhs) as f32);
            },
            "distance_diff_top"=>{
                let gap=self.horses.iter().map(|h|h.distance).fold(0.0,f32::max)-h.distance;
                gap.floor()
            },
            "distance_diff_rate"=>{
                let first=self.horses.iter().find(|o|o.order==0)?.distance;
                let last=self.horses.iter().find(|o|o.order==self.horses.len()-1)?.distance;
                let spread=first-last;
                if math::approximately(spread,0.0) {0.0} else {((first-h.distance)/spread*100.0) as i32 as f32}
            },
            "accumulatetime"=>self.time.floor(),"time"=>ival(self.master.find("race_instance","id",self.input.race_instance_id as i64)?,"time") as f32,
            "grade"=>ival(self.master.find("race","id",self.input.race_id as i64)?,"grade") as f32,
            "is_dirtgrade"=>ival(self.master.find("race","id",self.input.race_id as i64)?,"is_dirtgrade") as f32,
            "order"=>h.order as f32+1.0,"order_rate"=>{
                let threshold=(rhs as f32/100.0*self.horses.len() as f32+0.5) as i32;
                return Some((h.order as i32+1-threshold+rhs) as f32);
            },
            n if n.starts_with("order_rate_") && n.ends_with("_continue") => {
                let out=n.starts_with("order_rate_out");let rate:usize=n.trim_start_matches(if out {"order_rate_out"} else {"order_rate_in"}).trim_end_matches("_continue").parse().ok()?;
                boolean(h.order_continue[usize::from(out)][rate/10-1])
            },
            "is_behind_in"=>{
                match self.horses.iter().find(|o|o.order==h.order+1) {Some(b)=>boolean(b.lane<h.lane),None=>return None}
            },
            "hp_per"=>if h.max_hp>0.0 {(h.hp/h.max_hp*100.0) as i32 as f32} else {100.0},
            "is_last_straight_onetime"=>boolean(h.straight && c.final_corner().is_some_and(|s|h.distance>=s.end)),
            "base_speed"=>h.base[0],"base_stamina"=>h.base[1],"base_power"=>h.base[2],"base_guts"=>h.base[3],"base_wiz"=>h.base[4],
            "lastspurt"=>{
                let on=h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start;
                if !on {0.0} else if h.spurt_result==1 {2.0} else {1.0}
            }
            "is_lastspurt"=>boolean(h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start),
            "slope"=>if h.slope>0.0 {1.0} else if h.slope<0.0 {2.0} else {0.0},
            "is_badstart"=>boolean(h.original_delay>=0.08),"is_temptation"=>boolean(h.tempted()),
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
            "is_move_lane"=>if h.move_time<1.0 {0.0} else {(h.move_dir!=0) as i32 as f32+1.0},"blocked_front_continuetime"=>h.front_time as i32 as f32,"blocked_side_continuetime"=>h.side_time as i32 as f32,
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
            "lane_type"=>if h.lane<=0.2 {0.0} else if h.lane<=0.4 {1.0} else if h.lane<=0.6 {2.0} else {3.0},
            "is_activate_any_skill"=>h.activate_count as f32,
            "compete_fight_count"=>h.fight_count as f32,
            "change_order_onetime"=>(h.order as i32-h.prev_order as i32) as f32,
            "bashin_diff_infront"=>{let n=self.horses.iter().find(|n|n.order+1==h.order)?;(n.distance-h.distance).max(0.0)/2.5},
            "bashin_diff_behind"=>{let n=self.horses.iter().find(|n|n.order==h.order+1)?;(h.distance-n.distance).max(0.0)/2.5},
            "is_exist_chara_id"=>boolean(self.horses.iter().any(|n|n.input.chara_id==rhs)),
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
        for hi in 0..self.horses.len() {
            if priority==0 {self.horses[hi].mods.update_timed(dt);}
            let mut skills=std::mem::take(&mut self.horses[hi].skills);
            for skill in &mut skills {
                if ival(&skill.row,"priority")!=priority {continue;}
                if skill.details.iter().any(|d|d.active) {
                    for d in &mut skill.details {
                        if d.active && d.left>=0.0 {
                            {
                                let now=self.horses[hi].activate_count;
                                let pending=(now-d.additional_seen).max(0);
                                d.additional_seen=now;
                                for a in 1..=3 {
                                    if ival(&skill.row,&format!("additional_activate_type_{}_{}",d.number,a))!=3 {continue;}
                                    let cap=global_additional_cap(self.p(),3);
                                    let count=take_additional_count(pending,cap,&mut d.additional_used[a-1]);
                                    for _ in 0..count {
                                        self.ability_activate(hi,skill.level,&skill.row,d,a,true);
                                    }
                                }
                            }
                            d.left-=dt;
                            if d.left<=0.0 {
                                d.active=false;
                                stop_effect(&mut self.horses,d.uid);
                                skill.cooldown=ival(&skill.row,&format!("float_cooldown_time_{}",d.number)) as f32*0.0001*
                                    (self.course.distance/self.p().i("Skill.CoolDownTimeDivideDistance") as f32);
                            }
                        }
                    }
                    continue;
                }
                if skill.cooldown>0.0 {skill.cooldown=(skill.cooldown-dt).max(0.0);if skill.cooldown>0.0 {continue;}}
                for d in &mut skill.details {
                    if d.active && d.left>=0.0 {
                        d.left-=dt;
                        if d.left<=0.0 {
                            d.active=false;
                            stop_effect(&mut self.horses,d.uid);
                            d.cooldown=ival(&skill.row,&format!("float_cooldown_time_{}",d.number)) as f32*0.0001*
                                (self.course.distance/self.p().i("Skill.CoolDownTimeDivideDistance") as f32);
                        }
                    } else if d.cooldown>0.0 {d.cooldown=(d.cooldown-dt).max(0.0);}
                    let reserved=d.number==1 && self.horses[hi].reserved.contains(&skill.id);
                    if (!skill.enabled && !reserved) || d.active || d.cooldown>0.0 || self.horses[hi].finished {continue;}
                    self.cur_skill=skill.id;
                    if !d.pre_passed {d.pre_passed=triggers::evaluate(&mut d.pre,self.horses[hi].distance,&|n,r|self.query(hi,n,r));}
                    let activate=reserved || d.pre_passed && triggers::evaluate(&mut d.condition,self.horses[hi].distance,&|n,r|self.query(hi,n,r));
                    if activate {
                        let raw=ival(&skill.row,&format!("float_ability_time_{}",d.number));
                        d.left=if raw==-1 {-1.0} else {raw as f32*0.0001*(self.course.distance/self.p().i("Skill.AbilityTimeDivideDistance") as f32)};
                        let usage=ival(&skill.row,&format!("ability_time_usage_{}",d.number));
                        if usage==3 {
                            let mult=global_hp_duration_multiplier(self.p(),self.horses[hi].hp);
                            if raw!=-1 {d.left*=mult;}
                        }
                        d.active=true;
                        let h=&mut self.horses[hi];h.activate_count+=1;
                        h.activate_phase[h.phase.min(3)]+=1;
                        h.activate_half[usize::from(h.distance>=self.course.distance*0.5)]+=1;
                        d.additional_seen=h.activate_count;
                        let mut mask=0i64;
                        for a in 1..=3 {
                            let targets=self.ability(hi,skill.level,&skill.row,d,a);
                            if a==1 {
                                for t in targets {mask|=1<<t;}
                            }
                        }
                        let t0=self.time==0.0;
                        let enc=(d.left as f64*10000.0) as i32;
                        self.horses[hi].skill_time=self.time;
                        self.events.push(json!({"t":self.time,"type":3,"name":"Skill","params":[hi as i32,skill.id,if raw==-1 || t0 {-1} else {enc},d.number as i32-1,if t0 {0} else {mask},0],"horse":hi,"extra":""}));
                        break;
                    }
                }
            }
            if priority==0 {self.reserve_random(hi,&skills);}
            self.horses[hi].skills=skills;
        }
    }
    fn reserve_random(&mut self,hi:usize,skills:&[Skill]) {
        self.horses[hi].reserved.clear();
        for (index,rarity) in [(25usize,2i64),(24,1)] {
            let count=self.horses[hi].mods.consume(index,0.0) as i32;
            if count<=0 {continue;}
            let mut cand=skills.iter().filter(|k|ival(&k.row,"rarity")==rarity && ival(&k.row,"float_ability_time_1")!=-1 &&
                ival(&k.row,"is_general_skill")==1 && (k.cooldown<=0.0 && !k.details.iter().any(|d|d.active)) &&
                !self.horses[hi].reserved.contains(&k.id)).map(|k|k.id).collect::<Vec<_>>();
            for _ in 0..count.min(cand.len() as i32) {
                let i=self.random.int_range(0,cand.len() as i32) as usize;
                self.horses[hi].reserved.push(cand.remove(i));
            }
        }
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
            13=>{let mut v=vec![hi];if let Some(i)=self.horses[hi].front {v.push(i);}v},
            14=>[self.horses[hi].inside,self.horses[hi].outside].into_iter().flatten().collect(),
            17=>self.horses.iter().enumerate().filter(|(_,h)|h.input.running_style as i64==tv).map(|(i,_)|i).collect(),
            18=>self.horses.iter().enumerate().filter(|(i,h)|*i!=hi && h.input.running_style as i64==tv).map(|(i,_)|i).collect(),
            21=>self.horses.iter().enumerate().filter(|(i,h)|*i!=hi && h.input.running_style as i64==tv && h.tempted()).map(|(i,_)|i).collect(),
            22=>self.horses.iter().enumerate().filter(|(_,h)|h.input.chara_id as i64==tv).map(|(i,_)|i).collect(),
            _=>vec![],
        };
        let mut value=ival(row,&format!("float_ability_value_{suffix}")) as f32*0.0001;
        if value<0.0 {
            let team=self.horses[hi].input.team_id;
            targets.retain(|i|self.horses[*i].mods.apply(18,0.0)==0.0 && (*i==hi || team==0 || self.horses[*i].input.team_id==0 || self.horses[*i].input.team_id!=team));
        }
        let usage=ival(row,&format!("ability_value_usage_{suffix}"));
        let coef=self.master.rows("skill_level_value").iter().find(|r|ival(r,"ability_type")==kind && ival(r,"level")==level as i64)
            .map_or(1.0,|r|ival(r,"float_ability_value_coef") as f32*0.0001);
        match ival(row,&format!("ability_value_level_usage_{suffix}")) {1=>value=coef*value,3=>value=(1.0/coef)*value,_=>()};
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
        if matches!(usage,22|23) {
            let sp=self.horses[hi].stat(0);
            let mult=self.p().arr(if usage==22 {"Skill.SpeedAbilityValueCoefAscArray1"} else {"Skill.SpeedAbilityValueCoefAscArray2"}).iter()
                .find(|r|sp<r["SpeedThreshold"].as_i64().unwrap() as f32).map_or(1.0,|r|f32_of(&r["Multiply"]));
            value*=mult;
        }
        if ival(row,&format!("additional_activate_type_{suffix}"))!=0 {
            if !is_additional {return targets;}
        }
        if kind==9 && value>0.0 {self.horses[hi].heal_count+=1;}
        for &i in &targets {
            let h=&mut self.horses[i];let uid=detail.uid;
            let effect=match kind {
                1..=5=>Some((kind as usize-1,Operation::Add,false,value)),
                6=>Some((20,Operation::Set,false,1.0)),7=>Some((13,Operation::Set,false,value)),8=>Some((19,Operation::Add,false,value)),
                9=>{Some((12,Operation::Add,true,h.max_hp*value))},
                10=>Some((15,Operation::Set,true,value.max(0.0))),11=>Some((16,Operation::Set,false,1.0)),12=>Some((17,Operation::Set,false,1.0)),
                13=>Some((14,Operation::Set,true,value)),14=>Some((22,Operation::Set,true,value)),
                21=>Some((5,Operation::Add,false,value)),22=>{
                    Some((23,Operation::Add,false,value))},
                27=>Some((6,Operation::Add,false,value)),28=>{if value>0.0 {h.mods.add(21,1.0,Operation::Set,uid,false);}Some((7,Operation::Add,false,value))},
                29=>Some((8,Operation::Add,false,value)),30=>Some((9,Operation::Add,false,value)),31=>Some((10,Operation::Add,false,value)),
                32=>{for k in 0..5 {h.mods.add(k,value,Operation::Add,uid,false);}None},
                35=>Some((11,Operation::Add,false,value)),36=>Some((24,Operation::Set,true,value)),37=>Some((25,Operation::Set,true,value)),38=>Some((18,Operation::Set,false,1.0)),
                _=>None,
            };
            if let Some((index,op,once,value))=effect {
                if is_additional {
                    h.mods.add_timed(index,value,op,uid,once,detail.left);
                } else {h.mods.add(index,value,op,uid,once);}
            }
        }
        targets
    }
    fn ground_multi(&self)->f32 {
        let p=self.p();let a=p.arr(if self.course.ground==1 {"_groundModifierParamTurf"} else {"_groundModifierParamDirt"});
        f32_of(&a[self.input.ground_condition as usize-1]["multiHpSub"])
    }
    fn hp_rate(&self,h:&Horse,speed:f32,base:f32,rate:f32)->f32 {
        let p=self.p();
        let x=math::pow(speed-base+p.f("Hp.SpeedGapParam1"),2.0);
        let mut v=((x/p.f("Hp.SpeedGapParam1Pow")) as f64*p.f("Hp.HpDecBase") as f64*rate as f64*self.ground_multi() as f64) as f32;
        if h.phase>=2 {
            v*=(p.f("Hp.HpGutsCoef") as f64/(h.stat(3)*p.f("Hp.HpGutsCoefSqrt")).sqrt() as f64+1.0) as f32;
        }
        v
    }
    fn spurt(&mut self,hi:usize) {
        let h=&self.horses[hi];let p=self.p();let base=self.race_base();
        let normal=h.section_base.iter().enumerate().find(|(i,_)|self.course.phase(*i as f32*(self.course.distance/24.0))==h.phase).map_or(base,|(_,v)|*v);
        let max={
            let guts=math::pow(h.stat(3)*p.f("LastSpurtTargetSpeedGutsCoef1"),p.f("LastSpurtTargetSpeedGutsPow"));
            let speed=(h.stat(0)*p.f("LastSpurtTargetSpeedCoefSqrt")).sqrt();
            let cached_add=base*p.f("lastSpurtBaseTargetSpeedAddCoef");
            let base_arg=cached_add+normal;
            (guts as f64*p.f("LastSpurtTargetSpeedGutsCoef2") as f64+
                speed as f64*h.dist_speed as f64*p.f("addSpeedParamCoef") as f64+
                base_arg as f64*p.f("baseTargetSpeedCoef") as f64) as f32
        };
        let remain=(self.course.distance-h.distance-p.i("LastSpurtDistanceGoalBuffer") as f32).max(0.0);
        let need=|speed:f32|remain/speed*self.hp_rate(h,speed,base,p.f("Hp.HpDecRateBaseNormal"));
        let (result,start,speed)=if h.hp<need(h.min_speed) {(4,-1.0,base)}
            else if need(max)<=h.hp {(1,h.distance,max)}
            else {
                let candidate=|speed:f32|->Option<(f32,f32)> {
                    let spurt_rate=self.hp_rate(h,speed,base,p.f("Hp.HpDecRateBaseNormal"));
                    let normal_rate=self.hp_rate(h,normal,base,p.f("Hp.HpDecRateBaseNormal"));
                    let goal=self.course.distance-p.i("LastSpurtDistanceGoalBuffer") as f32;
                    let mut d=remain;
                    while d>0.0 {
                        let spurt_time=d/speed;let normal_time=(remain-d)/normal;
                        let fits=d as f64/speed as f64*spurt_rate as f64+(remain as f64-d as f64)/normal as f64*normal_rate as f64<=h.hp as f64;
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
                    netsort::sort(&mut candidates,&|a,b|((a.2-b.2)*10000.0) as i32);
                    let threshold=h.stat(4)*p.f("lastSpurtStep2PerVal2")+p.f("lastSpurtStep2PerVal1");
                    let mut chosen=candidates.last().copied().unwrap_or((h.distance,first.unwrap(),0.0));
                    for c in candidates {
                        if (self.random.int_range(0,100) as f32)<threshold {chosen=c;break;}
                    }
                    (2,chosen.0.max(h.distance),chosen.1)
                } else {(8,-1.0,base)}
            };
        let h=&mut self.horses[hi];
        if !h.spurt_calculated || result&3!=0 {
            h.spurt_result=result;h.spurt_speed=speed;
            if !h.spurt_calculated || h.spurt_start<0.0 {h.spurt_start=start;}
        } h.spurt_calculated=true;
    }
    fn around(&mut self,hi:usize) {
        let p=self.p();let h=&self.horses[hi];let mut front=None;let mut inside=None;let mut outside=None;let mut nearest=f32::MAX;let mut near=0;
        let side_gap=p.f("Block.SideBlockDistanceGap");
        let mut around=Vec::with_capacity(self.horses.len());let mut visible=vec![];
        let mut in_space=h.lane;let mut out_space=h.lane_max-h.lane;
        let vis=h.mods.apply(19,p.f("visibleDistance")).max(0.0);
        for (i,n) in self.horses.iter().enumerate() {
            if i==hi {continue;}
            let gap=n.distance-h.distance;let lane=n.lane-h.lane;let abs=lane.abs();
            around.push(Around{i,gap,gap_abs:gap.abs(),lane,lane_abs:abs,speed_gap:h.speed-n.speed});
            if gap.abs()<=p.f("Near.DistanceThreshold") && abs<=p.f("Near.LaneDistanceThreshold") {near+=1;}
            if gap>0.0 && gap<p.f("Block.FrontBlockDistanceGap") && abs<=(1.0-gap/p.f("Block.FrontBlockDistanceGap")*0.6)*p.f("Block.FrontBlockLaneGap") && gap<nearest {
                nearest=gap;front=Some(i);
            }
            if gap.abs()<side_gap && abs<p.f("Block.SideBlockLaneGap") && lane!=0.0 {
                if lane<0.0 {inside=Some(i);} else {outside=Some(i);}
            }
            if !(gap< -side_gap) {
                if gap.abs()<side_gap {
                    if lane<0.0 {if abs<in_space {in_space=abs;}} else if lane>0.0 && abs<out_space {out_space=abs;}
                }
                if vis>0.0 && gap>=0.0 && gap<=vis &&
                    abs<=((gap/vis)*f32::from_bits(0x3f238e39)+f32::from_bits(0x3de38e39))*0.5 {visible.push(i);}
            }
        }
        let h=&mut self.horses[hi];h.front=front;h.inside=inside;h.outside=outside;h.near_count=near;
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
            rank:h.order+1,final_straight:h.straight && c.final_corner().is_some_and(|s|h.distance>=s.end),tempted:h.tempted()}
    }
    fn compete_step(&mut self,dt:f32) {
        let runners=(0..self.horses.len()).map(|i|self.compete_runner(i,false)).collect::<Vec<_>>();
        let params=crate::compete::Params{
            fight_exit_inclusive: true,
            fight_select_before_history: true,
            top_cohort:true,top_end_exit:true,top_requires_lead:1,..Default::default()};
        let (mut next,mut changes)=crate::compete::step(&self.compete,&runners,self.course.distance,dt,&params);
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
                f=Some(if gap>fd {None} else {Some((l-a.lane).abs()<=fl)});
            }
            if o!=n-1 && o==po {
                let a=&self.horses[by_order[o+1]];let gap=d-a.distance;
                b=Some(if gap<=bd {Some((l-a.lane).abs()<=bl)} else {None});
            }
            let sets=self.assets.params().arr("Skill.BehindNearParamArray").iter().map(|r|(f32_of(&r["Distance"]),f32_of(&r["LaneDistance"]))).collect::<Vec<_>>();
            let behind=(o!=n-1 && o==po).then(||{let a=&self.horses[by_order[o+1]];(d-a.distance,(l-a.lane).abs())});
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
                if (self.random.int_max(100) as f32)<p.f("Temptation.TemptationEndPer") {h.temptation_mode=0;h.temptation_active=false;h.temptation_end=2;}
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
        }
    }
    fn downhill(&mut self,hi:usize,dt:f32) {
        let p=self.assets.params();let h=&mut self.horses[hi];
        if h.slope>=0.0 {if h.downhill {h.downhill=false;h.downhill_check=1.0;}return;}
        if h.downhill_check>0.0 {h.downhill_check-=dt;return;}
        h.downhill_check=1.0;
        let threshold=if h.downhill {p.f("Slope.DownSlopeAddSpeedEndPer")} else {(h.stat(4)*p.f("Slope.DownSlopeAddSpeedStartWizRate")).min(100.0)};
        if self.random.float_range(0.0,100.0)<threshold {h.downhill=!h.downhill;}
    }
    fn ot_check(&self,hi:usize,o:usize)->bool {
        let h=&self.horses[hi];
        let Some(a)=h.around.iter().find(|a|a.i==o).copied() else {return false};
        if a.gap< -1.0 {return false;}
        let other=&self.horses[o];
        let own_target=h.mods.apply(5,h.target_speed);
        if h.front==Some(o) {return own_target>other.speed;}
        if a.gap<1.0 || a.gap>20.0 || a.speed_gap<0.0 {return false;}
        if a.gap/a.speed_gap>self.p().f("overTakeDistPerSpeed") {return false;}
        own_target>if other.front.is_some() {other.speed} else {other.mods.apply(5,other.target_speed)}
    }
    fn update_overtake(&mut self,hi:usize,dt:f32) {
        let mut list=std::mem::take(&mut self.horses[hi].overtake);
        for v in self.horses[hi].visible.clone() {if self.ot_check(hi,v) && !list.contains(&v) {list.push(v);}}
        list.retain(|&o|self.ot_check(hi,o));
        if let Some(f)=self.horses[hi].front {if !list.contains(&f) {list.push(f);}}
        let cool=self.p().f("overTakeCoolDownTime");let h=&mut self.horses[hi];
        if !list.is_empty() {h.ot_cool=cool;}
        else if h.ot_cool>0.0 {h.ot_cool-=dt;if h.ot_cool<=0.0 {h.force_lane=true;}}
        h.overtake=list;
    }
    fn enable_in(&self,hi:usize)->bool {let h=&self.horses[hi];h.inside.is_none() && h.lane>0.0001}
    fn enable_out(&self,hi:usize)->bool {let h=&self.horses[hi];h.outside.is_none() && h.lane<h.lane_max}
    fn ot_enable(&self,hi:usize,l:f32)->bool {
        let h=&self.horses[hi];
        if l<0.0 || l>self.course.lane_max {return false;}
        let d=l-h.lane;
        if d<0.0 {self.enable_in(hi) && -d<=h.in_space} else if d<=0.0 {true} else {self.enable_out(hi) && d<=h.out_space}
    }
    fn ot_chain(&self,mut x:usize,inward:bool)->usize {
        let range=f32::from_bits(0x3de38e39);
        loop {
            let mut best=self.horses[x].lane;let mut next=None;
            for a in &self.horses[x].around {
                if a.gap>=0.0 && a.gap<=3.0 && (if inward {a.lane<0.0} else {a.lane>0.0}) && a.lane_abs<range {
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
        if h.phase<2 && self.enable_in(hi) && h.lane>c2 {
            if !self.ot_exist(hi,h.distance,min_dist+3.0,(h.lane-w)-c2,(h.lane+w)-c2) {cands.push(h.lane-c2);}
        } else if !self.ot_exist(hi,h.distance,min_dist+3.0,h.lane-w,h.lane+w) {cands.push(h.lane);}
        let (m_in,m_out)=(h.mods.apply(16,0.0)!=0.0,h.mods.apply(17,0.0)!=0.0);
        let mut chosen=h.lane;let mut best=f32::MAX;
        for c in cands {
            let d=h.lane-c;
            let phase=if h.phase==0 {if d>0.0 {100.0} else {1.0}} else if h.phase<2 {1.0}
                else if d>0.0 {p.f("overTakeOutLaneCoefEnd")} else {p.f("overTakeInLaneCoefEnd")};
            let skill=if m_in!=m_out && ((m_in && d>0.0) || (m_out && d<0.0)) {1.5} else {1.0};
            let cost=(phase*d*skill).abs();
            if cost<best && self.ot_enable(hi,c) {best=cost;chosen=c;}
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
        let enable_in=h.inside.is_none() && h.lane>0.0001;let enable_out=h.outside.is_none() && h.lane<h.lane_max;
        let end=self.course.final_corner().map(|c|c.start+(c.end-c.start)*p.f("LastMoveOutStartFinalCornerRate"));
        let blocked_target=(h.target_lane<h.lane && !enable_in) ||
            (h.target_lane>h.lane && !enable_out);
        if plan && !(self.course.empty_in_move && self.course.first_move>0.0) {return;}
        let overtaking=!h.overtake.is_empty();
        if plan && (h.force_lane || blocked_target || overtaking || (h.target_lane-h.lane).abs()<0.02777777798473835) {
            h.force_lane=false;h.target_lane=h.lane;
            if h.extra_lane.is_none() && end.is_some_and(|d|h.distance>=d) {
                let norm=p.f("lastMoveOutLaneNormalizeVal");let norm=if math::approximately(norm,0.0) {0.2} else {norm};
                h.extra_lane=Some(((h.lane/norm).clamp(0.0,1.0)*p.f("lastMoveOutLaneCoef")+
                    self.random.float_range(0.0,p.f("lastMoveOutLaneAddRandom"))).clamp(0.0,h.lane_max));
            }
            let fixed=h.mods.apply(11,0.0);
            let first=self.course.first_move;
            let use_ot=(!h.overtake.is_empty() || h.ot_cool>0.0) &&
                !(h.phase<2 && h.distance<=first && first-200.0<=h.distance);
            if use_ot {
                let t=self.ot_search(hi,fixed);
                let h=&mut self.horses[hi];h.target_lane=t.clamp(0.0,h.lane_max);
                return;
            }
            let h=&mut self.horses[hi];
            if !math::approximately(fixed,0.0) {
                h.target_lane=if fixed<h.lane {if enable_in {fixed.max(h.lane-0.05)} else {h.lane}}
                    else if h.lane<fixed {if enable_out {fixed.min(h.lane+0.05)} else {h.lane}} else {h.lane};
            } else if h.hp>0.0 {
                if h.keep.mode==4 {h.target_lane=p.f("PositionKeep.PositionKeepPaseDownTargetLane");}
                else if let Some(extra)=h.extra_lane {if extra>h.lane && enable_out {h.target_lane=extra.min(h.lane+0.05);}}
                else if h.phase<2 && enable_in {h.target_lane=h.lane-0.05;}
                else if h.phase==1 {
                    if let Some(lane)=inside_lane {
                        let gap=p.f("Block.SideBlockLaneGap");
                        let snap=h.inside.and_then(|i|h.around.iter().find(|a|a.i==i)).map_or((lane-h.lane).abs(),|a|a.lane_abs);
                        if snap<gap-f32::from_bits(0x3c638e39) {
                            h.target_lane=(lane+gap).min(h.lane+0.05);
                        }
                    }
                }
            }
            h.target_lane=h.target_lane.clamp(0.0,h.lane_max);
        }
        if plan {return;}
        let h=&mut self.horses[hi];
        h.prev_lane=h.lane;
        if math::approximately(h.lane,h.target_lane) {h.lane=h.lane.clamp(0.0,h.lane_max);h.move_time=0.0;return;}
        let prev_dir=h.move_dir;h.move_dir=(h.lane<=h.target_lane) as i32;
        if h.finished && h.after_goal>=1.0 {
            let old=h.lane;let m=if h.target_lane<old {-dt} else {dt};h.lane_move=m;
            let next=(dt as f64*m as f64+old as f64) as f32;
            let next=if h.target_lane<old {next.max(h.target_lane)} else {next.min(h.target_lane)}.clamp(0.0,h.lane_max);
            if (next>old && !enable_out) || (next<old && !enable_in) {return;}
            h.lane=next;if prev_dir==h.move_dir {h.move_time+=dt;}
            if math::approximately(h.lane,h.target_lane) {h.lane_speed=0.0;}
            return;
        }
        let mut base=((h.stat(2) as f64*p.f("laneMoveSpeedPowCoef") as f64+p.f("laneMoveSpeedAdd") as f64)*p.f("laneMoveSpeedBase") as f64) as f32;
        if h.phase<2 && h.distance<=self.course.first_move {
            let ratio=(h.lane/h.lane_max) as f64;
            base=(base as f64*(ratio*p.f("laneMoveSpeedOutCoef") as f64+1.0)) as f32;
        }
        if h.phase>=2 {base=(base as f64*(h.order as f64*0.01f32 as f64+1.0)) as f32;}
        let accel=p.f("laneMoveAccelMoveSpeedBaseCoef")*p.f("laneMoveSpeedBase");
        h.lane_speed=if h.lane_speed<base {((h.lane_speed as f64+accel as f64*dt as f64) as f32).min(base)} else {((h.lane_speed as f64-accel as f64*dt as f64) as f32).max(base)};
        h.lane_speed=h.lane_speed.max(0.0);
        let speed=h.mods.apply(7,h.lane_speed).clamp(0.0,0.6);
        let old=h.lane;
        if h.target_lane<old && enable_in {h.lane_move=((old as f64+1.0)*(-speed as f64)) as f32;}
        else if h.target_lane>old && enable_out {h.lane_move=speed;}
        {
            let m=if h.target_lane<old {((old as f64+1.0)*(-speed as f64)) as f32} else {speed};
            let next=(dt as f64*m as f64+old as f64) as f32;
            if h.target_lane<old && enable_in {h.lane=next.max(h.target_lane).clamp(0.0,h.lane_max);}
            else if h.target_lane>old && enable_out {h.lane=next.min(h.target_lane).clamp(0.0,h.lane_max);}
        }
        let moved=if h.target_lane<old {enable_in} else if h.target_lane>old {enable_out} else {true};
        if moved && !math::approximately(speed,0.0) && prev_dir==h.move_dir {h.move_time+=dt;}
        if math::approximately(h.lane,h.target_lane) {h.lane_speed=0.0;}
    }
    fn update_horse(&mut self,hi:usize,dt:f32) {
        let p=self.assets.params();let race_base=self.race_base();
        let default_min=race_base*p.f("Speed.MinSpeedRate");
        let front_min=self.horses[hi].front.map(|i|self.horses[i].min_speed);
        let h=&mut self.horses[hi];
        h.min_speed=if h.finished {5.0} else if h.start_dash {p.f("StartSpeed")} else {
            let v=(default_min as f64+(h.stat(3)*p.f("Speed.MinSpeedGutsCoefSqrt")).sqrt() as f64*
                p.f("Speed.MinSpeedGutsCoef") as f64) as f32;
            front_min.map_or(v,|m|v.min(m))
        };
        let heal=h.mods.consume(12,0.0);h.hp=(h.hp+heal).clamp(0.0,h.max_hp);
        let recalc=heal>0.0 && h.phase>=2;
        if h.delay>0.0 {h.delay-=dt;if h.delay>0.0 || dt==0.0 {return;}}
        if dt==0.0 {return;}
        if self.horses[hi].finished {
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
                let h=&mut self.horses[hi];let target=h.target_speed;
                let mut v=h.self_speed;
                if v<target {v=(v+dt).min(target);} else if v>target {v=(v-4.0*dt).max(target);}
                v=v.clamp(5.0,max.max(5.0));h.self_speed=v;h.speed=(h.mods.apply(23,0.0)+h.mods.apply(5,0.0)+v).clamp(5.0,max.max(5.0));
            }
            let plan=self.horses[hi].after_goal<1.0;
            if plan {self.update_overtake(hi,dt);self.move_lane(hi,dt,true);}
            self.move_lane(hi,dt,false);
            let h=&self.horses[hi];let mut movement=h.speed*dt;
            let position=self.geometry.world(h.distance,h.lane);let dx=position[0]-h.position[0];let dz=position[2]-h.position[2];
            let displacement=((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32;
            let cap=movement*self.geometry.distance_to_world;
            if cap<displacement {movement=((movement as f64)*(cap as f64)/(displacement as f64)) as f32;}
            let h=&mut self.horses[hi];
            h.position=position;h.prev_distance=h.distance;h.distance+=movement;
            return;
        }
        self.horses[hi].lane_move=0.0;
        {
            let runners=self.horses.iter().map(|h|crate::position_keep::Runner {
                distance:h.distance,order:h.order,style:h.input.running_style,ex:h.ex(),phase:h.phase,
                wiz:h.stat(4),temptation:if h.tempted() && h.temptation_mode==0 {3} else {h.temptation_mode},
                speed_skill:h.mods.apply(6,0.0)>0.0 || h.mods.apply(5,0.0)>0.0,
            }).collect::<Vec<_>>();
            self.horses[hi].keep.update(&Tree(p.get("PositionKeep")),self.course.distance as i32,24,
                &runners,hi,self.pacemaker,&mut self.random,dt,true);
        }
        if !self.horses[hi].cp.released {
            if self.compete.modes.get(hi).is_some_and(|m|m.top) {self.horses[hi].cp.top=true;}
            let cp=Tree(p.get("ConservePower"));
            {
                let h=&self.horses[hi];let style=h.input.running_style as i64;
                let sc=cp.arr("ConservePowerRunningStyleCoefArray").iter().find(|r|r["RunningStyle"].as_i64()==Some(style)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let rate=|m:i64|cp.arr("ConservePowerIncreaseCoefArray").iter().find(|r|r["Mode"].as_i64()==Some(m)).map_or(0.0,|r|f32_of(&r["Coef"]));
                let dec=|m:i64|cp.arr("ConservePowerDecreaseCoefArray").iter().find(|r|r["Mode"].as_i64()==Some(m)).map_or(1.0,|r|f32_of(&r["Coef"]));
                let (tm,tp,m)=(h.tempted(),self.compete.modes.get(hi).is_some_and(|m|m.top),h.keep.mode);
                let (r_pd,r_n,d_tp,d_tm)=(rate(1),rate(2),dec(3),dec(4));
                let cp_cool=cp.f("ConservePowerDefaultCoolDownTime");
                let end_kind=self.horses[hi].temptation_end;
                let c=&mut self.horses[hi].cp;
                if tm && !c.s_tm {c.spec-=c.last_credit;}
                if tm && !c.s_tm {c.spec*=d_tm;}
                c.last_credit=0.0;
                let rush_exit=!tm && c.s_tm && matches!(end_kind,1|2) && c.lag_mode==0;
                if tp && !c.s_tp {c.spec*=d_tp;}
                c.s_tm=tm;c.s_tp=tp;
                {
                    c.lag_mode=m;
                    if tm || tp || matches!(m,1|2|3|5) {c.ctimer=-cp_cool;}
                    else if self.horses[hi].distance<(self.course.distance as i32/24*10) as f32 {
                        let c=&mut self.horses[hi].cp;c.ctimer+=dt;if rush_exit {c.ctimer+=dt;}
                        if c.ctimer>=1.0 {let g=(if m==4 {r_pd} else {r_n})*sc;c.spec+=g;c.last_credit=g;c.ctimer-=1.0;}
                    }
                }
            }
        }
        self.update_overtake(hi,dt);
        self.temptation(hi,dt);
        self.move_lane(hi,dt,true);
        if self.horses[hi].phase>=2 && (!self.horses[hi].spurt_calculated || recalc) {self.spurt(hi);}
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
                    self.events.push(json!({"t":t,"type":6,"name":"ReleaseConservePower","params":[hi],"horse":hi,"extra":""}));
                }
            }
        }
        self.downhill(hi,dt);
        let section=({
            let length=(self.course.distance/24.0) as f64;
            (self.horses[hi].distance as f64/length) as usize
        }).min(23);
        let h=&self.horses[hi];
        let mut target=if h.spurt_start>0.0 && h.spurt_result&12==0 && h.distance>=h.spurt_start {h.spurt_speed} else {h.section_speed[section]};
        if h.keep.mode!=0 {target*=h.keep.multiply;}
        if self.course.empty_in_move && h.phase==0 && h.lane>=p.f("forceInMoveLaneThreshold") && (h.inside.is_some() || h.lane<=0.0001) {target+=h.force_in;}
        target=h.mods.apply(6,target);
        let ce=self.compete_effects(hi);let h=&self.horses[hi];
        if !math::approximately(h.lane,h.prev_lane) && h.mods.apply(21,0.0)!=0.0 {
            target+=math::pow(h.stat(2)*p.f("Skill.LaneMoveAddParam1"),p.f("Skill.LaneMoveAddParam2"));
        }
        if h.slope>0.0 {
            target+=(-h.slope.abs()*p.f("Slope.UpSlopeAddSpeedVal1")/h.stat(2)).min(0.0);
        }
        else if h.downhill {target+=(h.slope.abs()/p.f("Slope.DownSlopeAddSpeedVal2")+p.f("Slope.DownSlopeAddSpeedVal1")).max(0.0);}
        target+=ce.target_speed_add;
        if h.hp<=0.0 {target=h.min_speed;}
        target=target.clamp(p.f("Speed.TargetSpeedMin"),30.0);
        if h.finished {target=if h.after_goal<1.0 {h.target_speed} else if h.input.viewer_id!=0 {6.0} else {hi as f32*0.05+6.75};}
        let max=if let Some(i)=h.front {
            let front=&self.horses[i];
            {
                let g=h.around.iter().find(|a|a.i==i).map_or(front.distance-h.distance,|a|a.gap);
                let t=(g/p.f("Block.FrontBlockDistanceGap")).clamp(0.0,1.0);
                let (mx,mn)=(p.f("Block.FrontBlockMaxSpeedRateMax"),p.f("Block.FrontBlockMaxSpeedRateMin"));
                let v=front.speed*(((mx as f64-mn as f64)*t as f64+mn as f64) as f32);
                v.max(h.min_speed)
            }
        } else {30.0};
        let array=if h.ex() {"PhaseAccelCoefExArray[0]".into()} else {format!("PhaseAccelCoefArray[{}]",h.input.running_style-1)};
        let mut accel=if h.finished {1.0} else {
            ((h.stat(2)*p.f("AccelPowCoefSqrt")).sqrt() as f64*p.f(if h.slope>0.0 {"accelPowCoefUpSlope"} else {"accelPowCoef"}) as f64*
                h.ground_coef as f64*h.dist_power as f64*p.f(&format!("Speed.{array}.{}",PHASE_NAMES[h.phase.min(3)])) as f64) as f32
        };
        if !h.finished {accel=h.mods.apply(10,accel)+ce.acceleration_add;if h.cp.left>0.0 {accel+=h.cp.accel;}if h.start_dash {accel+=p.f("StartAccelAdd");}accel=accel.max(0.0);}
        let rate=if h.finished {4.0/p.f("declBase")} else {
            let rate=p.f(if h.keep.mode==4 {"DeclRatePositionKeepPaseDown"}
                else if h.phase==0 {"declRateStart"} else if h.phase==1 {"declRateMiddle"} else {"declRateEnd"});
            if h.hp<=0.0 {rate.max(p.f("declRateHpZero"))} else {rate}
        };
        self.move_lane(hi,dt,false);
        let h=&mut self.horses[hi];h.target_speed=target;
        let mut v=h.self_speed+h.mods.consume(23,0.0);
        if v<target {v=(v+accel*dt).min(target);} else if v>target {v=(v-rate*p.f("declBase")*dt).max(target);}
        if h.start_dash {v=v.min(default_min);}v=v.clamp(h.min_speed,max);
        h.self_speed=v;if h.cp.left>0.0 {h.cp.left-=dt;}
        h.speed=(h.mods.apply(23,0.0)+h.mods.apply(5,0.0)+v).clamp(h.min_speed,max);
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
        let drain=self.hp_rate(&self.horses[hi],v,drain_base,drain_rate);
        let h=&mut self.horses[hi];
        if !h.finished && h.hp>0.0 {h.hp=(h.hp-drain*dt).clamp(0.0,h.max_hp);}
        if !h.finished && h.hp<=0.0 {h.hp_empty=true;}
        h.phase=self.course.phase(h.distance);
        let position=self.geometry.world(h.distance,h.lane);
        let dx=position[0]-h.position[0];let dz=position[2]-h.position[2];
        let displacement=((dx as f64)*(dx as f64)+(dz as f64)*(dz as f64)).sqrt() as f32;
        let cap=movement*self.geometry.distance_to_world;
        if cap<displacement {
            movement=((movement as f64)*(cap as f64)/(displacement as f64)) as f32;
        }
        h.position=position;h.prev_distance=h.distance;h.distance+=movement;
        if h.speed>=default_min {h.start_dash=false;}
        if h.slope!=0.0 && h.distance>h.slope_end {h.slope=0.0;}
        if !h.finished && h.distance>self.course.distance {
            h.finished=true;
            let fraction=((h.distance-self.course.distance)/(h.distance-h.prev_distance)).clamp(0.0,1.0);
            h.raw_time=(self.time as f64-fraction as f64*dt as f64) as f32;h.scaled_time=h.raw_time*p.f("Global.FinishTimeCoef");
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
                    (2,[1,front,..])=>{h.straight=true;h.front_type=*front;h.corner=0;},
                    (2,[1])=>(),
                    (2,_)=>{h.straight=false;h.front_type=0;},
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
        for hi in 0..self.horses.len() {
            self.around(hi);let h=&mut self.horses[hi];
            if h.finished {h.after_goal+=dt;}
            if h.distance>h.corner_end {h.corner=0;}
        }
        if dt>0.0 {
            let style=|h:&Horse|if h.ex() {0} else {h.input.running_style};
            let sec=self.course.distance/24.0;
            let probe=self.horses.iter().map(|h|h.distance).fold(f32::MIN,f32::max);
            let s=(probe/sec) as i32;
            if s>self.pm_section {
                self.pm_section=s;
                let best=self.horses.iter().filter(|h|!h.finished).map(style).min().unwrap_or(1);
                let lead_i=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|h.order).map(|(i,_)|i);
                let keep=lead_i.is_some_and(|i|style(&self.horses[i])!=best);
                let ended=s>=self.p().f("PositionKeep.PositionKeepEndSection") as i32;
                if !keep && !ended {if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|style(h)==best).min_by_key(|(_,h)|h.prev_order) {self.pacemaker=i;}}
                let forward_gap=self.horses.iter().filter(|h|!h.finished).map(|h|h.distance).fold(f32::MIN,f32::max)-self.horses[self.pacemaker].distance;
                if forward_gap>self.p().f("PositionKeep.PaseMakerChangeForwardRunningStyleRange") {if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|h.order) {self.pacemaker=i;}}
                let runners=self.horses.iter().map(|h|crate::pacemaker::Runner {
                    style:style(h),order:h.order,distance:h.distance,finished:h.finished,
                }).collect::<Vec<_>>();
                if let Some(i)=crate::pacemaker::recalculate_with(&runners,&mut self.pm_not_forward,
                    self.assets.params().f("PositionKeep.PaseMakerChangeTopHorseNotMostForwardStyleCount") as i32,
                    self.assets.params().f("PositionKeep.PaseMakerChangeForwardRunningStyleRange"),crate::pacemaker::Rules {
                    early_forward:self.pm_handed,reset_on_forward:false,reset_on_recalc:true,order_first:true,forward_nige_only:true}) {
                    self.pacemaker=i;
                    if runners[i].style!=runners.iter().filter(|r|!r.finished).map(|r|r.style).min().unwrap_or(1) {self.pm_not_forward=0;self.pm_handed=true;}
                }
            }
        }
        self.skills_pass(dt,1);self.skills_pass(dt,0);
        for hi in 0..self.horses.len() {
            self.update_horse(hi,dt);
            if dt>0.0 {
                let o=self.horses[hi].order;
                let hit=self.horses.iter().any(|n|o<n.order && n.overtake.contains(&hi));
                let h=&mut self.horses[hi];h.ot_target_time=if hit {h.ot_target_time+dt} else {0.0};
            }
        }
        if dt>0.0 && (self.pm_handed || self.pm_not_forward>=self.p().f("PositionKeep.PaseMakerChangeTopHorseNotMostForwardStyleCount") as i32) {
            let st=|h:&Horse|if h.ex() {0} else {h.input.running_style};
            let best=self.horses.iter().filter(|h|!h.finished).map(st).min().unwrap_or(1);
            if let Some((i,_))=self.horses.iter().enumerate().filter(|(_,h)|!h.finished).min_by_key(|(_,h)|h.order) {
                if st(&self.horses[i])==best {self.pacemaker=i;self.pm_not_forward=0;self.pm_handed=false;}
            }
        }
        if dt>0.0 {for h in &mut self.horses {if h.front.is_some() {h.block_front_acc+=dt;}}}
        for h in &mut self.horses {
            h.front_time=if h.front.is_some() {h.front_time+dt} else {0.0};
            h.side_time=if h.inside.is_some() || h.outside.is_some() {h.side_time+dt} else {0.0};
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
        for (i,hi) in order.iter().copied().enumerate() {let h=&mut self.horses[hi];h.prev_order=h.order;h.order=i;}
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
        self.tick(0.0);let mut frames=vec![self.record()];let mut interval=0.0;let mut all=false;
        let mut rush_start=self.horses.iter().map(|h|if h.tempted() {Some(self.time)} else {None}).collect::<Vec<_>>();let mut rushing_events=Vec::new();
        for _ in 0..max_ticks {
            self.time+=DT;self.tick(DT);interval-=DT;all=self.horses.iter().all(|h|h.finished);
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
    match lane_type {
        1 if gate>8=>lane+=0.03333333507180214,
        3 if gate>13=>lane+=0.10333333164453506,
        4 if gate>7 && !half=>lane+=0.03333333507180214,
        _=>(),
    }
    lane
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
    params.arr("Skill.AdditionalActivateAbilityMaxCountArray").iter()
        .find(|r|r["AdditionalActivateType"].as_i64()==Some(kind))
        .map_or(0,|r|r["AdditionalActivateMaxCount"].as_i64().unwrap_or(0) as i32)
}

fn adjusted_stat(base:f32,coef:f32,ground:f32,career:f32,max:f32)->f32 {
    (base as f64*coef as f64+ground as f64+career as f64).clamp(1.0,max as f64) as f32
}

