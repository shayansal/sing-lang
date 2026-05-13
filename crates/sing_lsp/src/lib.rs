use serde::{Deserialize, Serialize};
use sing_ast::{File, ItemKind, Variant};
use sing_diag::Diagnostic;
use sing_parse::parse_file;
use sing_sem::check_source;
use sing_token::{token_cost, TokenCost};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentFacts {
    pub uri: String,
    pub symbols: Vec<DocumentSymbol>,
    pub diagnostics: Vec<Diagnostic>,
    pub token_cost: TokenCost,
    pub completions: Vec<Completion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    pub label: String,
    pub kind: String,
}

pub fn document_facts(uri: impl Into<String>, src: &str) -> DocumentFacts {
    let checked = check_source(src);
    let symbols = parse_file(src)
        .map(|file| symbols(&file))
        .unwrap_or_default();
    DocumentFacts {
        uri: uri.into(),
        symbols,
        diagnostics: checked.diagnostics,
        token_cost: token_cost(src),
        completions: completions(),
    }
}

pub fn completion_labels() -> Vec<String> {
    completions()
        .into_iter()
        .map(|completion| completion.label)
        .collect()
}

pub fn completions() -> Vec<Completion> {
    const KEYWORDS: &[&str] = &[
        "m", "u", "a", "k", "t", "e", "q", "i", "f", "x", "p", "#", "l", "b", "c",
    ];
    const TYPES: &[&str] = &[
        "b", "i1", "i2", "i4", "i8", "iz", "u1", "u2", "u4", "u8", "uz", "f4", "f8", "c", "s", "y",
        "v", "*",
    ];
    const EFFECTS: &[&str] = &[
        "h", "d", "g", "z", "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "at",
        "ff", "bk", "sy",
    ];
    const ATTRS: &[&str] = &[
        "K", "V", "P", "C", "U", "H", "D", "G", "Z", "R", "I", "N", "O", "E", "L", "S",
    ];
    const STD: &[&str] = &["out", "sqrt", "sum"];

    KEYWORDS
        .iter()
        .map(|label| completion(label, "kw"))
        .chain(TYPES.iter().map(|label| completion(label, "ty")))
        .chain(EFFECTS.iter().map(|label| completion(label, "fx")))
        .chain(ATTRS.iter().map(|label| completion(label, "attr")))
        .chain(STD.iter().map(|label| completion(label, "std")))
        .collect()
}

fn completion(label: &str, kind: &str) -> Completion {
    Completion {
        label: label.to_string(),
        kind: kind.to_string(),
    }
}

fn symbols(file: &File) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Module(path) => symbols.push(symbol(path_s(path), "module")),
            ItemKind::Alias { name, .. } => symbols.push(symbol(&name.0, "alias")),
            ItemKind::Const { name, .. } => symbols.push(symbol(&name.0, "const")),
            ItemKind::Type(decl) => symbols.push(symbol(&decl.name.0, "type")),
            ItemKind::Enum(decl) => {
                symbols.push(symbol(&decl.name.0, "enum"));
                for variant in &decl.variants {
                    let name = match variant {
                        Variant::Unit(name)
                        | Variant::Tuple(name, _)
                        | Variant::Struct(name, _) => &name.0,
                    };
                    symbols.push(symbol(name, "variant"));
                }
            }
            ItemKind::Trait(decl) => symbols.push(symbol(&decl.name.0, "trait")),
            ItemKind::Impl(decl) => symbols.push(symbol(&decl.trait_name.0, "impl")),
            ItemKind::Fn(decl) => symbols.push(symbol(path_s(&decl.sig.name), "fn")),
            ItemKind::Extern(sig) => symbols.push(symbol(path_s(&sig.name), "extern")),
            ItemKind::Macro(call) => symbols.push(symbol(&call.name.0, "macro")),
            ItemKind::Test(test) => symbols.push(symbol(
                test.name
                    .as_ref()
                    .map(|name| name.0.as_str())
                    .unwrap_or("_"),
                "test",
            )),
            ItemKind::Use(_) => {}
        }
    }
    symbols
}

fn symbol(name: impl Into<String>, kind: impl Into<String>) -> DocumentSymbol {
    DocumentSymbol {
        name: name.into(),
        kind: kind.into(),
    }
}

fn path_s(path: &sing_ast::Path) -> String {
    path.0
        .iter()
        .map(|part| part.0.as_str())
        .collect::<Vec<_>>()
        .join(".")
}
