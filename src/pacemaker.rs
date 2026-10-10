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
}

impl Default for Rules {
    fn default() -> Self {
        Self { early_forward: false, reset_on_forward: true, reset_on_recalc: false, order_first: false, forward_nige_only: false }
    }
}

pub fn recalculate_with(runners: &[Runner], count: &mut i32, threshold: i32,
                   forward_range: f32, rules: Rules) -> Option<usize> {
    let forward = runners.iter().filter(|h| !h.finished).map(|h| h.style).min()?;
    let leader = runners.iter().enumerate().filter(|(_, h)| !h.finished)
        .min_by_key(|(i, h)| (h.order, *i)).map(|(i, _)| i)?;
    if runners[leader].style == forward {
        if rules.reset_on_forward { *count = 0; }
        return None;
    }
    *count = count.saturating_add(1);
    let below = *count < threshold;
    if below && !rules.early_forward { return None; }
    if !below && rules.reset_on_recalc { *count = 0; }
    let pick = |i: usize| if below && (runners[i].style != forward || rules.forward_nige_only && forward > 1) { None } else { Some(i) };
    if runners[leader].style < 3 { return pick(leader); }
    let lead = runners[leader];
    let by_order = rules.order_first || runners.iter().filter(|h| !h.finished && h.style == forward).count() == 1;
    Some(runners.iter().enumerate().filter(|(_, h)| !h.finished && h.style < lead.style
        && lead.distance - h.distance <= forward_range)
        .min_by_key(|(i, h)| if by_order { (0, h.order, *i) } else { (h.style, h.order, *i) }).map_or(leader, |(i, _)| i)).and_then(pick)
}

