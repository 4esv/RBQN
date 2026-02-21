use crate::error::{BqnError, Result};

const C32_TAG: u16 = 0b0111111111110001; // 7FF1
const TAG_TAG: u16 = 0b0111111111110010; // 7FF2
const FUN_TAG: u16 = 0b1111111111110100; // FFF4
const ARR_TAG: u16 = 0b1111111111110111; // FFF7
const VAL_TAG: u16 = 0b1111111111110; // FFF.>>1, 13-bit prefix for heap objects
const MD1_TAG: u16 = 0b1111111111110010; // FFF2
const MD2_TAG: u16 = 0b1111111111110011; // FFF3
const NSP_TAG: u16 = 0b1111111111110101; // FFF5

pub const CHR_MAX: u32 = 1114111;

fn ftag(t: u16) -> u64 {
    (t as u64) << 48
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct B(pub u64);

impl B {
    pub const SENTINEL: B = B(0x7FF2000000000000); // bi_N

    pub fn is_f64(self) -> bool {
        // port of CBQN's isF64: (x.u<<1) - ((0xFFEull<<52) + 2) >= (1ull<<52) - 2
        (self.0 << 1).wrapping_sub((0xFFEu64 << 52) + 2) >= (1u64 << 52) - 2
    }

    pub fn is_arr(self) -> bool {
        (self.0 >> 48) as u16 == ARR_TAG
    }

    pub fn is_c32(self) -> bool {
        (self.0 >> 48) as u16 == C32_TAG
    }

    pub fn is_fun(self) -> bool {
        (self.0 >> 48) as u16 == FUN_TAG
    }

    pub fn is_md1(self) -> bool {
        (self.0 >> 48) as u16 == MD1_TAG
    }

    pub fn is_md2(self) -> bool {
        (self.0 >> 48) as u16 == MD2_TAG
    }

    pub fn is_nsp(self) -> bool {
        (self.0 >> 48) as u16 == NSP_TAG
    }

    pub fn is_tag(self) -> bool {
        (self.0 >> 48) as u16 == TAG_TAG
    }

    pub fn is_val(self) -> bool {
        // port of CBQN's isVal
        self.0.wrapping_sub(((VAL_TAG as u64) << 51) + 1) < ((1u64 << 51) - 1)
    }

    pub fn is_atom(self) -> bool {
        !self.is_arr()
    }

    pub fn is_num(self) -> bool {
        self.is_f64()
    }

    pub fn is_callable(self) -> bool {
        let tag = (self.0 >> 48) as u16;
        tag >= MD1_TAG && tag <= FUN_TAG
    }

    pub fn m_f64(n: f64) -> Self {
        let r = Self(n.to_bits());
        debug_assert!(r.is_f64());
        r
    }

    pub fn m_i32(n: i32) -> Self {
        Self::m_f64(n as f64)
    }

    pub fn m_usz(n: usize) -> Self {
        Self::m_f64(n as f64)
    }

    pub fn m_c32(n: u32) -> Self {
        Self(n as u64 | ftag(C32_TAG))
    }

    pub fn o2f(self) -> f64 {
        f64::from_bits(self.0)
    }

    pub fn to_f64(self) -> Result<f64> {
        if self.is_f64() {
            Ok(self.o2f())
        } else {
            Err(BqnError::Type("Expected number".into()))
        }
    }

    pub fn o2i(self) -> i32 {
        self.o2f() as i32
    }

    pub fn to_i32(self) -> Result<i32> {
        let f = self.to_f64()?;
        let i = f as i32;
        if f == i as f64 {
            Ok(i)
        } else {
            Err(BqnError::Type("Expected integer".into()))
        }
    }

    pub fn to_usz(self) -> Result<usize> {
        let f = self.to_f64()?;
        let u = f as usize;
        if f == u as f64 && f >= 0.0 {
            Ok(u)
        } else {
            Err(BqnError::Type("Expected non-negative integer".into()))
        }
    }

    pub fn o2c(self) -> Result<u32> {
        if self.is_c32() {
            Ok(self.0 as u32)
        } else {
            Err(BqnError::Type("Expected character".into()))
        }
    }

    pub fn q_i32(self) -> bool {
        if !self.is_f64() {
            return false;
        }
        let f = self.o2f();
        let i = f as i32;
        f == i as f64
    }

    pub fn q_bit(self) -> bool {
        self.is_num() && (self.o2f() == 0.0 || self.o2f() == 1.0)
    }

    pub fn q_i8(self) -> bool {
        if !self.is_f64() {
            return false;
        }
        let f = self.o2f();
        let i = f as i32;
        f == i as f64 && i == i as i8 as i32
    }

    pub fn q_i16(self) -> bool {
        if !self.is_f64() {
            return false;
        }
        let f = self.o2f();
        let i = f as i32;
        f == i as f64 && i == i as i16 as i32
    }

    pub fn atom_equal(self, other: B) -> bool {
        if self.is_f64() && other.is_f64() {
            return self.o2f() == other.o2f();
        }
        if self.is_c32() && other.is_c32() {
            return self.0 as u32 == other.0 as u32;
        }
        false
    }
}

impl std::fmt::Debug for B {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_f64() {
            write!(f, "B({:?})", self.o2f())
        } else if self.is_c32() {
            write!(f, "B('{}')", char::from_u32(self.0 as u32).unwrap_or('?'))
        } else if self.is_arr() {
            write!(f, "B(arr@{:#x})", self.0 & 0xFFFFFFFFFFFF)
        } else {
            write!(f, "B({:#018x})", self.0)
        }
    }
}
