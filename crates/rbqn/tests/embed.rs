use rbqn::Bqn;

#[test]
fn eval_number() {
    let b = Bqn::new().unwrap();
    assert_eq!(b.eval("6×7").unwrap().to_f64(), Some(42.0));
    assert_eq!(b.eval("6×7").unwrap().fmt(), "42");
}

#[test]
fn list_round_trip() {
    let b = Bqn::new().unwrap();
    let v = b.list_f64(&[1.5, 2.5, -3.0]);
    assert_eq!(v.shape(), vec![3]);
    assert_eq!(v.to_vec_f64().unwrap(), vec![1.5, 2.5, -3.0]);
    assert_eq!(b.list_i32(&[1, 2]).to_vec_i32().unwrap(), vec![1, 2]);
    assert!(v.to_vec_i32().is_err());
    let a = b.array_f64(&[2, 2], &[1.0, 2.0, 3.0, 4.0]).unwrap();
    assert_eq!(a.shape(), vec![2, 2]);
    assert!(b.array_f64(&[2, 2], &[1.0]).is_err());
}

#[test]
fn string_value() {
    let b = Bqn::new().unwrap();
    assert_eq!(b.eval("\"héllo\"").unwrap().to_string_lossy().as_deref(), Some("héllo"));
    assert_eq!(b.string("ab").to_string_lossy().as_deref(), Some("ab"));
    assert_eq!(b.list_f64(&[1.0]).to_string_lossy(), None);
}

#[test]
fn namespace_field() {
    let b = Bqn::new().unwrap();
    let ns = b.eval("{a⇐3 ⋄ b⇐\"x\"}").unwrap();
    assert_eq!(ns.field("a").unwrap().to_f64(), Some(3.0));
    assert_eq!(ns.field("b").unwrap().to_string_lossy().as_deref(), Some("x"));
    assert!(ns.field("zzz").is_err());
}

#[test]
fn error_with_site() {
    let b = Bqn::new().unwrap();
    let e = b.eval("1+1\n\"a\"+\"b\"").unwrap_err();
    assert!(!e.message.is_empty());
    let s = e.site.expect("site");
    assert_eq!(s.line, 2);
    assert_eq!(s.src_line, "\"a\"+\"b\"");
    assert!(s.caret.contains('^'));
    // The interpreter still works after an error.
    assert_eq!(b.eval("1+1").unwrap().to_f64(), Some(2.0));
}

#[test]
fn call_functions() {
    let b = Bqn::new().unwrap();
    let plus = b.eval("+").unwrap();
    let w = b.list_f64(&[1.0, 2.0]);
    let x = b.list_f64(&[10.0, 20.0]);
    assert_eq!(b.call2(&plus, &w, &x).unwrap().to_vec_f64().unwrap(), vec![11.0, 22.0]);
    let neg = b.eval("-").unwrap();
    assert_eq!(b.call1(&neg, &x).unwrap().to_vec_f64().unwrap(), vec![-10.0, -20.0]);
    assert!(b.call1(&w, &x).is_err());
}

// The panic hook is process-global and `Bqn` installs its quiet hook once on
// first use, so this runs in a child process where nothing else has touched it.
#[test]
fn host_panic_hook_survives_eval() {
    if std::env::var_os("RBQN_EMBED_HOOK_CHILD").is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "host_panic_hook_survives_eval", "--nocapture", "--test-threads=1"])
            .env("RBQN_EMBED_HOOK_CHILD", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "child failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        return;
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    std::panic::set_hook(Box::new(|_| {
        CALLS.fetch_add(1, Ordering::SeqCst);
    }));
    let b = Bqn::new().unwrap();
    // A BQN error comes back as Err and does not reach the host hook.
    assert!(b.eval("1+\"a\"+@+").is_err());
    assert!(b.eval("+").and_then(|f| b.call1(&f, &b.string("a"))).is_err());
    assert_eq!(CALLS.load(Ordering::SeqCst), 0, "BQN errors leaked to the host hook");
    // A genuine panic still reaches the host hook, and it is still installed.
    assert!(std::panic::catch_unwind(|| panic!("genuine")).is_err());
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    assert!(std::panic::catch_unwind(|| panic!("again")).is_err());
    assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    // Another thread's genuine panics must reach the host hook while this
    // thread is evaluating failing expressions (the old per-call swap lost them).
    const N: usize = 3000;
    let t = std::thread::spawn(|| {
        for _ in 0..N {
            let _ = std::panic::catch_unwind(|| panic!("other thread"));
        }
    });
    for _ in 0..N {
        assert!(b.eval("\"a\"+\"b\"").is_err());
    }
    t.join().unwrap();
    assert_eq!(CALLS.load(Ordering::SeqCst), 2 + N, "host hook lost panics or was replaced");
}
