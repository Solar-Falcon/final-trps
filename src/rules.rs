use crate::worker_thread::OpReport;
use bstr::{BString, ByteSlice, ByteVec};
use rand::{
    rngs::ThreadRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use regex::bytes::{Regex, RegexBuilder};
use regex_syntax::hir::{Class, ClassBytes, ClassUnicode, Hir, HirKind};
use std::{fmt::Debug, ops::RangeInclusive};

pub trait Rule: Debug {
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn validate(&self, text: &BString) -> OpReport;
    fn generate(&self) -> anyhow::Result<BString>;
}

#[derive(Debug)]
pub struct PlainText {
    text: String,
}

impl PlainText {
    #[inline]
    fn failure_msg(&self) -> String {
        format!("Ожидаемый вывод: \"{}\"", self.text.escape_debug())
    }
}

impl Rule for PlainText {
    #[inline]
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            text: text.to_owned(),
        })
    }

    #[inline]
    fn generate(&self) -> anyhow::Result<BString> {
        Ok(BString::from(self.text.as_str()))
    }

    #[inline]
    fn validate(&self, text: &BString) -> OpReport {
        if self.text.as_bytes() == text.as_slice() {
            OpReport::Success
        } else {
            OpReport::Failure {
                error_message: self.failure_msg(),
            }
        }
    }
}

#[derive(Debug)]
pub struct RegExpr {
    regex: Regex,
    syntax: Hir,
}

impl RegExpr {
    #[inline]
    fn failure_msg(&self) -> String {
        format!(
            "Ожидалось соответствие вывода регулярному выражению\n{}",
            self.regex.as_str()
        )
    }

    fn generate_regex_item(hir: &Hir) -> anyhow::Result<Item> {
        match hir.kind() {
            HirKind::Empty => Ok(Item::Literal(BString::from(""))),
            HirKind::Literal(lit) => Ok(Item::Literal(lit.0.to_vec().into())),
            HirKind::Class(class) => Ok(match class {
                Class::Bytes(bytes) => Item::ByteChoice(bytes),
                Class::Unicode(unic) => Item::CharChoice(unic),
            }),
            HirKind::Repetition(rep) => {
                let item = Self::generate_regex_item(&rep.sub)?;
                let range = match (rep.min, rep.max) {
                    (0, None) => 0..=40, // the `*`
                    (1, None) => 1..=40, // the `+`
                    (min, None) => min..=min.saturating_mul(2),
                    (min, Some(max)) => min..=max,
                };

                Ok(Item::Repeat(Box::new(item), range))
            }
            HirKind::Capture(cap) => Self::generate_regex_item(&cap.sub),
            HirKind::Concat(cat) => Ok(Item::Seq(
                cat.iter()
                    .map(Self::generate_regex_item)
                    .collect::<anyhow::Result<_>>()?,
            )),
            HirKind::Alternation(alt) => Ok(Item::AnyOf(
                alt.iter()
                    .map(Self::generate_regex_item)
                    .collect::<anyhow::Result<_>>()?,
            )),
            HirKind::Look(look) => Err(anyhow::format_err!(
                "Данный элемент не поддерживается при генерации: {}",
                look.as_char()
            )),
        }
    }
}

impl Rule for RegExpr {
    #[inline]
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let unicode = true;
        let ignore_ws = false;
        let multi_line = false;

        let syntax = regex_syntax::ParserBuilder::new()
            .ignore_whitespace(ignore_ws)
            .multi_line(multi_line)
            .unicode(unicode)
            .build()
            .parse(text)?;

        let regex = RegexBuilder::new(text)
            .ignore_whitespace(ignore_ws)
            .multi_line(multi_line)
            .unicode(unicode)
            .build()?;

        Ok(Self { regex, syntax })
    }

    fn generate(&self) -> anyhow::Result<BString> {
        let mut rng = rand::thread_rng();
        let mut result = BString::from("");

        Self::generate_regex_item(&self.syntax)?.append_to(&mut result, &mut rng);

        Ok(result)
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
pub struct IntRanges {
    ranges: Vec<RangeInclusive<i64>>,
    orig_text: String,
}

impl IntRanges {
    #[inline]
    fn failure_msg(&self) -> String {
        format!(
            "Ожидалось попадание целого числа в интервалы:\n{}",
            &self.orig_text
        )
    }

    #[inline]
    fn parse_int(s: &str, line: &str) -> anyhow::Result<i64> {
        match s.parse() {
            Ok(num) => Ok(num),
            Err(err) => {
                let offset = s.as_ptr() as usize - line.as_ptr() as usize;

                Err(anyhow::Error::msg(format!(
                    "Ошибка при обработке диапазонов чисел: {}\n{}\n{}^",
                    err,
                    line,
                    "  ".repeat(offset),
                )))
            }
        }
    }
}

impl Rule for IntRanges {
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

                    if start > end {
                        anyhow::bail!("Ошибка при обработке диапазонов чисел: начало диапазона больше, чем конец ({}..{})", start, end);
                    }

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
                "Ошибка при обработке диапазонов чисел: текстовое поле пустое",
            ))
        }
    }

    fn generate(&self) -> anyhow::Result<BString> {
        let mut rng = rand::thread_rng();

        let range = self
            .ranges
            .choose_weighted(&mut rng, |range| {
                (range.end().wrapping_sub(*range.start()).unsigned_abs() as u128).saturating_add(1)
            })
            .unwrap();
        let num = range.clone().choose(&mut rng).unwrap();

        Ok(BString::new(num.to_string().into()))
    }

    fn validate(&self, text: &BString) -> OpReport {
        match text.to_str_lossy().parse() {
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

//===================================================================================//
//===================================// TESTING //===================================//
//===================================================================================//

#[cfg(test)]
mod test_int_parsing {
    use super::{IntRanges, Rule};
    use rand::Rng;
    use std::ops::RangeInclusive;

    fn check(input: &str, ranges: &[RangeInclusive<i64>]) -> anyhow::Result<()> {
        match IntRanges::parse(input) {
            Ok(int_ranges) => {
                if int_ranges.orig_text != input {
                    Err(anyhow::format_err!("text doesn't match"))
                } else if int_ranges.ranges != ranges {
                    Err(anyhow::format_err!("ranges don't match"))
                } else {
                    Ok(())
                }
            }
            Err(error) => Err(error),
        }
    }

    #[inline]
    fn ok(input: &str, ranges: &[RangeInclusive<i64>]) {
        match check(input, ranges) {
            Ok(()) => {}
            Err(err) => panic!("{err}"),
        }
    }

    #[inline]
    fn err(input: &str) {
        assert!(check(input, &[]).is_err());
    }

    #[test]
    fn empty() {
        err("");
    }

    #[test]
    fn single_comma() {
        err(",");
    }

    #[test]
    fn some_text() {
        err("sungoua9180_");
    }

    #[test]
    fn single_number() {
        ok("100", &[100..=100]);
    }

    #[test]
    fn single_number_with_text() {
        err("100nan");
    }

    #[test]
    fn single_number_negative() {
        ok("-190583", &[-190583..=-190583]);
    }

    #[test]
    fn single_number_zero() {
        ok("0", &[0..=0]);
    }

    #[test]
    fn single_number_trailing_comma() {
        err("14391539,");
    }

    #[test]
    fn single_number_negative_separated_minus() {
        err("- 1");
    }

    #[test]
    fn single_number_with_spaces() {
        ok("   \t\t 10190309\t\t\t", &[10190309..=10190309]);
    }

    #[test]
    fn single_range() {
        ok("-123..-0", &[-123..=0]);
    }

    #[test]
    fn single_range_end_less_than_start() {
        err("123159..-9148");
    }

    #[test]
    fn single_range_trailing_comma() {
        err("149..150,");
    }

    #[test]
    fn single_range_half() {
        err("140..");
    }

    #[test]
    fn single_range_other_half() {
        err("..149");
    }

    #[test]
    fn single_range_with_spaces() {
        ok("  \t 149 \t\t..\t\t150                ", &[149..=150]);
    }

    #[test]
    fn multi_number() {
        ok(
            " 194 , 99     ,-150,   11037    ",
            &[194..=194, 99..=99, -150..=-150, 11037..=11037],
        );
    }

    #[test]
    fn multi_number_double_comma() {
        err("1,,2");
    }

    #[test]
    fn multi_number_with_text() {
        err(" 194, 99, -150, 11037, what is this?");
    }

    #[test]
    fn multi_ranges() {
        ok(
            "  -111111111 .. -1 ,-100..1000,1   ..   1,   -0..0",
            &[-111111111..=-1, -100..=1000, 1..=1, 0..=0],
        );
    }

    #[test]
    fn multi_mix() {
        ok(
            " 99 , -111 .. -1 ,1   ..   1,   -0..0   ,-150    ",
            &[99..=99, -111..=-1, 1..=1, 0..=0, -150..=-150],
        );
    }

    #[test]
    fn proptest() {
        let mut rng = rand::thread_rng();

        let len: usize = rng.gen_range(1..100);
        let mut ranges = Vec::with_capacity(len);

        for _i in 0..len {
            let start: i64 = rng.gen();
            let end: i64 = rng.gen_range(start..=i64::MAX);

            ranges.push(start..=end);
        }

        let input = ranges
            .iter()
            .map(|range| {
                let start = range.start();
                let end = range.end();

                let sep = " ".repeat(rng.gen_range(0..10));

                format!("{sep}{start}{sep}..{sep}{end}{sep}")
            })
            .collect::<Vec<_>>()
            .join(",");

        ok(&input, &ranges);
    }
}

#[cfg(test)]
mod test_int_gen {
    use crate::worker_thread::OpReport;
    use rand::Rng;
    use super::{IntRanges, Rule};

    #[test]
    fn proptest() {
        let mut rng = rand::thread_rng();

        let len: usize = rng.gen_range(1..100);
        let mut ranges = Vec::with_capacity(len);

        for _i in 0..len {
            let start: i64 = rng.gen();
            let end: i64 = rng.gen_range(start..=i64::MAX);

            ranges.push(start..=end);
        }

        let ranges = IntRanges {
            ranges,
            orig_text: String::new(),
        };

        for _i in 0..1000 {
            let n = ranges.generate().unwrap();
            assert_eq!(ranges.validate(&n), OpReport::Success);
        }
    }
}

#[cfg(test)]
mod test_regex_generation {
    use super::{RegExpr, Rule};
    use crate::worker_thread::OpReport;

    fn check(input: &str) {
        let regex = RegExpr::parse(input).unwrap();

        let generated = regex.generate().unwrap();

        match regex.validate(&generated) {
            OpReport::Success => {}
            OpReport::Failure { error_message } => panic!("{error_message} : {generated}"),
        }
    }

    fn check_invalid(input: &str) {
        let regex = RegExpr::parse(input).unwrap();

        assert!(regex.generate().is_err());
    }

    #[test]
    fn simple_regex() {
        check(r"\w+");
    }

    #[test]
    fn slightly_complex_regex() {
        check(r"[0-9]{0,15} \s*\S*888[\w\(\)]?")
    }

    #[test]
    fn regex_without_anchors() {
        check(r"\W+I!?(\\\d)* @/\[\p{Greek}  ");
    }

    #[test]
    fn regex_long() {
        check(
            r"(2020(-03)+)-+:34:([A-Z0-9]+(\.[a-z0-9]+)+) +~!@#\$%\^&*()=+_`\-\|\/'\[\]\{\}]|[?.,]*\w q#([A-Za-z]+( [A-Za-z]+)+) '[^']*'\.",
        );
    }

    #[test]
    fn regex_complex() {
        check_invalid(r".*[(0-9A-Xa-mz)&&[^MNO]]{10,20} ;\b(\P{Greek}|\d)+");
    }
}
