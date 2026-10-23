//! #64: ↓ on atoms, scan on rank>1 and with enclosed 𝕨, 1∾⁼𝕩. Expected values are CBQN output.

use std::process::Command;

fn run(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

#[test]
fn drop_atom_unit_zero() {
    assert_eq!(run(r####"0 ↓ 0"####), r####"⟨ 0 ⟩"####);
}

#[test]
fn drop_atom_unit_one() {
    assert_eq!(run(r####"1 ↓ 0"####), r####"⟨⟩"####);
}

#[test]
fn drop_enclosed_w_of_atom() {
    assert_eq!(run(r####"<5 ↓ 1"####), r####"┌·    
· ⟨⟩  
     ┘"####);
}

#[test]
fn drop_enclosed_w_atom_x() {
    assert_eq!(run(r####"(<5) ↓ 1"####), r####"⟨⟩"####);
}

#[test]
fn scan_join_rank2() {
    assert_eq!(run(r####"∾` 2‿3⥊↕6"####), r####"┌─                         
╵ 0       1       2        
  ⟨ 0 3 ⟩ ⟨ 1 4 ⟩ ⟨ 2 5 ⟩  
                          ┘"####);
}

#[test]
fn scan_join_dyadic_rank2() {
    assert_eq!(run(r####"1‿2‿3 ∾` 2‿3⥊↕6"####), r####"┌─                               
╵ ⟨ 1 0 ⟩   ⟨ 2 1 ⟩   ⟨ 3 2 ⟩    
  ⟨ 1 0 3 ⟩ ⟨ 2 1 4 ⟩ ⟨ 3 2 5 ⟩  
                                ┘"####);
}

#[test]
fn scan_enclose_rank2() {
    assert_eq!(run(r####"⋈` 2‿3⥊↕6"####), r####"┌─                         
╵ 0       1       2        
  ⟨ 0 3 ⟩ ⟨ 1 4 ⟩ ⟨ 2 5 ⟩  
                          ┘"####);
}

#[test]
fn scan_enclose_dyadic_rank2() {
    assert_eq!(run(r####"1‿2‿3 ⋈` 2‿3⥊↕6"####), r####"┌─                                           
╵ ⟨ 1 0 ⟩       ⟨ 2 1 ⟩       ⟨ 3 2 ⟩        
  ⟨ ⟨ 1 0 ⟩ 3 ⟩ ⟨ ⟨ 2 1 ⟩ 4 ⟩ ⟨ ⟨ 3 2 ⟩ 5 ⟩  
                                            ┘"####);
}

#[test]
fn scan_enclosed_w_initial() {
    assert_eq!(run(r####"(<"w") {"("∾𝕨∾")𝔽"∾𝕩}` "a"‿"b""####), r####"⟨ "(w)𝔽a" "((w)𝔽a)𝔽b" ⟩"####);
}

#[test]
fn undo_join_atom_w() {
    assert_eq!(run(r####"1 ∾⁼ 1‿2‿3"####), r####"⟨ 2 3 ⟩"####);
}

#[test]
fn undo_join_enclosed_w() {
    assert_eq!(run(r####"(<1)∾⁼ 1‿2"####), r####"⟨ 2 ⟩"####);
}

#[test]
fn undo_join_atom_w_empty() {
    assert_eq!(run(r####"1∾⁼ ⟨⟩"####), r####""####);
}

#[test]
fn scan_rank2_atomwise() {
    assert_eq!(run(r####"{𝕨‿𝕩}` 3‿2⥊"abcdef""####), r####"┌─                           
╵ 'a'          'b'           
  "ac"         "bd"          
  ⟨ "ac" 'e' ⟩ ⟨ "bd" 'f' ⟩  
                            ┘"####);
}

#[test]
fn scan_rank2_dyadic_atomwise() {
    assert_eq!(run(r####"1‿2 {𝕨∾𝕩}` 3‿2⥊"abcdef""####), r####"┌─                                     
╵ ⟨ 1 'a' ⟩         ⟨ 2 'b' ⟩          
  ⟨ 1 'a' 'c' ⟩     ⟨ 2 'b' 'd' ⟩      
  ⟨ 1 'a' 'c' 'e' ⟩ ⟨ 2 'b' 'd' 'f' ⟩  
                                      ┘"####);
}
