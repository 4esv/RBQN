use std::fmt;

#[derive(Clone, Debug)]
pub enum B {
    Num(f64),
    Char(char),
    Arr(Vec<B>, Vec<usize>),
    Fn(BFn),
    Md1(BMd1),
    Md2(BMd2),
    Ns(BNs),
    Nothing,
}

#[derive(Clone, Debug)]
pub struct BFn {
    pub name: String,
    pub prim_id: Option<u8>,
    pub c1: Option<fn(&B, &B) -> crate::error::Result<B>>,
    pub c2: Option<fn(&B, &B, &B) -> crate::error::Result<B>>,
}

#[derive(Clone, Debug)]
pub struct BMd1 {
    pub name: String,
    pub prim_id: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct BMd2 {
    pub name: String,
    pub prim_id: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct BNs {
    pub fields: Vec<(String, B)>,
}

impl B {
    pub fn num(n: f64) -> Self {
        B::Num(n)
    }
    pub fn char(c: char) -> Self {
        B::Char(c)
    }
    pub fn nothing() -> Self {
        B::Nothing
    }
    pub fn is_nothing(&self) -> bool {
        matches!(self, B::Nothing)
    }
    pub fn is_fn(&self) -> bool {
        matches!(self, B::Fn(_))
    }
    pub fn is_md1(&self) -> bool {
        matches!(self, B::Md1(_))
    }
    pub fn is_md2(&self) -> bool {
        matches!(self, B::Md2(_))
    }
    pub fn is_arr(&self) -> bool {
        matches!(self, B::Arr(..))
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            B::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_char(&self) -> Option<char> {
        match self {
            B::Char(c) => Some(*c),
            _ => None,
        }
    }
    pub fn empty_list() -> Self {
        B::Arr(Vec::new(), vec![0])
    }
    pub fn list(items: Vec<B>) -> Self {
        let len = items.len();
        B::Arr(items, vec![len])
    }
    pub fn c32_vec(s: &str) -> Self {
        let chars: Vec<B> = s.chars().map(B::Char).collect();
        let len = chars.len();
        B::Arr(chars, vec![len])
    }
}

impl fmt::Display for B {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            B::Num(n) => write!(f, "{n}"),
            B::Char(c) => write!(f, "'{c}'"),
            B::Arr(items, _) => {
                write!(f, "⟨")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "⟩")
            }
            B::Fn(bfn) => write!(f, "{}", bfn.name),
            B::Md1(m) => write!(f, "{}", m.name),
            B::Md2(m) => write!(f, "{}", m.name),
            B::Ns(_) => write!(f, "(namespace)"),
            B::Nothing => write!(f, "·"),
        }
    }
}

pub const RT_LEN: usize = 64;
