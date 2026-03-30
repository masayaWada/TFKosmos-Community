#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Number(f64),
    Boolean(bool),
    Operator(Operator),
    LogicalOp(LogicalOp),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Eq,
    Ne,
    Like,
    In,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
    Not,
}

pub struct Lexer<'a> {
    _input: &'a str,
    pos: usize,
    chars: Vec<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            _input: input,
            pos: 0,
            chars: input.chars().collect(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }
        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Option<Token>, String> {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Ok(None);
        }

        let ch = self.current_char();

        match ch {
            '(' => {
                self.advance();
                Ok(Some(Token::LParen))
            }
            ')' => {
                self.advance();
                Ok(Some(Token::RParen))
            }
            '[' => {
                self.advance();
                Ok(Some(Token::LBracket))
            }
            ']' => {
                self.advance();
                Ok(Some(Token::RBracket))
            }
            ',' => {
                self.advance();
                Ok(Some(Token::Comma))
            }
            '.' => {
                self.advance();
                Ok(Some(Token::Dot))
            }
            '"' | '\'' => self.read_string(),
            '!' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Ok(Some(Token::Operator(Operator::Ne)))
                } else {
                    Err("Unexpected character '!' (did you mean '!='?)".to_string())
                }
            }
            '=' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Ok(Some(Token::Operator(Operator::Eq)))
                } else {
                    Err("Unexpected character '=' (did you mean '=='?)".to_string())
                }
            }
            _ if ch.is_ascii_digit()
                || (ch == '-' && self.peek_char().is_some_and(|c| c.is_ascii_digit())) =>
            {
                self.read_number()
            }
            _ if ch.is_alphabetic() || ch == '_' => self.read_identifier(),
            _ => Err(format!("Unexpected character: '{}'", ch)),
        }
    }

    fn current_char(&self) -> char {
        if self.pos < self.chars.len() {
            self.chars[self.pos]
        } else {
            '\0'
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos + 1 < self.chars.len() {
            Some(self.chars[self.pos + 1])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.current_char().is_whitespace() {
            self.advance();
        }
    }

    fn read_string(&mut self) -> Result<Option<Token>, String> {
        let quote = self.current_char();
        self.advance();

        let mut value = String::new();
        while self.pos < self.chars.len() && self.current_char() != quote {
            if self.current_char() == '\\' {
                self.advance();
                if self.pos < self.chars.len() {
                    match self.current_char() {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        'r' => value.push('\r'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '\'' => value.push('\''),
                        _ => {
                            value.push('\\');
                            value.push(self.current_char());
                        }
                    }
                }
            } else {
                value.push(self.current_char());
            }
            self.advance();
        }

        if self.pos >= self.chars.len() {
            return Err("Unterminated string".to_string());
        }

        self.advance();
        Ok(Some(Token::String(value)))
    }

    fn read_number(&mut self) -> Result<Option<Token>, String> {
        let mut num_str = String::new();

        if self.current_char() == '-' {
            num_str.push('-');
            self.advance();
        }

        while self.pos < self.chars.len()
            && (self.current_char().is_ascii_digit() || self.current_char() == '.')
        {
            num_str.push(self.current_char());
            self.advance();
        }

        num_str
            .parse::<f64>()
            .map(|n| Some(Token::Number(n)))
            .map_err(|_| format!("Invalid number: {}", num_str))
    }

    fn read_identifier(&mut self) -> Result<Option<Token>, String> {
        let mut ident = String::new();

        while self.pos < self.chars.len()
            && (self.current_char().is_alphanumeric() || self.current_char() == '_')
        {
            ident.push(self.current_char());
            self.advance();
        }

        let token = match ident.as_str() {
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            "AND" => Token::LogicalOp(LogicalOp::And),
            "OR" => Token::LogicalOp(LogicalOp::Or),
            "NOT" => Token::LogicalOp(LogicalOp::Not),
            "LIKE" => Token::Operator(Operator::Like),
            "IN" => Token::Operator(Operator::In),
            _ => Token::Identifier(ident),
        };

        Ok(Some(token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_comparison() {
        let mut lexer = Lexer::new("user_name == \"admin\"");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0], Token::Identifier("user_name".to_string()));
        assert_eq!(tokens[1], Token::Operator(Operator::Eq));
        assert_eq!(tokens[2], Token::String("admin".to_string()));
        assert_eq!(tokens[3], Token::Eof);
    }

    #[test]
    fn test_tokenize_nested_field() {
        let mut lexer = Lexer::new("tags.env == \"production\"");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::Identifier("tags".to_string()));
        assert_eq!(tokens[1], Token::Dot);
        assert_eq!(tokens[2], Token::Identifier("env".to_string()));
        assert_eq!(tokens[3], Token::Operator(Operator::Eq));
        assert_eq!(tokens[4], Token::String("production".to_string()));
    }

    #[test]
    fn test_tokenize_like_operator() {
        let mut lexer = Lexer::new("path LIKE \"/admin/*\"");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::Identifier("path".to_string()));
        assert_eq!(tokens[1], Token::Operator(Operator::Like));
        assert_eq!(tokens[2], Token::String("/admin/*".to_string()));
    }

    #[test]
    fn test_tokenize_in_operator() {
        let mut lexer = Lexer::new("role IN [\"admin\", \"user\"]");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::Identifier("role".to_string()));
        assert_eq!(tokens[1], Token::Operator(Operator::In));
        assert_eq!(tokens[2], Token::LBracket);
        assert_eq!(tokens[3], Token::String("admin".to_string()));
        assert_eq!(tokens[4], Token::Comma);
        assert_eq!(tokens[5], Token::String("user".to_string()));
        assert_eq!(tokens[6], Token::RBracket);
    }

    #[test]
    fn test_tokenize_logical_operators() {
        let mut lexer = Lexer::new("a == \"1\" AND b == \"2\" OR NOT c == \"3\"");
        let tokens = lexer.tokenize().unwrap();

        assert!(tokens.contains(&Token::LogicalOp(LogicalOp::And)));
        assert!(tokens.contains(&Token::LogicalOp(LogicalOp::Or)));
        assert!(tokens.contains(&Token::LogicalOp(LogicalOp::Not)));
    }

    #[test]
    fn test_tokenize_parentheses() {
        let mut lexer = Lexer::new("(a == \"1\" OR b == \"2\") AND c == \"3\"");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::LParen);
        assert!(tokens.contains(&Token::RParen));
    }

    #[test]
    fn test_tokenize_numbers() {
        let mut lexer = Lexer::new("count == 42");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::Identifier("count".to_string()));
        assert_eq!(tokens[1], Token::Operator(Operator::Eq));
        assert_eq!(tokens[2], Token::Number(42.0));
    }

    #[test]
    fn test_tokenize_boolean() {
        let mut lexer = Lexer::new("enabled == true");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0], Token::Identifier("enabled".to_string()));
        assert_eq!(tokens[1], Token::Operator(Operator::Eq));
        assert_eq!(tokens[2], Token::Boolean(true));
    }

    #[test]
    fn test_error_unterminated_string() {
        let mut lexer = Lexer::new("name == \"unterminated");
        let result = lexer.tokenize();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unterminated string"));
    }
}
