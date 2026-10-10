pub const TICK: f32 = 0.0666;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style { Other, Nige, Oonige }

#[derive(Clone, Copy, Debug)]
pub struct Runner {
    pub style: Style,
    pub distance: f32,
    pub lane: f32,
    pub speed: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub guts: f32,
    pub strategy_proficiency: f32,
    pub rank: usize,
    pub final_straight: bool,
    pub tempted: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub start_distance: f32, pub check_end_section: u32, pub end_section: u32,
    pub top_distance: f32, pub top_lane: f32,
    pub top_exit_distance: f32, pub top_exit_lane: f32,
    pub top_time: [f32; 3], pub top_speed: [f32; 3],
    pub fight_distance: f32, pub fight_lane: f32, pub fight_time: f32,
    pub fight_speed_gap: f32, pub fight_order_percent: usize,
    pub fight_exit_distance: f32, pub fight_entry_hp_percent: f32,
    pub fight_exit_hp_percent: f32,
    pub fight_exit_inclusive: bool,
    pub fight_select_before_history: bool,
    pub fight_speed: [f32; 3], pub fight_accel: [f32; 3],
    pub top_hp: [f32; 4],
    pub top_requires_lead: u8,
    pub top_cohort: bool,
    pub top_end_exit: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            start_distance: 150.0, check_end_section: 6, end_section: 9,
            top_distance: 3.75, top_lane: 0.165,
            top_exit_distance: 5.0, top_exit_lane: 0.416,
            top_time: [700.0, 0.5, 0.012], top_speed: [500.0, 0.6, 0.0001],
            fight_distance: 3.0, fight_lane: 0.25, fight_time: 2.0,
            fight_speed_gap: 0.6, fight_order_percent: 50,
            fight_exit_distance: 5.0, fight_entry_hp_percent: 15.0,
            fight_exit_hp_percent: 5.0, fight_exit_inclusive: true, fight_select_before_history: true,
            fight_speed: [200.0, 0.708, 0.0001],
            fight_accel: [160.0, 0.59, 0.0001], top_hp: [1.4, 3.6, 3.5, 7.7],
            top_requires_lead: 1, top_cohort: false, top_end_exit: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mode {
    pub top: bool, pub top_remaining: f32, pub top_group: Option<usize>,
    pub top_distance_exit: bool,
    pub fight: bool, pub fight_count: u32,
    pub fight_group: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct State {
    pub modes: Vec<Mode>,
    pub top_fired: [bool; 2],
    pub near: Vec<Vec<Option<f32>>>,
    pub enter_seq: Vec<usize>,
}
impl State {
    pub fn new(count: usize) -> Self {
        Self { modes: vec![Mode::default(); count], top_fired: [false; 2],
            near: vec![vec![None; count]; count], enter_seq: vec![] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change { TopEnter(usize), TopExit(usize), FightEnter(usize), FightExit(usize) }

fn formula(guts: f32, c: [f32; 3]) -> f32 { (c[0] * guts.max(0.0)).powf(c[1]) * c[2] }
pub fn top_duration(r: &Runner, p: &Params) -> f32 {
    formula(r.guts, p.top_time) * r.strategy_proficiency
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    pub target_speed_add: f32, pub acceleration_add: f32,
    pub hp_base_rate: Option<f32>,
}
pub fn effects(r: &Runner, m: &Mode, p: &Params) -> Effects {
    let hp_base_rate = if m.top {
        Some(p.top_hp[match (r.style, r.tempted) {
            (Style::Oonige, false) => 2, (Style::Oonige, true) => 3,
            (_, false) => 0, (_, true) => 1,
        }])
    } else { None };
    Effects {
        target_speed_add: if m.top { formula(r.guts, p.top_speed) } else { 0.0 }
            + if m.fight { formula(r.guts, p.fight_speed) } else { 0.0 },
        acceleration_add: if m.fight { formula(r.guts, p.fight_accel) } else { 0.0 },
        hp_base_rate,
    }
}

pub fn step(old: &State, runners: &[Runner], course_distance: f32, dt: f32,
            p: &Params) -> (State, Vec<Change>) {
    let n = runners.len();
    assert_eq!(old.modes.len(), n);
    assert!(course_distance > 0.0 && dt >= 0.0 && dt.is_finite());
    let mut next = old.clone();
    next.enter_seq.clear();
    let mut changes = Vec::new();
    let end = course_distance / 24.0 * (p.end_section - 1) as f32;
    for i in 0..n {
        let r = &runners[i]; let m = &old.modes[i];
        if m.top {
            next.modes[i].top_remaining -= dt;
            if next.modes[i].top_remaining <= 0.0 || r.distance >= end {
                next.modes[i].top = false;
                if p.top_end_exit && r.distance >= end { next.modes[i].top_distance_exit = true; }
            } else {
                let peers: Vec<usize> = (0..n).filter(|&j| j != i &&
                    (p.top_cohort || old.modes[j].top) && old.modes[j].top_group == m.top_group).collect();
                if !peers.is_empty() && peers.iter().all(|&j|
                    (runners[j].distance - r.distance).abs() >= p.top_exit_distance ||
                    (runners[j].lane - r.lane).abs() >= p.top_exit_lane) {
                    next.modes[i].top = false;
                    next.modes[i].top_distance_exit = true;
                }
            }
        }
        if m.fight && (r.hp < r.max_hp * p.fight_exit_hp_percent / 100.0 ||
            !(0..n).any(|j| j != i && old.modes[j].fight_count > 0 &&
                old.modes[j].fight_group == m.fight_group &&
                (if p.fight_exit_inclusive {
                    (runners[j].distance - r.distance).abs() <= p.fight_exit_distance
                } else {
                    (runners[j].distance - r.distance).abs() < p.fight_exit_distance
                }))) {
            next.modes[i].fight = false;
        }
        for j in 0..n {
            let q = &runners[j];
            let close = i != j && (r.final_straight || q.final_straight) &&
                (r.distance - q.distance).abs() <= p.fight_distance &&
                (r.lane - q.lane).abs() <= p.fight_lane;
            next.near[i][j] = if close {
                Some(old.near[i][j].map_or(0.0, |t| t + dt))
            } else { None };
        }
    }
    for group in 0..2 {
        let members: Vec<usize> = (0..n).filter(|&i| old.modes[i].top_group == Some(group)).collect();
        let live: Vec<usize> = members.iter().copied().filter(|&i| next.modes[i].top).collect();
        if live.len() == 1 && members.len() > 1 && members.iter().all(|&i|
            i == live[0] || next.modes[i].top_distance_exit) {
            next.modes[live[0]].top = false;
            next.modes[live[0]].top_distance_exit = true;
        }
    }
    if runners.iter().any(|r| r.distance >= p.start_distance) {
        for (group, style) in [Style::Nige, Style::Oonige].into_iter().enumerate() {
            if next.top_fired[group] { continue; }
            let leader = (0..n).filter(|&i| runners[i].style == style).min_by_key(|&i| (runners[i].rank, i));
            if let Some(lead) = leader {
                let leads = match p.top_requires_lead {
                    1 => runners[lead].rank == 1,
                    2 => runners.iter().all(|q| q.distance <= runners[lead].distance),
                    _ => true,
                };
                if !leads { continue; }
                let members: Vec<usize> = (0..n).filter(|&i| {
                    let r = &runners[i]; let q = &runners[lead];
                    r.style == style && q.distance - r.distance < p.top_distance &&
                        (q.lane - r.lane).abs() <= p.top_lane
                }).collect();
                if members.len() > 1 && members.iter().any(|&i| runners[i].distance <
                    course_distance / 24.0 * p.check_end_section as f32) {
                    next.top_fired[group] = true;
                    for i in members {
                        next.modes[i].top = true;
                        next.modes[i].top_group = Some(group);
                        next.modes[i].top_remaining = top_duration(&runners[i], p) - dt;
                    }
                }
            }
        }
    }
    let top_rank = (n * p.fight_order_percent).div_ceil(100);
    let in_rank = |j: usize| runners[j].rank <= top_rank;
    let hp_ok = |i: usize| runners[i].hp >= runners[i].max_hp * p.fight_entry_hp_percent / 100.0;
    let target_ok: Vec<bool> = (0..n).map(|j| hp_ok(j) &&
        (old.modes[j].fight || old.modes[j].fight_count == 0) &&
        (!old.modes[j].fight || next.modes[j].fight)).collect();
    let can_enter = |i: usize| old.modes[i].fight_count == 0 && hp_ok(i);
    let candidate = |i: usize, j: usize| i != j && can_enter(i) && hp_ok(j) && (in_rank(i) || in_rank(j)) &&
        next.near[i][j].is_some_and(|t| t >= p.fight_time) &&
        (runners[i].speed - runners[j].speed).abs() <= p.fight_speed_gap;
    let q = |i: usize, j: usize| candidate(i,j) && target_ok[j];
    for i in 0..n {
        if !can_enter(i) { continue; }
        let qualified: Vec<usize> = (0..n).filter(|&j|
            if p.fight_select_before_history {candidate(i,j)} else {q(i, j)}).collect();
        let targets = qualified.iter().copied().min_by(|&a, &b| next.near[i][b].unwrap_or(0.0).total_cmp(&next.near[i][a].unwrap_or(0.0)).then(a.cmp(&b)));
        if let Some(j) = targets {
            if !target_ok[j] {continue;}
            let g = next.modes[j].fight_group.or(next.modes[i].fight_group).unwrap_or(i.min(j));
            for k in [i, j] {
                if next.modes[k].fight_group.is_none() { next.modes[k].fight_group = Some(g); }
                if !next.modes[k].fight {
                    next.modes[k].fight = true;
                    next.modes[k].fight_count += 1;
                    next.enter_seq.push(k);
                }
            }
        }
    }
    for i in 0..n {
        if old.modes[i].top != next.modes[i].top {
            changes.push(if next.modes[i].top { Change::TopEnter(i) } else { Change::TopExit(i) });
        }
        if old.modes[i].fight != next.modes[i].fight {
            changes.push(if next.modes[i].fight { Change::FightEnter(i) } else { Change::FightExit(i) });
        }
    }
    (next, changes)
}

