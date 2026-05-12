use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub module: Option<String>,
    pub imports: Vec<Vec<String>>,
    pub items: Vec<Item>,
    pub meta: Vec<NodeMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolRef(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMeta {
    pub symbol: SymbolRef,
    pub kind: String,
    pub name: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Const {
        name: String,
        ty: Type,
    },
    Struct {
        name: String,
        fields: Vec<Field>,
    },
    Enum {
        name: String,
        variants: Vec<Variant>,
    },
    Trait {
        name: String,
        fns: Vec<FnSig>,
    },
    Impl {
        trait_name: String,
        for_type: Type,
        items: usize,
    },
    Fn(Fn),
    Extern(FnSig),
    Macro {
        name: String,
        arg_count: usize,
    },
    Test {
        name: Option<String>,
        ty: Type,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub payload: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fn {
    pub sig: FnSig,
    pub body_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnSig {
    pub name: Vec<String>,
    pub params: Vec<Param>,
    pub ret: Type,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    Prim(String),
    Struct(String),
    Enum(String),
    Trait(String),
    Generic(String),
    Ref { mutable: bool, inner: Box<Type> },
    Raw(Box<Type>),
    Slice(Box<Type>),
    Array { inner: Box<Type> },
    Option(Box<Type>),
    Result { ok: Box<Type>, err: Box<Type> },
    Tuple(Vec<Type>),
    Fn { params: Vec<Type>, ret: Box<Type> },
    Dyn(Option<String>),
    Any,
    Unknown,
    Never,
}

impl Type {
    pub fn v() -> Self {
        Self::Prim("v".to_string())
    }

    pub fn b() -> Self {
        Self::Prim("b".to_string())
    }

    pub fn is_unknown_like(&self) -> bool {
        matches!(self, Self::Unknown | Self::Any)
    }
}
