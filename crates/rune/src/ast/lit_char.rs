use crate::ast;
use crate::{Ast, Parse, ParseError, ParseErrorKind, Parser, Resolve, Spanned, Storage};
use runestick::Source;

/// A character literal.
#[derive(Debug, Clone, Ast, Spanned)]
pub struct LitChar {
    /// The token corresponding to the literal.
    pub token: ast::Token,
    /// The source of the literal character.
    #[ast(skip)]
    #[spanned(skip)]
    pub source: ast::CopySource<char>,
}

/// Parse a character literal.
///
/// # Examples
///
/// ```rust
/// use rune::{parse_all, ast};
///
/// parse_all::<ast::LitChar>("'a'").unwrap();
/// parse_all::<ast::LitChar>("'\\0'").unwrap();
/// parse_all::<ast::LitChar>("'\\n'").unwrap();
/// parse_all::<ast::LitChar>("'\\r'").unwrap();
/// parse_all::<ast::LitChar>("'\\''").unwrap();
/// ```
impl Parse for LitChar {
    fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let token = parser.token_next()?;

        match token.kind {
            ast::Kind::LitChar(source) => Ok(LitChar { token, source }),
            _ => Err(ParseError::new(
                token,
                ParseErrorKind::ExpectedChar { actual: token.kind },
            )),
        }
    }
}

impl<'a> Resolve<'a> for LitChar {
    type Output = char;

    fn resolve(&self, _: &Storage, source: &'a Source) -> Result<char, ParseError> {
        match self.source {
            ast::CopySource::Inline(c) => return Ok(c),
            ast::CopySource::Text => (),
        }

        let span = self.token.span();
        let string = source
            .source(span.narrow(1))
            .ok_or_else(|| ParseError::new(span, ParseErrorKind::BadSlice))?;
        let mut it = string
            .char_indices()
            .map(|(n, c)| (span.start + n, c))
            .peekable();

        let (n, c) = match it.next() {
            Some(c) => c,
            None => {
                return Err(ParseError::new(span, ParseErrorKind::BadCharLiteral));
            }
        };

        let c = match c {
            '\\' => ast::utils::parse_char_escape(
                span.with_start(n),
                &mut it,
                ast::utils::WithBrace(false),
            )?,
            c => c,
        };

        // Too many characters in literal.
        if it.next().is_some() {
            return Err(ParseError::new(span, ParseErrorKind::BadCharLiteral));
        }

        Ok(c)
    }
}
