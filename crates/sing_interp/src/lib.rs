use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use sing_ast::{BinaryOp, Block, Expr, FnDecl, ItemKind, Path, Stmt, Type as AstType, UnaryOp};
use sing_parse::parse_file;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Void,
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Tuple(Vec<Value>),
    Struct {
        ty: String,
        fields: BTreeMap<String, Value>,
    },
    Enum {
        name: String,
        payload: Option<Box<Value>>,
    },
    Option(Option<Box<Value>>),
    ResultOk(Box<Value>),
    ResultErr(Box<Value>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutput {
    pub value: Value,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RunError {
    #[error("parse failed: {0}")]
    Parse(String),
    #[error("missing main function")]
    MissingMain,
    #[error("unknown function `{0}`")]
    UnknownFunction(String),
    #[error("unknown name `{0}`")]
    UnknownName(String),
    #[error("unsupported expression")]
    UnsupportedExpr,
    #[error("unsupported statement")]
    UnsupportedStmt,
    #[error("type error: {0}")]
    Type(String),
}

pub fn run_source(src: &str) -> Result<RunOutput, RunError> {
    let file = parse_file(src).map_err(|errors| RunError::Parse(format!("{errors:#?}")))?;
    let mut functions = HashMap::new();
    let mut enum_variants = HashMap::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Fn(decl) => {
                let name = decl
                    .sig
                    .name
                    .0
                    .iter()
                    .map(|part| part.0.clone())
                    .collect::<Vec<_>>()
                    .join(".");
                functions.insert(name, decl.clone());
            }
            ItemKind::Enum(decl) => {
                for variant in &decl.variants {
                    let name = match variant {
                        sing_ast::Variant::Unit(name)
                        | sing_ast::Variant::Tuple(name, _)
                        | sing_ast::Variant::Struct(name, _) => &name.0,
                    };
                    enum_variants.insert(name.clone(), decl.name.0.clone());
                }
            }
            _ => {}
        }
    }

    let mut interp = Interpreter {
        functions,
        enum_variants,
        output: String::new(),
    };
    let value = interp.call("main", Vec::new())?;
    Ok(RunOutput {
        value,
        output: interp.output,
    })
}

struct Interpreter {
    functions: HashMap<String, FnDecl>,
    enum_variants: HashMap<String, String>,
    output: String,
}

impl Interpreter {
    fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RunError> {
        match name {
            "out" => {
                if let Some(value) = args.first() {
                    self.output.push_str(&value.as_output_string());
                }
                return Ok(Value::Void);
            }
            "sqrt" => {
                let value = args
                    .first()
                    .ok_or_else(|| RunError::Type("sqrt expects one arg".to_string()))?;
                return Ok(Value::Float(value.as_f64()?.sqrt()));
            }
            "sum" => {
                let value = args
                    .first()
                    .ok_or_else(|| RunError::Type("sum expects one arg".to_string()))?;
                return value.sum();
            }
            _ => {}
        }

        let function = self.functions.get(name).cloned().ok_or_else(|| {
            if name == "main" {
                RunError::MissingMain
            } else {
                RunError::UnknownFunction(name.to_string())
            }
        })?;

        let mut env = HashMap::new();
        let mut arg_iter = args.into_iter();
        for param in &function.sig.params {
            for name in &param.names {
                env.insert(name.0.clone(), arg_iter.next().unwrap_or(Value::Void));
            }
        }

        let value = if let Some(body) = &function.body {
            match self.eval_block(body, &mut env)? {
                Control::Value(value) | Control::Return(value) => value,
                Control::Break | Control::Continue => return Err(RunError::UnsupportedStmt),
            }
        } else {
            Value::Void
        };

        Ok(wrap_return(value, function.sig.ret.as_ref()))
    }

    fn eval_block(
        &mut self,
        block: &Block,
        env: &mut HashMap<String, Value>,
    ) -> Result<Control, RunError> {
        let mut last = Value::Void;
        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env)? {
                Control::Value(value) => last = value,
                control @ (Control::Return(_) | Control::Break | Control::Continue) => {
                    return Ok(control)
                }
            }
        }
        Ok(Control::Value(last))
    }

    fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        env: &mut HashMap<String, Value>,
    ) -> Result<Control, RunError> {
        match stmt {
            Stmt::Return(expr) => Ok(Control::Return(self.eval_expr(expr, env)?)),
            Stmt::Bind { name, expr, .. } => {
                let value = self.eval_expr(expr, env)?;
                env.insert(name.0.clone(), value.clone());
                Ok(Control::Value(value))
            }
            Stmt::Mut { place, expr } => {
                let value = self.eval_expr(expr, env)?;
                match place {
                    sing_ast::Place::Ident(name) => {
                        env.insert(name.0.clone(), value);
                        Ok(Control::Value(Value::Void))
                    }
                    _ => Err(RunError::UnsupportedStmt),
                }
            }
            Stmt::Expr(expr) => Ok(Control::Value(self.eval_expr(expr, env)?)),
            Stmt::Require(expr) | Stmt::Ensure(expr) => {
                let value = self.eval_expr(expr, env)?;
                if value.truthy() {
                    Ok(Control::Value(Value::Void))
                } else {
                    Err(RunError::Type("contract failed".to_string()))
                }
            }
            Stmt::LoopRange {
                var,
                start,
                end,
                body,
            } => {
                let start = self.eval_expr(start, env)?.as_i64()?;
                let end = self.eval_expr(end, env)?.as_i64()?;
                for value in start..end {
                    env.insert(var.0.clone(), Value::Int(value));
                    match self.eval_stmt(body, env)? {
                        Control::Value(_) | Control::Continue => {}
                        Control::Break => break,
                        control @ Control::Return(_) => return Ok(control),
                    }
                }
                Ok(Control::Value(Value::Void))
            }
            Stmt::LoopWhile { cond, body } => {
                let mut guard = 0usize;
                while self.eval_expr(cond, env)?.truthy() {
                    guard += 1;
                    if guard > 1_000_000 {
                        return Err(RunError::Type("loop iteration limit exceeded".to_string()));
                    }
                    match self.eval_block(body, env)? {
                        Control::Value(_) | Control::Continue => {}
                        Control::Break => break,
                        control @ Control::Return(_) => return Ok(control),
                    }
                }
                Ok(Control::Value(Value::Void))
            }
            Stmt::Break => Ok(Control::Break),
            Stmt::Continue => Ok(Control::Continue),
        }
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        env: &mut HashMap<String, Value>,
    ) -> Result<Value, RunError> {
        match expr {
            Expr::Int(value) => Ok(Value::Int(parse_int(value))),
            Expr::Float(value) => Ok(Value::Float(parse_float(value))),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Char(value) => Ok(Value::String(value.to_string())),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::None => Ok(Value::Option(None)),
            Expr::Missing => Ok(Value::Void),
            Expr::Ident(name) => {
                if let Some(value) = env.get(&name.0) {
                    Ok(value.clone())
                } else if self.enum_variants.contains_key(&name.0) {
                    Ok(Value::Enum {
                        name: name.0.clone(),
                        payload: None,
                    })
                } else {
                    Err(RunError::UnknownName(name.0.clone()))
                }
            }
            Expr::Path(path) => {
                let name = path_name(path);
                if self.enum_variants.contains_key(&name) {
                    Ok(Value::Enum {
                        name,
                        payload: None,
                    })
                } else {
                    Err(RunError::UnknownName(name))
                }
            }
            Expr::Tuple(values) => values
                .iter()
                .map(|expr| self.eval_expr(expr, env))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Tuple),
            Expr::Object { ty, fields } => {
                let fields = fields
                    .iter()
                    .map(|(name, expr)| Ok((name.0.clone(), self.eval_expr(expr, env)?)))
                    .collect::<Result<BTreeMap<_, _>, RunError>>()?;
                Ok(Value::Struct {
                    ty: path_name(ty),
                    fields,
                })
            }
            Expr::Field { base, name } => {
                let base = self.eval_expr(base, env)?;
                match base {
                    Value::Struct { fields, .. } => fields
                        .get(&name.0)
                        .cloned()
                        .ok_or_else(|| RunError::UnknownName(name.0.clone())),
                    other => Err(RunError::Type(format!("expected struct, got {other:?}"))),
                }
            }
            Expr::Index { base, index } => {
                let base = self.eval_expr(base, env)?;
                let index = self.eval_expr(index, env)?.as_i64()? as usize;
                match base {
                    Value::Tuple(values) => values
                        .get(index)
                        .cloned()
                        .ok_or_else(|| RunError::Type("index out of bounds".to_string())),
                    other => Err(RunError::Type(format!("expected tuple, got {other:?}"))),
                }
            }
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => Ok(Value::Int(-value.as_i64()?)),
                    UnaryOp::Not => Ok(Value::Bool(!value.truthy())),
                    UnaryOp::Ref | UnaryOp::RefMut | UnaryOp::Raw => Ok(value),
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let left = self.eval_expr(lhs, env)?;
                let right = self.eval_expr(rhs, env)?;
                eval_binary(op, left, right)
            }
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                if self.eval_expr(cond, env)?.truthy() {
                    self.eval_expr(then_expr, env)
                } else {
                    self.eval_expr(else_expr, env)
                }
            }
            Expr::Call { callee, args } => {
                let name = callee_name(callee)?;
                let args = args
                    .iter()
                    .map(|arg| self.eval_expr(arg, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call(&name, args)
            }
            Expr::Propagate(expr) => match self.eval_expr(expr, env)? {
                Value::ResultOk(value) => Ok(*value),
                Value::ResultErr(value) => Ok(Value::ResultErr(value)),
                Value::Option(Some(value)) => Ok(*value),
                Value::Option(None) => Ok(Value::Option(None)),
                value => Ok(value),
            },
            Expr::Lambda { .. } => Err(RunError::UnsupportedExpr),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Control {
    Value(Value),
    Return(Value),
    Break,
    Continue,
}

impl Value {
    fn as_i64(&self) -> Result<i64, RunError> {
        match self {
            Value::Int(value) => Ok(*value),
            other => Err(RunError::Type(format!("expected int, got {other:?}"))),
        }
    }

    fn as_f64(&self) -> Result<f64, RunError> {
        match self {
            Value::Int(value) => Ok(*value as f64),
            Value::Float(value) => Ok(*value),
            other => Err(RunError::Type(format!("expected number, got {other:?}"))),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Value::Void => false,
            Value::Int(value) => *value != 0,
            Value::Float(value) => *value != 0.0,
            Value::Bool(value) => *value,
            Value::String(value) => !value.is_empty(),
            Value::Tuple(values) => !values.is_empty(),
            Value::Struct { .. } | Value::Enum { .. } => true,
            Value::Option(value) => value.is_some(),
            Value::ResultOk(_) => true,
            Value::ResultErr(_) => false,
        }
    }

    fn as_output_string(&self) -> String {
        match self {
            Value::Void => String::new(),
            Value::Int(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::String(value) => value.clone(),
            Value::Tuple(values) => values
                .iter()
                .map(Value::as_output_string)
                .collect::<Vec<_>>()
                .join(","),
            Value::Struct { ty, .. } => ty.clone(),
            Value::Enum { name, .. } => name.clone(),
            Value::Option(None) => "none".to_string(),
            Value::Option(Some(value)) => value.as_output_string(),
            Value::ResultOk(value) | Value::ResultErr(value) => value.as_output_string(),
        }
    }

    fn sum(&self) -> Result<Value, RunError> {
        let values = match self {
            Value::Tuple(values) => values,
            other => return Err(RunError::Type(format!("sum expected tuple, got {other:?}"))),
        };
        let has_float = values.iter().any(|value| matches!(value, Value::Float(_)));
        if has_float {
            let total = values
                .iter()
                .map(Value::as_f64)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum();
            Ok(Value::Float(total))
        } else {
            let total = values
                .iter()
                .map(Value::as_i64)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum();
            Ok(Value::Int(total))
        }
    }
}

fn eval_binary(op: &BinaryOp, left: Value, right: Value) -> Result<Value, RunError> {
    match op {
        BinaryOp::Add if left.is_floaty() || right.is_floaty() => {
            Ok(Value::Float(left.as_f64()? + right.as_f64()?))
        }
        BinaryOp::Sub if left.is_floaty() || right.is_floaty() => {
            Ok(Value::Float(left.as_f64()? - right.as_f64()?))
        }
        BinaryOp::Mul if left.is_floaty() || right.is_floaty() => {
            Ok(Value::Float(left.as_f64()? * right.as_f64()?))
        }
        BinaryOp::Div if left.is_floaty() || right.is_floaty() => {
            Ok(Value::Float(left.as_f64()? / right.as_f64()?))
        }
        BinaryOp::Add => Ok(Value::Int(left.as_i64()? + right.as_i64()?)),
        BinaryOp::Sub => Ok(Value::Int(left.as_i64()? - right.as_i64()?)),
        BinaryOp::Mul => Ok(Value::Int(left.as_i64()? * right.as_i64()?)),
        BinaryOp::Div => Ok(Value::Int(left.as_i64()? / right.as_i64()?)),
        BinaryOp::Mod => Ok(Value::Int(left.as_i64()? % right.as_i64()?)),
        BinaryOp::Eq => Ok(Value::Bool(left == right)),
        BinaryOp::Ne => Ok(Value::Bool(left != right)),
        BinaryOp::Lt => Ok(Value::Bool(left.as_i64()? < right.as_i64()?)),
        BinaryOp::Le => Ok(Value::Bool(left.as_i64()? <= right.as_i64()?)),
        BinaryOp::Gt => Ok(Value::Bool(left.as_i64()? > right.as_i64()?)),
        BinaryOp::Ge => Ok(Value::Bool(left.as_i64()? >= right.as_i64()?)),
        BinaryOp::And => Ok(Value::Bool(left.truthy() && right.truthy())),
        BinaryOp::Or => Ok(Value::Bool(left.truthy() || right.truthy())),
        _ => Err(RunError::UnsupportedExpr),
    }
}

impl Value {
    fn is_floaty(&self) -> bool {
        matches!(self, Value::Float(_))
    }
}

fn callee_name(expr: &Expr) -> Result<String, RunError> {
    match expr {
        Expr::Ident(name) => Ok(name.0.clone()),
        Expr::Path(path) => Ok(path_name(path)),
        Expr::Field { base, name } => Ok(format!("{}.{}", callee_name(base)?, name.0)),
        _ => Err(RunError::UnsupportedExpr),
    }
}

fn path_name(path: &Path) -> String {
    path.0
        .iter()
        .map(|part| part.0.clone())
        .collect::<Vec<_>>()
        .join(".")
}

fn wrap_return(value: Value, ret: Option<&AstType>) -> Value {
    match ret {
        Some(AstType::Option(_)) => match value {
            Value::Option(_) => value,
            other => Value::Option(Some(Box::new(other))),
        },
        Some(AstType::Result { err, .. }) => match value {
            Value::ResultOk(_) | Value::ResultErr(_) => value,
            Value::Enum { .. } if is_error_value_for(&value, err) => {
                Value::ResultErr(Box::new(value))
            }
            other => Value::ResultOk(Box::new(other)),
        },
        _ => value,
    }
}

fn is_error_value_for(value: &Value, err: &AstType) -> bool {
    matches!((value, err), (Value::Enum { .. }, AstType::Path(_)))
}

fn parse_int(value: &str) -> i64 {
    let mut digits = value.replace('_', "");
    for suffix in ["i1", "i2", "i4", "i8", "iz", "u1", "u2", "u4", "u8", "uz"] {
        if digits.ends_with(suffix) {
            digits.truncate(digits.len() - suffix.len());
            break;
        }
    }
    if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(bin) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        i64::from_str_radix(bin, 2).unwrap_or(0)
    } else {
        digits.parse().unwrap_or(0)
    }
}

fn parse_float(value: &str) -> f64 {
    let mut digits = value.replace('_', "");
    for suffix in ["f4", "f8"] {
        if digits.ends_with(suffix) {
            digits.truncate(digits.len() - suffix.len());
            break;
        }
    }
    digits.parse().unwrap_or(0.0)
}
