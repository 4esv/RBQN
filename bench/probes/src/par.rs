use rayon::prelude::*; use std::time::Instant;
fn ms(t: Instant) -> f64 { t.elapsed().as_secs_f64()*1e3 }
fn main() {
    for &n in &[100_000usize, 1_000_000, 10_000_000] {
        let a: Vec<i32> = (0..n as i32).collect(); let reps = 20;
        let exp: i64 = a.iter().map(|&v| v as i64).sum();
        let t = Instant::now(); for _ in 0..reps { let s: i64 = a.iter().map(|&v| v as i64).sum(); assert_eq!(s, exp); } let s1 = ms(t)/reps as f64;
        let t = Instant::now(); for _ in 0..reps { let s: i64 = a.par_chunks(1<<16).map(|c| c.iter().map(|&v| v as i64).sum::<i64>()).sum(); assert_eq!(s, exp); } let sp = ms(t)/reps as f64;
        // materialized map (a+a), single vs parallel
        let t = Instant::now(); for _ in 0..reps { let o: Vec<i32> = a.iter().map(|&v| v+v).collect(); std::hint::black_box(&o); } let m1 = ms(t)/reps as f64;
        let t = Instant::now(); for _ in 0..reps { let o: Vec<i32> = a.par_iter().map(|&v| v+v).collect(); std::hint::black_box(&o); } let mp = ms(t)/reps as f64;
        println!("n={n:>9}  sum 1T {s1:7.3}  par {sp:7.3}  ({:.1}x)   map 1T {m1:7.3}  par {mp:7.3}  ({:.1}x)", s1/sp, m1/mp);
    }
    println!("threads {}", rayon::current_num_threads());
}
