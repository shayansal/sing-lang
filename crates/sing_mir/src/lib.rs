use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sing_ast::{BinaryOp, Block, Expr, FnDecl, ItemKind, Path, Stmt, UnaryOp};
use sing_parse::parse_file;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirFunction {
    pub name: String,
    pub entry: usize,
    pub blocks: Vec<BasicBlock>,
    pub dataflow: Dataflow,
    pub dead_blocks: Vec<usize>,
    pub optimizations: Vec<OptimizationHook>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: usize,
    pub reachable: bool,
    pub ops: Vec<MirOp>,
    pub term: MirTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirOp {
    Assign { target: String, value: Rvalue },
    Mutate { target: String, value: Rvalue },
    Eval(Rvalue),
    Require(Rvalue),
    Ensure(Rvalue),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirTerminator {
    Goto(usize),
    Branch {
        cond: Rvalue,
        then_block: usize,
        else_block: usize,
    },
    Return(Option<Rvalue>),
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rvalue {
    Unit,
    Missing,
    Use(String),
    Path(String),
    ConstInt(i64),
    ConstFloat(String),
    ConstBool(bool),
    ConstString(String),
    ConstChar(char),
    None,
    Tuple(Vec<Rvalue>),
    Object {
        ty: String,
        fields: Vec<(String, Rvalue)>,
    },
    Field {
        base: Box<Rvalue>,
        name: String,
    },
    Index {
        base: Box<Rvalue>,
        index: Box<Rvalue>,
    },
    Call {
        callee: String,
        args: Vec<Rvalue>,
    },
    Unary {
        op: String,
        value: Box<Rvalue>,
    },
    Binary {
        op: String,
        lhs: Box<Rvalue>,
        rhs: Box<Rvalue>,
    },
    Ref {
        mutable: bool,
        value: Box<Rvalue>,
    },
    Raw(Box<Rvalue>),
    Propagate(Box<Rvalue>),
    Lambda,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dataflow {
    pub locals: BTreeMap<String, LocalFlow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFlow {
    pub defs: usize,
    pub uses: usize,
    pub moves: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationHook {
    pub name: String,
    pub detail: String,
}

pub fn lower_source(src: &str) -> Result<MirProgram, Vec<String>> {
    let file = parse_file(src).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
    })?;

    let functions = file
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Fn(decl) => Some(lower_fn(decl)),
            _ => None,
        })
        .collect();

    Ok(MirProgram { functions })
}

fn lower_fn(decl: &FnDecl) -> MirFunction {
    let name = path_name(&decl.sig.name);
    let mut lowerer = Lowerer::new();
    for param in &decl.sig.params {
        for name in &param.names {
            lowerer.def(&name.0);
        }
    }

    if let Some(body) = &decl.body {
        lowerer.lower_block(body);
    } else {
        lowerer.set_term(MirTerminator::Return(None));
    }
    lowerer.finish(name)
}

struct Lowerer {
    blocks: Vec<BasicBlock>,
    current: usize,
    dataflow: Dataflow,
    dead_blocks: Vec<usize>,
    optimizations: Vec<OptimizationHook>,
}

impl Lowerer {
    fn new() -> Self {
        Self {
            blocks: vec![BasicBlock {
                id: 0,
                reachable: true,
                ops: Vec::new(),
                term: MirTerminator::Unreachable,
            }],
            current: 0,
            dataflow: Dataflow::default(),
            dead_blocks: Vec::new(),
            optimizations: Vec::new(),
        }
    }

    fn lower_block(&mut self, block: &Block) {
        let last = block.stmts.len().saturating_sub(1);
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if self.is_current_terminated() {
                self.start_dead_block();
            }
            self.lower_stmt(stmt, idx == last);
        }
        if !self.is_current_terminated() {
            self.set_term(MirTerminator::Return(Some(Rvalue::Unit)));
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt, tail: bool) {
        match stmt {
            Stmt::Require(expr) => {
                let value = self.lower_expr(expr);
                self.push_op(MirOp::Require(value));
            }
            Stmt::Ensure(expr) => {
                let value = self.lower_expr(expr);
                self.push_op(MirOp::Ensure(value));
            }
            Stmt::Return(expr) => {
                let value = self.lower_expr(expr);
                self.set_term(MirTerminator::Return(Some(value)));
            }
            Stmt::Bind { name, expr, .. } => {
                let value = self.lower_expr(expr);
                self.def(&name.0);
                self.push_op(MirOp::Assign {
                    target: name.0.clone(),
                    value,
                });
            }
            Stmt::Mut { place, expr } => {
                let value = self.lower_expr(expr);
                let target = match place {
                    sing_ast::Place::Ident(name) => name.0.clone(),
                    sing_ast::Place::Field { name, .. } => name.0.clone(),
                    sing_ast::Place::Index { .. } => "[]".to_string(),
                };
                self.def(&target);
                self.push_op(MirOp::Mutate { target, value });
            }
            Stmt::Expr(expr) if tail => self.lower_tail_expr(expr),
            Stmt::Expr(expr) => {
                let value = self.lower_expr(expr);
                self.push_op(MirOp::Eval(value));
            }
            Stmt::LoopRange { .. } | Stmt::LoopWhile { .. } => {
                self.push_op(MirOp::Eval(Rvalue::Missing));
            }
            Stmt::Break | Stmt::Continue => self.set_term(MirTerminator::Unreachable),
        }
    }

    fn lower_tail_expr(&mut self, expr: &Expr) {
        if let Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } = expr
        {
            let cond = self.lower_expr(cond);
            let then_block = self.new_block(true);
            let else_block = self.new_block(true);
            self.set_term(MirTerminator::Branch {
                cond,
                then_block,
                else_block,
            });

            self.current = then_block;
            let then_value = self.lower_expr(then_expr);
            self.set_term(MirTerminator::Return(Some(then_value)));

            self.current = else_block;
            let else_value = self.lower_expr(else_expr);
            self.set_term(MirTerminator::Return(Some(else_value)));
        } else {
            let value = self.lower_expr(expr);
            self.set_term(MirTerminator::Return(Some(value)));
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> Rvalue {
        match expr {
            Expr::Missing => Rvalue::Missing,
            Expr::Ident(name) => {
                self.use_local(&name.0);
                Rvalue::Use(name.0.clone())
            }
            Expr::Path(path) => Rvalue::Path(path_name(path)),
            Expr::Int(lit) => Rvalue::ConstInt(parse_int_literal(lit)),
            Expr::Float(lit) => Rvalue::ConstFloat(lit.clone()),
            Expr::String(value) => Rvalue::ConstString(value.clone()),
            Expr::Char(value) => Rvalue::ConstChar(*value),
            Expr::Bool(value) => Rvalue::ConstBool(*value),
            Expr::None => Rvalue::None,
            Expr::Tuple(values) => {
                Rvalue::Tuple(values.iter().map(|expr| self.lower_expr(expr)).collect())
            }
            Expr::Object { ty, fields } => Rvalue::Object {
                ty: path_name(ty),
                fields: fields
                    .iter()
                    .map(|(name, expr)| (name.0.clone(), self.lower_expr(expr)))
                    .collect(),
            },
            Expr::Field { base, name } => Rvalue::Field {
                base: Box::new(self.lower_expr(base)),
                name: name.0.clone(),
            },
            Expr::Index { base, index } => Rvalue::Index {
                base: Box::new(self.lower_expr(base)),
                index: Box::new(self.lower_expr(index)),
            },
            Expr::Call { callee, args } => Rvalue::Call {
                callee: expr_callee(callee),
                args: args.iter().map(|expr| self.lower_expr(expr)).collect(),
            },
            Expr::Unary { op, expr } => match op {
                UnaryOp::Ref => Rvalue::Ref {
                    mutable: false,
                    value: Box::new(self.lower_expr(expr)),
                },
                UnaryOp::RefMut => Rvalue::Ref {
                    mutable: true,
                    value: Box::new(self.lower_expr(expr)),
                },
                UnaryOp::Raw => Rvalue::Raw(Box::new(self.lower_expr(expr))),
                UnaryOp::Neg | UnaryOp::Not => Rvalue::Unary {
                    op: format!("{op:?}"),
                    value: Box::new(self.lower_expr(expr)),
                },
            },
            Expr::Binary { op, lhs, rhs } => {
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                self.fold_binary(op, lhs, rhs)
            }
            Expr::Ternary { .. } => Rvalue::Missing,
            Expr::Propagate(expr) => Rvalue::Propagate(Box::new(self.lower_expr(expr))),
            Expr::Lambda { .. } => Rvalue::Lambda,
        }
    }

    fn fold_binary(&mut self, op: &BinaryOp, lhs: Rvalue, rhs: Rvalue) -> Rvalue {
        let folded = match (op, &lhs, &rhs) {
            (BinaryOp::Add, Rvalue::ConstInt(left), Rvalue::ConstInt(right)) => Some(left + right),
            (BinaryOp::Sub, Rvalue::ConstInt(left), Rvalue::ConstInt(right)) => Some(left - right),
            (BinaryOp::Mul, Rvalue::ConstInt(left), Rvalue::ConstInt(right)) => Some(left * right),
            (BinaryOp::Div, Rvalue::ConstInt(left), Rvalue::ConstInt(right)) if *right != 0 => {
                Some(left / right)
            }
            (BinaryOp::Mod, Rvalue::ConstInt(left), Rvalue::ConstInt(right)) if *right != 0 => {
                Some(left % right)
            }
            _ => None,
        };

        if let Some(value) = folded {
            self.optimizations.push(OptimizationHook {
                name: "const-fold".to_string(),
                detail: format!("{op:?}"),
            });
            Rvalue::ConstInt(value)
        } else {
            Rvalue::Binary {
                op: format!("{op:?}"),
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }
        }
    }

    fn def(&mut self, name: &str) {
        self.dataflow
            .locals
            .entry(name.to_string())
            .or_default()
            .defs += 1;
    }

    fn use_local(&mut self, name: &str) {
        self.dataflow
            .locals
            .entry(name.to_string())
            .or_default()
            .uses += 1;
    }

    fn push_op(&mut self, op: MirOp) {
        self.blocks[self.current].ops.push(op);
    }

    fn set_term(&mut self, term: MirTerminator) {
        self.blocks[self.current].term = term;
    }

    fn is_current_terminated(&self) -> bool {
        !matches!(self.blocks[self.current].term, MirTerminator::Unreachable)
            || !self.blocks[self.current].ops.is_empty() && !self.blocks[self.current].reachable
    }

    fn start_dead_block(&mut self) {
        let block = self.new_block(false);
        self.dead_blocks.push(block);
        self.optimizations.push(OptimizationHook {
            name: "dead-block".to_string(),
            detail: format!("block {block} is unreachable"),
        });
        self.current = block;
    }

    fn new_block(&mut self, reachable: bool) -> usize {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            reachable,
            ops: Vec::new(),
            term: MirTerminator::Unreachable,
        });
        id
    }

    fn finish(self, name: String) -> MirFunction {
        MirFunction {
            name,
            entry: 0,
            blocks: self.blocks,
            dataflow: self.dataflow,
            dead_blocks: self.dead_blocks,
            optimizations: self.optimizations,
        }
    }
}

fn path_name(path: &Path) -> String {
    path.0
        .iter()
        .map(|part| part.0.clone())
        .collect::<Vec<_>>()
        .join(".")
}

fn expr_callee(expr: &Expr) -> String {
    match expr {
        Expr::Ident(name) => name.0.clone(),
        Expr::Path(path) => path_name(path),
        Expr::Field { base, name } => format!("{}.{}", expr_callee(base), name.0),
        _ => "<expr>".to_string(),
    }
}

fn parse_int_literal(lit: &str) -> i64 {
    let mut text = lit.replace('_', "");
    for suffix in ["i1", "i2", "i4", "i8", "iz", "u1", "u2", "u4", "u8", "uz"] {
        if text.ends_with(suffix) {
            text.truncate(text.len() - suffix.len());
            break;
        }
    }

    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        i64::from_str_radix(bin, 2).unwrap_or(0)
    } else {
        text.parse().unwrap_or(0)
    }
}
