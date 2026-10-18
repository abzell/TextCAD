//! Expression evaluation with dimension checking.
//!
//! Every parameter expression is a constant expression, so evaluating it also type-checks it:
//! adding a length to an angle is an error whatever the values are. Trigonometric functions come
//! from `libm` (pure Rust) so results are identical on every platform.

use crate::ast::{BinOp, Expr, ExprKind, UnOp};
use crate::dim::{Dim, powi};
use crate::value::{Quantity, Value};
use tcad_diag::{Diagnostic, Span, codes};

/// Supplies values for plain names (parameters).
pub trait Env {
    /// The value of `name`, if it denotes one.
    fn get(&self, name: &str) -> Option<Value>;
}

/// An environment with no names.
pub struct EmptyEnv;

impl Env for EmptyEnv {
    fn get(&self, _: &str) -> Option<Value> {
        None
    }
}

/// Why evaluation stopped.
#[derive(Debug, PartialEq)]
pub enum EvalError {
    /// The expression contains something that is not a value here (a geometric reference, a
    /// direction, a query, an undefined name). Not an error by itself: the caller decides.
    NotAValue(Span, String),
    /// A real error.
    Diag(Diagnostic),
}

type Res = Result<Value, EvalError>;

fn err(code: tcad_diag::Code, span: Span, msg: impl Into<String>) -> EvalError {
    EvalError::Diag(Diagnostic::error(code, span, msg))
}

/// Evaluates an expression.
pub fn eval(e: &Expr, env: &dyn Env) -> Res {
    match &e.kind {
        ExprKind::Number(n) => finite(Quantity::number(*n), e.span),
        ExprKind::Quantity(v, d) => finite(Quantity { value: *v, dim: *d }, e.span),
        ExprKind::Str(s) => Ok(Value::Str(s.clone())),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Ref(r) => match r.simple_name().and_then(|n| env.get(n)) {
            Some(v) => Ok(v),
            None => Err(EvalError::NotAValue(r.span, r.dotted())),
        },
        ExprKind::Direction(d) => Err(EvalError::NotAValue(
            d.span,
            format!("direction {}", d.axis),
        )),
        ExprKind::Query(..) => Err(EvalError::NotAValue(e.span, "a topology query".into())),
        ExprKind::Profile(..) => Err(EvalError::NotAValue(e.span, "a profile".into())),
        ExprKind::Tuple(es) => Ok(Value::Tuple(eval_all(es, env)?)),
        ExprKind::List(es) => Ok(Value::List(eval_all(es, env)?)),
        ExprKind::Unary(op, x) => unary(*op, eval(x, env)?, e.span),
        ExprKind::Binary(op, a, b) => {
            let (av, bv) = (eval(a, env)?, eval(b, env)?);
            binary(*op, av, bv, e.span)
        }
        ExprKind::Call(name, args) => {
            let vals = eval_all(args, env)?;
            call(name, vals, e.span)
        }
    }
}

fn eval_all(es: &[Expr], env: &dyn Env) -> Result<Vec<Value>, EvalError> {
    es.iter().map(|e| eval(e, env)).collect()
}

fn finite(q: Quantity, span: Span) -> Res {
    if q.value.is_finite() {
        Ok(Value::Num(q))
    } else {
        Err(err(codes::E2205, span, "the result is not a finite number"))
    }
}

fn num(v: &Value, span: Span, what: &str) -> Result<Quantity, EvalError> {
    match v {
        Value::Num(q) => Ok(*q),
        other => Err(err(
            codes::E2202,
            span,
            format!("{what} needs a number, found {}", other.type_name()),
        )),
    }
}

fn unary(op: UnOp, v: Value, span: Span) -> Res {
    match (op, v) {
        (UnOp::Neg, Value::Num(q)) => finite(
            Quantity {
                value: -q.value,
                dim: q.dim,
            },
            span,
        ),
        (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnOp::Neg, other) => Err(err(
            codes::E2202,
            span,
            format!("cannot negate {}", other.type_name()),
        )),
        (UnOp::Not, other) => Err(err(
            codes::E2202,
            span,
            format!("`not` needs a boolean, found {}", other.type_name()),
        )),
    }
}

fn verb(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "subtract",
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => "compare",
        _ => "combine",
    }
}

fn binary(op: BinOp, a: Value, b: Value, span: Span) -> Res {
    match op {
        BinOp::And | BinOp::Or => match (&a, &b) {
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(if op == BinOp::And {
                *x && *y
            } else {
                *x || *y
            })),
            _ => Err(err(
                codes::E2202,
                span,
                format!(
                    "`and`/`or` need booleans, found {} and {}",
                    a.type_name(),
                    b.type_name()
                ),
            )),
        },
        BinOp::Eq | BinOp::Ne => {
            let equal = match (&a, &b) {
                (Value::Num(x), Value::Num(y)) => {
                    same_dim(*x, *y, op, span)?;
                    x.value == y.value
                }
                (Value::Bool(x), Value::Bool(y)) => x == y,
                (Value::Str(x), Value::Str(y)) => x == y,
                _ => {
                    return Err(err(
                        codes::E2202,
                        span,
                        format!("cannot compare {} with {}", a.type_name(), b.type_name()),
                    ));
                }
            };
            Ok(Value::Bool(equal == (op == BinOp::Eq)))
        }
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let (x, y) = (num(&a, span, "comparison")?, num(&b, span, "comparison")?);
            same_dim(x, y, op, span)?;
            let r = match op {
                BinOp::Lt => x.value < y.value,
                BinOp::Le => x.value <= y.value,
                BinOp::Gt => x.value > y.value,
                _ => x.value >= y.value,
            };
            Ok(Value::Bool(r))
        }
        BinOp::Add | BinOp::Sub => {
            let (x, y) = (num(&a, span, verb(op))?, num(&b, span, verb(op))?);
            same_dim(x, y, op, span)?;
            let v = if op == BinOp::Add {
                x.value + y.value
            } else {
                x.value - y.value
            };
            finite(
                Quantity {
                    value: v,
                    dim: x.dim,
                },
                span,
            )
        }
        BinOp::Mul | BinOp::Div => {
            let (x, y) = (num(&a, span, "arithmetic")?, num(&b, span, "arithmetic")?);
            if op == BinOp::Div && y.value == 0.0 {
                return Err(err(codes::E2205, span, "division by zero"));
            }
            let dim = if op == BinOp::Mul {
                x.dim.checked_mul(y.dim)
            } else {
                x.dim.checked_div(y.dim)
            };
            let Some(dim) = dim else {
                return Err(err(codes::E2204, span, "dimension exponent out of range"));
            };
            let v = if op == BinOp::Mul {
                x.value * y.value
            } else {
                x.value / y.value
            };
            finite(Quantity { value: v, dim }, span)
        }
        BinOp::Pow => pow(num(&a, span, "`^`")?, num(&b, span, "`^`")?, span),
    }
}

fn same_dim(x: Quantity, y: Quantity, op: BinOp, span: Span) -> Result<(), EvalError> {
    if x.dim == y.dim {
        Ok(())
    } else {
        Err(err(
            codes::E2201,
            span,
            format!("cannot {} {} and {}", verb(op), x.dim, y.dim),
        ))
    }
}

fn pow(base: Quantity, exp: Quantity, span: Span) -> Res {
    if !exp.dim.is_none() {
        return Err(err(
            codes::E2201,
            span,
            format!("the exponent must be dimensionless, found {}", exp.dim),
        ));
    }
    let n = exp.value;
    if base.dim.is_none() {
        let v = if n.fract() == 0.0 && n.abs() <= f64::from(i32::MAX) {
            powi(base.value, n as i32)
        } else {
            libm::pow(base.value, n)
        };
        return finite(Quantity::number(v), span)
            .map_err(|_| err(codes::E2205, span, "the power is not a finite number"));
    }
    if n.fract() != 0.0 || n.abs() > 64.0 {
        return Err(err(
            codes::E2204,
            span,
            format!("a {} can only be raised to a whole-number power", base.dim),
        ));
    }
    let Some(dim) = base.dim.pow(n as i32) else {
        return Err(err(codes::E2204, span, "dimension exponent out of range"));
    };
    finite(
        Quantity {
            value: powi(base.value, n as i32),
            dim,
        },
        span,
    )
}

fn call(name: &str, args: Vec<Value>, span: Span) -> Res {
    let arity = |n: usize| -> Result<(), EvalError> {
        if args.len() == n {
            Ok(())
        } else {
            Err(err(
                codes::E2206,
                span,
                format!(
                    "`{name}` takes {n} argument{}, found {}",
                    if n == 1 { "" } else { "s" },
                    args.len()
                ),
            ))
        }
    };
    let q = |i: usize| num(&args[i], span, &format!("`{name}`"));
    let out = |value: f64, dim: Dim| finite(Quantity { value, dim }, span);
    match name {
        "min" | "max" => {
            if args.is_empty() {
                return Err(err(
                    codes::E2206,
                    span,
                    format!("`{name}` needs at least one argument"),
                ));
            }
            let first = q(0)?;
            let mut best = first.value;
            for i in 1..args.len() {
                let x = q(i)?;
                if x.dim != first.dim {
                    return Err(err(
                        codes::E2201,
                        span,
                        format!(
                            "`{name}` arguments must share a dimension, found {} and {}",
                            first.dim, x.dim
                        ),
                    ));
                }
                best = if name == "min" {
                    best.min(x.value)
                } else {
                    best.max(x.value)
                };
            }
            out(best, first.dim)
        }
        "abs" | "floor" | "ceil" | "round" => {
            arity(1)?;
            let x = q(0)?;
            let v = match name {
                "abs" => x.value.abs(),
                "floor" => x.value.floor(),
                "ceil" => x.value.ceil(),
                _ => x.value.round(),
            };
            out(v, x.dim)
        }
        "sqrt" => {
            arity(1)?;
            let x = q(0)?;
            let Some(dim) = x.dim.sqrt() else {
                return Err(err(
                    codes::E2204,
                    span,
                    format!("cannot take the square root of a {}", x.dim),
                ));
            };
            if x.value < 0.0 {
                return Err(err(codes::E2205, span, "square root of a negative number"));
            }
            out(x.value.sqrt(), dim)
        }
        "sin" | "cos" | "tan" => {
            arity(1)?;
            let x = q(0)?;
            if x.dim != Dim::ANGLE {
                return Err(err(
                    codes::E2201,
                    span,
                    format!(
                        "`{name}` needs an angle (write e.g. `30 deg`), found {}",
                        x.dim
                    ),
                ));
            }
            let v = match name {
                "sin" => libm::sin(x.value),
                "cos" => libm::cos(x.value),
                _ => libm::tan(x.value),
            };
            out(v, Dim::NONE)
        }
        "asin" | "acos" | "atan" => {
            arity(1)?;
            let x = q(0)?;
            if !x.dim.is_none() {
                return Err(err(
                    codes::E2201,
                    span,
                    format!("`{name}` needs a dimensionless number, found {}", x.dim),
                ));
            }
            if name != "atan" && x.value.abs() > 1.0 {
                return Err(err(
                    codes::E2205,
                    span,
                    format!("`{name}` argument outside [-1, 1]"),
                ));
            }
            let v = match name {
                "asin" => libm::asin(x.value),
                "acos" => libm::acos(x.value),
                _ => libm::atan(x.value),
            };
            out(v, Dim::ANGLE)
        }
        "atan2" => {
            arity(2)?;
            let (y, x) = (q(0)?, q(1)?);
            if y.dim != x.dim {
                return Err(err(
                    codes::E2201,
                    span,
                    format!(
                        "`atan2` arguments must share a dimension, found {} and {}",
                        y.dim, x.dim
                    ),
                ));
            }
            out(libm::atan2(y.value, x.value), Dim::ANGLE)
        }
        other => Err(err(
            codes::E2206,
            span,
            format!("unknown function `{other}`"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Reference, Segment};

    fn sp() -> Span {
        Span::new(0, 1)
    }
    fn e(kind: ExprKind) -> Expr {
        Expr { kind, span: sp() }
    }
    fn mm(v: f64) -> Expr {
        e(ExprKind::Quantity(v, Dim::LENGTH))
    }
    fn deg(v: f64) -> Expr {
        e(ExprKind::Quantity(
            v * std::f64::consts::PI / 180.0,
            Dim::ANGLE,
        ))
    }
    fn n(v: f64) -> Expr {
        e(ExprKind::Number(v))
    }
    fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
        e(ExprKind::Binary(op, Box::new(a), Box::new(b)))
    }
    fn call_e(name: &str, args: Vec<Expr>) -> Expr {
        e(ExprKind::Call(name.into(), args))
    }
    fn name(s: &str) -> Expr {
        e(ExprKind::Ref(Reference {
            segments: vec![Segment {
                name: s.into(),
                args: None,
                span: sp(),
            }],
            ordinal: None,
            span: sp(),
        }))
    }
    fn code(r: Res) -> &'static str {
        match r {
            Err(EvalError::Diag(d)) => d.code.as_str(),
            Err(EvalError::NotAValue(..)) => "not-a-value",
            Ok(_) => "ok",
        }
    }
    fn q(r: Res) -> Quantity {
        r.expect("ok").as_quantity().expect("quantity")
    }

    struct P;
    impl Env for P {
        fn get(&self, n: &str) -> Option<Value> {
            (n == "w").then(|| Value::Num(Quantity::mm(10.0)))
        }
    }

    #[test]
    fn arithmetic_and_dimensions() {
        let r = q(eval(&bin(BinOp::Add, mm(2.0), mm(3.0)), &EmptyEnv));
        assert_eq!((r.value, r.dim), (5.0, Dim::LENGTH));
        let area = q(eval(&bin(BinOp::Mul, mm(2.0), mm(3.0)), &EmptyEnv));
        assert_eq!(
            area.dim,
            Dim {
                length: 2,
                mass: 0,
                angle: 0
            }
        );
        let ratio = q(eval(&bin(BinOp::Div, mm(6.0), mm(3.0)), &EmptyEnv));
        assert_eq!((ratio.value, ratio.dim), (2.0, Dim::NONE));
        assert_eq!(
            code(eval(&bin(BinOp::Add, mm(1.0), deg(1.0)), &EmptyEnv)),
            "E2201"
        );
        assert_eq!(
            code(eval(&bin(BinOp::Add, mm(1.0), n(1.0)), &EmptyEnv)),
            "E2201"
        );
        assert_eq!(
            code(eval(&bin(BinOp::Div, mm(1.0), n(0.0)), &EmptyEnv)),
            "E2205"
        );
    }

    #[test]
    fn powers() {
        let sq = q(eval(&bin(BinOp::Pow, mm(3.0), n(2.0)), &EmptyEnv));
        assert_eq!((sq.value, sq.dim.length), (9.0, 2));
        assert_eq!(
            code(eval(&bin(BinOp::Pow, mm(3.0), n(0.5)), &EmptyEnv)),
            "E2204"
        );
        assert_eq!(
            code(eval(&bin(BinOp::Pow, n(2.0), mm(1.0)), &EmptyEnv)),
            "E2201"
        );
        assert_eq!(
            q(eval(&bin(BinOp::Pow, n(2.0), n(-1.0)), &EmptyEnv)).value,
            0.5
        );
        assert_eq!(
            code(eval(&bin(BinOp::Pow, n(-8.0), n(0.5)), &EmptyEnv)),
            "E2205"
        );
    }

    #[test]
    fn functions() {
        assert_eq!(
            q(eval(&call_e("min", vec![mm(3.0), mm(2.0)]), &EmptyEnv)).value,
            2.0
        );
        assert_eq!(
            code(eval(&call_e("min", vec![mm(3.0), deg(2.0)]), &EmptyEnv)),
            "E2201"
        );
        let s = q(eval(&call_e("sin", vec![deg(30.0)]), &EmptyEnv));
        assert!((s.value - 0.5).abs() < 1e-12 && s.dim.is_none());
        assert_eq!(
            code(eval(&call_e("sin", vec![n(30.0)]), &EmptyEnv)),
            "E2201"
        );
        let a = q(eval(&call_e("asin", vec![n(0.5)]), &EmptyEnv));
        assert_eq!(a.dim, Dim::ANGLE);
        assert_eq!(
            q(eval(
                &call_e("sqrt", vec![bin(BinOp::Mul, mm(4.0), mm(9.0))]),
                &EmptyEnv
            ))
            .value,
            6.0
        );
        assert_eq!(
            code(eval(&call_e("sqrt", vec![mm(4.0)]), &EmptyEnv)),
            "E2204"
        );
        assert_eq!(
            q(eval(&call_e("round", vec![mm(2.5)]), &EmptyEnv)).value,
            3.0
        );
        assert_eq!(code(eval(&call_e("abs", vec![]), &EmptyEnv)), "E2206");
        let t = q(eval(&call_e("atan2", vec![mm(1.0), mm(1.0)]), &EmptyEnv));
        assert!((t.value - std::f64::consts::FRAC_PI_4).abs() < 1e-15);
    }

    #[test]
    fn booleans_and_comparison() {
        let lt = eval(&bin(BinOp::Lt, mm(1.0), mm(2.0)), &EmptyEnv).unwrap();
        assert_eq!(lt, Value::Bool(true));
        assert_eq!(
            code(eval(&bin(BinOp::Lt, mm(1.0), deg(2.0)), &EmptyEnv)),
            "E2201"
        );
        assert_eq!(
            code(eval(
                &bin(BinOp::And, n(1.0), e(ExprKind::Bool(true))),
                &EmptyEnv
            )),
            "E2202"
        );
        let not = eval(
            &e(ExprKind::Unary(
                UnOp::Not,
                Box::new(e(ExprKind::Bool(false))),
            )),
            &EmptyEnv,
        );
        assert_eq!(not.unwrap(), Value::Bool(true));
    }

    #[test]
    fn names() {
        assert_eq!(q(eval(&bin(BinOp::Mul, name("w"), n(2.0)), &P)).value, 20.0);
        assert_eq!(code(eval(&name("nope"), &P)), "not-a-value");
    }
}
