use super::lexer::{LogicalOp, Operator, Token};

#[derive(Debug, Clone)]
pub enum Expr {
    Comparison {
        field: Vec<String>,
        operator: Operator,
        value: Value,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<Value>),
}

pub struct QueryParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl QueryParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Expr, String> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and_expr()?;

        while self.match_token(&Token::LogicalOp(LogicalOp::Or)) {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not_expr()?;

        while self.match_token(&Token::LogicalOp(LogicalOp::And)) {
            self.advance();
            let right = self.parse_not_expr()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<Expr, String> {
        if self.match_token(&Token::LogicalOp(LogicalOp::Not)) {
            self.advance();
            let expr = self.parse_primary()?;
            Ok(Expr::Not(Box::new(expr)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        if self.match_token(&Token::LParen) {
            self.advance();
            let expr = self.parse_or_expr()?;
            if !self.match_token(&Token::RParen) {
                return Err("Expected closing parenthesis ')'".to_string());
            }
            self.advance();
            Ok(expr)
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let field = self.parse_field_path()?;

        let operator = self.parse_operator()?;

        let value = self.parse_value()?;

        Ok(Expr::Comparison {
            field,
            operator,
            value,
        })
    }

    fn parse_field_path(&mut self) -> Result<Vec<String>, String> {
        let mut path = Vec::new();

        if let Some(Token::Identifier(name)) = self.current_token() {
            path.push(name.clone());
            self.advance();
        } else {
            return Err(format!(
                "Expected identifier, got {:?}",
                self.current_token()
            ));
        }

        while self.match_token(&Token::Dot) {
            self.advance();
            if let Some(Token::Identifier(name)) = self.current_token() {
                path.push(name.clone());
                self.advance();
            } else {
                return Err(format!(
                    "Expected identifier after '.', got {:?}",
                    self.current_token()
                ));
            }
        }

        Ok(path)
    }

    fn parse_operator(&mut self) -> Result<Operator, String> {
        if let Some(Token::Operator(op)) = self.current_token() {
            let operator = op.clone();
            self.advance();
            Ok(operator)
        } else {
            Err(format!("Expected operator, got {:?}", self.current_token()))
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.current_token() {
            Some(Token::String(s)) => {
                let value = Value::String(s.clone());
                self.advance();
                Ok(value)
            }
            Some(Token::Number(n)) => {
                let value = Value::Number(*n);
                self.advance();
                Ok(value)
            }
            Some(Token::Boolean(b)) => {
                let value = Value::Boolean(*b);
                self.advance();
                Ok(value)
            }
            Some(Token::LBracket) => self.parse_array(),
            _ => Err(format!("Expected value, got {:?}", self.current_token())),
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        if !self.match_token(&Token::LBracket) {
            return Err("Expected '['".to_string());
        }
        self.advance();

        let mut elements = Vec::new();

        if !self.match_token(&Token::RBracket) {
            loop {
                let value = self.parse_value()?;
                elements.push(value);

                if self.match_token(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !self.match_token(&Token::RBracket) {
            return Err("Expected ']'".to_string());
        }
        self.advance();

        Ok(Value::Array(elements))
    }

    fn current_token(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn match_token(&self, expected: &Token) -> bool {
        if let Some(token) = self.current_token() {
            match (token, expected) {
                (Token::LogicalOp(a), Token::LogicalOp(b)) => a == b,
                (Token::Operator(a), Token::Operator(b)) => a == b,
                _ => std::mem::discriminant(token) == std::mem::discriminant(expected),
            }
        } else {
            false
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::query::lexer::Lexer;

    #[test]
    fn test_parse_simple_comparison() {
        let mut lexer = Lexer::new("user_name == \"admin\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Comparison {
                field,
                operator,
                value,
            } => {
                assert_eq!(field, vec!["user_name".to_string()]);
                assert!(matches!(operator, Operator::Eq));
                assert!(matches!(value, Value::String(s) if s == "admin"));
            }
            _ => panic!("Expected Comparison expression"),
        }
    }

    #[test]
    fn test_parse_nested_field() {
        let mut lexer = Lexer::new("tags.env == \"production\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Comparison { field, .. } => {
                assert_eq!(field, vec!["tags".to_string(), "env".to_string()]);
            }
            _ => panic!("Expected Comparison expression"),
        }
    }

    #[test]
    fn test_parse_and_expression() {
        let mut lexer = Lexer::new("a == \"1\" AND b == \"2\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::And(_, _) => {}
            _ => panic!("Expected And expression"),
        }
    }

    #[test]
    fn test_parse_or_expression() {
        let mut lexer = Lexer::new("a == \"1\" OR b == \"2\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Or(_, _) => {}
            _ => panic!("Expected Or expression, got: {:?}", expr),
        }
    }

    #[test]
    fn test_parse_not_expression() {
        let mut lexer = Lexer::new("NOT a == \"1\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Not(_) => {}
            _ => panic!("Expected Not expression"),
        }
    }

    #[test]
    fn test_parse_parentheses() {
        let mut lexer = Lexer::new("(a == \"1\" OR b == \"2\") AND c == \"3\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::And(left, _) => match *left {
                Expr::Or(_, _) => {}
                _ => panic!("Expected Or expression inside And, got: {:?}", *left),
            },
            _ => panic!("Expected And expression, got: {:?}", expr),
        }
    }

    #[test]
    fn test_parse_in_operator() {
        let mut lexer = Lexer::new("role IN [\"admin\", \"user\"]");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Comparison {
                operator, value, ..
            } => {
                assert!(matches!(operator, Operator::In));
                match value {
                    Value::Array(arr) => {
                        assert_eq!(arr.len(), 2);
                    }
                    _ => panic!("Expected Array value"),
                }
            }
            _ => panic!("Expected Comparison expression"),
        }
    }

    #[test]
    fn test_parse_like_operator() {
        let mut lexer = Lexer::new("path LIKE \"/admin/*\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        match expr {
            Expr::Comparison { operator, .. } => {
                assert!(matches!(operator, Operator::Like));
            }
            _ => panic!("Expected Comparison expression"),
        }
    }
}
