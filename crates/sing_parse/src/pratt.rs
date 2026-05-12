use sing_ast::BinaryOp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Infix {
    pub op: BinaryOp,
    pub precedence: u8,
}

pub(crate) const PREC_OR: u8 = 2;
pub(crate) const PREC_AND: u8 = 3;
pub(crate) const PREC_COMPARE: u8 = 4;
pub(crate) const PREC_ADD: u8 = 5;
pub(crate) const PREC_MUL: u8 = 6;
pub(crate) const PREC_TERNARY: u8 = 1;

pub(crate) fn infix_for(op: &str) -> Option<Infix> {
    let (op, precedence) = match op {
        "*" => (BinaryOp::Mul, PREC_MUL),
        "/" => (BinaryOp::Div, PREC_MUL),
        "%" => (BinaryOp::Mod, PREC_MUL),
        "*%" => (BinaryOp::MulWrap, PREC_MUL),
        "*|" => (BinaryOp::MulSat, PREC_MUL),
        "*?" => (BinaryOp::MulChk, PREC_MUL),
        "+" => (BinaryOp::Add, PREC_ADD),
        "-" => (BinaryOp::Sub, PREC_ADD),
        "+%" => (BinaryOp::AddWrap, PREC_ADD),
        "-%" => (BinaryOp::SubWrap, PREC_ADD),
        "+|" => (BinaryOp::AddSat, PREC_ADD),
        "-|" => (BinaryOp::SubSat, PREC_ADD),
        "+?" => (BinaryOp::AddChk, PREC_ADD),
        "-?" => (BinaryOp::SubChk, PREC_ADD),
        "<" => (BinaryOp::Lt, PREC_COMPARE),
        "<=" => (BinaryOp::Le, PREC_COMPARE),
        ">" => (BinaryOp::Gt, PREC_COMPARE),
        ">=" => (BinaryOp::Ge, PREC_COMPARE),
        "==" | "=" => (BinaryOp::Eq, PREC_COMPARE),
        "!=" => (BinaryOp::Ne, PREC_COMPARE),
        "&" => (BinaryOp::And, PREC_AND),
        "|" => (BinaryOp::Or, PREC_OR),
        _ => return None,
    };
    Some(Infix { op, precedence })
}
