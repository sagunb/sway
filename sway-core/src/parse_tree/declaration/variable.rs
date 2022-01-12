use crate::parse_tree::Expression;
use crate::Span;
use crate::{type_engine::TypeInfo, Ident};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclaration {
    pub name: Ident,
    pub type_ascription: TypeInfo,
    pub type_ascription_span: Option<Span>,
    pub body: Expression,
    pub is_mutable: bool,
}
