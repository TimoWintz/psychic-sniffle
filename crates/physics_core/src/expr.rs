//! The calculator: expressions with units and named variables.
//!
//! `12.5 m/s`, `sqrt(2*h1/g)`, `sin(35 deg)`. A number followed by a name multiplies (`2 m` is `2*m`),
//! at the same precedence as `*`, left to right — so `3 m / 2 s` reads as `((3 m)/2) s`; write
//! `3 m / (2 s)` for the other meaning.

use crate::units::{Dim, Quantity, lookup_unit};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ExprError {
    #[error("Nothing to evaluate.")]
    Empty,
    #[error("Could not read the expression at position {pos}: {msg}")]
    Syntax { pos: usize, msg: String },
    #[error("'{0}' is not a notebook entry, a unit or a function.")]
    UnknownName(String),
    #[error("Cannot {op} {left} and {right}: the dimensions differ.")]
    Mismatch { op: &'static str, left: String, right: String },
    #[error("{func} needs a pure number, but was given {got}.")]
    NeedsNumber { func: String, got: String },
    #[error("An exponent must be a pure number, but this one is {0}.")]
    DimensionedExponent(String),
    #[error("Cannot raise {0} to that power: the result has no sensible unit.")]
    BadPower(String),
    #[error("{0}")]
    Domain(String),
}

/// Supplies notebook variables to the calculator.
pub trait Env {
    fn var(&self, name: &str) -> Option<Quantity>;
}

/// An environment with no variables: only units and constants resolve.
pub struct NoVars;
impl Env for NoVars {
    fn var(&self, _: &str) -> Option<Quantity> {
        None
    }
}

impl<F: Fn(&str) -> Option<Quantity>> Env for F {
    fn var(&self, name: &str) -> Option<Quantity> {
        self(name)
    }
}

/// Evaluate `source`. Names resolve to notebook variables first, then to units and constants.
pub fn evaluate(source: &str, env: &dyn Env) -> Result<Quantity, ExprError> {
    let tokens = tokenize(source)?;
    if tokens.is_empty() {
        return Err(ExprError::Empty);
    }
    let mut parser = Parser { tokens: &tokens, at: 0, env };
    let value = parser.expr()?;
    if let Some(tok) = parser.peek() {
        return Err(ExprError::Syntax { pos: tok.pos, msg: "unexpected input here".into() });
    }
    if !value.value.is_finite() {
        return Err(ExprError::Domain("The result is not a finite number.".into()));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    pos: usize,
}

fn tokenize(source: &str) -> Result<Vec<Token>, ExprError> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let pos = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // Scientific notation only when an exponent really follows: "2e3", "1.5E-2".
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let value = text
                .parse::<f64>()
                .map_err(|_| ExprError::Syntax { pos, msg: format!("'{text}' is not a number") })?;
            out.push(Token { tok: Tok::Num(value), pos });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            // Superscript digits count as alphanumeric in Unicode but mean a power here.
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') && !matches!(chars[i], '²' | '³') {
                i += 1;
            }
            out.push(Token { tok: Tok::Name(chars[start..i].iter().collect()), pos });
            continue;
        }
        let tok = match c {
            '+' => Tok::Plus,
            '-' | '−' => Tok::Minus,
            '*' | '×' | '·' => Tok::Star,
            '/' | '÷' => Tok::Slash,
            '^' => Tok::Caret,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '°' => Tok::Name("deg".into()),
            '²' | '³' => {
                out.push(Token { tok: Tok::Caret, pos });
                Tok::Num(if c == '²' { 2.0 } else { 3.0 })
            }
            other => {
                return Err(ExprError::Syntax { pos, msg: format!("unexpected character '{other}'") });
            }
        };
        out.push(Token { tok, pos });
        i += 1;
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    env: &'a dyn Env,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn peek_tok(&self) -> Option<&Tok> {
        self.peek().map(|t| &t.tok)
    }

    fn end_pos(&self) -> usize {
        self.tokens.last().map_or(0, |t| t.pos + 1)
    }

    fn expect(&mut self, want: Tok, what: &str) -> Result<(), ExprError> {
        match self.peek() {
            Some(t) if t.tok == want => {
                self.at += 1;
                Ok(())
            }
            Some(t) => Err(ExprError::Syntax { pos: t.pos, msg: format!("expected {what}") }),
            None => Err(ExprError::Syntax { pos: self.end_pos(), msg: format!("expected {what}") }),
        }
    }

    fn expr(&mut self) -> Result<Quantity, ExprError> {
        let mut left = self.term()?;
        loop {
            let (op, sign) = match self.peek_tok() {
                Some(Tok::Plus) => ("add", 1.0),
                Some(Tok::Minus) => ("subtract", -1.0),
                _ => return Ok(left),
            };
            self.at += 1;
            let right = self.term()?;
            if left.dim != right.dim {
                return Err(ExprError::Mismatch {
                    op,
                    left: left.dim.describe(),
                    right: right.dim.describe(),
                });
            }
            left = Quantity::new(left.value + sign * right.value, left.dim);
        }
    }

    fn term(&mut self) -> Result<Quantity, ExprError> {
        let mut left = self.unary()?;
        loop {
            match self.peek_tok() {
                Some(Tok::Star) => {
                    self.at += 1;
                    let right = self.unary()?;
                    left = Quantity::new(left.value * right.value, left.dim.mul(right.dim));
                }
                Some(Tok::Slash) => {
                    self.at += 1;
                    let right = self.unary()?;
                    if right.value == 0.0 {
                        return Err(ExprError::Domain("Division by zero.".into()));
                    }
                    left = Quantity::new(left.value / right.value, left.dim.div(right.dim));
                }
                // Juxtaposition: "2 m", "3 (4)", "5 kg m".
                Some(Tok::Name(_)) | Some(Tok::Num(_)) | Some(Tok::LParen) => {
                    let right = self.power()?;
                    left = Quantity::new(left.value * right.value, left.dim.mul(right.dim));
                }
                _ => return Ok(left),
            }
        }
    }

    fn unary(&mut self) -> Result<Quantity, ExprError> {
        match self.peek_tok() {
            Some(Tok::Minus) => {
                self.at += 1;
                let v = self.unary()?;
                Ok(Quantity::new(-v.value, v.dim))
            }
            Some(Tok::Plus) => {
                self.at += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Quantity, ExprError> {
        let base = self.primary()?;
        if self.peek_tok() != Some(&Tok::Caret) {
            return Ok(base);
        }
        self.at += 1;
        let exponent = self.unary()?;
        if !exponent.dim.is_none() {
            return Err(ExprError::DimensionedExponent(exponent.dim.describe()));
        }
        pow(base, exponent.value)
    }

    fn primary(&mut self) -> Result<Quantity, ExprError> {
        let Some(token) = self.peek().cloned() else {
            return Err(ExprError::Syntax { pos: self.end_pos(), msg: "the expression stops early".into() });
        };
        self.at += 1;
        match token.tok {
            Tok::Num(v) => Ok(Quantity::number(v)),
            Tok::LParen => {
                let v = self.expr()?;
                self.expect(Tok::RParen, "a closing ')'")?;
                Ok(v)
            }
            Tok::Name(name) => {
                if let Some(v) = self.env.var(&name) {
                    return Ok(v);
                }
                if is_function(&name) && self.peek_tok() == Some(&Tok::LParen) {
                    self.at += 1;
                    let arg = self.expr()?;
                    self.expect(Tok::RParen, "a closing ')'")?;
                    return call(&name, arg);
                }
                lookup_unit(&name).ok_or(ExprError::UnknownName(name))
            }
            _ => Err(ExprError::Syntax { pos: token.pos, msg: "expected a number, a name or '('".into() }),
        }
    }
}

fn pow(base: Quantity, exponent: f64) -> Result<Quantity, ExprError> {
    // Dimensioned bases accept whole and half powers only.
    let doubled = exponent * 2.0;
    let dim = if base.dim.is_none() {
        Dim::NONE
    } else if (doubled - doubled.round()).abs() < 1e-9 {
        base.dim
            .pow_ratio(doubled.round() as i32, 2)
            .ok_or_else(|| ExprError::BadPower(base.dim.describe()))?
    } else {
        return Err(ExprError::BadPower(base.dim.describe()));
    };
    let value = base.value.powf(exponent);
    if value.is_nan() {
        return Err(ExprError::Domain("That power of a negative number is not a real number.".into()));
    }
    Ok(Quantity::new(value, dim))
}

fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "ln" | "exp" | "abs"
    )
}

fn call(name: &str, arg: Quantity) -> Result<Quantity, ExprError> {
    let pure = |f: fn(f64) -> f64| -> Result<Quantity, ExprError> {
        if !arg.dim.is_none() {
            return Err(ExprError::NeedsNumber { func: name.to_string(), got: arg.dim.describe() });
        }
        Ok(Quantity::number(f(arg.value)))
    };
    match name {
        "sqrt" => {
            if arg.value < 0.0 {
                return Err(ExprError::Domain("Square root of a negative value.".into()));
            }
            let dim = arg.dim.pow_ratio(1, 2).ok_or_else(|| ExprError::BadPower(arg.dim.describe()))?;
            Ok(Quantity::new(arg.value.sqrt(), dim))
        }
        "abs" => Ok(Quantity::new(arg.value.abs(), arg.dim)),
        "sin" => pure(f64::sin),
        "cos" => pure(f64::cos),
        "tan" => pure(f64::tan),
        "atan" => pure(f64::atan),
        "exp" => pure(f64::exp),
        "asin" | "acos" => {
            if arg.dim.is_none() && !(-1.0..=1.0).contains(&arg.value) {
                return Err(ExprError::Domain(format!("{name} is only defined between -1 and 1.")));
            }
            pure(if name == "asin" { f64::asin } else { f64::acos })
        }
        "ln" => {
            if arg.dim.is_none() && arg.value <= 0.0 {
                return Err(ExprError::Domain("ln is only defined for positive numbers.".into()));
            }
            pure(f64::ln)
        }
        _ => Err(ExprError::UnknownName(name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(s: &str) -> Result<Quantity, ExprError> {
        evaluate(s, &NoVars)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    #[test]
    fn numbers_with_units() {
        assert_eq!(eval("12.5 m/s").unwrap(), Quantity::speed(12.5));
        assert_eq!(eval("9.8 m/s^2").unwrap(), Quantity::acceleration(9.8));
        assert_eq!(eval("9.8 m/s²").unwrap(), Quantity::acceleration(9.8));
        assert_eq!(eval("2 km").unwrap(), Quantity::metres(2000.0));
        assert_eq!(eval("1.5e2 cm").unwrap(), Quantity::metres(1.5));
        assert!(close(eval("90 km/h").unwrap().value, 25.0));
        assert_eq!(eval("90 km/h").unwrap().dim, Dim::SPEED);
    }

    #[test]
    fn arithmetic_tracks_dimensions() {
        let v = eval("12 m / (1.5 s)").unwrap();
        assert_eq!(v, Quantity::speed(8.0));
        let f = eval("2 kg * 3 m/s^2").unwrap();
        assert_eq!(f.dim, Dim::FORCE);
        assert!(close(f.value, 6.0));
        assert_eq!(eval("-2^2").unwrap().value, -4.0);
        assert_eq!(eval("2^3^2").unwrap().value, 512.0);
        assert_eq!(eval("(1+2)*3").unwrap().value, 9.0);
    }

    #[test]
    fn mismatched_dimensions_are_explained() {
        let err = eval("3 m + 2 s").unwrap_err();
        assert_eq!(
            err.to_string(),
            "Cannot add a length (m) and a time (s): the dimensions differ."
        );
        assert!(matches!(eval("sin(3 m)"), Err(ExprError::NeedsNumber { .. })));
        assert!(matches!(eval("2^(1 s)"), Err(ExprError::DimensionedExponent(_))));
    }

    #[test]
    fn functions_and_angles() {
        assert!(close(eval("sin(30 deg)").unwrap().value, 0.5));
        assert!(close(eval("sin(30°)").unwrap().value, 0.5));
        assert!(close(eval("atan(1)/deg").unwrap().value, 45.0));
        let t = eval("sqrt(2 * 20 m / (10 m/s^2))").unwrap();
        assert_eq!(t.dim, Dim::TIME);
        assert!(close(t.value, 2.0));
        assert!(matches!(eval("sqrt(-1)"), Err(ExprError::Domain(_))));
        assert!(matches!(eval("asin(2)"), Err(ExprError::Domain(_))));
    }

    #[test]
    fn variables_shadow_units() {
        // `g` is gravity when the notebook has it; `h` would otherwise be the hour.
        let env = |name: &str| match name {
            "g" => Some(Quantity::acceleration(8.0)),
            "h" => Some(Quantity::metres(16.0)),
            _ => None,
        };
        let t = evaluate("sqrt(2*h/g)", &env).unwrap();
        assert_eq!(t.dim, Dim::TIME);
        assert!(close(t.value, 2.0));
        // Without the notebook entry, `h` is an hour.
        assert_eq!(eval("1 h").unwrap(), Quantity::seconds(3600.0));
    }

    #[test]
    fn errors() {
        assert_eq!(eval("  "), Err(ExprError::Empty));
        assert_eq!(eval("3 furlongs"), Err(ExprError::UnknownName("furlongs".into())));
        assert!(matches!(eval("(1 + 2"), Err(ExprError::Syntax { .. })));
        assert!(matches!(eval("1 +"), Err(ExprError::Syntax { .. })));
        assert!(matches!(eval("1 / 0"), Err(ExprError::Domain(_))));
        assert!(matches!(eval("2 $ 3"), Err(ExprError::Syntax { .. })));
    }
}
