//! Mean of 1e6 f64 values, computed by BQN: `cargo run --release --example embed`.
use rbqn::Bqn;

fn main() -> Result<(), rbqn::Error> {
    let bqn = Bqn::new()?;
    let data: Vec<f64> = (0..1_000_000).map(|i| i as f64).collect();
    let mean = bqn.eval("+´÷≠")?;
    let x = bqn.list_f64(&data);
    let m = bqn.call1(&mean, &x)?;
    println!("mean of {} values: {}", data.len(), m.fmt());
    Ok(())
}
