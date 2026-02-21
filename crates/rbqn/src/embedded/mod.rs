#[allow(dead_code)]
pub struct EmbeddedBytecode {
    pub bc: &'static [i32],
    pub objs: &'static [&'static [i32]],
    pub blocks: &'static [i32],
    pub bodies: &'static [i32],
}

impl EmbeddedBytecode {
    pub fn is_empty(&self) -> bool {
        self.bc.is_empty()
    }
}

include!(concat!(env!("OUT_DIR"), "/embedded_bytecode.rs"));
