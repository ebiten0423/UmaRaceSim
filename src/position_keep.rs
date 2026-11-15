use crate::{data::Tree, math, rng::RaceRandom};
use std::sync::atomic::{AtomicBool, Ordering};

pub static NO_CHECK: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub struct Runner {
    pub distance: f32,
    pub order: usize,
    pub style: i32,
    pub ex: bool,
    pub phase: usize,
    pub wiz: f32,
    pub temptation: i32,
    pub speed_skill: bool,
    pub speed_skill22: bool,
}

#[derive(Clone, Default)]
pub struct Keep {
    pub mode: i32,
    pub count: i32,
    pub multiply: f32,
    pub checked: i32,
    pub wanted: i32,
    pub cooldown: f32,
    pub interval: f32,
    pub end_distance: f32,
    pub target_gap: f32,
    was_tempted: bool,
    pub force_once: bool,
    pub end_pending: bool,
    dbg_prev_gap: f32,
}

fn offset(p: &Tree<'_>, distance: i32, style: i32) -> (f32, f32) {
    let scale = ((distance - p.i("PositionKeepPaseBaseDistance")) as f32
        * p.f("PositionKeepPaseBaseDistanceCoef") + 1.0).max(1.0);
    let name = match style { 2 => "Senko", 3 => "Sashi", 4 => "Oikomi", _ => return (0.0, 0.0) };
    let min = p.f(&format!("PositionKeepPaseUpDownDistanceDiffMin{name}"));
    let max = p.f(&format!("PositionKeepPaseUpDownDistanceDiffMax{name}"));
    (if style == 2 { min } else { scale * min }, scale * max)
}

impl Keep {
    pub fn force_ready(&mut self, reset_mode: bool) {
        self.cooldown = 0.0; self.interval = 0.0;
        if reset_mode { self.mode = 0; }
    }

    pub fn clear(&mut self, p: &Tree<'_>) {
        self.mode = 0; self.end_pending = false;
        self.multiply = 1.0;
        self.cooldown = p.i("PositionKeepCoolDownSec") as f32;
    }

    pub fn in_section(p: &Tree<'_>, distance: i32, sections: i32, position: f32) -> bool {
        let section = (distance / sections) as f32;
        position >= (p.i("PositionKeepStartSection") - 1) as f32 * section
            && position < crate::envc::var("JPRACE_PK_END").ok().and_then(|x| x.parse::<f32>().ok()).unwrap_or(p.i("PositionKeepEndSection") as f32) * section
    }

    pub fn update(&mut self, p: &Tree<'_>, distance: i32, sections: i32,
                  runners: &[Runner], owner: usize, pacemaker: usize,
                  random: &mut RaceRandom, dt: f32, pacemaker_ready: bool) {
        let h = runners[owner];
        let pace = runners[pacemaker];
        let tempted = h.temptation != 0;
        if self.was_tempted != tempted {
            self.clear(p);
            self.cooldown = 0.0;
            self.interval = 0.0;
            self.was_tempted = tempted;
        }
        let style = if tempted {
            match h.temptation { 1 => 3, 2 => 2, _ => 1 }
        } else { h.style };
        let backward_style_ahead = runners.iter().any(|other| other.order < h.order && other.style > h.style);
        let behind = runners.iter().find(|other| other.order == h.order + 1);
        let ex_suffix = if h.ex { "Oonige" } else { "" };
        let speed_up_end = if h.ex {
            p.f("PositionKeepSpeedUpEndDistanceDiffOonige")
        } else if !(pacemaker == owner && h.style != 1)
            && runners.iter().filter(|other| other.style == 1 && !other.ex).count() == 1 {
            p.f("PositionKeepSpeedUpEndDistanceDiffOne")
        } else { p.f("PositionKeepSpeedUpEndDistanceDiff") };
        let force_rush = tempted;
        let far_behind = behind.map_or(!force_rush, |other| h.distance - other.distance >= speed_up_end);
        let gap = pace.distance - h.distance;
        self.checked = -1;
        self.wanted = -1;
        if self.mode == 0 && Self::in_section(p, distance, sections, h.distance) && !NO_CHECK.load(Ordering::Relaxed) {
            let force = std::mem::take(&mut self.force_once);
            if !force {
            if self.cooldown > 0.0 { self.cooldown -= dt; }
            if self.cooldown > 0.0 { return; }
            if self.interval > 0.0 { self.interval -= dt; }
            if self.interval > 0.0 { return; }
            self.interval = p.i("PositionKeepCheckIntervalSec") as f32;
            }
            self.checked = 0;
            let mode = if style == 1 {
                if tempted { 1 } else if backward_style_ahead { 5 }
                else if h.order == 0 { 1 } else { 2 }
            } else if !pacemaker_ready { 0 }
            else if pace.order < h.order && h.style < pace.style { 5 }
            else {
                let (min, max) = offset(p, distance, style);
                if gap < min {
                    let same = pace.style == h.style && !runners.iter().any(|o| o.style == 1);
                    let ahead = match crate::envc::var("JPRACE_PD_AHEAD").as_deref() { Ok("ord") => h.order < pace.order, Ok(_) => gap < 0.0, Err(_) => false };
                    if pacemaker == owner || same || ahead { 0 } else { 4 }
                }
                else if gap > max { 3 } else { 0 }
            };
            self.wanted = mode;
            if crate::envc::var_os("JPRACE_LOG_PKCHK").is_some() { let (mn, mx) = offset(p, distance, style); eprintln!("pkchk tick={} owner={} pm={} mode={} gap={:.3} min={:.2} max={:.2} d={:.3} ord={} style={} tempt={} pmord={} pmstyle={}", crate::rng::CUR_TICK.load(std::sync::atomic::Ordering::Relaxed), owner, pacemaker, mode, gap, mn, mx, h.distance, h.order, style, h.temptation, pace.order, pace.style); }
            let s22 = crate::envc::var("JPRACE_22SKILL").ok();
            if mode == 0 || mode == 1 && far_behind || mode == 4 && (h.speed_skill || s22.as_deref() == Some("start") && h.speed_skill22) { return; }
            if !tempted && matches!(mode, 1 | 2 | 3) {
                let key = match mode {
                    1 => format!("PositionKeepSpeedUpStartPerVal1{ex_suffix}"),
                    2 => format!("PositionKeepOvertakeStartPerVal1{ex_suffix}"),
                    _ => "PositionKeepPaseUpStartPerVal1".into(),
                };
                let chance = math::log10(h.wiz * 0.1) * p.f(&key);
                let call = random.calls() + 1;
                let u = random.float_max(100.0);
                if crate::envc::var_os("JPRACE_LOG_PKDRAW").is_some() { eprintln!("pkdraw owner={} mode={} chance={:.3} u={:.3} call={} ok={}", owner, mode, chance, u, call, chance > u); }
                let win = crate::envc::var("JPRACE_PK_WIN").ok().is_some_and(|s| s.split(',').any(|kv| kv.split_once(':').is_some_and(|(a, b)| a.parse::<usize>().ok() == Some(owner) && b.parse::<u64>().ok() == Some(call as u64))));
                if chance <= u && !win { return; }
            }
            if crate::envc::var("JPRACE_PK_MAXCOUNT").ok().and_then(|x| x.parse::<i32>().ok()).is_some_and(|n| self.count >= n) { return; }
            self.mode = mode;
            self.checked = mode;
            self.count += 1;
            let continue_sections = if h.ex {
                p.arr("PositionKeepContinueSectionExArray")[0].as_i64().unwrap() as i32
            } else { p.i("PositionKeepContinueSection") };
            self.end_distance = h.distance + (distance / sections * continue_sections) as f32;
            self.multiply = match mode {
                1 => p.f(&format!("PositionKeepSpeedUpBaseTargetSpeedMultiply{ex_suffix}")),
                2 => p.f(&format!("PositionKeepOvertakeBaseTargetSpeedMultiply{ex_suffix}")),
                3 => p.f("PositionKeepPaseUpBaseTargetSpeedMultiply"),
                4 => p.f(if h.phase == 1 { "PositionKeepPaseDownBaseTargetSpeedMultiplyPhaseMiddle" }
                         else { "PositionKeepPaseDownBaseTargetSpeedMultiply" }),
                5 => p.f("PositionKeepPaseUpExBaseTargetSpeedMultiply"),
                _ => 1.0,
            };
            if matches!(mode, 3 | 4) {
                let (min, mut max) = offset(p, distance, style);
                if mode == 4 && h.phase == 1 {
                    max = (max - min) * p.f("PositionKeepPaseUpDownDistanceDiffMaxCoef").clamp(0.0, 1.0) + min;
                }
                self.target_gap = random.float_range(min, max);
                if let Ok(s) = crate::envc::var("JPRACE_PK_TGSET2") {
                    for kv in s.split(',') {
                        if let Some((a, b)) = kv.split_once(':') && let Some((h, n)) = a.split_once('@') && h.parse::<usize>().ok() == Some(owner) && n.parse::<i32>().ok() == Some(self.count) && let Ok(v) = b.parse::<f32>() { self.target_gap = v; }
                    }
                }
                if let Ok(s) = crate::envc::var("JPRACE_PK_TGSET") && self.count == 1 {
                    for kv in s.split(',') {
                        if let Some((a, b)) = kv.split_once(':') && a.parse::<usize>().ok() == Some(owner) && let Ok(v) = b.parse::<f32>() { self.target_gap = v; }
                    }
                }
                if let Ok(m) = crate::envc::var("JPRACE_PK_EXTRA") {
                    let sec = distance as f32 / sections as f32;
                    let late = runners.iter().any(|r| r.distance >= 10.0 * sec);
                    let sec10 = h.distance >= 9.0 * sec;
                    let ext = self.end_distance >= 10.0 * sec;
                    let hit = m.split('+').all(|c| match c { "late" => late, "sec10" => sec10, "ext" => ext, "all" => true, "m3" => mode == 3, "m4" => mode == 4, _ => false });
                    if hit {
                        let call = random.calls() + 1; let u = random.float_max(100.0);
                        if crate::envc::var_os("JPRACE_LOG_PKDRAW").is_some() { eprintln!("pkextra owner={} mode={} call={} u={:.3} d={:.2} end={:.2}", owner, mode, call, u, h.distance, self.end_distance); }
                    }
                }
            }
        }
        let out_sec = crate::envc::var("JPRACE_PK_ENDSEC").as_deref() == Ok("1")
            && h.distance >= p.i("PositionKeepEndSection") as f32 * (distance as f32 / sections as f32);
        let end = out_sec || h.distance >= self.end_distance || match self.mode {
            1 => far_behind,
            2 if h.order == 0 => behind.is_none_or(|other| h.distance - other.distance
                >= p.f(&format!("PositionKeepOvertakeEndDistanceDiff{ex_suffix}"))),
            3 => if crate::envc::var_os("JPRACE_PK_ENDLAG").is_some() {let e=self.end_pending;self.end_pending=gap <= self.target_gap;e} else {gap <= self.target_gap},
            4 => pacemaker == owner || h.speed_skill || crate::envc::var("JPRACE_22SKILL").as_deref() == Ok("end") && h.speed_skill22 || gap >= self.target_gap,
            5 => !backward_style_ahead,
            _ => false,
        };
        if self.mode != 0 && crate::envc::var("JPRACE_LOG_PKEND").ok().and_then(|x| x.parse::<usize>().ok()) == Some(owner) {
            eprintln!("pkend owner={} mode={} end={} gap={:.5} tg={:.5} d={:.4} endd={:.4} pm={}", owner, self.mode, end, gap, self.target_gap, h.distance, self.end_distance, pacemaker);
        }
        if self.mode != 0 && crate::envc::var_os("JPRACE_LOG_PKENDALL").is_some() && end {
            eprintln!("pkallend owner={} mode={} gap={:.5} prev={:.5} tg={:.5} d={:.4} endd={:.4} pm={} bydist={}", owner, self.mode, gap, self.dbg_prev_gap, self.target_gap, h.distance, self.end_distance, pacemaker, h.distance >= self.end_distance);
        }
        self.dbg_prev_gap = gap;
        if self.mode != 0 && end { self.clear(p); }
    }
}
