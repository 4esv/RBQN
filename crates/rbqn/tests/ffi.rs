//! •FFI end-to-end tests: compile tests/ffi/lib.c with `cc` and call it (and libc) from BQN.
//! Skipped with a message when `cc` is absent.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Build the fixture once; None when no C compiler is available.
fn fixture() -> Option<&'static Path> {
    static LIB: OnceLock<Option<PathBuf>> = OnceLock::new();
    LIB.get_or_init(|| {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ffi/lib.c");
        let dir = std::env::temp_dir().join(format!("rbqn-ffi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = dir.join(if cfg!(target_os = "macos") { "libtest.dylib" } else { "libtest.so" });
        match Command::new("cc").args(["-shared", "-fPIC", "-O1", "-o"]).arg(&lib).arg(&src).output() {
            Ok(o) if o.status.success() => Some(lib),
            Ok(o) => panic!("cc failed on tests/ffi/lib.c:\n{}", String::from_utf8_lossy(&o.stderr)),
            Err(_) => None,
        }
    }).as_deref()
}

fn libc_name() -> &'static str {
    if cfg!(target_os = "macos") { "libSystem.dylib" } else { "libc.so.6" }
}

fn run_with(bqn_args: &[&str]) -> (String, String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(bqn_args).output().expect("run rbqn");
    (String::from_utf8_lossy(&out.stdout).trim().to_string(),
     String::from_utf8_lossy(&out.stderr).trim().to_string(), out.status.success())
}

fn eval(src: &str) -> String {
    let (o, e, ok) = run_with(&["-p", src]);
    assert!(ok, "rbqn failed on {src:?}: {e}{o}");
    o
}

fn eval_err(src: &str) -> String {
    let (o, e, ok) = run_with(&["-p", src]);
    assert!(!ok, "expected an error from {src:?}, got {o}");
    format!("{o}{e}")
}

/// Evaluate `src` with `L` bound to the fixture loader: `L ← "<lib>" •FFI`.
fn eval_lib(src: &str) -> String {
    let lib = fixture().unwrap();
    eval(&format!("L←{{\"{}\"•FFI 𝕩}} ⋄ {}", lib.display(), src))
}

macro_rules! need_cc {
    () => {
        if fixture().is_none() {
            eprintln!("SKIP: no `cc` on PATH, can't build tests/ffi/lib.c");
            return;
        }
    };
}

#[test]
fn libc_abs_marker_gt() {
    assert_eq!(eval(&format!("f←\"{}\" •FFI \"i32\"‿\"abs\"‿\">i32\" ⋄ F ¯5", libc_name())), "5");
}

#[test]
fn libc_pow_w_marker() {
    assert_eq!(eval(&format!("f←\"{}\" •FFI \"f64\"‿\"pow\"‿\"𝕨f64\"‿\"f64\" ⋄ 2 F 10", libc_name())), "1024");
}

#[test]
fn libc_scalar_back_compat() {
    assert_eq!(eval(&format!("f←\"{}\" •FFI \"f64\"‿\"sqrt\"‿\"f64\" ⋄ F 2", libc_name())), "1.4142135623730951");
}

#[test]
fn libc_strlen() {
    assert_eq!(eval(&format!("f←\"{}\" •FFI \"u64\"‿\"strlen\"‿\"*u8:c8\" ⋄ F ⟨\"hello\"⟩", libc_name())), "5");
}

#[test]
fn libc_getenv() {
    // @ keeps meaning the process (libc) handle.
    assert_eq!(eval("f←\"@\" •FFI \"*u8:c8\"‿\"getenv\"‿\"*u8:c8\" ⋄ F ⟨\"PATH\"⟩"),
               format!("{:?}", std::env::var("PATH").unwrap()).replace("\\\\", "\\"));
}

#[test]
fn libc_qsort_mutable_buffer() {
    need_cc!();
    // The comparator pointer comes from the fixture; the `&` buffer is sorted in place by libc.
    let out = eval_lib(&format!(
        "g←L \"*\"‿\"get_cmp\" ⋄ cmp←G ⟨⟩ ⋄ q←\"{}\" •FFI \"&\"‿\"qsort\"‿\"&i32\"‿\"u64\"‿\"u64\"‿\"*\" ⋄ Q ⟨5‿3‿9‿1‿7‿¯2, 6, 4, cmp⟩",
        libc_name()));
    assert_eq!(out, "⟨ ¯2 1 3 5 7 9 ⟩");
}

#[test]
fn every_scalar_type() {
    need_cc!();
    for t in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        assert_eq!(eval_lib(&format!("f←L \"{t}\"‿\"t_{t}\"‿\"{t}\" ⋄ F 41")), "42", "{t}");
    }
    assert_eq!(eval_lib("f←L \"f32\"‿\"t_f32\"‿\"f32\" ⋄ F 1.5"), "3");
    assert_eq!(eval_lib("f←L \"f64\"‿\"t_f64\"‿\"f64\" ⋄ F 1.25"), "2.5");
    // Wrap-around lands at the type's edge, so range errors are the caller's job.
    assert_eq!(eval_lib("f←L \"i8\"‿\"t_i8\"‿\"i8\" ⋄ F ¯128"), "¯127");
    assert_eq!(eval_lib("f←L \"u8\"‿\"t_u8\"‿\"u8\" ⋄ F 254"), "255");
}

#[test]
fn out_of_range_and_non_integer_args_error() {
    need_cc!();
    assert!(eval_err(&format!("f←\"{}\" •FFI \"i32\"‿\"abs\"‿\"i32\" ⋄ F 3000000000", libc_name())).contains("doesn't fit in i32"));
    assert!(eval_lib_err("f←L \"u8\"‿\"t_u8\"‿\"u8\" ⋄ F ¯1").contains("doesn't fit in u8"));
    assert!(eval_lib_err("f←L \"i32\"‿\"t_i32\"‿\"i32\" ⋄ F 1.5").contains("doesn't fit in i32"));
}

fn eval_lib_err(src: &str) -> String {
    let lib = fixture().unwrap();
    eval_err(&format!("L←{{\"{}\"•FFI 𝕩}} ⋄ {}", lib.display(), src))
}

#[test]
fn pointer_lists_for_every_numeric_type() {
    need_cc!();
    for t in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64"] {
        let ret = if t.starts_with('f') { "f64" } else if t.starts_with('i') { "i64" } else { "u64" };
        assert_eq!(eval_lib(&format!("f←L \"{ret}\"‿\"sum_{t}\"‿\"*{t}\"‿\"i32\" ⋄ F ⟨1‿2‿3‿4, 4⟩")), "10", "{t}");
    }
}

#[test]
fn mutable_pointer_returns_buffer() {
    need_cc!();
    assert_eq!(eval_lib("f←L \"\"‿\"double_in_place\"‿\"&i32\"‿\"i32\" ⋄ F ⟨1‿2‿3, 3⟩"), "⟨ ⟨ 2 4 6 ⟩ ⟩");
    // Non-void result: the C result comes first, then each mutable buffer.
    assert_eq!(eval_lib("f←L \"i32\"‿\"fill_and_count\"‿\"&f64\"‿\"i32\" ⋄ F ⟨0‿0‿0, 3⟩"), "⟨ 3 ⟨ 0.5 1.5 2.5 ⟩ ⟩");
    // Result type "&": the buffer alone.
    assert_eq!(eval_lib("f←L \"&\"‿\"double_in_place\"‿\"&i32\"‿\"i32\" ⋄ F ⟨10‿20, 2⟩"), "⟨ 20 40 ⟩");
}

#[test]
fn mutable_string_buffer() {
    need_cc!();
    assert_eq!(eval_lib("f←L \"&\"‿\"upcase\"‿\"&u8:c8\" ⋄ F ⟨\"shout\"⟩"), "\"SHOUT\"");
}

#[test]
fn c_strings_and_raw_data() {
    need_cc!();
    assert_eq!(eval_lib("f←L \"*u8:c8\"‿\"greeting\" ⋄ F ⟨⟩"), "\"hello from C\"");
    assert_eq!(eval_lib("f←L \"i32\"‿\"byte_sum\"‿\"*u8:c8\"‿\"i32\" ⋄ F ⟨\"AB\", 2⟩"), "131");
    // :u8 reinterprets the region as bytes; :u32 on a u32 pointer result reads to the zero word.
    assert_eq!(eval_lib("f←L \"*u32:u32\"‿\"words\" ⋄ F ⟨⟩"), "⟨ 1 2 3 ⟩");
}

#[test]
fn bqn_values_pass_through() {
    need_cc!();
    assert_eq!(eval_lib("f←L \"a\"‿\"pass_a\"‿\"a\" ⋄ F ⟨\"a string\"⟩"), "\"a string\"");
    assert_eq!(eval_lib("f←L \"a\"‿\"pass_a\"‿\"a\" ⋄ F ⟨1‿2‿3⟩"), "⟨ 1 2 3 ⟩");
}

#[test]
fn structs_by_value() {
    need_cc!();
    assert_eq!(eval_lib("f←L \"{i32,f64}\"‿\"make_pair\"‿\"i32\"‿\"f64\" ⋄ F ⟨7, 0.5⟩"), "⟨ 7 0.5 ⟩");
    assert_eq!(eval_lib("f←L \"f64\"‿\"pair_sum\"‿\"{i32,f64}\" ⋄ F ⟨⟨3, 0.25⟩⟩"), "3.25");
    assert_eq!(eval_lib("f←L \"{f64,f64}\"‿\"vec_add\"‿\"{f64,f64}\"‿\"{f64,f64}\" ⋄ F ⟨⟨1,2⟩, ⟨10,20⟩⟩"), "⟨ 11 22 ⟩");
}

#[test]
fn nested_structs_and_arrays() {
    need_cc!();
    let t = "{u8,[3]i16,{i32,f64}}";
    assert_eq!(eval_lib(&format!("f←L \"i32\"‿\"nest_sum\"‿\"{t}\" ⋄ F ⟨⟨7, 1‿2‿3, ⟨40, 2⟩⟩⟩")), "55");
    assert_eq!(eval_lib(&format!("f←L \"{t}\"‿\"make_nest\" ⋄ F ⟨⟩")), "⟨ 7 ⟨ 1 2 3 ⟩ ⟨ 40 2 ⟩ ⟩");
    // A conversion on a struct field treats just that field as raw data.
    assert_eq!(eval_lib("f←L \"{u8,[3]i16:i16,{i32,f64}}\"‿\"make_nest\" ⋄ F ⟨⟩"), "⟨ 7 ⟨ 1 2 3 ⟩ ⟨ 40 2 ⟩ ⟩");
}

#[test]
fn pointer_objects() {
    need_cc!();
    let setup = "mk←L \"*{i32,f64}\"‿\"new_pairs\"‿\"i32\" ⋄ p←Mk ⟨4⟩ ⋄ ";
    // Read at an offset (scaled by the 16-byte stride), Add, Field.
    assert_eq!(eval_lib(&format!("{setup}p.Read 2")), "⟨ 2 20 ⟩");
    assert_eq!(eval_lib(&format!("{setup}(p.Add 3).Read 0")), "⟨ 3 30 ⟩");
    assert_eq!(eval_lib(&format!("{setup}(p.Add 1).Field 1 ⋄ ((p.Add 1).Field 1).Read 0")), "10");
    // Write at 𝕨 offset, then read it back.
    assert_eq!(eval_lib(&format!("{setup}w←1 p.Write ⟨99, 1.5⟩ ⋄ p.Read 1")), "⟨ 99 1.5 ⟩");
    // Sub: with an integer moves back, with a pointer gives the element distance.
    assert_eq!(eval_lib(&format!("{setup}q←p.Add 3 ⋄ q.Sub p")), "3");
    assert_eq!(eval_lib(&format!("{setup}((p.Add 3).Sub 2).Read 0")), "⟨ 1 10 ⟩");
}

#[test]
fn pointer_cast_and_pass_back() {
    need_cc!();
    let setup = "mk←L \"*i32\"‿\"new_ints\"‿\"i32\" ⋄ p←Mk ⟨5⟩ ⋄ ";
    assert_eq!(eval_lib(&format!("{setup}p.Read 4")), "104");
    // Cast reinterprets with the new element width; "" makes it untyped.
    assert_eq!(eval_lib(&format!("{setup}(p.Cast \"u8\").Read 4")), "101");
    assert_eq!(eval_lib(&format!("{setup}(p.Cast \"i16\").Add 2 ⋄ ((p.Cast \"i16\").Add 2).Read 0")), "101");
    // A pointer object is accepted wherever a pointer is, including after Add.
    assert_eq!(eval_lib(&format!("{setup}f←L \"i32\"‿\"first_int\"‿\"*i32\" ⋄ F ⟨p.Add 3⟩")), "103");
    // Untyped pointers go to `*` arguments and fail on Read/Add.
    assert_eq!(eval_lib(&format!("{setup}f←L \"i32\"‿\"first_int\"‿\"*\" ⋄ F ⟨p.Cast \"\"⟩")), "100");
}

#[test]
fn pointer_errors() {
    need_cc!();
    let setup = "mk←L \"*i32\"‿\"new_ints\"‿\"i32\" ⋄ p←Mk ⟨2⟩ ⋄ u←p.Cast \"\" ⋄ ";
    assert!(eval_lib_err(&format!("{setup}u.Read 0")).contains("untyped"));
    assert!(eval_lib_err(&format!("{setup}u.Add 1")).contains("untyped"));
    assert!(eval_lib_err(&format!("{setup}p.Field 0")).contains("struct or array"));
    assert!(eval_lib_err(&format!("{setup}p.Read 0.5")).contains("integer"));
    // A typed pointer of another type is rejected by a typed argument.
    assert!(eval_lib_err(&format!("{setup}f←L \"i32\"‿\"first_int\"‿\"*f64\" ⋄ F ⟨p⟩")).contains("compatible"));
    // A bare `*` can't take a list.
    assert!(eval_lib_err("f←L \"i32\"‿\"first_int\"‿\"*\" ⋄ F ⟨1‿2⟩").contains("untyped pointer"));
}

#[test]
fn bad_specs_error() {
    need_cc!();
    for (spec, needle) in [
        ("\"i32\"‿\"t_i32\"‿\"i7\"", "unsupported width"),
        ("\"i32\"‿\"t_i32\"‿\"z\"", "unexpected"),
        ("\"i32\"‿\"t_i32\"‿\">>i32\"", "markers"),
        ("\"i32\"‿\"t_i32\"‿\"𝕨𝕩i32\"", "markers"),
        ("\"&i32\"‿\"t_i32\"‿\"i32\"", "result type"),
        ("\"&\"‿\"t_i32\"‿\"i32\"", "exactly one mutable"),
        ("\"i32\"‿\"t_i32\"‿\"{i32\"", "expected"),
        ("\"i32\"‿\"nope\"‿\"i32\"", "couldn't find symbol"),
    ] {
        let e = eval_lib_err(&format!("L {spec}"));
        assert!(e.contains(needle), "{spec}: wanted {needle:?} in {e}");
    }
}

#[test]
fn marker_arity_rules() {
    need_cc!();
    // 𝕨-marked args need 𝕨; unmarked-only functions reject a non-empty 𝕨.
    assert!(eval_lib_err("f←L \"i32\"‿\"t_i32\"‿\"𝕨i32\" ⋄ F 1").contains("needs 𝕨"));
    assert!(eval_lib_err("f←L \"i32\"‿\"t_i32\"‿\"i32\" ⋄ 5 F 1").contains("empty list"));
    assert!(eval_lib_err("f←L \"i32\"‿\"t_i32\"‿\"i32\"‿\"i32\" ⋄ F ⟨1⟩").contains("expected 2"));
    // `>` hands the whole value over rather than a one-element list.
    assert_eq!(eval_lib("f←L \"f64\"‿\"pair_sum\"‿\">{i32,f64}\" ⋄ F ⟨3, 0.25⟩"), "3.25");
}

#[test]
fn relative_library_path_resolves_against_bqn_path() {
    need_cc!();
    let lib = fixture().unwrap();
    let dir = lib.parent().unwrap();
    let script = dir.join("rel.bqn");
    std::fs::write(&script, format!(
        "f←\"{}\" •FFI \"i32\"‿\"t_i32\"‿\"i32\"\n•Out •Fmt F 1\n", lib.file_name().unwrap().to_string_lossy())).unwrap();
    // cwd is elsewhere, so only resolution against •path can find the library.
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).arg(&script).current_dir("/").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");
}
