//! Dyadic `↕` over several axes (incl. size-0 windows) and `⍟` with nested count lists; expected output is CBQN's (`BQN -p`).

use std::process::Command;

fn run(expr: &str) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    (String::from_utf8_lossy(&out.stdout).trim_end().to_string(), out.status.code().unwrap_or(-1))
}

#[test]
fn multi_axis_windows() {
    let (out, code) = run("<⎉2 2‿2↕[\"0123\",\"abcd\",\"ABCD\"]");
    assert_eq!(code, 0);
    assert_eq!(out, "┌─                      \n╵ ┌─     ┌─     ┌─      \n  ╵\"01   ╵\"12   ╵\"23    \n    ab\"    bc\"    cd\"   \n       ┘      ┘      ┘  \n  ┌─     ┌─     ┌─      \n  ╵\"ab   ╵\"bc   ╵\"cd    \n    AB\"    BC\"    CD\"   \n       ┘      ┘      ┘  \n                       ┘");
}

#[test]
fn size_zero_window_no_panic() {
    let (out, code) = run("((↕1+≢)<⎉2∘↕¨<) 3‿2⥊\"abcdef\"");
    assert_eq!(code, 0);
    assert_eq!(out, "┌─                                         \n╵ ┌─           ┌─              ┌─          \n  ╵ ┌┐ ┌┐ ┌┐   ╵ ↕0‿1 ↕0‿1     ╵ ↕0‿2      \n    └┘ └┘ └┘     ↕0‿1 ↕0‿1       ↕0‿2      \n    ┌┐ ┌┐ ┌┐     ↕0‿1 ↕0‿1       ↕0‿2      \n    └┘ └┘ └┘     ↕0‿1 ↕0‿1       ↕0‿2      \n    ┌┐ ┌┐ ┌┐               ┘          ┘    \n    └┘ └┘ └┘                               \n    ┌┐ ┌┐ ┌┐                               \n    └┘ └┘ └┘                               \n             ┘                             \n  ┌─           ┌─              ┌─          \n  ╵ ┌┐ ┌┐ ┌┐   ╵ ┌─    ┌─      ╵ ┌─        \n    ╵  ╵  ╵      ╵\"a\"  ╵\"b\"      ╵\"ab\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n    ┌┐ ┌┐ ┌┐     ┌─    ┌─        ┌─        \n    ╵  ╵  ╵      ╵\"c\"  ╵\"d\"      ╵\"cd\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n    ┌┐ ┌┐ ┌┐     ┌─    ┌─        ┌─        \n    ╵  ╵  ╵      ╵\"e\"  ╵\"f\"      ╵\"ef\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n             ┘               ┘          ┘  \n  ┌─           ┌─              ┌─          \n  ╵ ┌┐ ┌┐ ┌┐   ╵ ┌─    ┌─      ╵ ┌─        \n    ╵  ╵  ╵      ╵\"a   ╵\"b       ╵\"ab      \n                   c\"    d\"        cd\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n    ┌┐ ┌┐ ┌┐     ┌─    ┌─        ┌─        \n    ╵  ╵  ╵      ╵\"c   ╵\"d       ╵\"cd      \n                   e\"    f\"        ef\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n             ┘               ┘          ┘  \n  ┌─           ┌─              ┌─          \n  ╵ ┌┐ ┌┐ ┌┐   ╵ ┌─    ┌─      ╵ ┌─        \n    ╵  ╵  ╵      ╵\"a   ╵\"b       ╵\"ab      \n                   c     d         cd      \n                   e\"    f\"        ef\"     \n     ┘  ┘  ┘         ┘     ┘          ┘    \n             ┘               ┘          ┘  \n                                          ┘");
}

#[test]
fn repeat_nested_count_list() {
    let (out, code) = run("2⊸×⍟⟨2,⟨4,¯2,1⟩⟩ 1");
    assert_eq!(code, 0);
    assert_eq!(out, "⟨ 4 ⟨ 16 0.25 2 ⟩ ⟩");
}
