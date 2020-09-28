use crate::ast;
use crate::{Spanned, ToTokens};

/// An index get operation `<target>[<index>]`.
#[derive(Debug, Clone, PartialEq, Eq, ToTokens, Spanned)]
pub struct ExprIndex {
    /// Attributes associated with expression.
    #[rune(iter)]
    pub attributes: Vec<ast::Attribute>,
    /// The target of the index set.
    pub target: Box<ast::Expr>,
    /// The opening bracket.
    pub open: ast::OpenBracket,
    /// The indexing expression.
    pub index: Box<ast::Expr>,
    /// The closening bracket.
    pub close: ast::CloseBracket,
}

expr_parse!(ExprIndex, "index expression");
