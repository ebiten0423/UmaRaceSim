const MBIG: i32 = 2147483647;
const MSEED: i32 = 161803398;
static INSERT: std::sync::OnceLock<Vec<u64>> = std::sync::OnceLock::new();
static SKIP: std::sync::OnceLock<Vec<u64>> = std::sync::OnceLock::new();
pub static CUR_TICK: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);

#[derive(Clone)]
pub struct NetRandom {
    sa: [i32; 56],
    inext: usize,
    inextp: usize,
    pub calls: u64,
}

impl NetRandom {
    pub fn new(seed: i32) -> Self {
        let subtraction = if seed == i32::MIN { MBIG } else { seed.wrapping_abs() };
        let mut mj = MSEED.wrapping_sub(subtraction);
        let mut sa = [0i32; 56];
        sa[55] = mj;
        let mut mk: i32 = 1;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            sa[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 {
                mk = mk.wrapping_add(MBIG);
            }
            mj = sa[ii];
        }
        for _ in 0..4 {
            for i in 1..56 {
                sa[i] = sa[i].wrapping_sub(sa[1 + (i + 30) % 55]);
                if sa[i] < 0 {
                    sa[i] = sa[i].wrapping_add(MBIG);
                }
            }
        }
        NetRandom { sa, inext: 0, inextp: 21, calls: 0 }
    }

    pub fn internal_sample(&mut self) -> i32 {
        let ins = INSERT.get_or_init(|| crate::envc::var("JPRACE_RNG_INSERT").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default());
        while !ins.is_empty() && ins.contains(&(self.calls + 1)) {
            self.calls += 1;
            self.step();
        }
        self.calls += 1;
        if let Ok(r) = crate::envc::var("JPRACE_RNG_BT") {
            if let Some((lo, hi)) = r.split_once('-') {
                if (lo.parse::<u64>().unwrap()..=hi.parse::<u64>().unwrap()).contains(&self.calls) {
                    let bt = std::backtrace::Backtrace::force_capture().to_string();
                    let frames: Vec<&str> = bt.lines().filter(|l| l.contains("jp_race_sim::") && !l.contains("rng::")).take(3).collect();
                    eprintln!("rng#{} {:?}", self.calls, frames);
                }
            }
        }
        let skip = SKIP.get_or_init(|| crate::envc::var("JPRACE_RNG_SKIP").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default());
        if !skip.is_empty() && skip.contains(&self.calls) {
            let (sa, a, b) = (self.sa, self.inext, self.inextp);
            let v = self.step();
            self.sa = sa; self.inext = a; self.inextp = b;
            return v;
        }
        let v = self.step();
        if let Ok(r) = crate::envc::var("JPRACE_LOG_RANDV") && let Some((lo, hi)) = r.split_once('-') && let (Ok(lo), Ok(hi)) = (lo.parse::<u64>(), hi.parse::<u64>()) && (lo..=hi).contains(&self.calls) {
            eprintln!("randv call={} u={:.6}", self.calls, v as f64 / MBIG as f64);
        }
        v
    }

    fn step(&mut self) -> i32 {
        let mut inext = self.inext + 1;
        if inext >= 56 {
            inext = 1;
        }
        let mut inextp = self.inextp + 1;
        if inextp >= 56 {
            inextp = 1;
        }
        let mut ret = self.sa[inext].wrapping_sub(self.sa[inextp]);
        if ret == MBIG {
            ret -= 1;
        }
        if ret < 0 {
            ret = ret.wrapping_add(MBIG);
        }
        self.sa[inext] = ret;
        self.inext = inext;
        self.inextp = inextp;
        ret
    }

    pub fn next_double(&mut self) -> f64 {
        self.internal_sample() as f64 * (1.0 / MBIG as f64)
    }

    pub fn next_range(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(lo <= hi, "Random.Next minimum exceeds maximum");
        let range = hi as i64 - lo as i64;
        if range <= MBIG as i64 {
            return (self.next_double() * range as f64) as i64 as i32 + lo;
        }
        let mut value = self.internal_sample() as i64;
        if self.internal_sample() % 2 == 0 {
            value = -value;
        }
        let sample = (value as f64 + (MBIG as f64 - 1.0)) / (2.0 * MBIG as f64 - 1.0);
        ((sample * range as f64) as i64 + lo as i64) as i32
    }
}

#[derive(Clone)]
pub struct RaceRandom {
    r: NetRandom,
    pub wide: bool,
    pub last_dd: f64,
}

impl RaceRandom {
    pub fn new(seed: i32) -> Self {
        RaceRandom { r: NetRandom::new(seed), wide: false, last_dd: 0.0 }
    }

    pub fn calls(&self) -> u64 {
        self.r.calls
    }

    pub fn raw(&mut self) -> i32 {
        self.r.internal_sample()
    }

    #[track_caller]
    fn trace(&self) {
        if crate::envc::var_os("JPRACE_LOG_RAND").is_some() {
            let l = std::panic::Location::caller();
            eprintln!("rand call={} at={}:{}", self.r.calls + 1, l.file(), l.line());
        }
    }

    #[track_caller]
    pub fn int_max(&mut self, max: i32) -> i32 {
        self.trace();
        self.r.next_range(0, max)
    }

    #[track_caller]
    pub fn int_range(&mut self, lo: i32, hi: i32) -> i32 {
        self.trace();
        self.r.next_range(lo, hi)
    }

    #[track_caller]
    pub fn float_max(&mut self, max: f32) -> f32 {
        self.trace();
        let d = self.r.next_double() as f32;
        let mut v = d * max.abs() + 0.0f32;
        if v < 0.0 {
            v = 0.0;
        } else if max < v {
            v = max;
        }
        v
    }

    #[track_caller]
    pub fn float_range(&mut self, lo: f32, hi: f32) -> f32 {
        let mode = if self.wide { Wide::Double } else { Wide::Float };
        self.float_range_with(lo, hi, mode)
    }

    #[track_caller]
    pub fn float_range_with(&mut self, lo: f32, hi: f32, mode: Wide) -> f32 {
        self.trace();
        let dd = self.r.next_double();
        self.last_dd = dd;
        let d = dd as f32;
        let mut v = match mode {
            Wide::Float => d * (hi - lo).abs() + lo,
            Wide::Double => (dd * (hi - lo).abs() as f64 + lo as f64) as f32,
            Wide::FloatDraw => (d as f64 * (hi - lo).abs() as f64 + lo as f64) as f32,
        };
        if v < lo {
            v = lo;
        } else if hi < v {
            v = hi;
        }
        v
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wide { Float, Double, FloatDraw }
