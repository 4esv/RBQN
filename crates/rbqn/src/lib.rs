// NOTE: lib.rs — shared library target for the rbqn crate.
// Exposes bootstrap and exec for use by both main.rs (REPL) and rbqn-gen (--verify).
pub mod bootstrap;
pub mod embed;
pub mod embedded;
pub mod exec;
pub mod timing;

pub use embed::{Bqn, Error, Site, Value};
