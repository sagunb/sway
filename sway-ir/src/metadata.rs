use std::collections::HashMap;

#[derive(Default)]
pub struct Metadata {
    metadata: HashMap<Metadatum, u64>,
}

pub enum Metadatum {
    FileLocation(std::path::PathBuf),
    StringLocation(String),
    Span {
        loc_token: u64,
        start: u64,
        end: u64,
    },
}
