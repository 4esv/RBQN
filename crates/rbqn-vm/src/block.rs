use std::sync::Arc;

use rbqn_core::value::B;

use crate::namespace::NSDesc;
use crate::scope::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompKind {
    Unknown,
    Repl,
}

#[derive(Debug)]
pub struct Comp {
    pub src: B,
    pub fullpath: B,
    pub indices: B,
    pub name_list: B,
    pub objs: Vec<B>,
    pub kind: CompKind,
    pub block_am: u32,
}

#[derive(Debug)]
pub struct Body {
    pub bc: Vec<u32>,
    pub bc_offset: usize,
    pub bl: Option<Arc<Block>>,
    pub ns_desc: Option<Arc<NSDesc>>,
    pub max_stack: u32,
    pub max_psc: u16,
    pub var_am: u16,
    pub exists: bool,
    pub var_data: Vec<i32>,
}

impl Body {
    pub fn new(var_am: u16, bc_offset: usize, max_stack: u32, max_psc: u16) -> Self {
        Body {
            bc: Vec::new(),
            bc_offset,
            bl: None,
            ns_desc: None,
            max_stack,
            max_psc,
            var_am,
            exists: true,
            var_data: vec![-1; var_am as usize * 2],
        }
    }

    pub fn fail_body() -> Self {
        let mut b = Body::new(6, 0, 1, 0);
        b.exists = false;
        b
    }
}

#[derive(Debug)]
pub struct Block {
    pub comp: Arc<Comp>,
    pub ty: u8,
    pub imm: bool,
    pub bc: Vec<i32>,
    pub map: Vec<i32>,
    pub blocks: Vec<Arc<Block>>,
    pub body_count: usize,
    pub bodies: Vec<Arc<Body>>,
    pub dy_body: Option<Arc<Body>>,
    pub inv_m_body: Option<Arc<Body>>,
    pub inv_x_body: Option<Arc<Body>>,
    pub inv_w_body: Option<Arc<Body>>,
}

pub fn arg_count(ty: u8, imm: bool) -> i32 {
    (if imm { 0 } else { 3 }) + ty as i32 + if ty > 0 { 1 } else { 0 }
}

pub fn block_given_vars(bl: &Block) -> i32 {
    arg_count(bl.ty, bl.imm)
}

pub fn eval_fun_block(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    if bl.imm {
        crate::vm::exec_block(&bl, bl.bodies[0].clone(), &psc)
    } else {
        crate::derive::m_fun_block(bl, psc)
    }
}

pub fn m_md1_block(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    crate::derive::m_md1_block_val(bl, psc)
}

pub fn m_md2_block(bl: Arc<Block>, psc: Arc<Scope>) -> B {
    crate::derive::m_md2_block_val(bl, psc)
}
