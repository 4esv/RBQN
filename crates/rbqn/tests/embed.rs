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
