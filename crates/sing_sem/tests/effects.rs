use sing_sem::check_source;

const EFFECTS: &[&str] = &[
    "h", "d", "g", "z", "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "at",
    "ff", "bk", "sy",
];

fn diagnostics(src: &str) -> Vec<String> {
    check_source(src)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn assert_clean(src: &str) {
    let checked = check_source(src);
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics for `{src}`: {:#?}",
        checked.diagnostics
    );
}

fn assert_has(src: &str, code: &str) {
    let codes = diagnostics(src);
    assert!(
        codes.iter().any(|actual| actual == code),
        "expected {code} in {codes:?} for `{src}`"
    );
}

#[test]
fn every_v1_effect_must_be_declared_by_callers() {
    for effect in EFFECTS {
        assert_clean(&format!("x fx()>v !{effect};f main()>v !{effect}:fx()"));
        assert_has(&format!("x fx()>v !{effect};f main()>v:fx()"), "E0602");
    }
}

#[test]
fn sy_declares_the_system_effect_family_compactly() {
    for effect in [
        "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "ff", "bk", "sy",
    ] {
        assert_clean(&format!("x fx()>v !{effect};f main()>v !sy:fx()"));
    }
}

#[test]
fn attrs_forbid_effects_even_when_the_effect_is_declared() {
    for (attr, effect) in [
        ("H", "h"),
        ("D", "d"),
        ("G", "g"),
        ("Z", "z"),
        ("R", "sy"),
        ("K", "g"),
        ("V", "d"),
        ("P", "iw"),
    ] {
        assert_has(
            &format!("x fx()>v !{effect};{attr} f main()>v !{effect}:fx()"),
            "E0701",
        );
    }
}

#[test]
fn attrs_are_known_placed_and_non_conflicting() {
    assert_has("Y f main()>v:0", "E0702");
    assert_has("L f main()>v:0", "E0704");
    assert_has("IN f main()>v:0", "E0703");

    assert_clean("L t P{x:i4}");
    assert_clean("C t V{x,y,z:f4}");
}

#[test]
fn attrs_enforce_abi_dynamic_heap_and_panic_contracts() {
    assert_has("C f id[T](x:T)>T{x}", "E0701");
    assert_has("C t Bad{name:s}", "E0701");
    assert_has("q Q{};D f main()>*Q:*", "E0701");
    assert_has("H f main()>v:\"heap\"", "E0701");
    assert_has("Z f main()>v:?1", "E0701");
}
