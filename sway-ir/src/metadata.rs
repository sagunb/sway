/// Associated metadata attached mostly to values.
///
/// Each value (instruction, function argument or constant) has associated metadata which helps
/// describe properties which aren't required for code generation, but help with other
/// introspective tools (e.g., the debugger) or compiler error messages.

use sway_types::span::Span;

use crate::context::Context;

pub enum Metadatum {
    FileLocation(std::path::PathBuf),
    StringLocation(String),
    Span {
        loc_token: u64,
        start: u64,
        end: u64,
    },
}

pub type MetadataIndex = generational_arena::Index;

impl Metadatum {
    pub fn from_span(_context: &mut Context, _span: &Span) -> MetadataIndex {
        todo!()
    }
}
