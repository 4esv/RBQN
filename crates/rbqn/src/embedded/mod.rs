#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ObjectEntry {
    Provide(usize),
    /// Reference to runtime[N] (used by compiler, formatter)
    Runtime(usize),
    /// Reference to runtime_0[N] (used by runtime1)
    RuntimePrev(usize),
    Float(f64),
    Char(u32),
    Str(&'static [u32]),
    IArr(usize),
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum BlockEntry {
    IArr(usize),
    Info {
        typ: u8,
        iarrs0_idx: usize,
        data_idx: usize,
    },
}

#[allow(dead_code)]
pub struct EmbeddedBytecode {
    pub bc: &'static [i32],
    pub iarrs: &'static [&'static [i32]],
    pub objs: &'static [ObjectEntry],
    pub blocks: &'static [BlockEntry],
    pub bodies: &'static [usize],
}

impl EmbeddedBytecode {
    pub fn is_empty(&self) -> bool {
        self.bc.is_empty()
    }
}

include!(concat!(env!("OUT_DIR"), "/embedded_bytecode.rs"));
