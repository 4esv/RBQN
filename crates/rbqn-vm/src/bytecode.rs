use rbqn_core::value::B;
use rbqn_core::error::Result;

pub struct CompiledBlock {
    pub bc: Vec<i32>,
    pub objs: Vec<B>,
    pub blocks: Vec<BlockInfo>,
    pub bodies: Vec<BodyInfo>,
}

pub struct BlockInfo {
    pub block_type: u8,
    pub immediate: bool,
    pub body_indices: Vec<usize>,
}

pub struct BodyInfo {
    pub bc_offset: usize,
    pub var_count: usize,
    pub var_ids: Option<Vec<usize>>,
    pub export_mask: Option<Vec<usize>>,
}

pub fn load_compiled(
    bc: Vec<i32>,
    objs: Vec<B>,
    blocks: Vec<BlockInfo>,
    bodies: Vec<BodyInfo>,
    _src: Option<&str>,
    _path: Option<&str>,
) -> Result<CompiledBlock> {
    Ok(CompiledBlock { bc, objs, blocks, bodies })
}
