/// Associated metadata attached mostly to values.
///
/// Each value (instruction, function argument or constant) has associated metadata which helps
/// describe properties which aren't required for code generation, but help with other
/// introspective tools (e.g., the debugger) or compiler error messages.
use std::sync::Arc;

use sway_types::span::Span;

use crate::context::Context;

pub enum Metadatum {
    FileLocation(Arc<std::path::PathBuf>),
    SourceString(Arc<str>),
    Span {
        src_idx: MetadataIndex,
        start: usize,
        end: usize,
        file_idx: Option<MetadataIndex>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MetadataIndex(pub generational_arena::Index);

impl MetadataIndex {
    pub fn from_span(context: &mut Context, span: &Span) -> MetadataIndex {
        // Search for an existing matching path, otherwise insert it.
        let file_idx = span.path.as_ref().map(|path_buf| {
            context
                .metadata
                .iter()
                .find_map(|(idx, md)| match md {
                    Metadatum::FileLocation(file_loc_path_buf)
                        if Arc::ptr_eq(path_buf, file_loc_path_buf) =>
                    {
                        Some(MetadataIndex(idx))
                    }
                    _otherwise => None,
                })
                .unwrap_or_else(|| {
                    MetadataIndex(
                        context
                            .metadata
                            .insert(Metadatum::FileLocation(path_buf.clone())),
                    )
                })
        });

        // Search for an existing matching source string, otherwise insert it.
        let src_idx = MetadataIndex(
            context
                .metadata
                .iter()
                .find_map(|(idx, md)| match md {
                    Metadatum::SourceString(src_str) if Arc::ptr_eq(src_str, span.span.input()) => {
                        Some(idx)
                    }
                    _otherwise => None,
                })
                .unwrap_or_else(|| {
                    context
                        .metadata
                        .insert(Metadatum::SourceString(span.span.input().clone()))
                }),
        );

        MetadataIndex(context.metadata.insert(Metadatum::Span {
            src_idx,
            start: span.start(),
            end: span.end(),
            file_idx,
        }))
    }

    pub fn to_span(&self, context: &Context) -> Result<Span, String> {
        match &context.metadata[self.0] {
            Metadatum::Span {
                src_idx,
                start,
                end,
                file_idx,
            } => {
                let src_str = match &context.metadata[src_idx.0] {
                    Metadatum::SourceString(src_str) => Ok(src_str.clone()),
                    _otherwise => {
                        Err("Metadata cannot be converted to a source string.".to_owned())
                    }
                }?;
                let path = file_idx
                    .map(|idx| match &context.metadata[idx.0] {
                        Metadatum::FileLocation(path) => Ok(path.clone()),
                        _otherwise => {
                            Err("Metadata cannot be converted to a file location.".to_owned())
                        }
                    })
                    .transpose()?;
                Ok(Span {
                    span: pest::Span::new(src_str, *start, *end)
                        .ok_or_else(|| "Cannot create span from invalid metadata.".to_owned())?,
                    path,
                })
            }
            _otherwise => Err("Metadata cannot be converted to Span.".to_owned()),
        }
    }
}
