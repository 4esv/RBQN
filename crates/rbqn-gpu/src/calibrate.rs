//! Per-machine calibration of the cost model (#167): the fit from measured timings to
//! `dispatch::Model`, and the cache file that keeps a measured model between runs. The
//! measurements themselves run in the `rbqn` crate (they need the device and the host kernels).
//!
//! Every quantity is timed at two sizes and fitted with a line `t = a + b·x`; the slopes give
//! the per-element terms and the intercept of the resident fold the fixed dispatch cost, the
//! same differences bench/wow/gpu.bqn used to fit them by hand (fold1 slope = map, fold2-fold1 =
//! upload, map-fold1 = download, scan = scan - up - down).
use crate::dispatch::{Model, MODEL_FIELDS};
use std::path::{Path, PathBuf};

/// Two timings of one quantity: `x` is the size (elements, or multiply-adds), `ms` the time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub x1: f64,
    pub ms1: f64,
    pub x2: f64,
    pub ms2: f64,
}

impl Pair {
    /// Intercept in ms and slope in ns per unit of the line through both points.
    pub fn fit(&self) -> (f64, f64) {
        let slope_ms = (self.ms2 - self.ms1) / (self.x2 - self.x1);
        (self.ms1 - slope_ms * self.x1, slope_ms * 1e6)
    }
    pub fn slope_ns(&self) -> f64 {
        self.fit().1
    }
}

/// Everything the calibration times. GPU pairs are per element (4-byte i32 elements); the
/// sizes are the same for the pairs that are subtracted from each other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Samples {
    pub gpu_init_ms: f64,
    /// First run of a tree shape minus its warm time at the same size (pipeline compile etc).
    pub gpu_cold_extra_ms: f64,
    /// `+´x+1` on a resident buffer.
    pub gpu_fold1: Pair,
    /// `+´(x×x)+2×x` on a resident buffer (3 ops).
    pub gpu_fold3: Pair,
    /// Upload, then `+´x+1`.
    pub gpu_up_fold1: Pair,
    /// `x+1` evaluated to a buffer and downloaded.
    pub gpu_map_down: Pair,
    /// Upload, `+`x`, download.
    pub gpu_scan_all: Pair,
    /// Upload, sort, download.
    pub gpu_sort: Pair,
    /// Square f32 matmul including its transfers, x = multiply-adds (d³ for side d).
    pub gpu_matmul: Pair,
    /// Host fold of a plain array.
    pub cpu_fold: Pair,
    /// Host evaluator fold of a leaf.
    pub cpu_leaf: Pair,
    /// Host evaluator fold of `x+1`.
    pub cpu_fold1: Pair,
    /// Host evaluator `x+1` materialized and squeezed.
    pub cpu_mat1: Pair,
    pub cpu_scan: Pair,
    pub cpu_sort: Pair,
    /// Serial host matmul, x = multiply-adds.
    pub cpu_mm_serial: Pair,
    /// Threaded host matmul on `threads` threads, x = multiply-adds.
    pub cpu_mm_par: Pair,
    pub threads: usize,
}

/// Smallest value any constant takes, so a noisy fit never produces a free or negative term.
pub const FLOOR_NS: f64 = 1e-3;

pub fn derive(s: &Samples) -> Model {
    let pos = |v: f64| if v.is_finite() { v.max(FLOOR_NS) } else { FLOOR_NS };
    let (fixed_ms, map) = s.gpu_fold1.fit();
    let op = (s.gpu_fold3.slope_ns() - map) / 2.0;
    let up = (s.gpu_up_fold1.slope_ns() - map) / 4.0;
    let down = (s.gpu_map_down.slope_ns() - map) / 4.0;
    let (up, down) = (pos(up), pos(down));
    let scan = s.gpu_scan_all.slope_ns() - 4.0 * up - 4.0 * down;
    // NOTE: the matmul model charges transfers per matrix element (up for the two operands, down
    // for the result), so the per-MAC term is what is left of the timing after them.
    let transfers_ms = |macs: f64| macs.powf(2.0 / 3.0) * 4.0 * (2.0 * up + down) * 1e-6;
    let mm = Pair {
        x1: s.gpu_matmul.x1,
        ms1: s.gpu_matmul.ms1 - transfers_ms(s.gpu_matmul.x1),
        x2: s.gpu_matmul.x2,
        ms2: s.gpu_matmul.ms2 - transfers_ms(s.gpu_matmul.x2),
    };
    let leaf = s.cpu_leaf.slope_ns();
    let cop = s.cpu_fold1.slope_ns() - leaf;
    let out = s.cpu_mat1.slope_ns() - cop.max(0.0) - leaf;
    let mm1 = s.cpu_mm_serial.slope_ns();
    // NOTE: per-MAC time of the threaded run over the serial one is `threads^-exp`.
    let exp = if s.threads > 1 {
        let speedup = mm1 / s.cpu_mm_par.slope_ns();
        let e = speedup.ln() / (s.threads as f64).ln();
        // NOTE: a run that was not faster (or a negative slope) has no speedup to take a log of;
        // read it as "threads barely help", the smallest exponent.
        if e.is_finite() { e.clamp(0.2, 1.0) } else { 0.2 }
    } else {
        1.0
    };
    Model {
        gpu_init_ms: s.gpu_init_ms.max(0.0),
        gpu_fixed_ms: fixed_ms.max(0.0) + s.gpu_cold_extra_ms.max(0.0),
        gpu_map_ns: pos(map),
        gpu_op_ns: pos(op),
        gpu_up_ns_per_b: up,
        gpu_down_ns_per_b: down,
        gpu_scan_ns: pos(scan),
        gpu_sort_ns: pos(s.gpu_sort.slope_ns()),
        gpu_matmul_ns_per_mac: pos(mm.slope_ns()),
        cpu_op_ns: pos(cop),
        cpu_leaf_ns: pos(leaf),
        cpu_out_ns: pos(out),
        cpu_fold_ns: pos(s.cpu_fold.slope_ns()),
        cpu_scan_ns: pos(s.cpu_scan.slope_ns()),
        cpu_sort_ns: pos(s.cpu_sort.slope_ns()),
        cpu_matmul_ns_per_mac: pos(mm1),
        cpu_matmul_thread_exp: exp,
    }
}

/// Cache key of an adapter: name, backend, driver and driver info.
pub fn adapter_key(info: &wgpu::AdapterInfo) -> String {
    format!("{}|{:?}|{}|{}", info.name, info.backend, info.driver, info.driver_info).replace(['\t', '\n'], " ")
}

/// The user cache dir for rbqn: `$RBQN_CACHE_DIR`, else `$XDG_CACHE_HOME/rbqn`, else
/// `~/Library/Caches/rbqn` on macOS, `%LOCALAPPDATA%\rbqn` on Windows, `~/.cache/rbqn`.
pub fn cache_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = var("RBQN_CACHE_DIR") {
        return Some(d);
    }
    if let Some(d) = var("XDG_CACHE_HOME") {
        return Some(d.join("rbqn"));
    }
    if cfg!(target_os = "windows") {
        return var("LOCALAPPDATA").map(|d| d.join("rbqn"));
    }
    let home = var("HOME")?;
    Some(if cfg!(target_os = "macos") { home.join("Library/Caches/rbqn") } else { home.join(".cache/rbqn") })
}

pub fn cache_path() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("gpu-model.txt"))
}

const HEADER: &str = "rbqn-gpu-model 1";

/// One measured model and what it was measured for.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    pub threads: usize,
    pub model: Model,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cache {
    /// Key of the entry used or written last: stands in for the adapter while the device is
    /// still being created.
    pub last: Option<String>,
    pub entries: Vec<Entry>,
}

impl Cache {
    pub fn get(&self, key: &str, threads: usize) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key && e.threads == threads)
    }
    pub fn last_entry(&self, threads: usize) -> Option<&Entry> {
        self.last.as_deref().and_then(|k| self.get(k, threads))
    }
    pub fn put(&mut self, e: Entry) {
        self.last = Some(e.key.clone());
        self.entries.retain(|o| !(o.key == e.key && o.threads == e.threads));
        self.entries.push(e);
    }
    pub fn to_text(&self) -> String {
        let mut s = format!("{HEADER}\n");
        if let Some(k) = &self.last {
            s += &format!("last\t{k}\n");
        }
        for e in &self.entries {
            s += &format!("entry\t{}\tthreads={}", e.key, e.threads);
            for (n, v) in MODEL_FIELDS.iter().zip(e.model.values()) {
                s += &format!("\t{n}={v:?}");
            }
            s.push('\n');
        }
        s
    }
    /// Lines that do not parse are dropped; a wrong header gives an empty cache.
    pub fn from_text(t: &str) -> Cache {
        let mut c = Cache::default();
        let mut lines = t.lines();
        if lines.next() != Some(HEADER) {
            return c;
        }
        for l in lines {
            let f: Vec<&str> = l.split('\t').collect();
            match f.as_slice() {
                ["last", k] => c.last = Some(k.to_string()),
                ["entry", key, threads, rest @ ..] => {
                    let Some(threads) = threads.strip_prefix("threads=").and_then(|v| v.parse().ok()) else { continue };
                    let mut vals = [0.0; MODEL_FIELDS.len()];
                    let mut seen = 0;
                    for kv in rest {
                        if let Some((k, v)) = kv.split_once('=')
                            && let Some(i) = MODEL_FIELDS.iter().position(|n| *n == k)
                            && let Ok(v) = v.parse::<f64>()
                            && v.is_finite()
                        {
                            vals[i] = v;
                            seen += 1;
                        }
                    }
                    if seen == MODEL_FIELDS.len() {
                        c.entries.push(Entry { key: key.to_string(), threads, model: Model::from_values(vals) });
                    }
                }
                _ => {}
            }
        }
        c
    }
}

pub fn read_cache(path: &Path) -> Cache {
    std::fs::read_to_string(path).map(|t| Cache::from_text(&t)).unwrap_or_default()
}

/// Write via a temp file and rename, so a concurrent reader never sees half a file.
pub fn write_cache(path: &Path, c: &Cache) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, c.to_text())?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Timings generated from known constants with the formulas `derive` inverts.
    fn synthetic(m: &Model, threads: usize) -> Samples {
        let (n1, n2) = (65536.0, 524288.0);
        let line = |x1: f64, x2: f64, a: f64, ns: f64| Pair { x1, ms1: a + ns * x1 * 1e-6, x2, ms2: a + ns * x2 * 1e-6 };
        let el = |a: f64, ns: f64| line(n1, n2, a, ns);
        let (up, down) = (m.gpu_up_ns_per_b, m.gpu_down_ns_per_b);
        let (mm1, mmp) = (m.cpu_matmul_ns_per_mac, m.cpu_matmul_ns_per_mac / (threads as f64).powf(m.cpu_matmul_thread_exp));
        Samples {
            gpu_init_ms: m.gpu_init_ms,
            gpu_cold_extra_ms: 0.7,
            gpu_fold1: el(m.gpu_fixed_ms - 0.7, m.gpu_map_ns),
            gpu_fold3: el(m.gpu_fixed_ms - 0.7, m.gpu_map_ns + 2.0 * m.gpu_op_ns),
            gpu_up_fold1: el(m.gpu_fixed_ms - 0.7, m.gpu_map_ns + 4.0 * up),
            gpu_map_down: el(m.gpu_fixed_ms - 0.7, m.gpu_map_ns + 4.0 * down),
            gpu_scan_all: el(m.gpu_fixed_ms - 0.7, m.gpu_scan_ns + 4.0 * up + 4.0 * down),
            gpu_sort: el(m.gpu_fixed_ms - 0.7, m.gpu_sort_ns),
            gpu_matmul: {
                let t = |d: f64| m.gpu_fixed_ms - 0.7 + (d * d * d * m.gpu_matmul_ns_per_mac + d * d * 4.0 * (2.0 * up + down)) * 1e-6;
                Pair { x1: 128f64.powi(3), ms1: t(128.0), x2: 256f64.powi(3), ms2: t(256.0) }
            },
            cpu_fold: el(0.01, m.cpu_fold_ns),
            cpu_leaf: el(0.01, m.cpu_leaf_ns),
            cpu_fold1: el(0.01, m.cpu_leaf_ns + m.cpu_op_ns),
            cpu_mat1: el(0.01, m.cpu_leaf_ns + m.cpu_op_ns + m.cpu_out_ns),
            cpu_scan: el(0.0, m.cpu_scan_ns),
            cpu_sort: el(0.0, m.cpu_sort_ns),
            cpu_mm_serial: line(1e6, 1.7e6, 0.0, mm1),
            cpu_mm_par: line(1.6e7, 6.4e7, 0.05, mmp),
            threads,
        }
    }

    fn known() -> Model {
        Model::from_values([
            12.5, 3.1, 1.9, 0.35, 0.8, 1.3, 2.2, 41.0, 0.09, 0.2, 0.4, 3.3, 0.05, 6.5, 19.0, 0.15, 0.7,
        ])
    }

    #[test]
    fn fit_recovers_known_constants_from_synthetic_timings() {
        let m = known();
        let got = derive(&synthetic(&m, 8));
        for ((n, a), b) in MODEL_FIELDS.iter().zip(got.values()).zip(m.values()) {
            assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{n}: fitted {a}, true {b}");
        }
    }

    #[test]
    fn noise_cannot_make_a_term_free_or_negative() {
        let m = known();
        let mut s = synthetic(&m, 8);
        // The larger run came out faster than the smaller one.
        s.gpu_up_fold1.ms2 = s.gpu_fold1.ms1;
        s.cpu_fold.ms2 = s.cpu_fold.ms1 - 1.0;
        // The threaded matmul came out slower than the serial one, with a negative slope.
        s.cpu_mm_par.ms2 = s.cpu_mm_par.ms1 - 1.0;
        let got = derive(&s);
        assert!(got.values().iter().all(|v| v.is_finite() && *v >= 0.0), "{got:?}");
        assert!(got.gpu_up_ns_per_b >= FLOOR_NS && got.cpu_fold_ns >= FLOOR_NS);
        assert_eq!(got.cpu_matmul_thread_exp, 0.2);
        // Whatever the timings were, the cache keeps the model (a NaN would not parse back).
        let mut c = Cache::default();
        c.put(Entry { key: "k".into(), threads: 8, model: got });
        assert_eq!(Cache::from_text(&c.to_text()).entries.len(), 1);
    }

    #[test]
    fn one_thread_has_no_exponent_to_measure() {
        assert_eq!(derive(&synthetic(&known(), 1)).cpu_matmul_thread_exp, 1.0);
    }

    #[test]
    fn cache_text_round_trips_and_keys_by_adapter_and_threads() {
        let mut c = Cache::default();
        c.put(Entry { key: "AMD|Metal||".into(), threads: 12, model: known() });
        let mut other = known();
        other.gpu_map_ns = 9.0;
        c.put(Entry { key: "Apple M3|Metal||".into(), threads: 12, model: other });
        let back = Cache::from_text(&c.to_text());
        assert_eq!(back, c);
        assert_eq!(back.get("AMD|Metal||", 12).unwrap().model, known());
        assert!(back.get("AMD|Metal||", 8).is_none());
        assert_eq!(back.last_entry(12).unwrap().model, other);
    }

    #[test]
    fn damaged_cache_reads_as_empty_or_drops_the_bad_line() {
        assert_eq!(Cache::from_text("garbage"), Cache::default());
        let mut c = Cache::default();
        c.put(Entry { key: "k".into(), threads: 1, model: known() });
        let t = c.to_text().replace("gpu_map_ns=", "gpu_map_nope=");
        assert!(Cache::from_text(&t).entries.is_empty());
    }
}
