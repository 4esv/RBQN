// NOTE: lib.rs — shared library target for the rbqn crate.
// Exposes bootstrap and exec for use by both main.rs (REPL) and rbqn-gen (--verify).
pub mod bootstrap;
pub mod embedded;
pub mod exec;
