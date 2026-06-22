//! 最小の純粋式評価器（ADR-0004）。
//!
//! binding（signal → element-prop）は **純粋式の AST ＋ target（element, prop）** で
//! 表す。ランタイムが式を評価（signal を read）して prop を write する。binding を
//! script クロージャで表すことはしない（再利用不可・言語固有になるため）。式は
//! 副作用を持たず、閉じた値モデル（[`Value`]）の上だけで閉じる。
//!
//! この評価器は binding・prop・`:each` の key 式の共通基盤になる（ADR-0004）。

use crate::reactive::{Memo, Signal};
use crate::value::Value;
use std::collections::HashMap;

/// 式 AST 内で名前を解決する束縛。
///
/// `Signal` / `Memo` を読むと（評価が Effect の中で走るぶんには）依存が自動追跡される。
#[derive(Clone)]
pub enum Binding {
    Signal(Signal),
    Memo(Memo),
    /// 定数（prop で渡された静的値や `:each` の item 値など）。
    Const(Value),
}

impl Binding {
    fn read(&self) -> Value {
        match self {
            Binding::Signal(s) => s.get(),
            Binding::Memo(m) => m.get(),
            Binding::Const(v) => v.clone(),
        }
    }

    /// 現在値を 1 回読む（setup 時に prop 値を取り出す等に使う）。effect の外で呼ぶ
    /// 前提で、依存追跡には依存しない。
    pub fn current(&self) -> Value {
        self.read()
    }
}

/// 名前 → 束縛の評価スコープ。コンポーネントの prop・signal・`:each` の item を持つ。
#[derive(Clone, Default)]
pub struct Scope {
    vars: HashMap<String, Binding>,
}

impl Scope {
    pub fn new() -> Self {
        Scope::default()
    }

    pub fn bind(&mut self, name: impl Into<String>, binding: Binding) -> &mut Self {
        self.vars.insert(name.into(), binding);
        self
    }

    pub fn with(mut self, name: impl Into<String>, binding: Binding) -> Self {
        self.bind(name, binding);
        self
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.vars.get(name)
    }
}

/// 二項演算子。閉じた値モデル上で算術・比較・論理・文字列連結を表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// 純粋式の AST。
#[derive(Clone, Debug)]
pub enum Expr {
    /// 定数リテラル。
    Literal(Value),
    /// スコープ内の名前参照（`count`、`item` 等）。
    Var(String),
    /// プロパティアクセス（`item.name`）。ランタイムが record を理解する。
    Member(Box<Expr>, String),
    /// インデックスアクセス（`items[i]`）。
    Index(Box<Expr>, Box<Expr>),
    /// 二項演算。
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// 単項否定（`!flag`）。
    Not(Box<Expr>),
}

impl Expr {
    pub fn var(name: impl Into<String>) -> Expr {
        Expr::Var(name.into())
    }

    pub fn lit(value: Value) -> Expr {
        Expr::Literal(value)
    }

    pub fn member(self, field: impl Into<String>) -> Expr {
        Expr::Member(Box::new(self), field.into())
    }

    pub fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary(op, Box::new(lhs), Box::new(rhs))
    }

    /// 式テキストを解析して [`Expr`] にする（ADR-0004 の DSL フロントエンド）。
    /// 例：`Expr::parse("count + 1")`、`Expr::parse("item.label")`。
    pub fn parse(input: &str) -> Result<Expr, crate::parse::ParseError> {
        crate::parse::parse_expr(input)
    }

    /// スコープ上で式を評価する。`Signal` / `Memo` を読むと依存が追跡される。
    pub fn eval(&self, scope: &Scope) -> Value {
        match self {
            Expr::Literal(v) => v.clone(),
            Expr::Var(name) => match scope.lookup(name) {
                Some(b) => b.read(),
                None => Value::Bool(false), // 未定義名は falsy に倒す（防御的）。
            },
            Expr::Member(base, field) => {
                base.eval(scope).member(field).unwrap_or(Value::Bool(false))
            }
            Expr::Index(base, index) => {
                let key = index.eval(scope).to_display_string();
                base.eval(scope).member(&key).unwrap_or(Value::Bool(false))
            }
            Expr::Not(inner) => Value::Bool(!inner.eval(scope).truthy()),
            Expr::Binary(op, lhs, rhs) => eval_binary(*op, lhs.eval(scope), rhs.eval(scope)),
        }
    }
}

fn eval_binary(op: BinOp, lhs: Value, rhs: Value) -> Value {
    use BinOp::*;
    match op {
        Add => {
            // 数値どうしは加算、それ以外は文字列連結に倒す。
            match (lhs.as_number(), rhs.as_number()) {
                (Some(a), Some(b)) => Value::number(a + b),
                _ => Value::string(format!(
                    "{}{}",
                    lhs.to_display_string(),
                    rhs.to_display_string()
                )),
            }
        }
        Sub | Mul | Div => {
            let a = lhs.as_number().unwrap_or(0.0);
            let b = rhs.as_number().unwrap_or(0.0);
            let r = match op {
                Sub => a - b,
                Mul => a * b,
                Div => a / b,
                _ => unreachable!(),
            };
            Value::number(r)
        }
        Eq => Value::Bool(lhs == rhs),
        Ne => Value::Bool(lhs != rhs),
        Lt | Le | Gt | Ge => {
            let a = lhs.as_number().unwrap_or(0.0);
            let b = rhs.as_number().unwrap_or(0.0);
            let r = match op {
                Lt => a < b,
                Le => a <= b,
                Gt => a > b,
                Ge => a >= b,
                _ => unreachable!(),
            };
            Value::Bool(r)
        }
        And => Value::Bool(lhs.truthy() && rhs.truthy()),
        Or => Value::Bool(lhs.truthy() || rhs.truthy()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::Runtime;

    #[test]
    fn literal_and_var_resolve() {
        let scope = Scope::new().with("name", Binding::Const(Value::string("Hayabusa")));
        assert_eq!(Expr::var("name").eval(&scope), Value::string("Hayabusa"));
        assert_eq!(Expr::lit(Value::number(7)).eval(&scope), Value::number(7));
    }

    #[test]
    fn member_access_reads_record_field() {
        let item = Value::record([("name".into(), Value::string("Sun"))]);
        let scope = Scope::new().with("item", Binding::Const(item));
        let expr = Expr::var("item").member("name");
        assert_eq!(expr.eval(&scope), Value::string("Sun"));
    }

    #[test]
    fn arithmetic_and_comparison() {
        let scope = Scope::new().with("n", Binding::Const(Value::number(3)));
        let plus = Expr::binary(BinOp::Add, Expr::var("n"), Expr::lit(Value::number(4)));
        assert_eq!(plus.eval(&scope), Value::number(7));

        let gt = Expr::binary(BinOp::Gt, Expr::var("n"), Expr::lit(Value::number(2)));
        assert_eq!(gt.eval(&scope), Value::Bool(true));
    }

    #[test]
    fn string_concat_via_add() {
        let scope = Scope::new();
        let expr = Expr::binary(
            BinOp::Add,
            Expr::lit(Value::string("count: ")),
            Expr::lit(Value::number(5)),
        );
        assert_eq!(expr.eval(&scope), Value::string("count: 5"));
    }

    #[test]
    fn reading_a_signal_var_tracks_dependency() {
        // Effect の中で式を評価すると、参照した signal が依存として追跡される。
        let rt = Runtime::new();
        let count = rt.signal(Value::number(0));
        let scope = Scope::new().with("count", Binding::Signal(count.clone()));
        let observed = std::rc::Rc::new(std::cell::RefCell::new(Value::number(-1)));

        let scope2 = scope.clone();
        let observed2 = observed.clone();
        rt.effect(move || {
            *observed2.borrow_mut() = Expr::var("count").eval(&scope2);
        });
        assert_eq!(*observed.borrow(), Value::number(0));

        count.set(Value::number(42));
        assert_eq!(*observed.borrow(), Value::number(42));
    }
}
