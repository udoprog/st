use crate::ast;
use crate::{ParseError, ParseErrorKind};
use runestick::Span;
use std::collections::VecDeque;
use std::fmt;

/// Lexer for the rune language.
#[derive(Debug)]
pub struct Lexer<'a> {
    /// Source iterator.
    iter: SourceIter<'a>,
    /// Current lexer mode.
    modes: LexerModes,
    /// Buffered tokens.
    buffer: VecDeque<ast::Token>,
}

impl<'a> Lexer<'a> {
    /// Construct a new lexer over the given source.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rune::Lexer;
    /// use rune::ast;
    /// use runestick::Span;
    ///
    /// assert_eq! {
    ///     Lexer::new("fn").next().unwrap().unwrap(),
    ///     ast::Token {
    ///         kind: ast::Kind::Fn,
    ///         span: Span { start: 0, end: 2 },
    ///     }
    /// };
    ///
    /// assert_eq! {
    ///     Lexer::new("name").next().unwrap().unwrap(),
    ///     ast::Token {
    ///         kind: ast::Kind::Ident(ast::StringSource::Text),
    ///         span: Span { start: 0, end: 4 },
    ///     }
    /// };
    /// ```
    pub fn new(source: &'a str) -> Self {
        Self {
            iter: SourceIter::new(source),
            modes: LexerModes::default(),
            buffer: VecDeque::new(),
        }
    }

    /// Access the span of the lexer.
    pub fn span(&self) -> Span {
        self.iter.end_span(0)
    }

    fn next_ident(&mut self, start: usize) -> Result<Option<ast::Token>, ParseError> {
        while let Some(c) = self.iter.peek() {
            if !matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9') {
                break;
            }

            self.iter.next();
        }

        let (ident, span) = self.iter.source_from(start);
        let kind =
            ast::Kind::from_keyword(ident).unwrap_or(ast::Kind::Ident(ast::StringSource::Text));
        Ok(Some(ast::Token { kind, span }))
    }

    /// Consume a number literal.
    fn next_number_literal(
        &mut self,
        c: char,
        start: usize,
    ) -> Result<Option<ast::Token>, ParseError> {
        let base = if let ('0', Some(m)) = (c, self.iter.peek()) {
            // This loop is useful.
            #[allow(clippy::never_loop)]
            loop {
                let number = match m {
                    'x' => ast::NumberBase::Hex,
                    'b' => ast::NumberBase::Binary,
                    'o' => ast::NumberBase::Octal,
                    _ => break ast::NumberBase::Decimal,
                };

                self.iter.next();
                break number;
            }
        } else {
            ast::NumberBase::Decimal
        };

        let mut is_fractional = false;

        while let Some(c) = self.iter.peek() {
            match c {
                c if char::is_alphanumeric(c) => {
                    self.iter.next();
                }
                '.' if !is_fractional => {
                    self.iter.next();
                    is_fractional = true;

                    // char immediately following a dot should be numerical.
                    if !self.iter.peek().map(char::is_numeric).unwrap_or_default() {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(Some(ast::Token {
            kind: ast::Kind::LitNumber(ast::NumberSource::Text(ast::NumberSourceText {
                is_fractional,
                base,
            })),
            span: self.iter.span_from(start),
        }))
    }

    /// Consume a string literal.
    fn next_char_or_label(&mut self, start: usize) -> Result<Option<ast::Token>, ParseError> {
        let mut is_label = true;
        let mut char_count = 0;

        loop {
            let c = match self.iter.peek() {
                Some(c) => c,
                None => {
                    if is_label {
                        let span = self.iter.end_span(start);
                        return Err(ParseError::new(span, ParseErrorKind::ExpectedCharClose));
                    }

                    break;
                }
            };

            match c {
                '\\' => {
                    is_label = false;
                    self.iter.next();
                    self.iter.next();
                    char_count += 1;
                }
                '\'' => {
                    is_label = false;
                    self.iter.next();
                    break;
                }
                // components of labels.
                '0'..='9' | 'a'..='z' => {
                    self.iter.next();
                    char_count += 1;
                }
                c if c.is_control() => {
                    let span = self.iter.span_from(start);
                    return Err(ParseError::new(span, ParseErrorKind::UnterminatedCharLit));
                }
                _ if is_label && char_count > 0 => {
                    break;
                }
                _ => {
                    is_label = false;
                    self.iter.next();
                    char_count += 1;
                }
            }
        }

        if is_label {
            Ok(Some(ast::Token {
                kind: ast::Kind::Label(ast::StringSource::Text),
                span: self.iter.span_from(start),
            }))
        } else {
            Ok(Some(ast::Token {
                kind: ast::Kind::LitChar(ast::CopySource::Text),
                span: self.iter.span_from(start),
            }))
        }
    }

    /// Consume a string literal.
    fn next_lit_byte(&mut self, start: usize) -> Result<Option<ast::Token>, ParseError> {
        loop {
            let c = match self.iter.next() {
                Some(c) => c,
                None => {
                    return Err(ParseError::new(
                        self.iter.span_from(start),
                        ParseErrorKind::ExpectedByteClose,
                    ));
                }
            };

            match c {
                '\\' => {
                    self.iter.next();
                }
                '\'' => {
                    break;
                }
                c if c.is_control() => {
                    let span = self.iter.span_from(start);
                    return Err(ParseError::new(span, ParseErrorKind::UnterminatedByteLit));
                }
                _ => (),
            }
        }

        Ok(Some(ast::Token {
            kind: ast::Kind::LitByte(ast::CopySource::Text),
            span: self.iter.span_from(start),
        }))
    }

    /// Consume a string literal.
    fn next_str(
        &mut self,
        start: usize,
        error_kind: impl FnOnce() -> ParseErrorKind + Copy,
        kind: impl FnOnce(ast::LitStrSource) -> ast::Kind,
    ) -> Result<Option<ast::Token>, ParseError> {
        let mut escaped = false;

        loop {
            let c = self
                .iter
                .next()
                .ok_or_else(|| ParseError::new(self.iter.span_from(start), error_kind()))?;

            match c {
                '"' => break,
                '\\' => {
                    if self.iter.peek().is_none() {
                        return Err(ParseError::new(
                            self.iter.end_span(start),
                            ParseErrorKind::ExpectedEscape,
                        ));
                    } else {
                        escaped = true;
                    }
                }
                _ => (),
            }
        }

        Ok(Some(ast::Token {
            kind: kind(ast::LitStrSource::Text(ast::LitStrSourceText {
                escaped,
                wrapped: true,
            })),
            span: self.iter.span_from(start),
        }))
    }

    /// Consume the entire line.
    fn consume_line(&mut self) {
        while !matches!(self.iter.next(), Some('\n') | None) {}
    }

    fn template_next(&mut self) -> Result<(), ParseError> {
        use std::mem::take;

        let start = self.iter.pos();
        let mut escaped = false;

        while let Some(c) = self.iter.peek() {
            match c {
                '{' => {
                    let expressions = self.modes.expression_count(&self.iter, start)?;

                    let span = self.iter.span_from(start);
                    let had_string = start != self.iter.pos();
                    let start = self.iter.pos();
                    self.iter.next();

                    if had_string {
                        if *expressions > 0 {
                            self.buffer.push_back(ast::Token {
                                kind: ast::Kind::Comma,
                                span,
                            });
                        }

                        self.buffer.push_back(ast::Token {
                            kind: ast::Kind::LitStr(ast::LitStrSource::Text(
                                ast::LitStrSourceText {
                                    escaped: take(&mut escaped),
                                    wrapped: false,
                                },
                            )),
                            span,
                        });

                        *expressions += 1;
                    }

                    if *expressions > 0 {
                        self.buffer.push_back(ast::Token {
                            kind: ast::Kind::Comma,
                            span: self.iter.span_from(start),
                        });
                    }

                    self.modes.push(LexerMode::Default(1));
                    return Ok(());
                }
                '}' => {
                    let start = self.iter.pos();
                    self.iter.next();

                    return Err(ParseError::new(
                        self.iter.span_from(start),
                        ParseErrorKind::UnexpectedCloseBrace,
                    ));
                }
                '\\' => {
                    self.iter.next();

                    if self.iter.next().is_none() {
                        return Err(ParseError::new(
                            self.iter.end_span(start),
                            ParseErrorKind::ExpectedEscape,
                        ));
                    } else {
                        escaped = true;
                    }
                }
                '`' => {
                    let span = self.iter.span_from(start);
                    let had_string = start != self.iter.pos();
                    let start = self.iter.pos();
                    self.iter.next();

                    let expressions = self.modes.expression_count(&self.iter, start)?;

                    if had_string {
                        if *expressions > 0 {
                            self.buffer.push_back(ast::Token {
                                kind: ast::Kind::Comma,
                                span,
                            });
                        }

                        self.buffer.push_back(ast::Token {
                            kind: ast::Kind::LitStr(ast::LitStrSource::Text(
                                ast::LitStrSourceText {
                                    escaped: take(&mut escaped),
                                    wrapped: false,
                                },
                            )),
                            span,
                        });

                        *expressions += 1;
                    }

                    self.buffer.push_back(ast::Token {
                        kind: ast::Kind::Close(ast::Delimiter::Brace),
                        span: self.iter.span_from(start),
                    });

                    let expressions = *expressions;
                    self.modes
                        .pop(&self.iter, LexerMode::Template(expressions))?;

                    return Ok(());
                }
                _ => {
                    self.iter.next();
                }
            }
        }

        Err(ParseError::new(
            self.iter.point_span(),
            ParseErrorKind::UnexpectedEof,
        ))
    }

    /// Consume the next token from the lexer.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<ast::Token>, ParseError> {
        'outer: loop {
            if let Some(token) = self.buffer.pop_front() {
                return Ok(Some(token));
            }

            let mode = self.modes.last();

            let level = match mode {
                LexerMode::Template(..) => {
                    self.template_next()?;
                    continue;
                }
                LexerMode::Default(level) => (level),
            };

            let (start, c) = match self.iter.next_with_pos() {
                Some(next) => next,
                None => {
                    self.modes.pop(&self.iter, LexerMode::Default(0))?;
                    return Ok(None);
                }
            };

            if char::is_whitespace(c) {
                continue;
            }

            // This loop is useful, at least until it's rewritten.
            #[allow(clippy::never_loop)]
            let kind = loop {
                if let Some(c2) = self.iter.peek() {
                    match (c, c2) {
                        ('+', '=') => {
                            self.iter.next();
                            break ast::Kind::PlusEq;
                        }
                        ('-', '=') => {
                            self.iter.next();
                            break ast::Kind::DashEq;
                        }
                        ('*', '=') => {
                            self.iter.next();
                            break ast::Kind::StarEq;
                        }
                        ('/', '=') => {
                            self.iter.next();
                            break ast::Kind::SlashEq;
                        }
                        ('%', '=') => {
                            self.iter.next();
                            break ast::Kind::PercEq;
                        }
                        ('&', '=') => {
                            self.iter.next();
                            break ast::Kind::AmpEq;
                        }
                        ('^', '=') => {
                            self.iter.next();
                            break ast::Kind::CaretEq;
                        }
                        ('|', '=') => {
                            self.iter.next();
                            break ast::Kind::PipeEq;
                        }
                        ('/', '/') => {
                            self.consume_line();
                            continue 'outer;
                        }
                        (':', ':') => {
                            self.iter.next();
                            break ast::Kind::ColonColon;
                        }
                        ('<', '=') => {
                            self.iter.next();
                            break ast::Kind::LtEq;
                        }
                        ('>', '=') => {
                            self.iter.next();
                            break ast::Kind::GtEq;
                        }
                        ('=', '=') => {
                            self.iter.next();
                            break ast::Kind::EqEq;
                        }
                        ('!', '=') => {
                            self.iter.next();
                            break ast::Kind::BangEq;
                        }
                        ('&', '&') => {
                            self.iter.next();
                            break ast::Kind::AmpAmp;
                        }
                        ('|', '|') => {
                            self.iter.next();
                            break ast::Kind::PipePipe;
                        }
                        ('<', '<') => {
                            self.iter.next();

                            break if matches!(self.iter.peek(), Some('=')) {
                                self.iter.next();
                                ast::Kind::LtLtEq
                            } else {
                                ast::Kind::LtLt
                            };
                        }
                        ('>', '>') => {
                            self.iter.next();

                            break if matches!(self.iter.peek(), Some('=')) {
                                self.iter.next();
                                ast::Kind::GtGtEq
                            } else {
                                ast::Kind::GtGt
                            };
                        }
                        ('.', '.') => {
                            self.iter.next();
                            break ast::Kind::DotDot;
                        }
                        ('=', '>') => {
                            self.iter.next();
                            break ast::Kind::Rocket;
                        }
                        ('-', '>') => {
                            self.iter.next();
                            break ast::Kind::Arrow;
                        }
                        ('b', '\'') => {
                            self.iter.next();
                            self.iter.next();
                            return self.next_lit_byte(start);
                        }
                        ('b', '"') => {
                            self.iter.next();
                            return self.next_str(
                                start,
                                || ParseErrorKind::UnterminatedByteStrLit,
                                ast::Kind::LitByteStr,
                            );
                        }
                        _ => (),
                    }
                }

                break match c {
                    '(' => ast::Kind::Open(ast::Delimiter::Parenthesis),
                    ')' => ast::Kind::Close(ast::Delimiter::Parenthesis),
                    '{' => {
                        if level > 0 {
                            self.modes.push(LexerMode::Default(level + 1));
                        }

                        ast::Kind::Open(ast::Delimiter::Brace)
                    }
                    '}' => {
                        if level > 0 {
                            self.modes.pop(&self.iter, LexerMode::Default(level))?;

                            // NB: end of expression in template.
                            if level == 1 {
                                let expressions = self.modes.expression_count(&self.iter, start)?;
                                *expressions += 1;
                                continue 'outer;
                            }
                        }

                        ast::Kind::Close(ast::Delimiter::Brace)
                    }
                    '[' => ast::Kind::Open(ast::Delimiter::Bracket),
                    ']' => ast::Kind::Close(ast::Delimiter::Bracket),
                    '_' => ast::Kind::Underscore,
                    ',' => ast::Kind::Comma,
                    ':' => ast::Kind::Colon,
                    '#' => ast::Kind::Pound,
                    '.' => ast::Kind::Dot,
                    ';' => ast::Kind::SemiColon,
                    '=' => ast::Kind::Eq,
                    '+' => ast::Kind::Plus,
                    '-' => ast::Kind::Dash,
                    '/' => ast::Kind::Div,
                    '*' => ast::Kind::Star,
                    '&' => ast::Kind::Amp,
                    '>' => ast::Kind::Gt,
                    '<' => ast::Kind::Lt,
                    '!' => ast::Kind::Bang,
                    '?' => ast::Kind::QuestionMark,
                    '|' => ast::Kind::Pipe,
                    '%' => ast::Kind::Perc,
                    '^' => ast::Kind::Caret,
                    '@' => ast::Kind::At,
                    '$' => ast::Kind::Dollar,
                    '~' => ast::Kind::Tilde,
                    'a'..='z' | 'A'..='Z' => {
                        return self.next_ident(start);
                    }
                    '0'..='9' => {
                        return self.next_number_literal(c, start);
                    }
                    '"' => {
                        return self.next_str(
                            start,
                            || ParseErrorKind::UnterminatedStrLit,
                            ast::Kind::LitStr,
                        );
                    }
                    '`' => {
                        let span = self.iter.span_from(start);

                        self.buffer.push_back(ast::Token {
                            kind: ast::Kind::Template,
                            span,
                        });

                        self.buffer.push_back(ast::Token {
                            kind: ast::Kind::Open(ast::Delimiter::Brace),
                            span,
                        });

                        self.modes.push(LexerMode::Template(0));
                        continue 'outer;
                    }
                    '\'' => {
                        return self.next_char_or_label(start);
                    }
                    _ => {
                        let span = self.iter.end_span(start);
                        return Err(ParseError::new(span, ParseErrorKind::UnexpectedChar { c }));
                    }
                };
            };

            return Ok(Some(ast::Token {
                kind,
                span: self.iter.span_from(start),
            }));
        }
    }
}

#[derive(Debug, Clone)]
struct SourceIter<'a> {
    source: &'a str,
    chars: std::str::Chars<'a>,
}

impl<'a> SourceIter<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars(),
        }
    }

    /// Get the current character position of the iterator.
    fn pos(&self) -> usize {
        self.source.len() - self.chars.as_str().len()
    }

    /// Get the source from the given start, to the current position.
    fn source_from(&self, start: usize) -> (&'a str, Span) {
        let span = self.span_from(start);
        (&self.source[start..span.end], span)
    }

    /// Get the current point span.
    fn point_span(&self) -> Span {
        Span::point(self.pos())
    }

    /// Get the span from the given start, to the current position.
    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.pos())
    }

    /// Get the end span from the given start to the end of the source.
    fn end_span(&self, start: usize) -> Span {
        Span::new(start, self.source.len())
    }

    /// Peek the next cursor.
    fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Next with position.
    fn next_with_pos(&mut self) -> Option<(usize, char)> {
        let p = self.pos();
        let c = self.next()?;
        Some((p, c))
    }
}

impl Iterator for SourceIter<'_> {
    type Item = char;

    /// Consume the next character.
    fn next(&mut self) -> Option<Self::Item> {
        self.chars.next()
    }
}

struct WithCharIndex<'s, 'a> {
    iter: &'s mut SourceIter<'a>,
}

impl Iterator for WithCharIndex<'_, '_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        let pos = self.iter.pos();
        Some((pos, self.iter.next()?))
    }
}

#[derive(Debug, Default)]
struct LexerModes {
    modes: Vec<LexerMode>,
}

impl LexerModes {
    /// Get the last mode.
    fn last(&self) -> LexerMode {
        self.modes.last().copied().unwrap_or_default()
    }

    /// Push the given lexer mode.
    fn push(&mut self, mode: LexerMode) {
        self.modes.push(mode);
    }

    /// Pop the expected lexer mode.
    fn pop(&mut self, iter: &SourceIter<'_>, expected: LexerMode) -> Result<(), ParseError> {
        let mode = self.modes.pop().unwrap_or_default();

        if mode != expected {
            return Err(ParseError::new(
                iter.point_span(),
                ParseErrorKind::BadLexerMode { mode, expected },
            ));
        }

        Ok(())
    }

    /// Get the expression count.
    fn expression_count<'a>(
        &'a mut self,
        iter: &SourceIter<'_>,
        start: usize,
    ) -> Result<&'a mut usize, ParseError> {
        match self.modes.last_mut() {
            Some(LexerMode::Template(expression)) => Ok(expression),
            _ => {
                let span = iter.span_from(start);
                return Err(ParseError::new(span, ParseErrorKind::ExpectedTemplateMode));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexerMode {
    /// Default mode, boolean indicating if we are inside a template or not.
    Default(usize),
    /// We are parsing a template string.
    Template(usize),
}

impl Default for LexerMode {
    fn default() -> Self {
        Self::Default(0)
    }
}

impl fmt::Display for LexerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexerMode::Default(level) => {
                if *level > 0 {
                    write!(f, "default in template ({})", level)?;
                } else {
                    write!(f, "default")?;
                }
            }
            LexerMode::Template(expressions) => {
                write!(f, "template {}", expressions)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use crate::ast;
    use runestick::Span;

    macro_rules! span {
        ($start:expr, $end:expr) => {
            Span {
                start: $start,
                end: $end,
            }
        };
    }

    macro_rules! test_lexer {
        ($source:expr $(, $pat:pat)* $(,)?) => {{
            let mut it = Lexer::new($source);

            #[allow(never_used)]
            #[allow(unused_assignments)]
            {
                let mut n = 0;

                $(
                    match it.next().unwrap().expect("expected token") {
                        $pat => (),
                        #[allow(unreachable_patterns)]
                        other => {
                            panic!("\nGot bad token #{}.\nExpected: `{}`\nBut got: {:?}", n, stringify!($pat), other);
                        }
                    }

                    n += 1;
                )*
            }

            assert_eq!(it.next().unwrap(), None);
        }}
    }

    #[test]
    fn test_number_literals() {
        test_lexer! {
            "(10)",
            ast::Token {
                span: span!(0, 1),
                kind: ast::Kind::Open(ast::Delimiter::Parenthesis),
            },
            ast::Token {
                span: span!(1, 3),
                kind: ast::Kind::LitNumber(ast::NumberSource::Text(ast::NumberSourceText {
                    is_fractional: false,
                    base: ast::NumberBase::Decimal,
                })),
            },
            ast::Token {
                span: span!(3, 4),
                kind: ast::Kind::Close(ast::Delimiter::Parenthesis),
            },
        };

        test_lexer! {
            "(10.)",
            _,
            ast::Token {
                span: span!(1, 4),
                kind: ast::Kind::LitNumber(ast::NumberSource::Text(ast::NumberSourceText {
                    is_fractional: true,
                    base: ast::NumberBase::Decimal,
                })),
            },
            _,
        };
    }

    #[test]
    fn test_char_literal() {
        test_lexer! {
            "'a'",
            ast::Token {
                span: span!(0, 3),
                kind: ast::Kind::LitChar(ast::CopySource::Text),
            }
        };

        test_lexer! {
            "'\\u{abcd}'",
            ast::Token {
                span: span!(0, 10),
                kind: ast::Kind::LitChar(ast::CopySource::Text),
            }
        };
    }

    #[test]
    fn test_label() {
        test_lexer! {
            "'asdf 'a' \"foo bar\"",
            ast::Token {
                span: span!(0, 5),
                kind: ast::Kind::Label(ast::StringSource::Text),
            },
            ast::Token {
                span: span!(6, 9),
                kind: ast::Kind::LitChar(ast::CopySource::Text),
            },
            ast::Token {
                span: span!(10, 19),
                kind: ast::Kind::LitStr(ast::LitStrSource::Text(ast::LitStrSourceText { escaped: false, wrapped: true })),
            }
        };
    }

    #[test]
    fn test_operators() {
        test_lexer! {
            "+ += - -= * *= / /=",
            ast::Token {
                span: span!(0, 1),
                kind: ast::Kind::Plus,
            },
            ast::Token {
                span: span!(2, 4),
                kind: ast::Kind::PlusEq,
            },
            ast::Token {
                span: span!(5, 6),
                kind: ast::Kind::Dash,
            },
            ast::Token {
                span: span!(7, 9),
                kind: ast::Kind::DashEq,
            },
            ast::Token {
                span: span!(10, 11),
                kind: ast::Kind::Star,
            },
            ast::Token {
                span: span!(12, 14),
                kind: ast::Kind::StarEq,
            },
            ast::Token {
                span: span!(15, 16),
                kind: ast::Kind::Div,
            },
            ast::Token {
                span: span!(17, 19),
                kind: ast::Kind::SlashEq,
            }
        };
    }

    #[test]
    fn test_idents() {
        test_lexer! {
            "a.checked_div(10)",
            ast::Token {
                span: span!(0, 1),
                kind: ast::Kind::Ident(ast::StringSource::Text),
            },
            ast::Token {
                span: span!(1, 2),
                kind: ast::Kind::Dot,
            },
            ast::Token {
                span: span!(2, 13),
                kind: ast::Kind::Ident(ast::StringSource::Text),
            },
            ast::Token {
                span: span!(13, 14),
                kind: ast::Kind::Open(ast::Delimiter::Parenthesis),
            },
            ast::Token {
                span: span!(14, 16),
                kind: ast::Kind::LitNumber(ast::NumberSource::Text(ast::NumberSourceText {
                    is_fractional: false,
                    base: ast::NumberBase::Decimal,
                })),
            },
            ast::Token {
                span: span!(16, 17),
                kind: ast::Kind::Close(ast::Delimiter::Parenthesis),
            },
        };
    }

    #[test]
    fn test_template_literals() {
        test_lexer! {
            "`foo {bar} \\` baz`",
            ast::Token {
                kind: ast::Kind::Template,
                span: span!(0, 1),
            },
            ast::Token {
                kind: ast::Kind::Open(ast::Delimiter::Brace),
                span: span!(0, 1),
            },
            ast::Token {
                kind: ast::Kind::LitStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: false,
                    wrapped: false,
                })),
                span: span!(1, 5),
            },
            ast::Token {
                kind: ast::Kind::Comma,
                span: span!(5, 6),
            },
            ast::Token {
                kind: ast::Kind::Ident(ast::StringSource::Text),
                span: span!(6, 9),
            },
            ast::Token {
                kind: ast::Kind::Comma,
                span: span!(10, 17),
            },
            ast::Token {
                kind: ast::Kind::LitStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: true,
                    wrapped: false,
                })),
                span: span!(10, 17),
            },
            ast::Token {
                kind: ast::Kind::Close(ast::Delimiter::Brace),
                span: span!(17, 18),
            },
        };
    }

    #[test]
    fn test_template_literals_multi() {
        test_lexer! {
            "`foo {bar} {baz}`",
            ast::Token {
                kind: ast::Kind::Template,
                span: span!(0, 1),
            },
            ast::Token {
                kind: ast::Kind::Open(ast::Delimiter::Brace),
                span: span!(0, 1),
            },
            ast::Token {
                kind: ast::Kind::LitStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: false,
                    wrapped: false,
                })),
                span: span!(1, 5),
            },
            ast::Token {
                kind: ast::Kind::Comma,
                span: span!(5, 6),
            },
            ast::Token {
                kind: ast::Kind::Ident(ast::StringSource::Text),
                span: span!(6, 9),
            },
            ast::Token {
                kind: ast::Kind::Comma,
                span: span!(10, 11),
            },
            ast::Token {
                kind: ast::Kind::LitStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: false,
                    wrapped: false,
                })),
                span: span!(10, 11),
            },
            ast::Token {
                kind: ast::Kind::Comma,
                span: span!(11, 12),
            },
            ast::Token {
                kind: ast::Kind::Ident(ast::StringSource::Text),
                span: span!(12, 15),
            },
            ast::Token {
                kind: ast::Kind::Close(ast::Delimiter::Brace),
                span: span!(16, 17),
            },
        };
    }

    #[test]
    fn test_literals() {
        test_lexer! {
            r#"b"""#,
            ast::Token {
                span: span!(0, 3),
                kind: ast::Kind::LitByteStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: false,
                    wrapped: true,
                })),
            },
        };

        test_lexer! {
            r#"b"hello world""#,
            ast::Token {
                span: span!(0, 14),
                kind: ast::Kind::LitByteStr(ast::LitStrSource::Text(ast::LitStrSourceText {
                    escaped: false,
                    wrapped: true,
                })),
            },
        };

        test_lexer! {
            "b'\\\\''",
            ast::Token {
                span: span!(0, 6),
                kind: ast::Kind::LitByte(ast::CopySource::Text),
            },
        };

        test_lexer! {
            "'label 'a' b'a'",
            ast::Token {
                span: span!(0, 6),
                kind: ast::Kind::Label(ast::StringSource::Text),
            },
            ast::Token {
                span: span!(7, 10),
                kind: ast::Kind::LitChar(ast::CopySource::Text),
            },
            ast::Token {
                span: span!(11, 15),
                kind: ast::Kind::LitByte(ast::CopySource::Text),
            },
        };

        test_lexer! {
            "b'a'",
            ast::Token {
                span: span!(0, 4),
                kind: ast::Kind::LitByte(ast::CopySource::Text),
            },
        };

        test_lexer! {
            "b'\\n'",
            ast::Token {
                span: span!(0, 5),
                kind: ast::Kind::LitByte(ast::CopySource::Text),
            },
        };
    }
}
