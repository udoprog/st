use crate::ast;
use crate::{Ast, Parse, ParseError, Parser, Spanned};
use runestick::Span;

/// A let expression `let <name> = <expr>;`
#[derive(Debug, Clone, Ast)]
pub struct ExprWhile {
    /// A label for the while loop.
    pub label: Option<(ast::Label, ast::Colon)>,
    /// The `while` keyword.
    pub while_: ast::While,
    /// The name of the binding.
    pub condition: ast::Condition,
    /// The body of the while loop.
    pub body: Box<ast::ExprBlock>,
}

impl ExprWhile {
    /// Parse with the given label.
    pub fn parse_with_label(
        parser: &mut Parser<'_>,
        label: Option<(ast::Label, ast::Colon)>,
    ) -> Result<Self, ParseError> {
        Ok(ExprWhile {
            label,
            while_: parser.parse()?,
            condition: parser.parse()?,
            body: Box::new(parser.parse()?),
        })
    }
}

impl Spanned for ExprWhile {
    fn span(&self) -> Span {
        self.while_.span().join(self.body.span())
    }
}

impl Parse for ExprWhile {
    fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let label = parser.parse()?;
        Self::parse_with_label(parser, label)
    }
}
