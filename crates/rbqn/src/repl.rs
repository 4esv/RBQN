use std::cell::RefCell;
use std::rc::Rc;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{
    Cmd, ConditionalEventHandler, Context, Editor, Event, EventContext, EventHandler, Helper,
    KeyEvent, Movement, RepeatCount,
};

use rbqn::bootstrap::Runtime;
use crate::ReplState;

pub fn run_repl(rt: &Runtime, silent: bool) {
    let mut rl = match Editor::<BqnHelper, DefaultHistory>::new() {
        Ok(rl) => rl,
        Err(e) => {
            eprintln!("rbqn: failed to initialize REPL: {e}");
            return;
        }
    };
    let session_names = Rc::new(RefCell::new(Vec::new()));
    rl.set_helper(Some(BqnHelper { session_names: Rc::clone(&session_names) }));
    // Backslash glyph input: replace `\k` with its glyph as soon as `k` is typed.
    for &(key, _) in KEYMAP {
        rl.bind_sequence(
            KeyEvent::from(key),
            EventHandler::Conditional(Box::new(GlyphHandler)),
        );
    }

    // Load history
    let history_path = history_file();
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    let mut state = ReplState::new();

    loop {
        match rl.readline("   ") {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                eval_line(rt, &line, silent, &mut state);
                *session_names.borrow_mut() = state.var_names.clone();
            }
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("rbqn: readline error: {e}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }
}

fn eval_line(rt: &Runtime, line: &str, silent: bool, state: &mut ReplState) {
    // Handle REPL commands
    if let Some(stripped) = line.strip_prefix(')') {
        let cmd = stripped.trim();
        if cmd == "exit" || cmd == "off" {
            std::process::exit(0);
        }
        eprintln!("Unknown REPL command: {line}");
        return;
    }

    rbqn_vm::vm::clear_error_site();
    match crate::exec_repl_line(rt, line, state) {
        Ok(result) => {
            if !silent {
                let formatted = crate::format_result(rt, &result);
                println!("{formatted}");
            }
        }
        Err(e) => {
            crate::print_error(&e, "(repl)");
        }
    }
}

fn history_file() -> Option<String> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Some(format!("{xdg}/.rbqn_repl_history"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(format!("{home}/.rbqn_repl_history"));
    }
    None
}

/// Standard BQN editor keymap (`\` + key → glyph), from BQN/editors/inputrc.
const KEYMAP: &[(char, char)] = &[
    ('\\', '\\'),
    ('`', '˜'),
    ('1', '˘'),
    ('2', '¨'),
    ('3', '⁼'),
    ('4', '⌜'),
    ('5', '´'),
    ('6', '˝'),
    ('7', '7'),
    ('8', '∞'),
    ('9', '¯'),
    ('0', '•'),
    ('-', '÷'),
    ('=', '×'),
    ('~', '¬'),
    ('!', '⎉'),
    ('@', '⚇'),
    ('#', '⍟'),
    ('$', '◶'),
    ('%', '⊘'),
    ('^', '⎊'),
    ('&', '⍎'),
    ('*', '⍕'),
    ('(', '⟨'),
    (')', '⟩'),
    ('_', '√'),
    ('+', '⋆'),
    ('q', '⌽'),
    ('w', '𝕨'),
    ('e', '∊'),
    ('r', '↑'),
    ('t', '∧'),
    ('y', 'y'),
    ('u', '⊔'),
    ('i', '⊏'),
    ('o', '⊐'),
    ('p', 'π'),
    ('[', '←'),
    (']', '→'),
    ('Q', '↙'),
    ('W', '𝕎'),
    ('E', '⍷'),
    ('R', '𝕣'),
    ('T', '⍋'),
    ('Y', 'Y'),
    ('U', 'U'),
    ('I', '⊑'),
    ('O', '⊒'),
    ('P', '⍳'),
    ('{', '⊣'),
    ('}', '⊢'),
    ('a', '⍉'),
    ('s', '𝕤'),
    ('d', '↕'),
    ('f', '𝕗'),
    ('g', '𝕘'),
    ('h', '⊸'),
    ('j', '∘'),
    ('k', '○'),
    ('l', '⟜'),
    (';', '⋄'),
    ('\'', '↩'),
    ('A', '↖'),
    ('S', '𝕊'),
    ('D', 'D'),
    ('F', '𝔽'),
    ('G', '𝔾'),
    ('H', '«'),
    ('J', 'J'),
    ('K', '⌾'),
    ('L', '»'),
    (':', '·'),
    ('"', '˙'),
    ('|', '|'),
    ('z', '⥊'),
    ('x', '𝕩'),
    ('c', '↓'),
    ('v', '∨'),
    ('b', '⌊'),
    ('n', 'n'),
    ('m', '≡'),
    (',', '∾'),
    ('.', '≍'),
    ('/', '≠'),
    ('Z', '⋈'),
    ('X', '𝕏'),
    ('C', 'C'),
    ('V', '⍒'),
    ('B', '⌈'),
    ('N', 'N'),
    ('M', '≢'),
    ('<', '≤'),
    ('>', '≥'),
    ('?', '⇐'),
    (' ', '‿'),
];

fn glyph_for(key: char) -> Option<char> {
    KEYMAP.iter().find(|&&(k, _)| k == key).map(|&(_, g)| g)
}

/// True when the text before the cursor ends in an unpaired backslash.
fn pending_backslash(before_cursor: &str) -> bool {
    before_cursor.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1
}

/// Fires on every keymap key; replaces a preceding `\` plus this key with the glyph.
struct GlyphHandler;

impl ConditionalEventHandler for GlyphHandler {
    fn handle(&self, evt: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        let key = match evt.get(0)? {
            KeyEvent(rustyline::KeyCode::Char(c), _) => *c,
            _ => return None,
        };
        if !pending_backslash(&ctx.line()[..ctx.pos()]) {
            return None;
        }
        let glyph = glyph_for(key)?;
        Some(Cmd::Replace(Movement::BackwardChar(1), Some(glyph.to_string())))
    }
}

/// `•` system values, canonical spelling. Every entry must resolve in
/// `sys_name_to_b` (checked by rbqn_vm::derive::tests).
fn sys_values() -> impl Iterator<Item = String> {
    rbqn_vm::derive::SYS_VALUE_NAMES.iter().map(|n| format!("•{n}"))
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '•' || c == '.'
}

/// Complete the identifier ending at `pos`. Returns (start offset, candidates).
/// `•` prefixes match system values case-insensitively; others match session names.
fn complete_word(line: &str, pos: usize, sys: &[String], session: &[String]) -> (usize, Vec<String>) {
    let before = &line[..pos];
    let start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_word_char(c))
        .last()
        .map_or(pos, |(i, _)| i);
    let word = &before[start..];
    let mut out: Vec<String> = if word.starts_with('•') {
        let w = word.to_lowercase();
        sys.iter().filter(|n| n.to_lowercase().starts_with(&w)).cloned().collect()
    } else if word.is_empty() {
        Vec::new()
    } else {
        session.iter().filter(|n| n.starts_with(word) && n.as_str() != word).cloned().collect()
    };
    out.sort();
    out.dedup();
    // Candidates matching the typed case exactly come first (`•ma` → `•math` before `•MakeRand`).
    out.sort_by_key(|n| !n.starts_with(word));
    (start, out)
}

struct BqnHelper {
    session_names: Rc<RefCell<Vec<String>>>,
}

impl Completer for BqnHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let sys: Vec<String> = sys_values().collect();
        let (start, names) = complete_word(line, pos, &sys, &self.session_names.borrow());
        Ok((start, names.into_iter().map(|n| Pair { display: n.clone(), replacement: n }).collect()))
    }
}

impl Hinter for BqnHelper {
    type Hint = String;
}
impl Highlighter for BqnHelper {}
impl Validator for BqnHelper {}
impl Helper for BqnHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymap_entries_unique_ascii_keys() {
        let mut seen = std::collections::HashSet::new();
        for &(k, _) in KEYMAP {
            assert!(k.is_ascii() && !k.is_ascii_control(), "key {k:?} not printable ASCII");
            assert!(seen.insert(k), "duplicate key {k:?}");
        }
        assert_eq!(KEYMAP.len(), 95);
    }

    #[test]
    fn keymap_known_glyphs() {
        assert_eq!(glyph_for('B'), Some('⌈'));
        assert_eq!(glyph_for('+'), Some('⋆'));
        assert_eq!(glyph_for('-'), Some('÷'));
        assert_eq!(glyph_for('\\'), Some('\\'));
        assert_eq!(glyph_for('\u{1}'), None);
    }

    #[test]
    fn pending_backslash_parity() {
        assert!(pending_backslash("1 \\"));
        assert!(!pending_backslash("1 \\\\"));
        assert!(!pending_backslash("1 "));
    }

    fn fake() -> (Vec<String>, Vec<String>) {
        let sys = ["•Out", "•math", "•Min", "•Show"].map(String::from).to_vec();
        let session = ["foo", "fooBar", "Baz", "_v3"].map(String::from).to_vec();
        (sys, session)
    }

    #[test]
    fn completes_system_values() {
        let (sys, ses) = fake();
        assert_eq!(complete_word("1+•ma", 7, &sys, &ses), (2, vec!["•math".into()]));
        assert_eq!(complete_word("•m", 4, &sys, &ses).1, vec!["•math", "•Min"]);
        let sys2 = ["•MakeRand", "•math"].map(String::from).to_vec();
        assert_eq!(complete_word("•ma", 4, &sys2, &ses).1, vec!["•math", "•MakeRand"]);
        assert!(complete_word("•zz", 5, &sys, &ses).1.is_empty());
    }

    #[test]
    fn completes_session_names() {
        let (sys, ses) = fake();
        assert_eq!(complete_word("a←fo", 6, &sys, &ses), (4, vec!["foo".into(), "fooBar".into()]));
        assert_eq!(complete_word("foo", 3, &sys, &ses).1, vec!["fooBar"]);
        assert!(complete_word("1+ ", 3, &sys, &ses).1.is_empty());
    }
}
