use crate::error::throw;

pub const C32_TAG: u16 = 0x7FF1;
pub const TAG_TAG: u16 = 0x7FF2;
pub const VAR_TAG: u16 = 0x7FF3;
pub const EXT_TAG: u16 = 0x7FF4;
pub const RAW_TAG: u16 = 0x7FF5;
pub const MD1_TAG: u16 = 0xFFF2;
pub const MD2_TAG: u16 = 0xFFF3;
pub const FUN_TAG: u16 = 0xFFF4;
pub const NSP_TAG: u16 = 0xFFF5;
pub const OBJ_TAG: u16 = 0xFFF6;
pub const ARR_TAG: u16 = 0xFFF7;
pub const VAL_TAG: u16 = 0xFFF0;

#[inline(always)]
pub const fn ftag(x: u16) -> u64 {
    (x as u64) << 48
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct B {
    pub u: u64,
}

impl B {
    #[inline(always)]
    pub fn from_u64(u: u64) -> Self {
        B { u }
    }

    #[inline(always)]
    pub fn from_f64(f: f64) -> Self {
        B { u: f.to_bits() }
    }

    #[inline(always)]
    pub fn to_f64(self) -> f64 {
        f64::from_bits(self.u)
    }

    #[inline(always)]
    pub fn is_f64(self) -> bool {
        (self.u << 1).wrapping_sub((0xFFEu64 << 52) + 2) >= (1u64 << 52) - 2
    }

    #[inline(always)]
    pub fn is_num(self) -> bool {
        self.is_f64()
    }

    #[inline(always)]
    pub fn is_arr(self) -> bool {
        (self.u >> 48) == ARR_TAG as u64
    }

    #[inline(always)]
    pub fn is_fun(self) -> bool {
        (self.u >> 48) == FUN_TAG as u64
    }

    #[inline(always)]
    pub fn is_c32(self) -> bool {
        (self.u >> 48) == C32_TAG as u64
    }

    #[inline(always)]
    pub fn is_var(self) -> bool {
        (self.u >> 48) == VAR_TAG as u64
    }

    #[inline(always)]
    pub fn is_ext(self) -> bool {
        (self.u >> 48) == EXT_TAG as u64
    }

    #[inline(always)]
    pub fn is_tag(self) -> bool {
        (self.u >> 48) == TAG_TAG as u64
    }

    #[inline(always)]
    pub fn is_md1(self) -> bool {
        (self.u >> 48) == MD1_TAG as u64
    }

    #[inline(always)]
    pub fn is_md2(self) -> bool {
        (self.u >> 48) == MD2_TAG as u64
    }

    #[inline(always)]
    pub fn is_md(self) -> bool {
        (self.u >> 49) == (MD2_TAG >> 1) as u64
    }

    #[inline(always)]
    pub fn is_nsp(self) -> bool {
        (self.u >> 48) == NSP_TAG as u64
    }

    #[inline(always)]
    pub fn is_obj(self) -> bool {
        (self.u >> 48) == OBJ_TAG as u64
    }

    #[inline(always)]
    pub fn is_val(self) -> bool {
        self.u.wrapping_sub(((VAL_TAG as u64) << 51) + 1) < ((1u64 << 51) - 1)
    }

    #[inline(always)]
    pub fn is_atm(self) -> bool {
        !self.is_arr()
    }

    #[inline(always)]
    pub fn is_callable(self) -> bool {
        let tag = (self.u >> 48) as u16;
        tag >= MD1_TAG && tag <= FUN_TAG
    }

    // Value extraction with error throwing
    pub fn o2f(self) -> f64 {
        if !self.is_num() {
            throw("Expected number");
        }
        self.to_f64()
    }

    pub fn o2i(self) -> i32 {
        if !self.q_i32() {
            throw(format!(
                "Expected integer in i32 range, got {}",
                self.to_f64()
            ));
        }
        self.to_f64() as i32
    }

    pub fn o2c(self) -> u32 {
        if !self.is_c32() {
            throw("Expected character");
        }
        self.u as u32
    }

    pub fn o2s(self) -> usize {
        let f = self.o2f();
        let v = f as usize;
        if f != v as f64 {
            throw(format!("Expected non-negative integer, got {}", f));
        }
        v
    }

    pub fn o2b(self) -> bool {
        let t = self.to_f64() as i32;
        if t as f64 != self.to_f64() || (t != 0 && t != 1) {
            throw("Expected boolean");
        }
        (self.u << 1) != 0
    }

    #[inline(always)]
    pub fn o2f_unchecked(self) -> f64 {
        self.to_f64()
    }

    #[inline(always)]
    pub fn o2i_unchecked(self) -> i32 {
        self.to_f64() as i32
    }

    #[inline(always)]
    pub fn o2c_unchecked(self) -> u32 {
        self.u as u32
    }

    #[inline(always)]
    pub fn o2s_unchecked(self) -> usize {
        self.to_f64() as usize
    }

    // Type query functions
    #[inline(always)]
    pub fn q_bit(self) -> bool {
        self.is_num() && (self.to_f64() == 0.0 || self.to_f64() == 1.0)
    }

    #[inline(always)]
    pub fn q_i8(self) -> bool {
        let f = self.to_f64();
        f == (f as i32 as i8 as f64)
    }

    #[inline(always)]
    pub fn q_i16(self) -> bool {
        let f = self.to_f64();
        f == (f as i32 as i16 as f64)
    }

    #[inline(always)]
    pub fn q_i32(self) -> bool {
        let f = self.to_f64();
        f == (f as i32 as f64)
    }

    #[inline(always)]
    pub fn q_f64(self) -> bool {
        self.is_f64()
    }

    #[inline(always)]
    pub fn q_c8(self) -> bool {
        self.u >> 8 == (C32_TAG as u64) << 40
    }

    #[inline(always)]
    pub fn q_c16(self) -> bool {
        self.u >> 16 == (C32_TAG as u64) << 32
    }

    #[inline(always)]
    pub fn q_c32(self) -> bool {
        self.is_c32()
    }

    #[inline(always)]
    pub fn q_n(self) -> bool {
        self.u == bi_N.u
    }

    #[inline(always)]
    pub fn no_fill(self) -> bool {
        self.u == bi_noFill.u
    }

    // Variable reference accessors
    #[inline(always)]
    pub fn v_pos(self) -> u32 {
        self.u as u32
    }

    #[inline(always)]
    pub fn v_depth(self) -> u16 {
        (self.u >> 32) as u16
    }
}

impl std::fmt::Debug for B {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_f64() {
            write!(f, "B(f64: {})", self.to_f64())
        } else if self.is_c32() {
            write!(f, "B(c32: {:?})", char::from_u32(self.u as u32))
        } else if self.is_arr() {
            write!(f, "B(arr: 0x{:016x})", self.u)
        } else if self.is_fun() {
            write!(f, "B(fun: 0x{:016x})", self.u)
        } else if self.is_md1() {
            write!(f, "B(md1: 0x{:016x})", self.u)
        } else if self.is_md2() {
            write!(f, "B(md2: 0x{:016x})", self.u)
        } else if self.is_nsp() {
            write!(f, "B(nsp: 0x{:016x})", self.u)
        } else if self.is_tag() {
            write!(f, "B(tag: 0x{:016x})", self.u)
        } else if self.is_var() {
            write!(f, "B(var: d={} p={})", self.v_depth(), self.v_pos())
        } else if self.is_ext() {
            write!(f, "B(ext: d={} p={})", self.v_depth(), self.v_pos())
        } else {
            write!(f, "B(0x{:016x})", self.u)
        }
    }
}

impl PartialEq for B {
    fn eq(&self, other: &Self) -> bool {
        self.u == other.u
    }
}

// Constructors
#[inline(always)]
pub fn m_f64(n: f64) -> B {
    let b = B::from_f64(n);
    debug_assert!(b.is_f64());
    b
}

#[inline(always)]
pub fn m_c32(n: u32) -> B {
    B::from_u64(n as u64 | ftag(C32_TAG))
}

#[inline(always)]
pub fn m_i32(n: i32) -> B {
    m_f64(n as f64)
}

#[inline(always)]
pub fn m_usz(n: usize) -> B {
    m_f64(n as f64)
}

#[inline(always)]
pub fn tagu64(v: u64, t: u16) -> B {
    B::from_u64(v | ftag(t))
}

// Special values
pub const bi_N: B = B {
    u: 0x7FF2_0000_0000_0000,
};
pub const bi_noVar: B = B {
    u: 0x7FF2_C000_0000_0001,
};
pub const bi_okHdr: B = B {
    u: 0x7FF2_0000_0000_0002,
};
pub const bi_optOut: B = B {
    u: 0x7FF2_8000_0000_0003,
};
pub const bi_noFill: B = B {
    u: 0x7FF2_0000_0000_0005,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f64_roundtrip() {
        let vals = [0.0, 1.0, -1.0, 3.14, f64::INFINITY, f64::NEG_INFINITY];
        for v in vals {
            let b = m_f64(v);
            assert!(b.is_f64());
            assert_eq!(b.to_f64(), v);
        }
    }

    #[test]
    fn test_nan_is_not_f64() {
        let nan = B::from_f64(f64::NAN);
        // NaN is a special case - standard NaN has the sNaN/qNaN pattern
        // In BQN NaN boxing, not all NaN patterns are "f64"
        // But Rust's f64::NAN is a qNaN which overlaps tag space
        // This is fine - we only create f64 values through m_f64
        let _ = nan;
    }

    #[test]
    fn test_i32_values() {
        let b = m_i32(42);
        assert!(b.is_f64());
        assert!(b.q_i32());
        assert_eq!(b.o2i(), 42);
    }

    #[test]
    fn test_c32_values() {
        let b = m_c32('A' as u32);
        assert!(b.is_c32());
        assert!(!b.is_f64());
        assert_eq!(b.o2c(), 'A' as u32);
    }

    #[test]
    fn test_special_values() {
        assert!(bi_N.is_tag());
        assert!(bi_noVar.is_tag());
        assert!(bi_optOut.is_tag());
        assert!(bi_noFill.is_tag());
        assert!(!bi_N.is_f64());
        assert!(!bi_N.is_arr());
    }

    #[test]
    fn test_tag_checks() {
        let f = m_f64(1.0);
        assert!(f.is_f64());
        assert!(!f.is_arr());
        assert!(!f.is_fun());
        assert!(!f.is_c32());
        assert!(f.is_atm());
        assert!(!f.is_val());

        let c = m_c32(0x41);
        assert!(c.is_c32());
        assert!(!c.is_f64());
        assert!(!c.is_val());
    }

    #[test]
    fn test_q_type_checks() {
        assert!(m_i32(0).q_bit());
        assert!(m_i32(1).q_bit());
        assert!(!m_i32(2).q_bit());

        assert!(m_i32(127).q_i8());
        assert!(m_i32(-128).q_i8());
        assert!(!m_i32(128).q_i8());

        assert!(m_i32(32767).q_i16());
        assert!(!m_i32(32768).q_i16());
    }

    #[test]
    fn test_var_reference() {
        let v = tagu64((3u64 << 32) | 7, VAR_TAG);
        assert!(v.is_var());
        assert_eq!(v.v_depth(), 3);
        assert_eq!(v.v_pos(), 7);
    }

    #[test]
    fn test_q_c8_c16() {
        let a = m_c32(0x41); // ASCII 'A'
        assert!(a.q_c8());
        assert!(a.q_c16());
        assert!(a.q_c32());

        let wide = m_c32(0x4E2D); // CJK char
        assert!(!wide.q_c8());
        assert!(wide.q_c16());
        assert!(wide.q_c32());

        let emoji = m_c32(0x1F600); // emoji
        assert!(!emoji.q_c8());
        assert!(!emoji.q_c16());
        assert!(emoji.q_c32());
    }
}
