//! 閉じた値モデル（ADR-0003）。
//!
//! signal が保持できる値は閉じた集合 — number / string / bool / list / record
//! （string キー）。ランタイムが所有し、DSL（式評価器・ADR-0004）は script
//! コールバックなしでこの値を評価できる（`item.name` のようなプロパティアクセスを
//! ランタイムが理解する）。
//!
//! 値が閉じているため、DSL 評価・ABI marshal・変更検知（`PartialEq`）・テキスト
//! への表示（[`Value::to_display_string`]）がすべて一様に機械化できる。

use std::collections::BTreeMap;
use std::rc::Rc;

/// ランタイムが所有するリアクティブ値の閉じた集合。
///
/// `List` / `Record` は構造共有のため `Rc` で包む（signal 間のコピーが安価）。
/// `record` のキーは決定的な反復順を得るため `BTreeMap`（string キー）で持つ。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    String(Rc<str>),
    Bool(bool),
    List(Rc<Vec<Value>>),
    Record(Rc<BTreeMap<String, Value>>),
}

impl Value {
    pub fn number(n: impl Into<f64>) -> Self {
        Value::Number(n.into())
    }

    pub fn string(s: impl AsRef<str>) -> Self {
        Value::String(Rc::from(s.as_ref()))
    }

    pub fn list(items: impl IntoIterator<Item = Value>) -> Self {
        Value::List(Rc::new(items.into_iter().collect()))
    }

    pub fn record(fields: impl IntoIterator<Item = (String, Value)>) -> Self {
        Value::Record(Rc::new(fields.into_iter().collect()))
    }

    /// 数値として解釈する。number はそのまま、bool は 0/1、それ以外は `None`。
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// `list` の要素スライス（list でなければ空）。`:each` のリストを読み出して更新する
    /// スクリプト（行の追加・削除）が、Rc の中身を直接触らずに済むようにする。
    pub fn as_slice(&self) -> &[Value] {
        match self {
            Value::List(items) => items,
            _ => &[],
        }
    }

    /// 真偽値としての解釈（`:if` 等の条件評価で使う）。
    pub fn truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            Value::List(items) => !items.is_empty(),
            Value::Record(fields) => !fields.is_empty(),
        }
    }

    /// `record` のフィールド、または `list` の数値インデックスを引く。
    pub fn member(&self, key: &str) -> Option<Value> {
        match self {
            Value::Record(fields) => fields.get(key).cloned(),
            Value::List(items) => key
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i).cloned()),
            _ => None,
        }
    }

    /// element-prop（`text` 等）へ書き込むための文字列表現。
    ///
    /// 整数値の `Number` は小数点を付けない（`1` であって `1.0` ではない）。これは
    /// `<text>{count}</text>` の表示がブラウザ同様に自然な整数になるため。
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Number(n) => {
                if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Value::String(s) => s.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(Value::to_display_string).collect();
                parts.join(",")
            }
            Value::Record(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, v.to_display_string()))
                    .collect();
                format!("{{{}}}", parts.join(","))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_render_without_decimals() {
        assert_eq!(Value::number(1).to_display_string(), "1");
        assert_eq!(Value::number(0).to_display_string(), "0");
        assert_eq!(Value::number(-42).to_display_string(), "-42");
    }

    #[test]
    fn fractional_numbers_keep_precision() {
        assert_eq!(Value::number(1.5).to_display_string(), "1.5");
    }

    #[test]
    fn member_access_reads_record_and_list() {
        let rec = Value::record([("name".into(), Value::string("Hayabusa"))]);
        assert_eq!(rec.member("name"), Some(Value::string("Hayabusa")));

        let list = Value::list([Value::number(10), Value::number(20)]);
        assert_eq!(list.member("1"), Some(Value::number(20)));
        assert_eq!(list.member("9"), None);
    }

    #[test]
    fn truthiness_matches_closed_model() {
        assert!(Value::Bool(true).truthy());
        assert!(!Value::Bool(false).truthy());
        assert!(!Value::number(0).truthy());
        assert!(Value::number(3).truthy());
        assert!(!Value::string("").truthy());
        assert!(Value::string("x").truthy());
    }

    #[test]
    fn equality_drives_change_detection() {
        assert_eq!(Value::number(2), Value::number(2));
        assert_ne!(Value::number(2), Value::number(3));
        assert_eq!(Value::string("a"), Value::string("a"));
    }
}
