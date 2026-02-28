use std::io::Read;

// ---------------------------------------------------------------------------
// Static types (used by the old generated code path and kept for reference)
// ---------------------------------------------------------------------------

// NOTE: ObjectEntry::Str holds Vec<u32> for owned decode. Cannot derive Copy.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ObjectEntry {
    Provide(usize),
    /// Reference to runtime[N] (used by compiler, formatter)
    Runtime(usize),
    /// Reference to runtime_0[N] (used by runtime1)
    RuntimePrev(usize),
    Float(f64),
    Char(u32),
    // NOTE: Changed from &'static [u32] to Vec<u32> to support owned decode path.
    // Pattern matches on ObjectEntry::Str(s) — s is &Vec<u32>, auto-derefs to &[u32].
    Str(Vec<u32>),
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

// ---------------------------------------------------------------------------
// Owned bytecode (decoded from .bin files)
// ---------------------------------------------------------------------------

pub struct OwnedBytecode {
    pub bc: Vec<i32>,
    pub iarrs: Vec<Vec<i32>>,
    pub objs: Vec<ObjectEntry>,
    pub blocks: Vec<BlockEntry>,
    pub bodies: Vec<usize>,
    pub source_tag: String,
}

impl OwnedBytecode {
    pub fn is_empty(&self) -> bool {
        self.bc.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Embedded .bin files (committed to repo)
// ---------------------------------------------------------------------------

pub static RUNTIME0_BIN: &[u8] = include_bytes!("runtime0.bin");
pub static RUNTIME1_BIN: &[u8] = include_bytes!("runtime1x.bin");
pub static COMPILER_BIN: &[u8] = include_bytes!("compiler.bin");
pub static FORMATTER_BIN: &[u8] = include_bytes!("formatter.bin");

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

pub fn decode_bytecode(bytes: &[u8]) -> OwnedBytecode {
    let mut cur = std::io::Cursor::new(bytes);

    // Magic
    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic).expect("decode_bytecode: failed to read magic");
    assert_eq!(&magic, b"RBQN", "decode_bytecode: bad magic");

    // Version
    let version = read_u32(&mut cur);
    assert_eq!(version, 1, "decode_bytecode: unsupported version {version}");

    // Source tag
    let tag_len = read_u16(&mut cur) as usize;
    let mut tag_bytes = vec![0u8; tag_len];
    cur.read_exact(&mut tag_bytes).expect("decode_bytecode: failed to read source_tag");
    let source_tag = String::from_utf8(tag_bytes).unwrap_or_default();

    // Bytecode
    let bc_len = read_u32(&mut cur) as usize;
    let mut bc = Vec::with_capacity(bc_len);
    for _ in 0..bc_len {
        bc.push(read_i32(&mut cur));
    }

    // iarrs
    let iarrs_count = read_u32(&mut cur) as usize;
    let mut iarrs = Vec::with_capacity(iarrs_count);
    for _ in 0..iarrs_count {
        let len = read_u32(&mut cur) as usize;
        let mut arr = Vec::with_capacity(len);
        for _ in 0..len {
            arr.push(read_i32(&mut cur));
        }
        iarrs.push(arr);
    }

    // objs
    let objs_count = read_u32(&mut cur) as usize;
    let mut objs = Vec::with_capacity(objs_count);
    for _ in 0..objs_count {
        let tag = read_u8(&mut cur);
        let entry = match tag {
            0 => ObjectEntry::Provide(read_u32(&mut cur) as usize),
            1 => ObjectEntry::Runtime(read_u32(&mut cur) as usize),
            2 => ObjectEntry::RuntimePrev(read_u32(&mut cur) as usize),
            3 => ObjectEntry::Float(read_f64(&mut cur)),
            4 => ObjectEntry::Char(read_u32(&mut cur)),
            5 => {
                let len = read_u32(&mut cur) as usize;
                let mut chars = Vec::with_capacity(len);
                for _ in 0..len {
                    chars.push(read_u32(&mut cur));
                }
                ObjectEntry::Str(chars)
            }
            6 => ObjectEntry::IArr(read_u32(&mut cur) as usize),
            _ => panic!("decode_bytecode: unknown obj tag {tag}"),
        };
        objs.push(entry);
    }

    // blocks
    let blocks_count = read_u32(&mut cur) as usize;
    let mut blocks = Vec::with_capacity(blocks_count);
    for _ in 0..blocks_count {
        let tag = read_u8(&mut cur);
        let entry = match tag {
            0 => BlockEntry::IArr(read_u32(&mut cur) as usize),
            1 => {
                let typ = read_u8(&mut cur);
                let iarrs0_idx = read_u32(&mut cur) as usize;
                let data_idx = read_u32(&mut cur) as usize;
                BlockEntry::Info { typ, iarrs0_idx, data_idx }
            }
            _ => panic!("decode_bytecode: unknown block tag {tag}"),
        };
        blocks.push(entry);
    }

    // bodies
    let bodies_count = read_u32(&mut cur) as usize;
    let mut bodies = Vec::with_capacity(bodies_count);
    for _ in 0..bodies_count {
        bodies.push(read_u32(&mut cur) as usize);
    }

    OwnedBytecode { bc, iarrs, objs, blocks, bodies, source_tag }
}

// ---------------------------------------------------------------------------
// Cursor read helpers
// ---------------------------------------------------------------------------

fn read_u8(cur: &mut std::io::Cursor<&[u8]>) -> u8 {
    let mut buf = [0u8; 1];
    cur.read_exact(&mut buf).expect("decode_bytecode: unexpected EOF reading u8");
    buf[0]
}

fn read_u16(cur: &mut std::io::Cursor<&[u8]>) -> u16 {
    let mut buf = [0u8; 2];
    cur.read_exact(&mut buf).expect("decode_bytecode: unexpected EOF reading u16");
    u16::from_le_bytes(buf)
}

fn read_u32(cur: &mut std::io::Cursor<&[u8]>) -> u32 {
    let mut buf = [0u8; 4];
    cur.read_exact(&mut buf).expect("decode_bytecode: unexpected EOF reading u32");
    u32::from_le_bytes(buf)
}

fn read_i32(cur: &mut std::io::Cursor<&[u8]>) -> i32 {
    let mut buf = [0u8; 4];
    cur.read_exact(&mut buf).expect("decode_bytecode: unexpected EOF reading i32");
    i32::from_le_bytes(buf)
}

fn read_f64(cur: &mut std::io::Cursor<&[u8]>) -> f64 {
    let mut buf = [0u8; 8];
    cur.read_exact(&mut buf).expect("decode_bytecode: unexpected EOF reading f64");
    f64::from_le_bytes(buf)
}
