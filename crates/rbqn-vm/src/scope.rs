use rbqn_core::value::B;

pub struct Scope {
    pub vars: Vec<B>,
    pub parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new(var_count: usize) -> Self {
        Scope {
            vars: vec![B::nothing(); var_count],
            parent: None,
        }
    }
}
