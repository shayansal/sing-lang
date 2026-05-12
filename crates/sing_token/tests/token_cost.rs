use sing_token::{compare_snippets, token_cost};

#[test]
fn sing_dot_product_is_cheaper_than_common_equivalents() {
    let sing = r#"KV f dot(a,b:&f4[])>f4:?a.len=b.len;sum(a*b)"#;
    let rust = r#"fn dot(a:&[f32],b:&[f32])->f32{assert_eq!(a.len(),b.len());a.iter().zip(b).map(|(x,y)|x*y).sum()}"#;
    let c = r#"float dot(float*a,float*b,size_t n){float s=0;for(size_t i=0;i<n;i++){s+=a[i]*b[i];}return s;}"#;
    let zig = r#"fn dot(a:[]const f32,b:[]const f32)f32{std.debug.assert(a.len==b.len);var s:f32=0;for(a,b)|x,y|s+=x*y;return s;}"#;
    let go = r#"func dot(a,b[]float32)float32{if len(a)!=len(b){panic("len")}var s float32;for i:=range a{s+=a[i]*b[i]}return s}"#;
    let python = r#"def dot(a,b): assert len(a)==len(b); return sum(x*y for x,y in zip(a,b))"#;

    let report = compare_snippets([
        ("Sing", sing),
        ("Rust", rust),
        ("C", c),
        ("Zig", zig),
        ("Go", go),
        ("Python", python),
    ]);

    let sing_cost = report
        .iter()
        .find(|row| row.name == "Sing")
        .expect("Sing row should exist")
        .cost
        .total;
    for lang in ["Rust", "C", "Zig", "Go", "Python"] {
        let other = report
            .iter()
            .find(|row| row.name == lang)
            .expect("language row should exist");
        assert!(sing_cost < other.cost.total, "Sing should beat {lang}");
    }
}

#[test]
fn token_cost_counts_bytes_chars_lexemes_and_llmish_tokens() {
    let cost = token_cost(r#"m H;f main()>v:0"#);
    assert_eq!(cost.bytes, 16);
    assert_eq!(cost.chars, 16);
    assert!(cost.lexemes > 0);
    assert!(cost.llm_tokens > 0);
    assert!(cost.total >= cost.llm_tokens);
}
