#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Op {
    PUSH = 0x00,
    DFND = 0x01,
    SYSV = 0x02,

    POPS = 0x06,
    RETN = 0x07,
    RETD = 0x08,
    LSTO = 0x0B,
    LSTM = 0x0C,
    ARMO = 0x0D,
    ARMM = 0x0E,

    FN1C = 0x10,
    FN2C = 0x11,
    FN1O = 0x12,
    FN2O = 0x13,
    TR2D = 0x14,
    TR3D = 0x15,
    CHKV = 0x16,
    TR3O = 0x17,

    MD1C = 0x1A,
    MD2C = 0x1B,
    MD2L = 0x1C,
    MD2R = 0x1D,

    VARO = 0x20,
    VARM = 0x21,
    VARU = 0x22,
    DYNO = 0x26,
    DYNM = 0x27,

    PRED = 0x2A,
    VFYM = 0x2B,
    NOTM = 0x2C,
    SETH = 0x2F,
    SETN = 0x30,
    SETU = 0x31,
    SETM = 0x32,
    SETC = 0x33,

    FLDO = 0x40,
    FLDM = 0x41,
    ALIM = 0x42,

    // Internal opcodes (auto-numbered from here)
    EXTO = 0x50,
    EXTM,
    EXTU,
    FLDG,
    ADDI,
    ADDU,
    FN1Ci,
    FN1Oi,
    FN2Ci,
    FN2Oi,
    SETNi,
    SETUi,
    SETMi,
    SETCi,
    SETNv,
    SETUv,
    SETMv,
    SETCv,
    SETH1,
    SETH2,
    PRED1,
    PRED2,
    DFND0,
    DFND1,
    DFND2,
    FAIL,
    BC_SIZE,
}

impl Op {
    pub fn from_u32(v: u32) -> Option<Op> {
        if v < Op::BC_SIZE as u32 {
            Some(unsafe { std::mem::transmute::<u32, Op>(v) })
        } else {
            None
        }
    }

    pub fn repr(self) -> &'static str {
        match self {
            Op::PUSH => "PUSH", Op::DFND => "DFND", Op::SYSV => "SYSV",
            Op::POPS => "POPS", Op::RETN => "RETN", Op::RETD => "RETD",
            Op::LSTO => "LSTO", Op::LSTM => "LSTM", Op::ARMO => "ARMO", Op::ARMM => "ARMM",
            Op::FN1C => "FN1C", Op::FN2C => "FN2C", Op::FN1O => "FN1O", Op::FN2O => "FN2O",
            Op::TR2D => "TR2D", Op::TR3D => "TR3D", Op::CHKV => "CHKV", Op::TR3O => "TR3O",
            Op::MD1C => "MD1C", Op::MD2C => "MD2C", Op::MD2L => "MD2L", Op::MD2R => "MD2R",
            Op::VARO => "VARO", Op::VARM => "VARM", Op::VARU => "VARU",
            Op::DYNO => "DYNO", Op::DYNM => "DYNM",
            Op::PRED => "PRED", Op::VFYM => "VFYM", Op::NOTM => "NOTM", Op::SETH => "SETH",
            Op::SETN => "SETN", Op::SETU => "SETU", Op::SETM => "SETM", Op::SETC => "SETC",
            Op::FLDO => "FLDO", Op::FLDM => "FLDM", Op::ALIM => "ALIM",
            Op::EXTO => "EXTO", Op::EXTM => "EXTM", Op::EXTU => "EXTU",
            Op::FLDG => "FLDG",
            Op::ADDI => "ADDI", Op::ADDU => "ADDU",
            Op::FN1Ci => "FN1Ci", Op::FN1Oi => "FN1Oi", Op::FN2Ci => "FN2Ci", Op::FN2Oi => "FN2Oi",
            Op::SETNi => "SETNi", Op::SETUi => "SETUi", Op::SETMi => "SETMi", Op::SETCi => "SETCi",
            Op::SETNv => "SETNv", Op::SETUv => "SETUv", Op::SETMv => "SETMv", Op::SETCv => "SETCv",
            Op::SETH1 => "SETH1", Op::SETH2 => "SETH2",
            Op::PRED1 => "PRED1", Op::PRED2 => "PRED2",
            Op::DFND0 => "DFND0", Op::DFND1 => "DFND1", Op::DFND2 => "DFND2",
            Op::FAIL => "FAIL",
            Op::BC_SIZE => "(BC_SIZE)",
        }
    }
}

pub fn bc_len(op: Op) -> u32 {
    match op {
        Op::PUSH | Op::DFND | Op::SYSV => 2,
        Op::LSTO | Op::LSTM | Op::ARMO | Op::ARMM => 2,
        Op::DYNO | Op::DYNM => 2,
        Op::FLDO | Op::FLDM | Op::ALIM => 2,
        Op::VARO | Op::VARM | Op::VARU => 3,

        Op::EXTO | Op::EXTM | Op::EXTU => 3,
        Op::FLDG => 2,
        Op::ADDI | Op::ADDU => 3, // op + 2xu32 for the u64
        Op::FN1Ci | Op::FN1Oi => 3,
        Op::FN2Ci => 3,
        // FIX: FN2Oi encodes TWO u64 immediates (monadic + dyadic fn ptr)
        Op::FN2Oi => 5,
        Op::SETNi | Op::SETUi | Op::SETMi | Op::SETCi => 3,
        Op::SETNv | Op::SETUv | Op::SETMv | Op::SETCv => 3,
        Op::SETH1 => 3, // op + 2xu32 (one u64 body ptr)
        Op::SETH2 => 5, // op + 4xu32 (two u64 body ptrs)
        Op::PRED1 => 3,
        Op::PRED2 => 5,
        Op::DFND0 | Op::DFND1 | Op::DFND2 => 3, // op + u64 block ptr

        _ => 1,
    }
}

pub fn stack_diff(op: Op) -> i32 {
    match op {
        Op::PUSH | Op::DFND | Op::SYSV => 1,
        Op::ADDI | Op::ADDU => 1,
        Op::DFND0 | Op::DFND1 | Op::DFND2 => 1,
        Op::VARO | Op::VARM | Op::VARU => 1,
        Op::EXTO | Op::EXTM | Op::EXTU => 1,
        Op::DYNO | Op::DYNM => 1,
        Op::NOTM => 1,
        Op::VFYM => 0,
        Op::POPS => -1,
        Op::RETN => 0,
        Op::RETD => 0,
        Op::FN1C | Op::FN1O => -1,
        Op::FN1Ci | Op::FN1Oi => 0,
        Op::FN2C | Op::FN2O => -2,
        Op::FN2Ci | Op::FN2Oi => -1,
        Op::TR2D => -1,
        Op::TR3D | Op::TR3O => -2,
        Op::CHKV => 0,
        Op::MD1C => -1,
        Op::MD2C => -2,
        Op::MD2L => -1,
        Op::MD2R => -1,
        Op::SETN | Op::SETU => -1,
        // FIX: SETM pops 3 (s,f,x) pushes 1 = -2; SETC pops 2 (s,f) pushes 1 = -1
        Op::SETM => -2,
        Op::SETC => -1,
        Op::SETNi | Op::SETUi => 0,
        // FIX: SETMi pops 2 (f,x) pushes 1 = -1; SETCi pops 1 (f) pushes 1 = 0
        Op::SETMi => -1,
        Op::SETCi => 0,
        Op::SETNv | Op::SETUv => -1,
        // FIX: SETMv pops 2 (f,x) pushes 0 = -2; SETCv pops 1 (f) pushes 0 = -1
        Op::SETMv => -2,
        Op::SETCv => -1,
        Op::SETH | Op::SETH1 | Op::SETH2 => -2,
        Op::PRED | Op::PRED1 | Op::PRED2 => -1,
        Op::FLDO | Op::FLDM | Op::FLDG => 0,
        Op::ALIM => 0,
        Op::FAIL => 0,
        // LSTO, LSTM, ARMO, ARMM are special: 1 - N
        _ => 0,
    }
}

pub fn stack_consumed(op: Op) -> i32 {
    match op {
        Op::PUSH | Op::DFND | Op::SYSV => 0,
        Op::ADDI | Op::ADDU => 0,
        Op::DFND0 | Op::DFND1 | Op::DFND2 => 0,
        Op::VARO | Op::VARM | Op::VARU => 0,
        Op::EXTO | Op::EXTM | Op::EXTU => 0,
        Op::DYNO | Op::DYNM => 0,
        Op::NOTM => 0,
        Op::VFYM => 1,
        Op::POPS => 1,
        Op::RETN => 0,
        Op::RETD => 0,
        Op::FN1C | Op::FN1O => 2,
        Op::FN1Ci | Op::FN1Oi => 1,
        Op::FN2C | Op::FN2O => 3,
        Op::FN2Ci | Op::FN2Oi => 2,
        Op::TR2D => 2,
        Op::TR3D | Op::TR3O => 3,
        Op::CHKV => 0,
        Op::MD1C => 2,
        Op::MD2C => 3,
        Op::MD2L => 2,
        Op::MD2R => 2,
        Op::SETN | Op::SETU => 2,
        // FIX: SETM consumes 3 (s,f,x); SETC consumes 2 (s,f)
        Op::SETM => 3,
        Op::SETC => 2,
        Op::SETNi | Op::SETUi => 1,
        // FIX: SETMi consumes 2 (f,x); SETCi consumes 1 (f)
        Op::SETMi => 2,
        Op::SETCi => 1,
        Op::SETNv | Op::SETUv => 1,
        // FIX: SETMv consumes 2 (f,x); SETCv consumes 1 (f)
        Op::SETMv => 2,
        Op::SETCv => 1,
        Op::SETH | Op::SETH1 | Op::SETH2 => 2,
        Op::PRED | Op::PRED1 | Op::PRED2 => 1,
        Op::FLDO | Op::FLDM | Op::FLDG => 1,
        Op::ALIM => 1,
        Op::FAIL => 0,
        // LSTO, LSTM, ARMO, ARMM are special: N
        _ => 0,
    }
}
