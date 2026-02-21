use std::sync::Arc;

use crate::eltype::ElType;
use crate::error::throw;
use crate::value::{B, bi_noFill, m_c32, m_f64, m_i32};

#[derive(Debug, Clone)]
pub enum ArrayStorage {
    HArr(Vec<B>),
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    F64(Vec<f64>),
    C8(Vec<u8>),
    C16(Vec<u16>),
    C32(Vec<u32>),
    Bit(Vec<u64>),
}

impl ArrayStorage {
    pub fn el_type(&self) -> ElType {
        match self {
            ArrayStorage::Bit(_) => ElType::Bit,
            ArrayStorage::I8(_) => ElType::I8,
            ArrayStorage::I16(_) => ElType::I16,
            ArrayStorage::I32(_) => ElType::I32,
            ArrayStorage::F64(_) => ElType::F64,
            ArrayStorage::C8(_) => ElType::C8,
            ArrayStorage::C16(_) => ElType::C16,
            ArrayStorage::C32(_) => ElType::C32,
            ArrayStorage::HArr(_) => ElType::B,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrayStorage::HArr(v) => v.len(),
            ArrayStorage::I8(v) => v.len(),
            ArrayStorage::I16(v) => v.len(),
            ArrayStorage::I32(v) => v.len(),
            ArrayStorage::F64(v) => v.len(),
            ArrayStorage::C8(v) => v.len(),
            ArrayStorage::C16(v) => v.len(),
            ArrayStorage::C32(v) => v.len(),
            ArrayStorage::Bit(v) => v.len() * 64, // approximate; actual count tracked by ia
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(pub u8);

impl Flags {
    pub const NONE: Flags = Flags(0);
    pub const SQUOZE: Flags = Flags(1);
    pub const ASC: Flags = Flags(2);
    pub const DSC: Flags = Flags(4);
}

#[derive(Debug, Clone)]
pub struct BqnArr {
    pub ia: usize,
    pub shape: Vec<usize>,
    pub storage: ArrayStorage,
    pub fill: B,
    pub flags: Flags,
}

impl BqnArr {
    pub fn new(storage: ArrayStorage, shape: Vec<usize>, fill: B) -> Self {
        let ia = if shape.is_empty() {
            1
        } else {
            shape.iter().product()
        };
        BqnArr {
            ia,
            shape,
            storage,
            fill,
            flags: Flags::NONE,
        }
    }

    pub fn rank(&self) -> usize {
        self.shape.len()
    }

    pub fn get(&self, idx: usize) -> B {
        if idx >= self.ia {
            throw(format!("Index {} out of bounds for array of size {}", idx, self.ia));
        }
        self.get_unchecked(idx)
    }

    pub fn get_unchecked(&self, idx: usize) -> B {
        match &self.storage {
            ArrayStorage::HArr(v) => v[idx],
            ArrayStorage::I8(v) => m_i32(v[idx] as i32),
            ArrayStorage::I16(v) => m_i32(v[idx] as i32),
            ArrayStorage::I32(v) => m_i32(v[idx]),
            ArrayStorage::F64(v) => m_f64(v[idx]),
            ArrayStorage::C8(v) => m_c32(v[idx] as u32),
            ArrayStorage::C16(v) => m_c32(v[idx] as u32),
            ArrayStorage::C32(v) => m_c32(v[idx]),
            ArrayStorage::Bit(v) => {
                let word = idx / 64;
                let bit = idx % 64;
                m_i32(((v[word] >> bit) & 1) as i32)
            }
        }
    }

    pub fn from_b_vec(v: Vec<B>) -> Self {
        let ia = v.len();
        BqnArr {
            ia,
            shape: vec![ia],
            storage: ArrayStorage::HArr(v),
            fill: bi_noFill,
            flags: Flags::NONE,
        }
    }

    pub fn from_i32_vec(v: Vec<i32>) -> Self {
        let ia = v.len();
        BqnArr {
            ia,
            shape: vec![ia],
            storage: ArrayStorage::I32(v),
            fill: m_f64(0.0),
            flags: Flags::NONE,
        }
    }

    pub fn from_string(s: &str) -> Self {
        let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
        let ia = chars.len();
        let all_ascii = chars.iter().all(|&c| c < 128);
        let storage = if all_ascii {
            ArrayStorage::C8(chars.iter().map(|&c| c as u8).collect())
        } else {
            ArrayStorage::C32(chars)
        };
        BqnArr {
            ia,
            shape: vec![ia],
            storage,
            fill: m_c32(' ' as u32),
            flags: Flags::NONE,
        }
    }

    pub fn empty_harr() -> Self {
        BqnArr {
            ia: 0,
            shape: vec![0],
            storage: ArrayStorage::HArr(Vec::new()),
            fill: bi_noFill,
            flags: Flags::NONE,
        }
    }
}

pub type ArcArr = Arc<BqnArr>;

pub fn new_arr(arr: BqnArr) -> ArcArr {
    Arc::new(arr)
}
