use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub program_cmdl_args: Vec<String>,

    pub successes_required: u32,

    pub use_persistance: bool,
    pub args: Vec<Argument>,
}

#[derive(Clone, Debug)]
pub struct Argument {
    pub name: String,
    pub field: Field,
}

#[derive(Clone, Debug)]
pub enum Field {
    Input { kind: FieldKind, text: String },
    Output { kind: FieldKind, text: String },
}

#[derive(Clone, Debug)]
pub enum FieldKind {
    Plain,
    Regex,
}
