use std::fmt::Display;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum RuleType {
    #[default]
    Input,
    Output,
}

impl Display for RuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "входное"),
            Self::Output => write!(f, "выходное"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ContentType {
    #[default]
    PlainText,
    Regex,
    IntRanges,
}

#[derive(Clone, Debug, Default)]
pub struct RuleData {
    pub name: String,
    pub rule_type: RuleType,
    pub content_type: ContentType,
    pub text: String,
}
