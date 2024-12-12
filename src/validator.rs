use bstr::BString;
use regex::bytes::Regex;
use std::{fmt::Display, ops::RangeInclusive, sync::Arc};

use crate::parser::parse_int;

#[derive(Clone, Debug)]
pub enum Validation {
    Empty,
    Plain(Arc<String>),
    Regex(Arc<Regex>),
    Int(RangeInclusive<i64>),
}

impl Display for Validation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "пустая строка"),
            Self::Plain(s) => write!(f, "строка \"{}\"", s.escape_debug()),
            Self::Regex(r) => write!(f, "соответствие регулярному выражению\n{}", r.as_str()),
            Self::Int(range) => write!(
                f,
                "целое в диапазоне от {} до {} включительно",
                range.start(),
                range.end()
            ),
        }
    }
}

impl Validation {
    #[inline]
    pub fn validate(&self, text: &BString) -> bool {
        match self {
            Self::Empty => text.is_empty(),
            Self::Plain(correct) => text == correct.as_bytes(),
            Self::Regex(regex) => regex.is_match(text.as_slice()),
            Self::Int(range) => parse_int(text)
                .map(|val| range.contains(&val))
                .unwrap_or(false),
        }
    }
}
