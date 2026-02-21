use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU64;

use rbqn_core::value::B;
use rbqn_core::{tagu64, NSP_TAG};

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

static NS_COUNTER: AtomicU64 = AtomicU64::new(1);

static NS_STORE: std::sync::LazyLock<Mutex<HashMap<u64, Arc<NS>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn store_ns(ns: NS) -> B {
    let id = NS_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    NS_STORE.lock().unwrap().insert(id, Arc::new(ns));
    tagu64(id << 3, NSP_TAG)
}

pub fn get_ns(b: B) -> Arc<NS> {
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    NS_STORE.lock().unwrap().get(&id).cloned()
        .unwrap_or_else(|| rbqn_core::error::throw("Invalid namespace reference"))
}

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
