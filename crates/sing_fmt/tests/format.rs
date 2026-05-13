use sing_fmt::{format_source, minify_source};
use sing_parse::parse_file;
use sing_token::token_cost;

#[test]
fn minifier_roundtrips_and_beats_formatter() {
    let src = r#"m H;u io;f main()>v !iw:out("hi")"#;
    let formatted = format_source(src).expect("format should succeed");
    let minimized = minify_source(src).expect("min should succeed");

    let original_ast = parse_file(src).expect("source should parse");
    assert_eq!(
        parse_file(&formatted).expect("formatted source should parse"),
        original_ast
    );
    assert_eq!(
        parse_file(&minimized).expect("minimized source should parse"),
        original_ast
    );
    assert!(token_cost(&minimized).total < token_cost(&formatted).total);
    assert!(!minimized.contains('\n'));
}

#[test]
fn formatter_keeps_grouped_fields_and_attrs_readable() {
    let src = "C t V{x,y,z:f4};KV f dot(a,b:&f4[])>f4:?a.len=b.len;sum(a*b)";
    let formatted = format_source(src).expect("format should succeed");

    assert!(formatted.contains("C t V{x,y,z:f4}"));
    assert!(formatted.contains("KV f dot"));
    assert!(formatted.contains("?a.len=b.len"));
}
