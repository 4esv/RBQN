//! Inputs found by the cargo-fuzz targets (fuzz/), kept as plain tests (#172).

use rbqn::Bqn;

fn err(src: &str) -> String {
    let bqn = Bqn::new().unwrap();
    match bqn.eval(src) {
        Ok(_) => panic!("{src} should be an error"),
        Err(e) => e.message,
    }
}

#[test]
fn prefixes_and_suffixes_of_a_unit_are_rank_errors() {
    assert_eq!(err("↑ <5"), "↑𝕩: 𝕩 must have rank at least 1");
    assert_eq!(err("↓ <5"), "↓𝕩: 𝕩 must have rank at least 1");
}
