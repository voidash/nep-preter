#[derive(Debug)]
pub enum Source {
    Literal,
    File(String),
}

#[derive(Debug)]
pub struct Input {
    pub source: Source,
    pub content: String,
}
