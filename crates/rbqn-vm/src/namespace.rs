use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rbqn_core::value::B;

use crate::scope::Scope;

#[derive(Debug)]
pub struct NSDesc {
    pub var_am: i32,
    pub exp_gids: Vec<i32>,
}

#[derive(Debug)]
pub struct NS {
    pub desc: Arc<NSDesc>,
    pub sc: Arc<Scope>,
}

static GID_MAP: std::sync::LazyLock<Mutex<HashMap<String, i32>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

static GID_NAMES: std::sync::LazyLock<Mutex<Vec<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(Vec::new()));

pub fn str2gid(s: &str) -> i32 {
    let mut map = GID_MAP.lock().unwrap();
    if let Some(&id) = map.get(s) {
        return id;
    }
    let mut names = GID_NAMES.lock().unwrap();
    let id = names.len() as i32;
    names.push(s.to_string());
    map.insert(s.to_string(), id);
    id
}

pub fn gid2str(id: i32) -> String {
    let names = GID_NAMES.lock().unwrap();
    names[id as usize].clone()
}

impl NS {
    pub fn get_by_gid(&self, gid: i32) -> Option<B> {
        for (i, &exp_gid) in self.desc.exp_gids.iter().enumerate() {
            if exp_gid == gid {
                if i < self.sc.vars.len() {
                    return Some(self.sc.vars[i]);
                }
            }
        }
        None
    }
}
