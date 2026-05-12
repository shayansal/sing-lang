use std::collections::HashSet;

use sing_ast::{Attr, ItemKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttrContext {
    Module,
    Use,
    Alias,
    Const,
    Type,
    Enum,
    Trait,
    Impl,
    Fn,
    Extern,
    Macro,
    Test,
}

pub(crate) fn context(kind: &ItemKind) -> AttrContext {
    match kind {
        ItemKind::Module(_) => AttrContext::Module,
        ItemKind::Use(_) => AttrContext::Use,
        ItemKind::Alias { .. } => AttrContext::Alias,
        ItemKind::Const { .. } => AttrContext::Const,
        ItemKind::Type(_) => AttrContext::Type,
        ItemKind::Enum(_) => AttrContext::Enum,
        ItemKind::Trait(_) => AttrContext::Trait,
        ItemKind::Impl(_) => AttrContext::Impl,
        ItemKind::Fn(_) => AttrContext::Fn,
        ItemKind::Extern(_) => AttrContext::Extern,
        ItemKind::Macro(_) => AttrContext::Macro,
        ItemKind::Test(_) => AttrContext::Test,
    }
}

pub(crate) fn names(attrs: &[Attr]) -> HashSet<String> {
    attrs.iter().map(|attr| attr.0.clone()).collect()
}

pub(crate) fn is_known(name: &str) -> bool {
    matches!(
        name,
        "K" | "V"
            | "P"
            | "C"
            | "U"
            | "H"
            | "D"
            | "G"
            | "Z"
            | "R"
            | "I"
            | "N"
            | "O"
            | "E"
            | "L"
            | "S"
    )
}

pub(crate) fn is_allowed_on(name: &str, context: AttrContext) -> bool {
    match name {
        "K" | "V" | "P" | "U" | "H" | "D" | "G" | "Z" | "R" | "I" | "N" | "O" => {
            matches!(context, AttrContext::Fn)
        }
        "C" => matches!(
            context,
            AttrContext::Type | AttrContext::Enum | AttrContext::Fn | AttrContext::Extern
        ),
        "E" => matches!(
            context,
            AttrContext::Const
                | AttrContext::Type
                | AttrContext::Enum
                | AttrContext::Trait
                | AttrContext::Impl
                | AttrContext::Fn
                | AttrContext::Extern
                | AttrContext::Macro
        ),
        "L" | "S" => matches!(context, AttrContext::Type),
        _ => false,
    }
}

pub(crate) fn has(attrs: &HashSet<String>, name: &str) -> bool {
    attrs.contains(name)
}

pub(crate) fn conflicts(attrs: &HashSet<String>) -> Vec<(&'static str, &'static str)> {
    let mut conflicts = Vec::new();
    if has(attrs, "I") && has(attrs, "N") {
        conflicts.push(("I", "N"));
    }
    conflicts
}

pub(crate) fn forbids_effect(attr: &str, effect: &str) -> bool {
    match attr {
        "H" => effect == "h",
        "D" => effect == "d",
        "G" => effect == "g",
        "Z" => effect == "z",
        "R" => matches!(
            effect,
            "h" | "d"
                | "g"
                | "z"
                | "fr"
                | "fw"
                | "ir"
                | "iw"
                | "nr"
                | "nw"
                | "dr"
                | "dw"
                | "tm"
                | "rn"
                | "th"
                | "ff"
                | "bk"
                | "sy"
        ),
        "K" => matches!(effect, "h" | "d" | "g" | "z" | "ff" | "bk" | "th"),
        "V" => matches!(
            effect,
            "h" | "d"
                | "g"
                | "z"
                | "fr"
                | "fw"
                | "ir"
                | "iw"
                | "nr"
                | "nw"
                | "dr"
                | "dw"
                | "th"
                | "ff"
                | "bk"
                | "sy"
        ),
        "P" => matches!(
            effect,
            "h" | "d"
                | "g"
                | "z"
                | "fr"
                | "fw"
                | "ir"
                | "iw"
                | "nr"
                | "nw"
                | "dr"
                | "dw"
                | "tm"
                | "rn"
                | "th"
                | "ff"
                | "bk"
                | "sy"
        ),
        _ => false,
    }
}
