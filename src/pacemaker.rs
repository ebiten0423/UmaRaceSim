#[derive(Clone, Copy)]
pub struct Runner {
    pub style: i32,
    pub order: usize,
    pub distance: f32,
    pub finished: bool,
}

#[derive(Clone, Copy)]
pub struct Rules {
    pub early_forward: bool,
    pub reset_on_forward: bool,
    pub reset_on_recalc: bool,
    pub order_first: bool,
    pub forward_nige_only: bool,
    pub most_forward_range: f32,
    pub pm_style: i32,
    pub hand_elapsed: i64,
}

impl Default for Rules {
    fn default() -> Self {
        Self { early_forward: false, reset_on_forward: true, reset_on_recalc: false, order_first: false, forward_nige_only: false, most_forward_range: 0.0, pm_style: 0, hand_elapsed: -1 }
    }
}

pub fn recalculate_with(runners: &[Runner], count: &mut i32, threshold: i32,
                   forward_range: f32, rules: Rules) -> Option<usize> {
    let forward = runners.iter().filter(|h| !h.finished).map(|h| h.style).min()?;
    let leader = runners.iter().enumerate().filter(|(_, h)| !h.finished)
        .min_by_key(|(i, h)| (h.order, *i)).map(|(i, _)| i)?;
    if crate::envc::var_os("JPRACE_DEBUG_PM").is_some() {
        let fb = runners.iter().enumerate().filter(|(_, h)| !h.finished && h.style == forward).max_by(|a, b| a.1.distance.total_cmp(&b.1.distance));
        eprintln!("pmrecalc leader={} style={} forward={} count={} fwd_best={:?} gap={:.3}", leader, runners[leader].style, forward, *count, fb.map(|x| x.0), fb.map_or(0.0, |x| runners[leader].distance - x.1.distance)); }
    if runners[leader].style == forward {
        if rules.reset_on_forward { *count = 0; }
        return None;
    }
    let mfr_lead_ok = crate::envc::var("JPRACE_PM_MFR_LEAD").ok().and_then(|x| x.parse::<i32>().ok()).is_none_or(|n| runners[leader].style <= n);
    if rules.most_forward_range > 0.0 && mfr_lead_ok {
        let fwd_best = runners.iter().filter(|h| !h.finished && h.style == forward).map(|h| h.distance).fold(f32::MIN, f32::max);
        if runners[leader].distance - fwd_best > rules.most_forward_range {
            if crate::envc::var("JPRACE_PM_MFR_KEEPCNT").as_deref() == Ok("0") { *count = 0; }
            if runners[leader].style >= 3 && crate::envc::var("JPRACE_PM_MFR_CAND").as_deref() != Ok("0") {
                let lead = runners[leader];
                return Some(runners.iter().enumerate().filter(|(_, h)| !h.finished && h.style < lead.style && lead.distance - h.distance <= forward_range)
                    .min_by_key(|(i, h)| (0, h.order, *i)).map_or(leader, |(i, _)| i));
            }
            return Some(leader);
        }
    }
    if rules.early_forward && crate::envc::var("JPRACE_PM_HCOUNT").as_deref() == Ok("0") { return None; }
    *count = count.saturating_add(1);
    let hm = if rules.early_forward { crate::envc::var("JPRACE_PM_HANDED").ok() } else { None };
    let htime = rules.early_forward && rules.hand_elapsed >= 0 && crate::envc::var("JPRACE_PM_HTIME").ok().and_then(|x| x.parse::<i64>().ok()).is_some_and(|p| rules.hand_elapsed >= p);
    let below = *count < threshold && !(htime || hm.as_deref() == Some("now") || (hm.as_deref() == Some("same") && runners[leader].style == rules.pm_style));
    if below && !rules.early_forward { return None; }
    if below && crate::envc::var("JPRACE_PM_BACK").as_deref() == Ok("1") {
        return runners.iter().enumerate().filter(|(_, h)| !h.finished && h.style == forward).min_by_key(|(i, h)| (h.order, *i)).map(|(i, _)| i);
    }
    if !below && rules.reset_on_recalc { *count = 0; }
    let pick = |i: usize| if below && (runners[i].style != forward || rules.forward_nige_only && forward > 1) { None } else { Some(i) };
    if runners[leader].style < 3 { return pick(leader); }
    let lead = runners[leader];
    let by_order = rules.order_first || runners.iter().filter(|h| !h.finished && h.style == forward).count() == 1;
    Some(runners.iter().enumerate().filter(|(_, h)| !h.finished && h.style < lead.style
        && lead.distance - h.distance <= forward_range)
        .min_by_key(|(i, h)| if by_order { (0, h.order, *i) } else { (h.style, h.order, *i) }).map_or(leader, |(i, _)| i)).and_then(pick)
}

