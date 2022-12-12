use crate::expr;
use crate::scanner;

use std::fmt;

#[derive(Default)]
struct Parser {
    tokens: Vec<scanner::Token>,
    current: usize,
    in_fundec: bool, // in rust, booleans default to false: https://doc.rust-lang.org/std/primitive.bool.html#impl-Default
}

pub enum Error {
    Unअपेक्षितToken(scanner::Token),
    TokenMismatch {
        अपेक्षित: scanner::TokenType,
        found: scanner::Token,
        maybe_on_err_string: Option<String>,
    },
    MaxParamsExceeded {
        kind: FunctionKind,
        line: usize,
        col: i64,
    },
    ReturnNotInFun {
        line: usize,
        col: i64,
    },
    InvalidAssignment {
        line: usize,
        col: i64,
    },
    TooManyArguments {
        line: usize,
        col: i64,
    },
    अपेक्षितExpression {
        token_type: scanner::TokenType,
        line: usize,
        col: i64,
    },
    InvalidTokenInUnaryOp {
        token_type: scanner::TokenType,
        line: usize,
        col: i64,
    },
    InvalidTokenInBinaryOp {
        token_type: scanner::TokenType,
        line: usize,
        col: i64,
    },
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Error::Unअपेक्षितToken(tok) => write!(
                f,
                "Unअपेक्षित token {:?} at line={},col={}",
                tok.ty, tok.line, tok.col
            ),
            Error::TokenMismatch {
                maybe_on_err_string,
                अपेक्षित,
                found,
            } => {
                write!(
                    f,
                    " अपेक्षित टोकन {:?} तर भेट्टियो {:?}.  रेखा={},स्नम्भ={}",
                    अपेक्षित, found.ty, found.line, found.col
                )?;
                if let Some(on_err_string) = maybe_on_err_string {
                    write!(f, ": {}", on_err_string)?;
                }
                fmt::Result::Ok(())
            }
            Error::MaxParamsExceeded { kind, line, col } => write!(
                f,
                "{:?} घोषणा(function) मा २५५ भन्दा बढी प्यारामिटरहरू हुन सक्दैन। रेखा={}, स्तम्भ={}",
                kind, line, col
            ),
            Error::ReturnNotInFun { line, col } => write!(
                f,
                "रिटर्न स्टेटमेन्ट लाइन={}, स्नम्भ={} मा कार्य (function) मा संलग्न छैन",
                line, col
            ),
            Error::InvalidAssignment { line, col } => {
                write!(f, "लाइन={}, स्नम्भ={} मा अवैध असाइनमेन्ट लक्ष्य", line, col)
            }
            Error::TooManyArguments { line, col } => write!(
                f,
                "घोषणा(function) मा २५५ भन्दा बढी प्यारामिटरहरू हुन सक्दैन। रेखा={}, स्तम्भ={}",
                line, col
            ),
            Error::अपेक्षितExpression {
                token_type,
                line,
                col,
            } => write!(
                f,
                "अपेक्षित अभिव्यक्ति(अपेक्षित expression), तर लाइन={}, col={} मा टोकन(Token) {:?} भेटियो",
                 line, col, token_type
            ),
            Error::InvalidTokenInUnaryOp {
                token_type,
                line,
                col,
            } => write!(
                f,
                "अवैध टोकन in unary op {:?} at लाइन={}, स्नम्भ={}",
                token_type, line, col
            ),
            Error::InvalidTokenInBinaryOp {
                token_type,
                line,
                col,
            } => write!(
                f,
                "अवैध टोकन in binary op {:?} at लाइन={}, स्नम्भ={}",
                token_type, line, col
            ),
        }
    }
}

#[derive(Debug)]
pub enum FunctionKind {
    Function,
    Method,
}

pub fn parse(
    tokens: Vec<scanner::Token>,
) -> Result<Vec<expr::Stmt>, Error> {
    let mut p = Parser {
        tokens,
        ..Default::default()
    };
    let stmts_or_err = p.parse();

    match stmts_or_err {
        Ok(stmts_or_err) => {
            if !p.is_at_end() {
                let tok = &p.tokens[p.current];
                Err(Error::Unअपेक्षितToken(tok.clone()))
            } else {
                Ok(stmts_or_err)
            }
        }
        Err(err) => Err(err),
    }
}

impl Parser {
    pub fn parse(&mut self) -> Result<Vec<expr::Stmt>, Error> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            let stmt = self.declaration()?;
            statements.push(stmt);
        }

        Ok(statements)
    }

    fn declaration(&mut self) -> Result<expr::Stmt, Error> {
        if self.matches(scanner::TokenType::Var) {
            return self.var_decl();
        }

        if self.matches(scanner::TokenType::Fun) {
            return Ok(expr::Stmt::FunDecl(self.fun_decl(FunctionKind::Function)?));
        }

        if self.matches(scanner::TokenType::Class) {
            return self.class_decl();
        }

        self.statement()
    }

    fn class_decl(&mut self) -> Result<expr::Stmt, Error> {
        let name_tok = self
            .consume(scanner::TokenType::Identifier, "अपेक्षित class name")?
            .clone();

        let class_symbol = expr::Symbol {
            name: name_tok.lexeme.iter().collect(),
            line: name_tok.line,
            col: name_tok.col,
        };

        let superclass_maybe = if self.matches(scanner::TokenType::Less) {
            let superclass_tok =
                self.consume(scanner::TokenType::Identifier, "अपेक्षित class  नाम (name) .")?;
            Some(expr::Symbol {
                name: superclass_tok.lexeme.iter().collect(),
                line: superclass_tok.line,
                col: superclass_tok.col,
            })
        } else {
            None
        };

        self.consume(scanner::TokenType::LeftBrace, "अपेक्षित {  पछि  class name")?;

        let mut methods = Vec::new();
        while !self.check(scanner::TokenType::RightBrace) && !self.is_at_end() {
            methods.push(self.fun_decl(FunctionKind::Method)?);
        }
        let methods = methods;

        self.consume(
            scanner::TokenType::RightBrace,
            "अपेक्षित }  पछि  class body",
        )?;

        Ok(expr::Stmt::ClassDecl(expr::ClassDecl {
            name: class_symbol,
            superclass: superclass_maybe,
            methods,
        }))
    }

    fn fun_decl(&mut self, kind: FunctionKind) -> Result<expr::FunDecl, Error> {
        let name_tok = self
            .consume(
                scanner::TokenType::Identifier,
                format!("अपेक्षित {:?} name", kind).as_ref(),
            )?
            .clone();

        let fun_symbol = expr::Symbol {
            name: name_tok.lexeme.iter().collect(),
            line: name_tok.line,
            col: name_tok.col,
        };

        let (parameters, body) = self.params_and_body(kind)?;

        Ok(expr::FunDecl {
            name: fun_symbol,
            params: parameters,
            body,
        })
    }

    fn params_and_body(
        &mut self,
        kind: FunctionKind,
    ) -> Result<(Vec<expr::Symbol>, Vec<expr::Stmt>), Error> {
        self.consume(
            scanner::TokenType::LeftParen,
            format!("अपेक्षित (  पछि  {:?} name", kind).as_ref(),
        )?;

        let mut parameters = Vec::new();

        if !self.check(scanner::TokenType::RightParen) {
            loop {
                if parameters.len() >= 255 {
                    let peek_tok = self.peek();
                    return Err(Error::MaxParamsExceeded {
                        kind,
                        line: peek_tok.line,
                        col: peek_tok.col,
                    });
                }

                let tok = self
                    .consume(scanner::TokenType::Identifier, "अपेक्षित  प्यारामीटर(parameter) name")?
                    .clone();

                parameters.push(expr::Symbol {
                    name: tok.lexeme.iter().collect(),
                    line: tok.line,
                    col: tok.col,
                });

                if !self.matches(scanner::TokenType::Comma) {
                    break;
                }
            }
        }
        let parameters = parameters;

        self.consume(
            scanner::TokenType::RightParen,
            "parameter",
        )?;
        self.consume(
            scanner::TokenType::LeftBrace,
            "अपेक्षित { before function body",
        )?;
        let saved_is_in_fundec = self.in_fundec;
        self.in_fundec = true;
        let body = self.block()?;
        self.in_fundec = saved_is_in_fundec;

        Ok((parameters, body))
    }

    fn var_decl(&mut self) -> Result<expr::Stmt, Error> {
        let name_token = self
            .consume(scanner::TokenType::Identifier, "अपेक्षित  भेरिएबल (variable)  name")?
            .clone();

        let maybe_initializer = if self.matches(scanner::TokenType::Equal) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            scanner::TokenType::Semicolon,
            "अपेक्षित ;  पछि   भेरिएबल (variable)  declaration",
        )?;

        Ok(expr::Stmt::VarDecl(
            expr::Symbol {
                name: name_token.lexeme.iter().collect(),
                line: name_token.line,
                col: name_token.col,
            },
            maybe_initializer,
        ))
    }

    fn statement(&mut self) -> Result<expr::Stmt, Error> {
        if self.matches(scanner::TokenType::Print) {
            return self.print_statement();
        }

        if self.matches(scanner::TokenType::While) {
            return self.while_statement();
        }

        if self.matches(scanner::TokenType::LeftBrace) {
            return Ok(expr::Stmt::Block(self.block()?));
        }

        if self.matches(scanner::TokenType::For) {
            return self.for_statement();
        }

        if self.matches(scanner::TokenType::If) {
            return self.if_statement();
        }

        if self.matches(scanner::TokenType::Return) {
            return self.return_statement();
        }

        self.expression_statement()
    }

    fn return_statement(&mut self) -> Result<expr::Stmt, Error> {
        let prev_tok = self.previous().clone();

        if !self.in_fundec {
            return Err(Error::ReturnNotInFun {
                line: prev_tok.line,
                col: prev_tok.col,
            });
        }

        let maybe_retval = if !self.matches(scanner::TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        if maybe_retval.is_some() {
            self.consume(
                scanner::TokenType::Semicolon,
                "अपेक्षित ;  पछि   फर्कने (return)   भ्यालु (value)",
            )?;
        }

        Ok(expr::Stmt::Return(
            expr::SourceLocation {
                line: prev_tok.line,
                col: prev_tok.col,
            },
            maybe_retval,
        ))
    }

    fn for_statement(&mut self) -> Result<expr::Stmt, Error> {
        self.consume(scanner::TokenType::LeftParen, "अपेक्षित (  पछि  for.")?;

        let mut maybe_initializer: Option<expr::Stmt> = None;
        if self.matches(scanner::TokenType::Semicolon) {
        } else if self.matches(scanner::TokenType::Var) {
            maybe_initializer = Some(self.var_decl()?)
        } else {
            maybe_initializer = Some(self.expression_statement()?)
        }
        let maybe_initializer = maybe_initializer;

        let mut maybe_condition: Option<expr::Expr> = None;
        if !self.check(scanner::TokenType::Semicolon) {
            maybe_condition = Some(self.expression()?)
        }
        let maybe_condition = maybe_condition;

        self.consume(
            scanner::TokenType::Semicolon,
            "अपेक्षित ;  पछि  loop condition",
        )?;

        let maybe_increment = if !self.check(scanner::TokenType::RightParen) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            scanner::TokenType::RightParen,
            "अपेक्षित )  पछि  for clauses",
        )?;

        let mut body = self.statement()?;

        if let Some(increment) = maybe_increment {
            body = expr::Stmt::Block(vec![body, expr::Stmt::Expr(increment)])
        }

        let condition = match maybe_condition {
            Some(cond) => cond,
            None => expr::Expr::Literal(expr::Literal::True),
        };
        body = expr::Stmt::While(condition, Box::new(body));

        if let Some(initializer) = maybe_initializer {
            body = expr::Stmt::Block(vec![initializer, body])
        }
        let body = body;

        Ok(body)
    }

    fn while_statement(&mut self) -> Result<expr::Stmt, Error> {
        self.consume(scanner::TokenType::LeftParen, "अपेक्षित (  पछि  while")?;
        let cond = self.expression()?;
        self.consume(
            scanner::TokenType::RightParen,
            "अपेक्षित )  पछि  while condition",
        )?;
        let body = Box::new(self.statement()?);
        Ok(expr::Stmt::While(cond, body))
    }

    fn if_statement(&mut self) -> Result<expr::Stmt, Error> {
        self.consume(scanner::TokenType::LeftParen, "अपेक्षित (  पछि  if.")?;
        let cond = self.expression()?;
        self.consume(
            scanner::TokenType::RightParen,
            "अपेक्षित )  पछि  if condition.",
        )?;
        let then_branch = Box::new(self.statement()?);
        let maybe_else_branch = if self.matches(scanner::TokenType::Else) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Ok(expr::Stmt::If(cond, then_branch, maybe_else_branch))
    }

    fn block(&mut self) -> Result<Vec<expr::Stmt>, Error> {
        let mut stmts = Vec::new();

        while !self.check(scanner::TokenType::RightBrace) && !self.is_at_end() {
            stmts.push(self.declaration()?)
        }

        self.consume(scanner::TokenType::RightBrace, "अपेक्षित }  पछि  block.")?;

        Ok(stmts)
    }

    fn print_statement(&mut self) -> Result<expr::Stmt, Error> {
        let expr = self.expression()?;
        self.consume(scanner::TokenType::Semicolon, "अपेक्षित ;  पछि  भ्यालु (value)")?;
        Ok(expr::Stmt::Print(expr))
    }

    fn expression_statement(&mut self) -> Result<expr::Stmt, Error> {
        let expr = self.expression()?;
        self.consume(scanner::TokenType::Semicolon, "अपेक्षित ;  पछि  भ्यालु (value)")?;
        Ok(expr::Stmt::Expr(expr))
    }

    fn expression(&mut self) -> Result<expr::Expr, Error> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<expr::Expr, Error> {
        let expr = self.or()?;

        if self.matches(scanner::TokenType::Equal) {
            let equals = self.previous().clone();
            let new_value = self.assignment()?;

            if let expr::Expr::Variable(sym) = &expr {
                return Ok(expr::Expr::Assign(sym.clone(), Box::new(new_value)));
            } else if let expr::Expr::Get(e, attr) = expr {
                return Ok(expr::Expr::Set(e, attr, Box::new(new_value)));
            } else {
                return Err(Error::InvalidAssignment {
                    line: equals.line,
                    col: equals.col,
                });
            }
        }

        Ok(expr)
    }

    fn or(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.and()?;

        while self.matches(scanner::TokenType::Or) {
            let right = self.and()?;
            expr = expr::Expr::Logical(Box::new(expr), expr::LogicalOp::Or, Box::new(right));
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.equality()?;

        while self.matches(scanner::TokenType::And) {
            let right = self.equality()?;
            expr = expr::Expr::Logical(Box::new(expr), expr::LogicalOp::And, Box::new(right));
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.addition()?;

        while self.match_one_of(vec![
            scanner::TokenType::Greater,
            scanner::TokenType::GreaterEqual,
            scanner::TokenType::Less,
            scanner::TokenType::LessEqual,
        ]) {
            let operator_token = self.previous().clone();
            let right = Box::new(self.addition()?);
            let binop_maybe = Parser::op_token_to_binop(&operator_token);

            match binop_maybe {
                Ok(binop) => {
                    let left = Box::new(expr);
                    expr = expr::Expr::Binary(left, binop, right);
                }
                Err(err) => return Err(err),
            }
        }
        Ok(expr)
    }

    fn addition(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.multiplication()?;

        while self.match_one_of(vec![scanner::TokenType::Minus, scanner::TokenType::Plus]) {
            let operator_token = self.previous().clone();
            let right = Box::new(self.multiplication()?);
            let binop_maybe = Parser::op_token_to_binop(&operator_token);

            match binop_maybe {
                Ok(binop) => {
                    let left = Box::new(expr);
                    expr = expr::Expr::Binary(left, binop, right);
                }
                Err(err) => return Err(err),
            }
        }
        Ok(expr)
    }

    fn multiplication(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.unary()?;

        while self.match_one_of(vec![scanner::TokenType::Slash, scanner::TokenType::Star]) {
            let operator_token = self.previous().clone();
            let right = Box::new(self.unary()?);
            let binop_maybe = Parser::op_token_to_binop(&operator_token);

            match binop_maybe {
                Ok(binop) => {
                    let left = Box::new(expr);
                    expr = expr::Expr::Binary(left, binop, right);
                }
                Err(err) => return Err(err),
            }
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<expr::Expr, Error> {
        if self.match_one_of(vec![scanner::TokenType::Bang, scanner::TokenType::Minus]) {
            let operator_token = self.previous().clone();
            let right = Box::new(self.unary()?);
            let unary_op_maybe = Parser::op_token_to_unary_op(&operator_token);

            return match unary_op_maybe {
                Ok(unary_op) => Ok(expr::Expr::Unary(unary_op, right)),
                Err(err) => Err(err),
            };
        }
        self.call()
    }

    fn call(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.primary()?;

        loop {
            if self.matches(scanner::TokenType::LeftParen) {
                expr = self.finish_call(expr)?;
            } else if self.matches(scanner::TokenType::Dot) {
                let name_tok = self
                    .consume(
                        scanner::TokenType::Identifier,
                        "अपेक्षित  गुड (property)   नाम (name)   पछि  '.'.",
                    )?
                    .clone();
                expr = expr::Expr::Get(
                    Box::new(expr),
                    expr::Symbol {
                        name: name_tok.lexeme.iter().collect(),
                        line: name_tok.line,
                        col: name_tok.col,
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn finish_call(&mut self, callee: expr::Expr) -> Result<expr::Expr, Error> {
        let mut arguments = Vec::new();

        if !self.check(scanner::TokenType::RightParen) {
            loop {
                if arguments.len() >= 255 {
                    let peek_tok = self.peek();
                    return Err(Error::TooManyArguments {
                        line: peek_tok.line,
                        col: peek_tok.col,
                    });
                }
                arguments.push(self.expression()?);
                if !self.matches(scanner::TokenType::Comma) {
                    break;
                }
            }
        }

        let token = self.consume(
            scanner::TokenType::RightParen,
            "अपेक्षित )  पछि  arguments.",
        )?;

        Ok(expr::Expr::Call(
            Box::new(callee),
            expr::SourceLocation {
                line: token.line,
                col: token.col,
            },
            arguments,
        ))
    }

    fn primary(&mut self) -> Result<expr::Expr, Error> {
        if self.matches(scanner::TokenType::False) {
            return Ok(expr::Expr::Literal(expr::Literal::False));
        }
        if self.matches(scanner::TokenType::True) {
            return Ok(expr::Expr::Literal(expr::Literal::True));
        }
        if self.matches(scanner::TokenType::Nil) {
            return Ok(expr::Expr::Literal(expr::Literal::Nil));
        }
        if self.matches(scanner::TokenType::Super) {
            let super_tok = self.previous().clone();
            self.consume(scanner::TokenType::Dot, "अपेक्षित '.'  पछि  'super'.")?;
            let method_tok = self.consume(
                scanner::TokenType::Identifier,
                "अपेक्षित superclass method  नाम (name) .",
            )?;
            return Ok(expr::Expr::Super(
                expr::SourceLocation {
                    line: super_tok.line,
                    col: super_tok.col,
                },
                expr::Symbol {
                    name: method_tok.lexeme.iter().collect(),
                    line: method_tok.line,
                    col: method_tok.col,
                },
            ));
        }
        if self.matches(scanner::TokenType::Number) {
            match &self.previous().literal {
                Some(scanner::Literal::Number(n)) => {
                    return Ok(expr::Expr::Literal(expr::Literal::Number(*n)))
                }
                Some(l) => panic!(
                    "internal error in parser: when parsing number, found literal {:?}",
                    l
                ),
                None => panic!("internal error in parser: when parsing number, found no literal"),
            }
        }
        if self.matches(scanner::TokenType::String) {
            match &self.previous().literal {
                Some(scanner::Literal::Str(s)) => {
                    return Ok(expr::Expr::Literal(expr::Literal::String(s.clone())))
                }
                Some(l) => panic!(
                    "internal error in parser: when parsing string, found literal {:?}",
                    l
                ),
                None => panic!("internal error in parser: when parsing string, found no literal"),
            }
        }
        if self.matches(scanner::TokenType::This) {
            let prev = self.previous();
            return Ok(expr::Expr::This(expr::SourceLocation {
                line: prev.line,
                col: prev.col,
            }));
        }
        if self.matches(scanner::TokenType::Identifier) {
            match &self.previous().literal {
                Some(scanner::Literal::Identifier(s)) => {
                    return Ok(expr::Expr::Variable(expr::Symbol {
                        name: s.clone(),
                        line: self.previous().line,
                        col: self.previous().col,
                    }))
                }
                Some(l) => panic!(
                    "internal error in parser: when parsing identifier, found literal {:?}",
                    l
                ),
                None => {
                    panic!("internal error in parser: when parsing identifier, found no literal")
                }
            }
        }
        if self.matches(scanner::TokenType::LeftParen) {
            let expr = Box::new(self.expression()?);
            self.consume(
                scanner::TokenType::RightParen,
                "अपेक्षित ')'  पछि  expression.",
            )?;
            return Ok(expr::Expr::Grouping(expr));
        }

        Err(Error::अपेक्षितExpression {
            token_type: self.peek().ty,
            line: self.peek().line,
            col: self.peek().col,
        })
    }

    fn consume(
        &mut self,
        tok: scanner::TokenType,
        on_err_str: &str,
    ) -> Result<&scanner::Token, Error> {
        if self.check(tok) {
            return Ok(self.advance());
        }
        Err(Error::TokenMismatch {
            अपेक्षित: tok,
            found: self.peek().clone(),
            maybe_on_err_string: Some(on_err_str.into()),
        })
    }

    fn op_token_to_unary_op(tok: &scanner::Token) -> Result<expr::UnaryOp, Error> {
        match tok.ty {
            scanner::TokenType::Minus => Ok(expr::UnaryOp {
                ty: expr::UnaryOpTy::Minus,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Bang => Ok(expr::UnaryOp {
                ty: expr::UnaryOpTy::Bang,
                line: tok.line,
                col: tok.col,
            }),
            _ => Err(Error::InvalidTokenInUnaryOp {
                token_type: tok.ty,
                line: tok.line,
                col: tok.col,
            }),
        }
    }

    fn equality(&mut self) -> Result<expr::Expr, Error> {
        let mut expr = self.comparison()?;

        while self.match_one_of(vec![
            scanner::TokenType::BangEqual,
            scanner::TokenType::EqualEqual,
        ]) {
            let operator_token = self.previous().clone();
            let right = Box::new(self.comparison()?);

            let binop_maybe = Parser::op_token_to_binop(&operator_token);

            match binop_maybe {
                Ok(binop) => {
                    let left = Box::new(expr);
                    expr = expr::Expr::Binary(left, binop, right);
                }
                Err(err) => return Err(err),
            }
        }
        Ok(expr)
    }

    fn op_token_to_binop(tok: &scanner::Token) -> Result<expr::BinaryOp, Error> {
        match tok.ty {
            scanner::TokenType::EqualEqual => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::EqualEqual,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::BangEqual => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::NotEqual,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Less => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Less,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::LessEqual => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::LessEqual,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Greater => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Greater,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::GreaterEqual => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::GreaterEqual,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Plus => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Plus,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Minus => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Minus,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Star => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Star,
                line: tok.line,
                col: tok.col,
            }),
            scanner::TokenType::Slash => Ok(expr::BinaryOp {
                ty: expr::BinaryOpTy::Slash,
                line: tok.line,
                col: tok.col,
            }),
            _ => Err(Error::InvalidTokenInBinaryOp {
                token_type: tok.ty,
                line: tok.line,
                col: tok.col,
            }),
        }
    }

    fn match_one_of(&mut self, types: Vec<scanner::TokenType>) -> bool {
        for ty in types.iter() {
            if self.matches(*ty) {
                return true;
            }
        }
        false
    }

    fn matches(&mut self, ty: scanner::TokenType) -> bool {
        if self.check(ty) {
            self.advance();
            return true;
        }
        false
    }

    fn check(&self, ty: scanner::TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }

        self.peek().ty == ty
    }

    fn advance(&mut self) -> &scanner::Token {
        if !self.is_at_end() {
            self.current += 1
        }

        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().ty == scanner::TokenType::Eof
    }

    fn peek(&self) -> &scanner::Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &scanner::Token {
        &self.tokens[self.current - 1]
    }
}
