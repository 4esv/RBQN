#![allow(non_upper_case_globals, unused_assignments)]

pub mod bytecode;
pub mod block;
pub mod body {
    pub use crate::block::Body;
}
pub mod scope;
pub mod env;
pub mod derive;
pub mod modifiers;
pub mod namespace;
pub mod compiler;
pub mod vm;
pub mod ffi;
