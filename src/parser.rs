use crate::{
    tokenizer::{Interpol as TokenInterpol, Span, Token},
    value::Value
};
use std::iter::Peekable;

#[derive(Clone, Debug, Fail, PartialEq)]
pub enum ParseError {
    #[fail(display = "unexpected eof")]
    UnexpectedEOF,
    #[fail(display = "expected {:?}, found {:?}", _0, _1)]
    Expected(Token, Option<Token>),
    #[fail(display = "unexpected token {:?} not applicable in this context", _0)]
    Unexpected(Token)
}

pub type Set = Vec<(String, AST)>;
pub type SetNoSpan = Vec<(String, ASTNoSpan)>;

#[derive(Clone, Debug, PartialEq)]
pub struct AST(Span, ASTType);

#[derive(Clone, Debug, PartialEq)]
pub enum Interpol {
    Literal(String),
    AST(AST)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ASTType {
    Set {
        recursive: bool,
        values: Set
    },
    LetIn(Set, Box<AST>),
    With(Box<(AST, AST)>),
    Import(Box<AST>),
    Var(String),
    Interpol(Vec<Interpol>),
    IndexSet(Box<AST>, String),

    // Could also do Add(Box<AST>, Box<AST>), but I believe this is more
    // efficient.
    Negate(Box<AST>),
    Add(Box<(AST, AST)>),
    Sub(Box<(AST, AST)>),
    Mul(Box<(AST, AST)>),
    Div(Box<(AST, AST)>),
    Value(Value)
}

#[derive(Clone, Debug, PartialEq)]
pub enum InterpolNoSpan {
    Literal(String),
    AST(ASTNoSpan)
}

impl From<Interpol> for InterpolNoSpan {
    fn from(interpol: Interpol) -> Self {
        match interpol {
            Interpol::Literal(text) => InterpolNoSpan::Literal(text),
            Interpol::AST(ast) => InterpolNoSpan::AST(ASTNoSpan::from(ast))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ASTNoSpan {
    Set {
        recursive: bool,
        values: SetNoSpan
    },
    LetIn(SetNoSpan, Box<ASTNoSpan>),
    With(Box<(ASTNoSpan, ASTNoSpan)>),
    Import(Box<ASTNoSpan>),
    Var(String),
    Interpol(Vec<InterpolNoSpan>),
    IndexSet(Box<ASTNoSpan>, String),

    Negate(Box<ASTNoSpan>),
    Add(Box<(ASTNoSpan, ASTNoSpan)>),
    Sub(Box<(ASTNoSpan, ASTNoSpan)>),
    Mul(Box<(ASTNoSpan, ASTNoSpan)>),
    Div(Box<(ASTNoSpan, ASTNoSpan)>),
    Value(Value)
}

fn set_discard_span(set: Set) -> SetNoSpan {
    set.into_iter()
        .map(|(name, ast)| (name, ASTNoSpan::from(ast)))
        .collect()
}
fn vec_into<F, T: From<F>>(vec: Vec<F>) -> Vec<T> {
    vec.into_iter()
        .map(|item| T::from(item))
        .collect()
}
fn discard_span(ast: Box<AST>) -> Box<ASTNoSpan> {
    Box::new((*ast).into())
}
fn tuple_discard_span(ast: Box<(AST, AST)>) -> Box<(ASTNoSpan, ASTNoSpan)> {
    Box::new((ast.0.into(), ast.1.into()))
}

impl From<AST> for ASTNoSpan {
    fn from(ast: AST) -> ASTNoSpan {
        match ast.1 {
            ASTType::Set { recursive, values } => ASTNoSpan::Set { recursive, values: set_discard_span(values) },
            ASTType::LetIn(set, ast) => ASTNoSpan::LetIn(set_discard_span(set), discard_span(ast)),
            ASTType::With(inner) => ASTNoSpan::With(tuple_discard_span(inner)),
            ASTType::Import(inner) => ASTNoSpan::Import(discard_span(inner)),
            ASTType::Var(inner) => ASTNoSpan::Var(inner),
            ASTType::Interpol(inner) => ASTNoSpan::Interpol(vec_into(inner)),
            ASTType::IndexSet(set, key) => ASTNoSpan::IndexSet(discard_span(set), key),

            ASTType::Negate(inner) => ASTNoSpan::Negate(discard_span(inner)),
            ASTType::Add(inner) => ASTNoSpan::Add(tuple_discard_span(inner)),
            ASTType::Sub(inner) => ASTNoSpan::Sub(tuple_discard_span(inner)),
            ASTType::Mul(inner) => ASTNoSpan::Mul(tuple_discard_span(inner)),
            ASTType::Div(inner) => ASTNoSpan::Div(tuple_discard_span(inner)),
            ASTType::Value(inner) => ASTNoSpan::Value(inner)
        }
    }
}

type Error = (Option<Span>, ParseError);
type Result<T> = std::result::Result<T, Error>;

macro_rules! math {
    ($self:expr, $next:block, $($token:pat => $ast:expr),*) => {{
        let mut val = { $next };
        loop {
            match $self.iter.peek() {
                $(Some(&(_, $token)) => {
                    $self.next()?;
                    let AST(end, expr) = { $next };
                    val = AST(val.0.until(end), $ast(Box::new((val, AST(end, expr)))));
                },)*
                _ => break
            }
        }
        Ok(val)
    }}
}

pub struct Parser<I>
    where I: Iterator<Item = (Span, Token)>
{
    iter: Peekable<I>
}
impl<I> Parser<I>
    where I: Iterator<Item = (Span, Token)>
{
    pub fn new(iter: Peekable<I>) -> Self {
        Self { iter }
    }

    pub fn peek(&mut self) -> Option<&Token> {
        self.iter.peek().map(|(_, token)| token)
    }
    pub fn next(&mut self) -> Result<(Span, Token)> {
        self.iter.next()
            .map(|entry| entry)
            .ok_or((None, ParseError::UnexpectedEOF))
    }
    pub fn expect(&mut self, expected: Token) -> Result<Span> {
        if let Some((span, actual)) = self.iter.next() {
            if actual == expected {
                Ok(span)
            } else {
                Err((Some(span), ParseError::Expected(expected, Some(actual))))
            }
        } else {
            Err((None, ParseError::Expected(expected, None)))
        }
    }
    pub fn expect_peek(&mut self, expected: Token) -> Result<Span> {
        if let Some(&(span, ref actual)) = self.iter.peek() {
            if *actual == expected {
                Ok(span)
            } else {
                Err((Some(span), ParseError::Expected(expected, Some(actual.clone()))))
            }
        } else {
            Err((None, ParseError::Expected(expected, None)))
        }
    }

    pub fn parse_val(&mut self) -> Result<AST> {
        let mut next = match self.next()? {
            (start, Token::Rec) => {
                self.expect_peek(Token::BracketOpen)?;
                let AST(end, mut set) = self.parse_val()?;
                if let ASTType::Set { ref mut recursive, .. } = set {
                    *recursive = true;
                }
                AST(start.until(end), set)
            },
            (start, Token::BracketOpen) => {
                let values = self.parse_set()?;
                let end = self.expect(Token::BracketClose)?;
                AST(start.until(end), ASTType::Set {
                    recursive: false,
                    values
                })
            },
            (start, Token::ParenOpen) => {
                let AST(_, expr) = self.parse_expr()?;
                let end = self.expect(Token::ParenClose)?;
                AST(start.until(end), expr)
            },
            (start, Token::Sub) => {
                let AST(end, expr) = self.parse_val()?;
                AST(start.until(end), ASTType::Negate(Box::new(AST(end, expr))))
            },
            (span, Token::Value(val)) => AST(span, ASTType::Value(val)),
            (span, Token::Ident(name)) => AST(span, ASTType::Var(name)),
            (span, Token::Interpol(values)) => {
                let mut parsed = Vec::new();
                for value in values {
                    parsed.push(match value {
                        TokenInterpol::Literal(text) => Interpol::Literal(text),
                        TokenInterpol::Tokens(tokens) => Interpol::AST(parse(tokens.into_iter())?)
                    });
                }
                AST(span, ASTType::Interpol(parsed))
            },
            (span, token) => return Err((Some(span), ParseError::Unexpected(token)))
        };

        while self.peek() == Some(&Token::Dot) {
            self.next()?;
            if let (end, Token::Ident(ident)) = self.next()? {
                next = AST(next.0.until(end), ASTType::IndexSet(Box::new(next), ident));
            }
        }

        Ok(next)
    }

    pub fn parse_mul(&mut self) -> Result<AST> {
        math!(
            self, { self.parse_val()? },
            Token::Mul => ASTType::Mul,
            Token::Div => ASTType::Div
        )
    }

    pub fn parse_add(&mut self) -> Result<AST> {
        math!(
            self, { self.parse_mul()? },
            Token::Add => ASTType::Add,
            Token::Sub => ASTType::Sub
        )
    }

    pub fn parse_set(&mut self) -> Result<Set> {
        let mut values = Vec::new();
        while let Some(&Token::Ident(_)) = self.peek() {
            let key = match self.next()? {
                (_, Token::Ident(name)) => name,
                _ => unreachable!()
            };
            self.expect(Token::Equal)?;
            let value = self.parse_expr()?;
            self.expect(Token::Semicolon)?;

            values.push((key, value));
        }
        Ok(values)
    }

    pub fn parse_expr(&mut self) -> Result<AST> {
        Ok(match self.peek() {
            Some(Token::Let) => {
                let (start, _) = self.next()?;
                let vars = self.parse_set()?;
                self.expect(Token::In)?;
                let AST(end, expr) = self.parse_expr()?;
                AST(start.until(end), ASTType::LetIn(vars, Box::new(AST(end, expr))))
            },
            Some(Token::With) => {
                let (start, _) = self.next()?;
                let vars = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                let AST(end, expr) = self.parse_expr()?;
                AST(start.until(end), ASTType::With(Box::new((vars, AST(end, expr)))))
            },
            Some(Token::Import) => {
                let (start, _) = self.next()?;
                let AST(end, expr) = self.parse_expr()?;
                AST(start.until(end), ASTType::Import(Box::new(AST(end, expr))))
            },
            _ => self.parse_add()?
        })
    }
}

pub fn parse<I>(iter: I) -> Result<AST>
    where I: IntoIterator<Item = (Span, Token)>
{
    Parser::new(iter.into_iter().peekable()).parse_expr()
}

#[cfg(test)]
mod tests {
    use crate::{
        tokenizer::{Interpol as TokenInterpol, Span, Token},
        value::{Anchor, Value}
    };
    use super::{AST as ASTSpan, ASTNoSpan as AST, ASTType, InterpolNoSpan as Interpol, ParseError};

    macro_rules! parse {
        ($($token:expr),*) => {
            super::parse(vec![$((Span::default(), $token)),*].into_iter())
                .map(AST::from)
        }
    }

    #[test]
    fn set() {
        assert_eq!(
            parse![
                Token::BracketOpen,

                Token::Ident("meaning_of_life".into()), Token::Equal, Token::Value(42.into()), Token::Semicolon,
                Token::Ident("H4X0RNUM83R".into()), Token::Equal, Token::Value(1.337.into()), Token::Semicolon,

                Token::BracketClose
            ],
            Ok(AST::Set {
                recursive: false,
                values: vec![
                    ("meaning_of_life".into(), AST::Value(42.into())),
                    ("H4X0RNUM83R".into(), AST::Value(1.337.into()))
                ]
            })
        );
        assert_eq!(
            parse![
                Token::Rec, Token::BracketOpen,
                Token::Ident("test".into()), Token::Equal, Token::Value(1.into()), Token::Semicolon,
                Token::BracketClose
            ],
            Ok(AST::Set {
                recursive: true,
                values: vec![("test".into(), AST::Value(1.into()))]
            })
        );
    }
    #[test]
    fn spans() {
        assert_eq!(
            super::parse(vec![
                (Span::default(), Token::BracketOpen),
                (Span { start: (4, 2), end: None }, Token::Semicolon),
            ].into_iter()),
            Err((
                Some(Span { start: (4, 2), end: None }),
                ParseError::Expected(Token::BracketClose, Some(Token::Semicolon))
            ))
        );
        assert_eq!(
            super::parse(vec![
                (Span { start: (0, 0), end: Some((0, 1)) }, Token::Value(1.into())),
                (Span { start: (0, 2), end: Some((0, 3)) }, Token::Add),
                (Span { start: (0, 4), end: Some((0, 5)) }, Token::Value(2.into())),
                (Span { start: (0, 6), end: Some((0, 7)) }, Token::Mul),
                (Span { start: (0, 8), end: Some((0, 9)) }, Token::Value(3.into())),
            ].into_iter()),
            Ok(ASTSpan(
                Span { start: (0, 0), end: Some((0, 9)) },
                ASTType::Add(Box::new((
                    ASTSpan(
                        Span { start: (0, 0), end: Some((0, 1)) },
                        ASTType::Value(1.into())
                    ),
                    ASTSpan(
                        Span { start: (0, 4), end: Some((0, 9)) },
                        ASTType::Mul(Box::new((
                            ASTSpan(
                                Span { start: (0, 4), end: Some((0, 5)) },
                                ASTType::Value(2.into())
                            ),
                            ASTSpan(
                                Span { start: (0, 8), end: Some((0, 9)) },
                                ASTType::Value(3.into())
                            )
                        )))
                    )
                )))
            ))
        );
    }
    #[test]
    fn math() {
        assert_eq!(
            parse![
                Token::Value(1.into()), Token::Add, Token::Value(2.into()), Token::Mul, Token::Value(3.into())
            ],
            Ok(AST::Add(Box::new((
                AST::Value(1.into()),
                AST::Mul(Box::new((
                    AST::Value(2.into()),
                    AST::Value(3.into()),
                )))
            ))))
        );
        assert_eq!(
            parse![
                Token::Value(5.into()), Token::Mul,
                Token::Sub, Token::ParenOpen,
                    Token::Value(3.into()), Token::Sub, Token::Value(2.into()),
                Token::ParenClose
            ],
            Ok(AST::Mul(Box::new((
                AST::Value(5.into()),
                AST::Negate(Box::new(AST::Sub(Box::new((
                    AST::Value(3.into()),
                    AST::Value(2.into()),
                )))))
            ))))
        );
    }
    #[test]
    fn let_in() {
        assert_eq!(
            parse![
                Token::Let,
                    Token::Ident("a".into()), Token::Equal, Token::Value(42.into()), Token::Semicolon,
                Token::In,
                    Token::Ident("a".into())
            ],
            Ok(AST::LetIn(
                vec![("a".into(), AST::Value(42.into()))],
                Box::new(AST::Var("a".into()))
            ))
        );
    }
    #[test]
    fn with() {
        assert_eq!(
            parse![
                Token::With, Token::Ident("namespace".into()), Token::Semicolon,
                Token::Ident("expr".into())
            ],
            Ok(AST::With(Box::new((
                AST::Var("namespace".into()),
                AST::Var("expr".into())
            ))))
        );
    }
    #[test]
    fn import() {
        assert_eq!(
            parse![
                Token::Import,
                Token::Value(Value::Path(Anchor::Store, "nixpkgs".into()))
            ],
            Ok(AST::Import(Box::new(
                AST::Value(Value::Path(Anchor::Store, "nixpkgs".into()))
            )))
        );
    }
    #[test]
    fn index_set() {
        assert_eq!(
            parse![Token::Ident("hello".into()), Token::Dot, Token::Ident("world".into())],
            Ok(AST::IndexSet(
                Box::new(AST::Var("hello".into())),
                "world".into()
            ))
        );
    }
    #[test]
    fn interpolation() {
        assert_eq!(
            parse![
                Token::Interpol(vec![
                    TokenInterpol::Literal("Hello, ".into()),
                    TokenInterpol::Tokens(vec![
                        (Span { start: (0, 12), end: Some((0, 13)) }, Token::BracketOpen),
                        (Span { start: (0, 14), end: Some((0, 19)) }, Token::Ident("world".into())),
                        (Span { start: (0, 20), end: Some((0, 21)) }, Token::Equal),
                        (Span { start: (0, 22), end: Some((0, 29)) }, Token::Value("World".into())),
                        (Span { start: (0, 29), end: Some((0, 30)) }, Token::Semicolon),
                        (Span { start: (0, 31), end: Some((0, 32)) }, Token::BracketClose),
                        (Span { start: (0, 32), end: Some((0, 33)) }, Token::Dot),
                        (Span { start: (0, 33), end: Some((0, 38)) }, Token::Ident("world".into()))
                    ]),
                    TokenInterpol::Literal("!".into())
                ])
            ],
            Ok(AST::Interpol(vec![
                Interpol::Literal("Hello, ".into()),
                Interpol::AST(AST::IndexSet(
                    Box::new(AST::Set {
                        recursive: false,
                        values: vec![("world".into(), AST::Value("World".into()))]
                    }),
                    "world".into()
                )),
                Interpol::Literal("!".into())
            ]))
        );
    }
}