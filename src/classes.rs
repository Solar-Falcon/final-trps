use crate::runner::{ArgumentTrait, OpReport};
use bstr::{BString, ByteSlice, ByteVec};
use rand::{
    rngs::ThreadRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use regex::bytes::Regex;
use regex_syntax::hir::{Class, ClassBytes, ClassUnicode, Hir, HirKind};
use std::ops::RangeInclusive;

#[derive(Debug)]
pub struct PlainTextArg(String);

impl PlainTextArg {
    #[inline]
    fn failure_msg(&self) -> String {
        format!("Ожидаемый вывод: \"{}\"", self.0.escape_debug())
    }
}

impl ArgumentTrait for PlainTextArg {
    #[inline]
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self(text.to_owned()))
    }

    #[inline]
    fn generate(&self) -> BString {
        BString::from(self.0.as_str())
    }

    #[inline]
    fn validate(&self, text: &BString) -> OpReport {
        if self.0.as_bytes() == text.as_slice() {
            OpReport::Success
        } else {
            OpReport::Failure {
                error_message: self.failure_msg(),
            }
        }
    }
}

#[derive(Debug)]
pub struct RegexArg {
    regex: Regex,
    syntax: Hir,
}

impl RegexArg {
    #[inline]
    fn failure_msg(&self) -> String {
        format!(
            "Ожидалось соответствие вывода регулярному выражению\n{}",
            self.regex.as_str()
        )
    }
}

impl ArgumentTrait for RegexArg {
    #[inline]
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let syntax = regex_syntax::ParserBuilder::new()
            .ignore_whitespace(true)
            .build()
            .parse(text)?;

        let regex = Regex::new(text)?;

        Ok(Self { regex, syntax })
    }

    fn generate(&self) -> BString {
        let mut rng = rand::thread_rng();
        let mut result = BString::from("");

        generate_regex_item(&self.syntax).append_to(&mut result, &mut rng);

        result
    }

    #[inline]
    fn validate(&self, text: &BString) -> OpReport {
        if self.regex.is_match(text.as_slice()) {
            OpReport::Success
        } else {
            OpReport::Failure {
                error_message: self.failure_msg(),
            }
        }
    }
}

fn generate_regex_item(hir: &Hir) -> Item {
    match hir.kind() {
        HirKind::Empty => Item::Literal(BString::from("")),
        HirKind::Literal(lit) => Item::Literal(lit.0.to_vec().into()),
        HirKind::Class(class) => match class {
            Class::Bytes(bytes) => Item::ByteChoice(bytes),
            Class::Unicode(unic) => Item::CharChoice(unic),
        },
        HirKind::Repetition(rep) => {
            let item = generate_regex_item(&rep.sub);
            let range = match (rep.min, rep.max) {
                (0, None) => 0..=40, // the `*`
                (1, None) => 1..=40, // the `+`
                (min, None) => min..=min.saturating_mul(2),
                (min, Some(max)) => min..=max,
            };

            Item::Repeat(Box::new(item), range)
        }
        HirKind::Capture(cap) => generate_regex_item(&cap.sub),
        HirKind::Concat(cat) => Item::Seq(cat.iter().map(generate_regex_item).collect()),
        HirKind::Alternation(alt) => Item::AnyOf(alt.iter().map(generate_regex_item).collect()),
        HirKind::Look(_) => {
            eprintln!("Warning: anchors and boundaries in input regexes are useless");
            Item::Literal(BString::from(""))
        }
    }
}

#[derive(Debug)]
enum Item<'a> {
    Literal(BString),
    ByteChoice(&'a ClassBytes),
    CharChoice(&'a ClassUnicode),
    Repeat(Box<Item<'a>>, RangeInclusive<u32>),
    Seq(Vec<Item<'a>>),
    AnyOf(Vec<Item<'a>>),
}

impl Item<'_> {
    fn append_to(&self, string: &mut BString, rng: &mut ThreadRng) {
        match self {
            Self::Literal(lit) => {
                string.extend_from_slice(&lit[..]);
            }
            Self::ByteChoice(bytes) => {
                if let Some(byte) = bytes
                    .iter()
                    .flat_map(|range| range.start()..=range.end())
                    .choose(rng)
                {
                    string.push_byte(byte);
                }
            }
            Self::CharChoice(chars) => {
                if let Some(ch) = chars
                    .iter()
                    .flat_map(|range| range.start()..=range.end())
                    .choose(rng)
                {
                    string.push_char(ch);
                }
            }
            Self::Repeat(item, range) => {
                for _i in 0..rng.gen_range(range.clone()) {
                    item.append_to(string, rng);
                }
            }
            Self::Seq(seq) => {
                for item in seq.iter() {
                    item.append_to(string, rng);
                }
            }
            Self::AnyOf(choices) => {
                if let Some(item) = choices.choose(rng) {
                    item.append_to(string, rng);
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct IntRangesArg {
    ranges: Vec<RangeInclusive<i128>>,
    orig_text: String,
}

impl IntRangesArg {
    #[inline]
    fn failure_msg(&self) -> String {
        format!(
            "Ожидалось попадание целого числа в интервалы:\n{}",
            &self.orig_text
        )
    }

    #[inline]
    fn parse_error(line: &str, offset: usize, msg: &str) -> String {
        format!(
            "Ошибка при парсинге диапазонов чисел: {}\n{}\n{}^",
            msg,
            line,
            " ".repeat(offset),
        )
    }

    #[inline]
    fn parse_int(s: &str, line: &str) -> anyhow::Result<i128> {
        match s.parse() {
            Ok(num) => Ok(num),
            Err(err) => {
                let offset = s.as_ptr() as usize - line.as_ptr() as usize;

                Err(anyhow::Error::msg(Self::parse_error(line, offset, &err.to_string())))
            }
        }
    }
}

impl ArgumentTrait for IntRangesArg {
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let mut ranges = Vec::new();

        for line in text.lines() {
            for elem in line.split(',').map(str::trim) {
                if let Some((start, end)) = elem.split_once("..") {
                    let start = Self::parse_int(start.trim(), line)?;
                    let end = Self::parse_int(end.trim(), line)?;

                    ranges.push(start..=end);
                } else {
                    let num = Self::parse_int(elem.trim(), line)?;

                    ranges.push(num..=num);
                }
            }
        }

        if !ranges.is_empty() {
            Ok(Self {
                ranges,
                orig_text: text.to_owned(),
            })
        } else {
            Err(anyhow::Error::msg(
                "Ошибка при парсинге диапазонов чисел: текстовое поле пустое",
            ))
        }
    }

    fn generate(&self) -> BString {
        let mut rng = rand::thread_rng();

        let num = self
            .ranges
            .iter()
            .cloned()
            .flatten()
            .choose(&mut rng)
            .unwrap();

        BString::new(num.to_string().into())
    }

    fn validate(&self, text: &BString) -> OpReport {
        match parse_int(text) {
            Ok(num) => {
                if self.ranges.iter().any(|range| range.contains(&num)) {
                    OpReport::Success
                } else {
                    OpReport::Failure {
                        error_message: self.failure_msg(),
                    }
                }
            }
            Err(err) => OpReport::Failure {
                error_message: format!(
                    "Ожидалось целое число (ошибка преобразования к числу: {})",
                    err
                ),
            },
        }
    }
}

#[inline]
fn parse_int(text: &BString) -> anyhow::Result<i128> {
    Ok(text.to_str_lossy().parse()?)
}
