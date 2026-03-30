use super::lexer::Operator;
use super::parser::{Expr, Value};
use serde_json::Value as JsonValue;

pub struct QueryEvaluator;

impl QueryEvaluator {
    pub fn evaluate(expr: &Expr, resource: &JsonValue) -> bool {
        match expr {
            Expr::Comparison {
                field,
                operator,
                value,
            } => {
                let field_value = Self::get_nested_field(resource, field);
                Self::compare(field_value, operator, value)
            }
            Expr::And(left, right) => {
                Self::evaluate(left, resource) && Self::evaluate(right, resource)
            }
            Expr::Or(left, right) => {
                Self::evaluate(left, resource) || Self::evaluate(right, resource)
            }
            Expr::Not(inner) => !Self::evaluate(inner, resource),
        }
    }

    fn get_nested_field<'a>(resource: &'a JsonValue, field: &[String]) -> Option<&'a JsonValue> {
        let mut current = resource;
        for key in field {
            current = current.get(key)?;
        }
        Some(current)
    }

    fn compare(field_value: Option<&JsonValue>, op: &Operator, expected: &Value) -> bool {
        let field_value = match field_value {
            Some(v) => v,
            None => return false,
        };

        match op {
            Operator::Eq => Self::compare_eq(field_value, expected),
            Operator::Ne => !Self::compare_eq(field_value, expected),
            Operator::Like => Self::compare_like(field_value, expected),
            Operator::In => Self::compare_in(field_value, expected),
        }
    }

    fn compare_eq(field_value: &JsonValue, expected: &Value) -> bool {
        match (field_value, expected) {
            (JsonValue::String(a), Value::String(b)) => a == b,
            (JsonValue::Number(a), Value::Number(b)) => {
                a.as_f64().is_some_and(|av| (av - b).abs() < f64::EPSILON)
            }
            (JsonValue::Bool(a), Value::Boolean(b)) => a == b,
            _ => false,
        }
    }

    fn compare_like(field_value: &JsonValue, pattern: &Value) -> bool {
        if let (JsonValue::String(s), Value::String(pattern_str)) = (field_value, pattern) {
            Self::wildcard_match(s, pattern_str)
        } else {
            false
        }
    }

    fn wildcard_match(text: &str, pattern: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('*').collect();

        if pattern_parts.len() == 1 {
            return text == pattern;
        }

        let mut text_pos = 0;

        for (i, part) in pattern_parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                if !text.starts_with(part) {
                    return false;
                }
                text_pos = part.len();
            } else if i == pattern_parts.len() - 1 {
                if !text.ends_with(part) {
                    return false;
                }
                if text_pos > text.len() - part.len() {
                    return false;
                }
            } else if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }

        true
    }

    fn compare_in(field_value: &JsonValue, array: &Value) -> bool {
        if let Value::Array(arr) = array {
            for item in arr {
                if Self::compare_eq(field_value, item) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::query::lexer::Lexer;
    use crate::infra::query::parser::QueryParser;
    use serde_json::json;

    #[test]
    fn test_evaluate_simple_eq() {
        let resource = json!({
            "user_name": "admin",
            "role": "administrator"
        });

        let mut lexer = Lexer::new("user_name == \"admin\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_simple_ne() {
        let resource = json!({
            "user_name": "admin",
            "role": "administrator"
        });

        let mut lexer = Lexer::new("user_name != \"user\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_nested_field() {
        let resource = json!({
            "user_name": "admin",
            "tags": {
                "env": "production",
                "team": "platform"
            }
        });

        let mut lexer = Lexer::new("tags.env == \"production\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_like_wildcard() {
        let resource = json!({
            "path": "/admin/users/123"
        });

        let mut lexer = Lexer::new("path LIKE \"/admin/*\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_like_no_match() {
        let resource = json!({
            "path": "/user/profile"
        });

        let mut lexer = Lexer::new("path LIKE \"/admin/*\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(!QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_in_operator() {
        let resource = json!({
            "role": "admin"
        });

        let mut lexer = Lexer::new("role IN [\"admin\", \"moderator\"]");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_in_operator_no_match() {
        let resource = json!({
            "role": "user"
        });

        let mut lexer = Lexer::new("role IN [\"admin\", \"moderator\"]");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(!QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_and_expression() {
        let resource = json!({
            "user_name": "admin",
            "role": "administrator"
        });

        let mut lexer = Lexer::new("user_name == \"admin\" AND role == \"administrator\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_or_expression() {
        let resource = json!({
            "user_name": "admin",
            "role": "user"
        });

        let mut lexer = Lexer::new("role == \"administrator\" OR user_name == \"admin\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_not_expression() {
        let resource = json!({
            "user_name": "admin"
        });

        let mut lexer = Lexer::new("NOT user_name == \"user\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_evaluate_complex_expression() {
        let resource = json!({
            "user_name": "app-user-123",
            "tags": {
                "env": "production"
            }
        });

        let mut lexer = Lexer::new("tags.env == \"production\" AND user_name LIKE \"app-*\"");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = QueryParser::new(tokens);
        let expr = parser.parse().unwrap();

        assert!(QueryEvaluator::evaluate(&expr, &resource));
    }

    #[test]
    fn test_wildcard_match() {
        assert!(QueryEvaluator::wildcard_match("hello world", "hello*"));
        assert!(QueryEvaluator::wildcard_match("hello world", "*world"));
        assert!(QueryEvaluator::wildcard_match("hello world", "hello*world"));
        assert!(QueryEvaluator::wildcard_match("hello world", "*"));
        assert!(QueryEvaluator::wildcard_match("hello", "hello"));
        assert!(!QueryEvaluator::wildcard_match("hello", "world"));
        assert!(QueryEvaluator::wildcard_match(
            "/admin/users/123",
            "/admin/*"
        ));
    }
}
