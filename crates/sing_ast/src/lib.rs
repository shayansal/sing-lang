use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub attrs: Vec<Attr>,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Module(Path),
    Use(Vec<Path>),
    Alias {
        name: Ident,
        ty: Type,
    },
    Const {
        name: Ident,
        ty: Option<Type>,
        expr: Expr,
    },
    Type(TypeDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    Fn(FnDecl),
    Extern(FnSig),
    Macro(MacroCall),
    Test(TestDecl),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeDecl {
    pub name: Ident,
    pub generics: Vec<Generic>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDecl {
    pub name: Ident,
    pub generics: Vec<Generic>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitDecl {
    pub name: Ident,
    pub generics: Vec<Generic>,
    pub fns: Vec<FnSig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplDecl {
    pub trait_name: Ident,
    pub for_type: Type,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnDecl {
    pub sig: FnSig,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnSig {
    pub name: Path,
    pub generics: Vec<Generic>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub names: Vec<Ident>,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub names: Vec<Ident>,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Variant {
    Unit(Ident),
    Tuple(Ident, Type),
    Struct(Ident, Vec<Field>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generic {
    pub name: Ident,
    pub bound: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    Prim(Prim),
    Path(Path),
    Ref { mutable: bool, inner: Box<Type> },
    Raw(Box<Type>),
    Slice(Box<Type>),
    Array { inner: Box<Type>, len: Expr },
    Option(Box<Type>),
    Result { ok: Box<Type>, err: Box<Type> },
    Tuple(Vec<Type>),
    Fn { params: Vec<Type>, ret: Box<Type> },
    Dyn(Option<Path>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prim {
    B,
    I1,
    I2,
    I4,
    I8,
    Iz,
    U1,
    U2,
    U4,
    U8,
    Uz,
    F4,
    F8,
    C,
    S,
    Y,
    V,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stmt {
    Require(Expr),
    Ensure(Expr),
    Return(Expr),
    LoopRange {
        var: Ident,
        start: Expr,
        end: Expr,
        body: Box<Stmt>,
    },
    LoopWhile {
        cond: Expr,
        body: Block,
    },
    Break,
    Continue,
    Bind {
        name: Ident,
        ty: Option<Type>,
        expr: Expr,
    },
    Mut {
        place: Place,
        expr: Expr,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    Missing,
    Ident(Ident),
    Path(Path),
    Int(String),
    Float(String),
    String(String),
    Char(char),
    Bool(bool),
    None,
    Tuple(Vec<Expr>),
    Object {
        ty: Path,
        fields: Vec<(Ident, Expr)>,
    },
    Field {
        base: Box<Expr>,
        name: Ident,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Propagate(Box<Expr>),
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    Ident(Ident),
    Field { base: Box<Expr>, name: Ident },
    Index { base: Box<Expr>, index: Expr },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
    Ref,
    RefMut,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    AddWrap,
    SubWrap,
    MulWrap,
    AddSat,
    SubSat,
    MulSat,
    AddChk,
    SubChk,
    MulChk,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ident(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Path(pub Vec<Ident>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attr(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroCall {
    pub name: Ident,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestDecl {
    pub name: Option<Ident>,
    pub expr: Expr,
}
