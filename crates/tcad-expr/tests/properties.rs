//! Randomized property tests for units and expressions (no external dependencies).

use tcad_diag::Span;
use tcad_expr::{
    BinOp, Dim, Expr, ExprKind, Quantity, UnOp, Value, base_unit,
    eval::{EmptyEnv, eval},
};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// A float in [-1000, 1000] with a few decimals.
    fn float(&mut self) -> f64 {
        (self.below(2_000_000) as f64 - 1_000_000.0) / 1000.0
    }
}

fn e(kind: ExprKind) -> Expr {
    Expr {
        kind,
        span: Span::new(0, 1),
    }
}
fn q(v: f64, d: Dim) -> Expr {
    e(ExprKind::Quantity(v, d))
}
fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    e(ExprKind::Binary(op, Box::new(a), Box::new(b)))
}
fn quantity(x: Result<Value, tcad_expr::EvalError>) -> Option<Quantity> {
    match x {
        Ok(Value::Num(q)) => Some(q),
        _ => None,
    }
}

const DIMS: [Dim; 4] = [Dim::NONE, Dim::LENGTH, Dim::ANGLE, Dim::MASS];

fn random_dim(r: &mut Rng) -> Dim {
    DIMS[r.below(DIMS.len())]
}

#[test]
fn addition_requires_equal_dimensions() {
    let mut r = Rng(1);
    for _ in 0..2000 {
        let (da, db) = (random_dim(&mut r), random_dim(&mut r));
        let (a, b) = (r.float(), r.float());
        let res = eval(&bin(BinOp::Add, q(a, da), q(b, db)), &EmptyEnv);
        assert_eq!(res.is_ok(), da == db, "{da:?} + {db:?}");
        if let Some(sum) = quantity(res) {
            assert_eq!(sum.dim, da);
            assert_eq!(sum.value, a + b);
        }
    }
}

#[test]
fn multiplication_adds_and_division_subtracts_dimensions() {
    let mut r = Rng(2);
    for _ in 0..2000 {
        let (da, db) = (random_dim(&mut r), random_dim(&mut r));
        let (a, b) = (r.float(), r.float());
        let prod =
            quantity(eval(&bin(BinOp::Mul, q(a, da), q(b, db)), &EmptyEnv)).expect("product");
        assert_eq!(Some(prod.dim), da.checked_mul(db));
        if b != 0.0 {
            let quo =
                quantity(eval(&bin(BinOp::Div, q(a, da), q(b, db)), &EmptyEnv)).expect("quotient");
            assert_eq!(Some(quo.dim), da.checked_div(db));
            // (a / b) * b has the dimension of a again
            let back = bin(BinOp::Mul, bin(BinOp::Div, q(a, da), q(b, db)), q(b, db));
            assert_eq!(quantity(eval(&back, &EmptyEnv)).expect("back").dim, da);
        } else {
            assert!(eval(&bin(BinOp::Div, q(a, da), q(b, db)), &EmptyEnv).is_err());
        }
    }
}

#[test]
fn add_then_subtract_round_trips() {
    let mut r = Rng(3);
    for _ in 0..2000 {
        let d = random_dim(&mut r);
        let (a, b) = (r.float(), r.float());
        let e1 = bin(BinOp::Sub, bin(BinOp::Add, q(a, d), q(b, d)), q(b, d));
        let got = quantity(eval(&e1, &EmptyEnv)).expect("ok").value;
        assert!(
            (got - a).abs() <= 1e-9 * (a.abs() + b.abs() + 1.0),
            "{a} {b} {got}"
        );
    }
}

#[test]
fn negation_is_an_involution() {
    let mut r = Rng(4);
    for _ in 0..500 {
        let d = random_dim(&mut r);
        let a = r.float();
        let neg = |x: Expr| e(ExprKind::Unary(UnOp::Neg, Box::new(x)));
        let got = quantity(eval(&neg(neg(q(a, d))), &EmptyEnv)).expect("ok");
        assert_eq!((got.value, got.dim), (a, d));
    }
}

#[test]
fn power_dimension_matches_repeated_multiplication() {
    let mut r = Rng(5);
    for _ in 0..500 {
        let d = random_dim(&mut r);
        let n = r.below(5) as i32;
        let a = (r.below(2000) as f64 - 1000.0) / 100.0;
        let p = quantity(eval(
            &bin(BinOp::Pow, q(a, d), e(ExprKind::Number(f64::from(n)))),
            &EmptyEnv,
        ));
        let p = p.expect("integer power");
        assert_eq!(Some(p.dim), d.pow(n));
        let mut want = 1.0;
        for _ in 0..n {
            want *= a;
        }
        assert!(
            (p.value - want).abs() <= 1e-9 * want.abs().max(1.0),
            "{a}^{n}"
        );
    }
}

#[test]
fn unit_conversions_are_consistent() {
    for (a, b, ratio) in [
        ("cm", "mm", 10.0),
        ("m", "cm", 100.0),
        ("ft", "in", 12.0),
        ("kg", "g", 1000.0),
    ] {
        let (ua, ub) = (base_unit(a).expect(a), base_unit(b).expect(b));
        assert_eq!(ua.dim, ub.dim);
        assert!((ua.factor / ub.factor - ratio).abs() < 1e-9, "{a}/{b}");
    }
    let deg = base_unit("deg").expect("deg");
    assert!((deg.factor * 180.0 - std::f64::consts::PI).abs() < 1e-15);
    assert!((base_unit("lb").expect("lb").factor - 0.453_592_37).abs() < 1e-12);
}

#[test]
fn random_expressions_never_panic() {
    fn gen_expr(r: &mut Rng, depth: u32) -> Expr {
        if depth == 0 || r.below(4) == 0 {
            return match r.below(3) {
                0 => e(ExprKind::Number(r.float())),
                1 => q(r.float(), random_dim(r)),
                _ => e(ExprKind::Bool(r.below(2) == 0)),
            };
        }
        const OPS: [BinOp; 13] = [
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::Div,
            BinOp::Pow,
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Gt,
            BinOp::Ge,
            BinOp::And,
            BinOp::Or,
        ];
        const FUNCS: [&str; 15] = [
            "min", "max", "abs", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
            "floor", "ceil", "round", "bogus",
        ];
        match r.below(3) {
            0 => bin(
                OPS[r.below(OPS.len())],
                gen_expr(r, depth - 1),
                gen_expr(r, depth - 1),
            ),
            1 => e(ExprKind::Unary(
                if r.below(2) == 0 {
                    UnOp::Neg
                } else {
                    UnOp::Not
                },
                Box::new(gen_expr(r, depth - 1)),
            )),
            _ => {
                let n = r.below(4);
                let args = (0..n).map(|_| gen_expr(r, depth - 1)).collect();
                e(ExprKind::Call(FUNCS[r.below(FUNCS.len())].into(), args))
            }
        }
    }
    let mut r = Rng(6);
    for _ in 0..20_000 {
        let ex = gen_expr(&mut r, 4);
        let _ = eval(&ex, &EmptyEnv); // must not panic
    }
}
