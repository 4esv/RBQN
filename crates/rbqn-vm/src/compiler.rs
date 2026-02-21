use rbqn_core::value::B;
use rbqn_core::error::Result;
use crate::bytecode::{CompiledBlock, BlockInfo, BodyInfo};

pub fn compile_bytecode(
    bc: Vec<i32>,
    objs: Vec<B>,
    blocks: Vec<BlockInfo>,
    bodies: Vec<BodyInfo>,
    src: Option<B>,
    path: Option<B>,
) -> Result<CompiledBlock> {
    let _ = (src, path);
    Ok(CompiledBlock { bc, objs, blocks, bodies })
}
