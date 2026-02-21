use rbqn_core::value::B;
use rbqn_core::error::Result;
use crate::bytecode::CompiledBlock;
use crate::scope::Scope;

pub fn eval_block(block: &CompiledBlock, scope: &mut Scope) -> Result<B> {
    let _ = (block, scope);
    Ok(B::nothing())
}

pub fn call1(f: &B, x: B) -> Result<B> {
    let _ = (f, x);
    Ok(B::nothing())
}

pub fn call2(f: &B, w: B, x: B) -> Result<B> {
    let _ = (f, w, x);
    Ok(B::nothing())
}
