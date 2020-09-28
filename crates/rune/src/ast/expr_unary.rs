use crate::ast;
use crate::ast::expr::EagerBrace;
use crate::{ParseError, Parser, Spanned, ToTokens};
use std::fmt;

/// A unary expression.
///
/// # Examples
///
/// ```rust
/// use rune::{testing, ast};
///
/// testing::roundtrip::<ast::ExprUnary>("!0");
/// testing::roundtrip::<ast::ExprUnary>("*foo");
/// testing::roundtrip::<ast::ExprUnary>("&foo");
/// testing::roundtrip::<ast::ExprUnary>("&Foo {
///     a: 42,
/// }");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, ToTokens, Spanned)]
pub struct ExprUnary {
    /// Attributes associated with expression.
    #[rune(iter)]
    pub attributes: Vec<ast::Attribute>,
    /// Token associated with operator.
    pub op_token: ast::Token,
    /// The expression of the operation.
    pub expr: Box<ast::Expr>,
    /// The operation to apply.
    #[rune(skip)]
    pub op: UnaryOp,
}

impl ExprUnary {
    /// Parse the uniary expression with the given meta and configuration.
    pub(crate) fn parse_with_meta(
        parser: &mut Parser,
        attributes: Vec<ast::Attribute>,
        eager_brace: EagerBrace,
    ) -> Result<Self, ParseError> {
        let op_token = parser.token_next()?;
        let op = UnaryOp::from_token(op_token)?;

        Ok(Self {
            attributes,
            op_token,
            expr: Box::new(ast::Expr::parse_with(parser, eager_brace)?),
            op,
        })
    }
}

expr_parse!(ExprUnary, "try expression");

/// A unary operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Not `!<thing>`.
    Not,
    /// Reference `&<thing>`.
    BorrowRef,
    /// Dereference `*<thing>`.
    Deref,
}

impl UnaryOp {
    /// Convert a unary operator from a token.
    pub fn from_token(token: ast::Token) -> Result<Self, ParseError> {
        match token.kind {
            ast::Kind::Bang => Ok(Self::Not),
            ast::Kind::Amp => Ok(Self::BorrowRef),
            ast::Kind::Star => Ok(Self::Deref),
            _ => Err(ParseError::expected(token, "unary operator `!`")),
        }
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Not => write!(fmt, "!")?,
            Self::BorrowRef => write!(fmt, "&")?,
            Self::Deref => write!(fmt, "*")?,
        }

        Ok(())
    }
}
