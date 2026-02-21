#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct B(pub u64);

impl B {
    pub fn m_f64(n: f64) -> Self {
        Self(n.to_bits())
    }

    pub fn o2f(self) -> f64 {
        f64::from_bits(self.0)
    }
}
