use crate::runner::Rules;
use bstr::BString;
use std::sync::Arc;

pub fn generate(rules: &Rules) -> Arc<BString> {
    match rules {
        Rules::Empty => Arc::new(BString::default()),
        Rules::Plain(text) => Arc::new(BString::from(text.as_bytes())),
        Rules::Regex(hir) => {
            todo!()
        }
    }
}
