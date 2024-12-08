use crate::runner::{ArgType, Argument, ContentType, Operation, Rules, Validation};
use anyhow::Result;
use bstr::{BString, ByteSlice};
use regex::bytes::Regex;
use std::sync::Arc;

#[inline]
pub fn parse_int(text: &BString) -> Option<i64> {
    text.to_str().ok().and_then(|s| s.parse().ok())
}

#[inline]
pub fn parse_float(text: &BString) -> Option<f64> {
    text.to_str().ok().and_then(|s| s.parse().ok())
}

pub fn parse_args(args: &[Argument]) -> Result<Vec<Operation>> {
    let mut ops = Vec::new();

    for arg in args.iter() {
        ops.push(parse_arg(arg)?);
    }

    Ok(ops)
}

#[inline]
fn parse_arg(arg: &Argument) -> Result<Operation> {
    match &arg.arg_type {
        ArgType::Input => Ok(Operation::Input {
            rules: parse_input_arg(arg)?,
        }),
        ArgType::Output => Ok(Operation::Output {
            validation: parse_output_arg(arg)?,
        }),
    }
}

#[inline]
fn parse_input_arg(arg: &Argument) -> Result<Rules> {
    match &arg.content_type {
        ContentType::Empty => Ok(Rules::Empty),
        ContentType::Plain => {
            let text = arg.text.trim_end().to_owned();

            Ok(Rules::Plain(Arc::new(text)))
        }
        ContentType::Regex => Ok(Rules::Regex(Arc::new(
            regex_syntax::ParserBuilder::new()
                .ignore_whitespace(true)
                .unicode(false)
                .build()
                .parse(&arg.text)?,
        ))),
        ContentType::Int => Ok(Rules::Int(arg.min..=arg.max)),
    }
}

#[inline]
fn parse_output_arg(arg: &Argument) -> Result<Validation> {
    match &arg.content_type {
        ContentType::Empty => Ok(Validation::Empty),
        ContentType::Plain => {
            let text = arg.text.trim_end().to_owned();

            Ok(Validation::Plain(Arc::new(text)))
        }
        ContentType::Regex => Ok(Validation::Regex(Arc::new(Regex::new(&arg.text)?))),
        ContentType::Int => Ok(Validation::Int(arg.min..=arg.max)),
    }
}
