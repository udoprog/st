use crate::{ast, Resolve, Storage};
use crate::{Parse, ParseError, ParseErrorKind, Parser, Peek, Spanned, ToTokens};
use runestick::Source;
use std::borrow::Cow;

/// A path, where each element is separated by a `::`.
#[derive(Debug, Clone, PartialEq, Eq, Parse, ToTokens, Spanned)]
pub struct Path {
    /// The optional leading colon `::`
    #[rune(iter)]
    pub leading_colon: Option<ast::Scope>,
    /// The first component in the path.
    pub first: PathSegment,
    /// The rest of the components in the path.
    #[rune(iter)]
    pub rest: Vec<(ast::Scope, PathSegment)>,
    /// Trailing scope.
    #[rune(iter)]
    pub trailing: Option<ast::Scope>,
}

impl Path {
    /// Borrow as an identifier used for field access calls.
    ///
    /// This is only allowed if there are no other path components
    /// and the PathSegment is not `Crate` or `Super`.
    pub fn try_as_ident(&self) -> Option<&ast::Ident> {
        if self.rest.is_empty() && self.trailing.is_none() && self.leading_colon.is_none() {
            self.first.try_as_ident()
        } else {
            None
        }
    }

    /// Borrow as an identifier used for field access calls.
    ///
    /// This is only allowed if there are no other path components
    /// and the PathSegment is not `Crate` or `Super`.
    pub fn try_as_ident_mut(&mut self) -> Option<&mut ast::Ident> {
        if self.rest.is_empty() && self.trailing.is_none() && self.leading_colon.is_none() {
            self.first.try_as_ident_mut()
        } else {
            None
        }
    }

    /// Iterate over all components in path.
    pub fn into_components(&self) -> impl Iterator<Item = &'_ PathSegment> + '_ {
        self.iter()
    }

    /// Iterate over the components of the path
    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item = &'a ast::PathSegment> {
        Some(&self.first)
            .into_iter()
            .chain(self.rest.iter().map(|(_, i)| i))
    }
}

impl Peek for Path {
    fn peek(t1: Option<ast::Token>, t2: Option<ast::Token>) -> bool {
        matches!(peek!(t1).kind, ast::Kind::ColonColon) || PathSegment::peek(t1, t2)
    }
}

/// A path, where each element is separated by a `::`.
#[derive(Debug, Clone, PartialEq, Eq, ToTokens, Spanned)]
pub enum PathSegment {
    /// A path segment that is an identifier.
    Ident(ast::Ident),
    /// The `crate` keyword used as a path segment.
    Crate(ast::Crate),
    /// The `super` keyword use as a path segment.
    Super(ast::Super),
    /// The `self` keyword used as a path segment: `self::foo`.
    SelfValue(ast::Self_),
    /// The `Self` keyword used as a path segment: `Self::Bar`.
    SelfType(ast::SelfType),
}

impl PathSegment {
    /// Borrow as an identifier.
    ///
    /// This is only allowed if the PathSegment is `Ident(_)`
    /// and not `Crate` or `Super`.
    pub fn try_as_ident(&self) -> Option<&ast::Ident> {
        if let PathSegment::Ident(ident) = self {
            Some(ident)
        } else {
            None
        }
    }

    /// Borrow as a mutable identifier.
    ///
    /// This is only allowed if the PathSegment is `Ident(_)`
    /// and not `Crate` or `Super`.
    pub fn try_as_ident_mut(&mut self) -> Option<&mut ast::Ident> {
        if let PathSegment::Ident(ident) = self {
            Some(ident)
        } else {
            None
        }
    }
}

impl Parse for PathSegment {
    fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let token = parser.token_peek_eof()?;
        match token.kind {
            ast::Kind::Ident(_) => Ok(PathSegment::Ident(parser.parse()?)),
            ast::Kind::Crate => Ok(PathSegment::Crate(parser.parse()?)),
            ast::Kind::Super => Ok(PathSegment::Super(parser.parse()?)),
            ast::Kind::Self_ => Ok(PathSegment::SelfValue(parser.parse()?)),
            ast::Kind::SelfType => Ok(PathSegment::SelfType(parser.parse()?)),
            _ => {
                return Err(ParseError::new(
                    token,
                    ParseErrorKind::TokenMismatch {
                        expected: ast::Kind::Ident(ast::StringSource::Text),
                        actual: token.kind,
                    },
                ))
            }
        }
    }
}

impl Peek for PathSegment {
    fn peek(t1: Option<ast::Token>, _t2: Option<ast::Token>) -> bool {
        matches!(peek!(t1).kind,
            ast::Kind::Ident(_)
            | ast::Kind::Crate
            | ast::Kind::Super
            | ast::Kind::SelfType)
    }
}

impl<'a> Resolve<'a> for PathSegment {
    type Output = Cow<'a, str>;

    fn resolve(&self, storage: &Storage, source: &'a Source) -> Result<Cow<'a, str>, ParseError> {
        match self {
            Self::Ident(ident) => ident.resolve(storage, source),
            Self::Crate(crate_) => ast::utils::resolve_text(crate_, source),
            Self::Super(super_) => ast::utils::resolve_text(super_, source),
            Self::SelfValue(self_) => ast::utils::resolve_text(self_, source),
            Self::SelfType(self_type) => ast::utils::resolve_text(self_type, source),
        }
    }
}
