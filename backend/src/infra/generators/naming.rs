pub struct NamingGenerator;

impl NamingGenerator {
    pub fn to_snake_case(s: &str) -> String {
        s.replace(['-', '.'], "_").to_lowercase()
    }

    pub fn to_kebab_case(s: &str) -> String {
        s.replace(['_', '.'], "-").to_lowercase()
    }

    pub fn apply_naming_convention(s: &str, convention: &str) -> String {
        match convention {
            "snake_case" => Self::to_snake_case(s),
            "kebab-case" => Self::to_kebab_case(s),
            "original" => s.to_string(),
            _ => s.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod to_snake_case_tests {
        use super::*;

        #[test]
        fn test_with_hyphen() {
            // Arrange
            let input = "my-resource-name";

            // Act
            let result = NamingGenerator::to_snake_case(input);

            // Assert
            assert_eq!(
                result, "my_resource_name",
                "ハイフンをアンダースコアに変換するべき"
            );
        }

        #[test]
        fn test_with_dot() {
            // Arrange
            let input = "my.resource.name";

            // Act
            let result = NamingGenerator::to_snake_case(input);

            // Assert
            assert_eq!(
                result, "my_resource_name",
                "ドットをアンダースコアに変換するべき"
            );
        }

        #[test]
        fn test_with_uppercase() {
            // Arrange
            let input = "MyResourceName";

            // Act
            let result = NamingGenerator::to_snake_case(input);

            // Assert
            assert_eq!(result, "myresourcename", "大文字を小文字に変換するべき");
        }

        #[test]
        fn test_mixed() {
            // Arrange
            let input = "My-Resource.Name";

            // Act
            let result = NamingGenerator::to_snake_case(input);

            // Assert
            assert_eq!(
                result, "my_resource_name",
                "混合パターンを正しく変換するべき"
            );
        }

        #[test]
        fn test_empty_string() {
            // Arrange
            let input = "";

            // Act
            let result = NamingGenerator::to_snake_case(input);

            // Assert
            assert_eq!(result, "", "空文字列は空文字列を返すべき");
        }
    }

    mod to_kebab_case_tests {
        use super::*;

        #[test]
        fn test_with_underscore() {
            // Arrange
            let input = "my_resource_name";

            // Act
            let result = NamingGenerator::to_kebab_case(input);

            // Assert
            assert_eq!(
                result, "my-resource-name",
                "アンダースコアをハイフンに変換するべき"
            );
        }

        #[test]
        fn test_with_dot() {
            // Arrange
            let input = "my.resource.name";

            // Act
            let result = NamingGenerator::to_kebab_case(input);

            // Assert
            assert_eq!(result, "my-resource-name", "ドットをハイフンに変換するべき");
        }

        #[test]
        fn test_with_uppercase() {
            // Arrange
            let input = "MyResourceName";

            // Act
            let result = NamingGenerator::to_kebab_case(input);

            // Assert
            assert_eq!(result, "myresourcename", "大文字を小文字に変換するべき");
        }

        #[test]
        fn test_mixed() {
            // Arrange
            let input = "My_Resource.Name";

            // Act
            let result = NamingGenerator::to_kebab_case(input);

            // Assert
            assert_eq!(
                result, "my-resource-name",
                "混合パターンを正しく変換するべき"
            );
        }

        #[test]
        fn test_empty_string() {
            // Arrange
            let input = "";

            // Act
            let result = NamingGenerator::to_kebab_case(input);

            // Assert
            assert_eq!(result, "", "空文字列は空文字列を返すべき");
        }
    }

    mod apply_naming_convention_tests {
        use super::*;

        #[test]
        fn test_snake_case() {
            // Arrange
            let input = "my-name";
            let convention = "snake_case";

            // Act
            let result = NamingGenerator::apply_naming_convention(input, convention);

            // Assert
            assert_eq!(result, "my_name", "snake_case規約を適用するべき");
        }

        #[test]
        fn test_kebab_case() {
            // Arrange
            let input = "my_name";
            let convention = "kebab-case";

            // Act
            let result = NamingGenerator::apply_naming_convention(input, convention);

            // Assert
            assert_eq!(result, "my-name", "kebab-case規約を適用するべき");
        }

        #[test]
        fn test_original() {
            // Arrange
            let input = "My-Name_Test";
            let convention = "original";

            // Act
            let result = NamingGenerator::apply_naming_convention(input, convention);

            // Assert
            assert_eq!(
                result, "My-Name_Test",
                "original規約では入力をそのまま返すべき"
            );
        }

        #[test]
        fn test_unknown_returns_original() {
            // Arrange
            let input = "My-Name";
            let convention = "unknown";

            // Act
            let result = NamingGenerator::apply_naming_convention(input, convention);

            // Assert
            assert_eq!(result, "My-Name", "不明な規約では入力をそのまま返すべき");
        }

        #[test]
        fn test_empty_convention() {
            // Arrange
            let input = "My-Name";
            let convention = "";

            // Act
            let result = NamingGenerator::apply_naming_convention(input, convention);

            // Assert
            assert_eq!(result, "My-Name", "空の規約では入力をそのまま返すべき");
        }
    }
}
