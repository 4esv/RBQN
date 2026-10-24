//! Embed BQN in a Rust program: evaluate source, pass host data in, read results out.
//!
//! ```
//! use rbqn::{Bqn, Error};
//!
//! # fn main() -> Result<(), Error> {
//! let bqn = Bqn::new()?;
//! let data: Vec<f64> = (0..1_000_000).map(|i| i as f64).collect();
//! let mean = bqn.eval("+´÷≠")?; // a function value
//! let x = bqn.list_f64(&data); // one copy into the BQN store
//! let m = bqn.call1(&mean, &x)?;
//! assert_eq!(m.to_f64(), Some(499_999.5));
//! # Ok(())
//! # }
//! ```
//!
//! See [`embed`] for threading rules and the full API: [`Bqn`], [`Value`],
//! [`Error`], [`Site`].

// NOTE: lib.rs — shared library target for the rbqn crate.
// Exposes bootstrap and exec for use by both main.rs (REPL) and rbqn-gen (--verify).
pub mod bootstrap;
pub mod embed;
pub mod embedded;
pub mod exec;
pub mod timing;

pub use embed::{Bqn, Error, Site, Value};
