#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ElType {
    Bit = 0,
    I8 = 1,
    I16 = 2,
    I32 = 3,
    F64 = 4,
    C8 = 5,
    C16 = 6,
    C32 = 7,
    B = 8,
}

impl ElType {
    pub fn is_num(self) -> bool {
        (self as u8) <= ElType::F64 as u8
    }

    pub fn is_int(self) -> bool {
        (self as u8) <= ElType::I32 as u8
    }

    pub fn is_chr(self) -> bool {
        let v = self as u8;
        v >= ElType::C8 as u8 && v <= ElType::C32 as u8
    }

    pub fn width(self) -> usize {
        match self {
            ElType::Bit => 0,
            ElType::I8 | ElType::C8 => 1,
            ElType::I16 | ElType::C16 => 2,
            ElType::I32 | ElType::C32 => 4,
            ElType::F64 => 8,
            ElType::B => 8,
        }
    }
}
