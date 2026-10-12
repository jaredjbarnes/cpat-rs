#![allow(dead_code)]

use cpat_core::{Choice, Cut, Literal, Not, Optional, Pattern, Regex, Sequence};

pub fn lit(name: &str, token: &str) -> Pattern {
    Literal::new(name, token).unwrap().into()
}

pub fn re(name: &str, source: &str) -> Pattern {
    Regex::new(name, source).unwrap().into()
}

pub fn seq(name: &str, children: Vec<Pattern>) -> Pattern {
    Sequence::new(name, children).unwrap().into()
}

pub fn choice(name: &str, alternatives: Vec<Pattern>) -> Pattern {
    Choice::new(name, alternatives).unwrap().into()
}

pub fn opt(name: &str, child: Pattern) -> Pattern {
    Optional::new(name, child).into()
}

pub fn not(name: &str, child: Pattern) -> Pattern {
    Not::new(name, child).into()
}

pub fn cut() -> Pattern {
    Cut::default().into()
}
