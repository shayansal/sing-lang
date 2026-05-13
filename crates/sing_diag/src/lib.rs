use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            labels: vec![Label {
                span,
                message: label.into(),
            }],
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub code: String,
    pub title: String,
    pub hint: String,
}

pub fn explain(code: &str) -> DiagnosticInfo {
    let (title, hint) = match code {
        "E0001" => (
            "parse error",
            "check the nearby token and expected-token list",
        ),
        "E0101" => (
            "duplicate symbol",
            "rename one item or move it into another namespace",
        ),
        "E0102" => (
            "duplicate field",
            "field names inside a type must be unique",
        ),
        "E0103" => ("duplicate variant", "enum variant names must be unique"),
        "E0104" => (
            "duplicate parameter",
            "parameter names in a function must be unique",
        ),
        "E0201" => (
            "unknown name",
            "import or define the referenced value before use",
        ),
        "E0202" => (
            "unknown type",
            "define, import, or bind the type/generic name",
        ),
        "E0301" => ("bad field", "use a field declared by the struct type"),
        "E0302" => (
            "bad call",
            "check callee name, argument count, and argument types",
        ),
        "E0303" => (
            "bad index",
            "index only slices, arrays, tuples, or supported refs",
        ),
        "E0304" => ("bad object", "construct objects with declared fields only"),
        "E0401" => (
            "type mismatch",
            "make both sides share one type or add an explicit type",
        ),
        "E0402" => (
            "invalid condition",
            "conditions must be bool-like or numeric",
        ),
        "E0403" => (
            "contract type",
            "require and ensure expressions must be condition-like",
        ),
        "E0404" => (
            "control type",
            "loop bounds, branches, or propagation need compatible types",
        ),
        "E0405" => (
            "unary type",
            "use unary operators with supported operand types",
        ),
        "E0406" => (
            "binary type",
            "use binary operators with supported operand pairs",
        ),
        "E0407" => ("ternary type", "then and else branches must unify"),
        "E0408" => (
            "propagation type",
            "propagation works on result or option values",
        ),
        "E0410" => (
            "numeric suffix",
            "pick a primitive numeric suffix that fits the context",
        ),
        "E0501" => (
            "unsafe raw pointer",
            "raw pointer creation requires an unsafe boundary",
        ),
        "E0502" => (
            "unreachable code",
            "remove statements after a terminating control flow",
        ),
        "E0503" => (
            "use after move",
            "copy the value or stop using it after ownership moves",
        ),
        "E0504" => (
            "borrow conflict",
            "do not mix shared and mutable active borrows",
        ),
        "E0505" => ("mutate borrowed", "end active borrows before mutation"),
        "E0506" => (
            "borrow escape",
            "do not return references to locals that will disappear",
        ),
        "E0507" => (
            "parallel mutation",
            "P functions cannot mutate or take mutable borrows",
        ),
        "E0601" => ("unknown effect", "use one of the frozen v1 effect atoms"),
        "E0602" => (
            "missing effect",
            "declare the callee effect on the caller with `!`",
        ),
        "E0701" => (
            "attribute contract",
            "the item body violates an attribute promise",
        ),
        "E0702" => (
            "attribute placement",
            "move the attribute to a supported item kind",
        ),
        "E0703" => ("attribute conflict", "remove mutually exclusive attributes"),
        "E0704" => (
            "layout attribute",
            "layout attributes apply only to data declarations",
        ),
        "E0801" => (
            "import cycle",
            "break the cycle or move shared code to a third module",
        ),
        _ => (
            "unknown diagnostic",
            "keep the short code and inspect JSON diagnostics",
        ),
    };
    DiagnosticInfo {
        code: code.to_string(),
        title: title.to_string(),
        hint: hint.to_string(),
    }
}
