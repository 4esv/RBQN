use crate::eltype::ElType;
use crate::error::{BqnError, Result};
use crate::value::B;

pub type Rank = u8;
pub const RANK_MAX: Rank = 255;

#[derive(Debug, Clone)]
pub struct BqnArr {
    pub shape: Vec<usize>,
    pub data: ArrData,
    pub fill: Option<B>,
}

#[derive(Debug, Clone)]
pub enum ArrData {
    Bit(Vec<u64>),
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    F64(Vec<f64>),
    C8(Vec<u8>),
    C16(Vec<u16>),
    C32(Vec<u32>),
    Boxed(Vec<B>),
}

impl ArrData {
    pub fn el_type(&self) -> ElType {
        match self {
            ArrData::Bit(_) => ElType::Bit,
            ArrData::I8(_) => ElType::I8,
            ArrData::I16(_) => ElType::I16,
            ArrData::I32(_) => ElType::I32,
            ArrData::F64(_) => ElType::F64,
            ArrData::C8(_) => ElType::C8,
            ArrData::C16(_) => ElType::C16,
            ArrData::C32(_) => ElType::C32,
            ArrData::Boxed(_) => ElType::B,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrData::Bit(v) => {
                // bit arrays store ia separately, but for now use bit count heuristic
                // actual ia is in BqnArr.shape product
                v.len() * 64 // overestimate; real length from shape
            }
            ArrData::I8(v) => v.len(),
            ArrData::I16(v) => v.len(),
            ArrData::I32(v) => v.len(),
            ArrData::F64(v) => v.len(),
            ArrData::C8(v) => v.len(),
            ArrData::C16(v) => v.len(),
            ArrData::C32(v) => v.len(),
            ArrData::Boxed(v) => v.len(),
        }
    }
}

impl BqnArr {
    pub fn ia(&self) -> usize {
        self.shape.iter().copied().product::<usize>().max(0)
    }

    pub fn rank(&self) -> Rank {
        self.shape.len() as Rank
    }

    pub fn el_type(&self) -> ElType {
        self.data.el_type()
    }

    pub fn get(&self, i: usize) -> Result<B> {
        if i >= self.ia() {
            return Err(BqnError::Domain(format!("Index {i} out of bounds")));
        }
        Ok(match &self.data {
            ArrData::Bit(v) => {
                let word = i / 64;
                let bit = i % 64;
                B::m_i32(((v[word] >> bit) & 1) as i32)
            }
            ArrData::I8(v) => B::m_i32(v[i] as i32),
            ArrData::I16(v) => B::m_i32(v[i] as i32),
            ArrData::I32(v) => B::m_i32(v[i]),
            ArrData::F64(v) => B::m_f64(v[i]),
            ArrData::C8(v) => B::m_c32(v[i] as u32),
            ArrData::C16(v) => B::m_c32(v[i] as u32),
            ArrData::C32(v) => B::m_c32(v[i]),
            ArrData::Boxed(v) => v[i],
        })
    }

    pub fn i32_iter(&self) -> Result<Vec<i32>> {
        match &self.data {
            ArrData::Bit(v) => {
                let ia = self.ia();
                Ok((0..ia)
                    .map(|i| ((v[i / 64] >> (i % 64)) & 1) as i32)
                    .collect())
            }
            ArrData::I8(v) => Ok(v.iter().map(|&x| x as i32).collect()),
            ArrData::I16(v) => Ok(v.iter().map(|&x| x as i32).collect()),
            ArrData::I32(v) => Ok(v.clone()),
            ArrData::F64(v) => v
                .iter()
                .map(|&x| {
                    let i = x as i32;
                    if x == i as f64 {
                        Ok(i)
                    } else {
                        Err(BqnError::Type("Expected integer".into()))
                    }
                })
                .collect(),
            _ => Err(BqnError::Type("Expected numeric array".into())),
        }
    }

    pub fn f64_iter(&self) -> Result<Vec<f64>> {
        match &self.data {
            ArrData::Bit(v) => {
                let ia = self.ia();
                Ok((0..ia)
                    .map(|i| ((v[i / 64] >> (i % 64)) & 1) as f64)
                    .collect())
            }
            ArrData::I8(v) => Ok(v.iter().map(|&x| x as f64).collect()),
            ArrData::I16(v) => Ok(v.iter().map(|&x| x as f64).collect()),
            ArrData::I32(v) => Ok(v.iter().map(|&x| x as f64).collect()),
            ArrData::F64(v) => Ok(v.clone()),
            _ => Err(BqnError::Type("Expected numeric array".into())),
        }
    }

    pub fn new_vec_i32(data: Vec<i32>) -> Self {
        let len = data.len();
        BqnArr {
            shape: vec![len],
            data: ArrData::I32(data),
            fill: Some(B::m_i32(0)),
        }
    }

    pub fn new_vec_f64(data: Vec<f64>) -> Self {
        let len = data.len();
        BqnArr {
            shape: vec![len],
            data: ArrData::F64(data),
            fill: Some(B::m_f64(0.0)),
        }
    }

    pub fn new_vec_b(data: Vec<B>) -> Self {
        let len = data.len();
        BqnArr {
            shape: vec![len],
            data: ArrData::Boxed(data),
            fill: None,
        }
    }

    pub fn new_vec_c32(data: Vec<u32>) -> Self {
        let len = data.len();
        BqnArr {
            shape: vec![len],
            data: ArrData::C32(data),
            fill: Some(B::m_c32(b' ' as u32)),
        }
    }

    pub fn empty_vec() -> Self {
        BqnArr {
            shape: vec![0],
            data: ArrData::Boxed(vec![]),
            fill: None,
        }
    }

    pub fn with_shape(mut self, shape: Vec<usize>) -> Self {
        self.shape = shape;
        self
    }
}

// Squeeze: try to narrow an f64 array to a smaller integer type
pub fn squeeze_num(arr: BqnArr) -> BqnArr {
    if arr.el_type() != ElType::F64 {
        return arr;
    }
    let vals = match &arr.data {
        ArrData::F64(v) => v,
        _ => return arr,
    };

    let mut all_bit = true;
    let mut all_i8 = true;
    let mut all_i16 = true;
    let mut all_i32 = true;

    for &v in vals {
        let i = v as i32;
        if v != i as f64 {
            all_bit = false;
            all_i8 = false;
            all_i16 = false;
            all_i32 = false;
            break;
        }
        if v != 0.0 && v != 1.0 {
            all_bit = false;
        }
        if i != i as i8 as i32 {
            all_i8 = false;
        }
        if i != i as i16 as i32 {
            all_i16 = false;
        }
        all_i32 = true; // already checked v == i as f64
    }

    let data = if all_bit {
        let ia = vals.len();
        let nwords = (ia + 63) / 64;
        let mut words = vec![0u64; nwords];
        for (i, &v) in vals.iter().enumerate() {
            if v == 1.0 {
                words[i / 64] |= 1 << (i % 64);
            }
        }
        ArrData::Bit(words)
    } else if all_i8 {
        ArrData::I8(vals.iter().map(|&v| v as i8).collect())
    } else if all_i16 {
        ArrData::I16(vals.iter().map(|&v| v as i16).collect())
    } else if all_i32 {
        ArrData::I32(vals.iter().map(|&v| v as i32).collect())
    } else {
        return arr;
    };

    BqnArr {
        shape: arr.shape,
        data,
        fill: arr.fill,
    }
}
