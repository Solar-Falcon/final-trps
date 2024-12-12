use bstr::{BString, ByteVec};
use regex_syntax::hir::{Class, ClassBytes, ClassUnicode, Hir, HirKind};
use std::{ops::RangeInclusive, sync::Arc};

#[derive(Clone, Debug)]
pub enum Rules {
    Empty,
    Plain(Arc<String>),
    Regex(Arc<Hir>),
    Int(RangeInclusive<i64>),
}

impl Rules {
    pub fn generate(&self) -> Arc<BString> {
        match self {
            Rules::Empty => Arc::new(BString::default()),
            Rules::Plain(text) => Arc::new(BString::from(text.as_bytes())),
            Rules::Regex(hir) => {
                let mut result = BString::from("");
                generate_regex_item(hir).append_to(&mut result);

                Arc::new(result)
            }
            Rules::Int(range) => Arc::new(fastrand::i64(range.clone()).to_string().into()),
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
    fn append_to(&self, string: &mut BString) {
        match self {
            Self::Literal(lit) => {
                string.extend_from_slice(&lit[..]);
            }
            Self::ByteChoice(bytes) => {
                let choices: Vec<u8> = bytes
                    .iter()
                    .flat_map(|range| range.start()..=range.end())
                    .collect();

                if let Some(byte) = fastrand::choice(choices) {
                    string.push_byte(byte);
                }
            }
            Self::CharChoice(chars) => {
                let choices: Vec<char> = chars
                    .iter()
                    .flat_map(|range| range.start()..=range.end())
                    .collect();

                if let Some(ch) = fastrand::choice(choices) {
                    string.push_char(ch);
                }
            }
            Self::Repeat(item, range) => {
                for _i in 0..fastrand::u32(range.clone()) {
                    item.append_to(string);
                }
            }
            Self::Seq(seq) => {
                for item in seq.iter() {
                    item.append_to(string);
                }
            }
            Self::AnyOf(choices) => {
                if let Some(item) = fastrand::choice(choices) {
                    item.append_to(string);
                }
            }
        }
    }
}
