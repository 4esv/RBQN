use std::sync::Arc;

use crate::scope::Scope;

#[derive(Debug, Clone)]
pub struct Env {
    pub pos: u64,
    pub sc: Arc<Scope>,
}

pub struct EnvStack {
    stack: Vec<Env>,
}

impl EnvStack {
    pub fn new() -> Self {
        EnvStack {
            stack: Vec::with_capacity(1024),
        }
    }

    pub fn push(&mut self, sc: Arc<Scope>, bc_pos: u64) {
        if self.stack.len() >= 10000 {
            rbqn_core::error::throw("Stack overflow");
        }
        self.stack.push(Env { pos: bc_pos, sc });
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn current(&self) -> Option<&Env> {
        self.stack.last()
    }

    pub fn height(&self) -> usize {
        self.stack.len()
    }
}

impl Default for EnvStack {
    fn default() -> Self {
        Self::new()
    }
}
