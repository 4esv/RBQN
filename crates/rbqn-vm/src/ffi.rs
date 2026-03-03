// •FFI — Foreign Function Interface for calling C functions from BQN.
//
// `path •FFI spec` returns a callable BQN function.
// spec = "result"‿"funcname"‿"arg1"‿"arg2"‿...

use std::collections::HashMap;
use std::sync::{Arc, Mutex, LazyLock};
use rbqn_core::value::B;

/// Cached loaded libraries (path -> Library)
static FFI_LIBS: LazyLock<Mutex<HashMap<String, Arc<libloading::Library>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// FFI spec storage
static FFI_SPECS: LazyLock<Mutex<Vec<FfiSpec>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

#[derive(Debug, Clone)]
enum FfiType {
    Void,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    PtrU8C8,  // *u8:c8 — C string (const char*)
}

impl FfiType {
    fn parse(s: &str) -> Self {
        match s.trim() {
            "" => FfiType::Void,
            "i8" => FfiType::I8,
            "i16" => FfiType::I16,
            "i32" => FfiType::I32,
            "i64" => FfiType::I64,
            "u8" => FfiType::U8,
            "u16" => FfiType::U16,
            "u32" => FfiType::U32,
            "u64" => FfiType::U64,
            "f32" => FfiType::F32,
            "f64" => FfiType::F64,
            "*u8:c8" | "*:c8" => FfiType::PtrU8C8,
            other => rbqn_core::error::throw(
                &format!("•FFI: unsupported type \"{}\"", other)
            ),
        }
    }

    fn to_libffi(&self) -> libffi::middle::Type {
        use libffi::middle::Type;
        match self {
            FfiType::Void => Type::void(),
            FfiType::I8 => Type::i8(),
            FfiType::I16 => Type::i16(),
            FfiType::I32 => Type::i32(),
            FfiType::I64 => Type::i64(),
            FfiType::U8 => Type::u8(),
            FfiType::U16 => Type::u16(),
            FfiType::U32 => Type::u32(),
            FfiType::U64 => Type::u64(),
            FfiType::F32 => Type::f32(),
            FfiType::F64 => Type::f64(),
            FfiType::PtrU8C8 => Type::pointer(),
        }
    }
}

#[derive(Debug, Clone)]
struct FfiSpec {
    ret_type: FfiType,
    func_name: String,
    arg_types: Vec<FfiType>,
    lib_path: String,
}

fn load_library(path: &str) -> Arc<libloading::Library> {
    let mut cache = FFI_LIBS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(lib) = cache.get(path) {
        return Arc::clone(lib);
    }
    let lib = if path == "@" {
        // @ = current process handle (includes libc on all platforms)
        #[cfg(unix)]
        {
            // dlopen(NULL) gives access to all symbols in the current process
            libloading::os::unix::Library::this().into()
        }
        #[cfg(not(unix))]
        { rbqn_core::error::throw("•FFI: @ (libc) not supported on this platform") }
    } else {
        unsafe { libloading::Library::new(path) }
            .unwrap_or_else(|e| {
                rbqn_core::error::throw(&format!("•FFI: couldn't load library \"{}\": {}", path, e))
            })
    };
    let arc = Arc::new(lib);
    cache.insert(path.to_string(), Arc::clone(&arc));
    arc
}

fn parse_spec(spec: B, lib_path: String) -> FfiSpec {
    let arr = crate::vm::get_arr(spec)
        .unwrap_or_else(|| rbqn_core::error::throw("•FFI: 𝕩 must be a list"));
    if arr.ia() < 2 {
        rbqn_core::error::throw("•FFI: 𝕩 must have at least 2 elements (result type and function name)");
    }
    let ret_str = b_to_rust_string(arr.get(0).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)));
    let func_name = b_to_rust_string(arr.get(1).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)));
    let arg_types: Vec<FfiType> = (2..arr.ia())
        .map(|i| FfiType::parse(&b_to_rust_string(arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)))))
        .collect();

    FfiSpec {
        ret_type: FfiType::parse(&ret_str),
        func_name,
        arg_types,
        lib_path,
    }
}

fn b_to_rust_string(b: B) -> String {
    if let Some(arr) = crate::vm::get_arr(b) {
        (0..arr.ia()).map(|i| {
            let v = arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            if v.is_c32() {
                char::from_u32(v.o2c().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))).unwrap_or('?')
            } else {
                '?'
            }
        }).collect()
    } else if b.is_c32() {
        char::from_u32(b.o2c().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))).map(|c| c.to_string()).unwrap_or_default()
    } else {
        rbqn_core::error::throw("•FFI: expected string element")
    }
}

fn store_ffi_spec(spec: FfiSpec) -> usize {
    let mut specs = FFI_SPECS.lock().unwrap_or_else(|e| e.into_inner());
    let id = specs.len();
    specs.push(spec);
    id
}

fn get_ffi_spec(id: usize) -> FfiSpec {
    let specs = FFI_SPECS.lock().unwrap_or_else(|e| e.into_inner());
    specs[id].clone()
}

/// Main entry: `path •FFI spec` → returns a callable BQN function.
pub fn ffi_load(path: B, spec: B) -> B {
    let path_str = b_to_rust_string(path);

    // Validate library
    let _lib = load_library(&path_str);

    // Parse spec
    let ffi_spec = parse_spec(spec, path_str);

    // Validate symbol
    let lib = load_library(&ffi_spec.lib_path);
    unsafe {
        let _sym: libloading::Symbol<*const ()> = lib.get(ffi_spec.func_name.as_bytes())
            .unwrap_or_else(|e| {
                rbqn_core::error::throw(&format!(
                    "•FFI: couldn't find symbol \"{}\" in \"{}\": {}",
                    ffi_spec.func_name, ffi_spec.lib_path, e
                ))
            });
    }

    let spec_id = store_ffi_spec(ffi_spec);
    crate::derive::m_ffi_fn(spec_id)
}

/// Call an FFI function with BQN arguments.
pub fn ffi_call(spec_id: usize, args: &[B]) -> B {
    let spec = get_ffi_spec(spec_id);
    let lib = load_library(&spec.lib_path);

    if args.len() != spec.arg_types.len() {
        rbqn_core::error::throw(&format!(
            "•FFI {}: expected {} argument(s), got {}",
            spec.func_name, spec.arg_types.len(), args.len()
        ));
    }

    let ffi_arg_types: Vec<libffi::middle::Type> = spec.arg_types.iter()
        .map(|t| t.to_libffi())
        .collect();
    let ffi_ret_type = spec.ret_type.to_libffi();
    let cif = libffi::middle::Cif::new(ffi_arg_types, ffi_ret_type);

    let func_ptr: *const () = unsafe {
        let sym: libloading::Symbol<*const ()> = lib.get(spec.func_name.as_bytes())
            .unwrap_or_else(|e| rbqn_core::error::throw(&format!("•FFI: symbol lookup failed: {}", e)));
        *sym
    };
    let code_ptr = libffi::middle::CodePtr::from_ptr(func_ptr as *const _);

    // Marshal arguments — storage must outlive the call
    let mut i8_vals = Vec::new();
    let mut i16_vals = Vec::new();
    let mut i32_vals = Vec::new();
    let mut i64_vals = Vec::new();
    let mut u8_vals = Vec::new();
    let mut u16_vals = Vec::new();
    let mut u32_vals = Vec::new();
    let mut u64_vals = Vec::new();
    let mut f32_vals = Vec::new();
    let mut f64_vals = Vec::new();
    let mut cstrings: Vec<std::ffi::CString> = Vec::new();
    let mut ptr_vals: Vec<*const u8> = Vec::new();
    let mut ffi_args: Vec<libffi::middle::Arg> = Vec::new();

    for (arg_type, bqn_arg) in spec.arg_types.iter().zip(args.iter()) {
        match arg_type {
            FfiType::I8 => {
                i8_vals.push(bqn_arg.o2f() as i8);
                ffi_args.push(libffi::middle::Arg::new(i8_vals.last().unwrap()));
            }
            FfiType::I16 => {
                i16_vals.push(bqn_arg.o2f() as i16);
                ffi_args.push(libffi::middle::Arg::new(i16_vals.last().unwrap()));
            }
            FfiType::I32 => {
                i32_vals.push(bqn_arg.o2f() as i32);
                ffi_args.push(libffi::middle::Arg::new(i32_vals.last().unwrap()));
            }
            FfiType::I64 => {
                i64_vals.push(bqn_arg.o2f() as i64);
                ffi_args.push(libffi::middle::Arg::new(i64_vals.last().unwrap()));
            }
            FfiType::U8 => {
                u8_vals.push(bqn_arg.o2f() as u8);
                ffi_args.push(libffi::middle::Arg::new(u8_vals.last().unwrap()));
            }
            FfiType::U16 => {
                u16_vals.push(bqn_arg.o2f() as u16);
                ffi_args.push(libffi::middle::Arg::new(u16_vals.last().unwrap()));
            }
            FfiType::U32 => {
                u32_vals.push(bqn_arg.o2f() as u32);
                ffi_args.push(libffi::middle::Arg::new(u32_vals.last().unwrap()));
            }
            FfiType::U64 => {
                u64_vals.push(bqn_arg.o2f() as u64);
                ffi_args.push(libffi::middle::Arg::new(u64_vals.last().unwrap()));
            }
            FfiType::F32 => {
                f32_vals.push(bqn_arg.o2f() as f32);
                ffi_args.push(libffi::middle::Arg::new(f32_vals.last().unwrap()));
            }
            FfiType::F64 => {
                f64_vals.push(bqn_arg.o2f());
                ffi_args.push(libffi::middle::Arg::new(f64_vals.last().unwrap()));
            }
            FfiType::PtrU8C8 => {
                let s = b_to_rust_string(*bqn_arg);
                let cs = std::ffi::CString::new(s)
                    .unwrap_or_else(|_| rbqn_core::error::throw("•FFI: string contains null byte"));
                ptr_vals.push(cs.as_ptr() as *const u8);
                cstrings.push(cs);
                ffi_args.push(libffi::middle::Arg::new(ptr_vals.last().unwrap()));
            }
            FfiType::Void => {
                rbqn_core::error::throw("•FFI: void is not valid as argument type");
            }
        }
    }

    unsafe {
        match spec.ret_type {
            FfiType::Void => {
                cif.call::<()>(code_ptr, &ffi_args);
                B::m_c32(0) // @ (NUL char) for void return
            }
            FfiType::I8 => B::m_f64(cif.call::<i8>(code_ptr, &ffi_args) as f64),
            FfiType::I16 => B::m_f64(cif.call::<i16>(code_ptr, &ffi_args) as f64),
            FfiType::I32 => B::m_f64(cif.call::<i32>(code_ptr, &ffi_args) as f64),
            FfiType::I64 => B::m_f64(cif.call::<i64>(code_ptr, &ffi_args) as f64),
            FfiType::U8 => B::m_f64(cif.call::<u8>(code_ptr, &ffi_args) as f64),
            FfiType::U16 => B::m_f64(cif.call::<u16>(code_ptr, &ffi_args) as f64),
            FfiType::U32 => B::m_f64(cif.call::<u32>(code_ptr, &ffi_args) as f64),
            FfiType::U64 => B::m_f64(cif.call::<u64>(code_ptr, &ffi_args) as f64),
            FfiType::F32 => B::m_f64(cif.call::<f32>(code_ptr, &ffi_args) as f64),
            FfiType::F64 => B::m_f64(cif.call::<f64>(code_ptr, &ffi_args)),
            FfiType::PtrU8C8 => {
                let ptr: *const i8 = cif.call::<*const i8>(code_ptr, &ffi_args);
                if ptr.is_null() {
                    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(vec![]))
                } else {
                    let cstr = std::ffi::CStr::from_ptr(ptr);
                    let s = cstr.to_str().unwrap_or("");
                    let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
                    crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(chars))
                }
            }
        }
    }
}
