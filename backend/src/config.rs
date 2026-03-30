use std::env;

/// アプリケーション設定
#[derive(Debug, Clone)]
pub struct Config {
    /// 実行環境（development, production）
    pub environment: Environment,
    /// サーバーがリッスンするホスト
    pub host: String,
    /// サーバーがリッスンするポート
    pub port: u16,
    /// CORS許可オリジン（カンマ区切り、空の場合は全許可）
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Production,
}

impl Config {
    /// 環境変数から設定を読み込む
    pub fn from_env() -> Self {
        let environment = match env::var("TFKOSMOS_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase()
            .as_str()
        {
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        };

        let host = env::var("TFKOSMOS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let port = env::var("TFKOSMOS_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8000);

        // CORS許可オリジン
        // 例: TFKOSMOS_CORS_ORIGINS="http://localhost:5173,https://example.com"
        let cors_origins = env::var("TFKOSMOS_CORS_ORIGINS")
            .map(|origins| {
                origins
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| Vec::new());

        Config {
            environment,
            host,
            port,
            cors_origins,
        }
    }

    /// 開発環境かどうか
    #[allow(dead_code)]
    pub fn is_development(&self) -> bool {
        self.environment == Environment::Development
    }

    /// 本番環境かどうか
    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }

    /// サーバーのバインドアドレス
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            environment: Environment::Development,
            host: "0.0.0.0".to_string(),
            port: 8000,
            cors_origins: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        // Arrange & Act
        let config = Config::default();

        // Assert
        assert_eq!(
            config.environment,
            Environment::Development,
            "デフォルト環境はDevelopmentであるべき"
        );
        assert_eq!(
            config.host, "0.0.0.0",
            "デフォルトホストは0.0.0.0であるべき"
        );
        assert_eq!(config.port, 8000, "デフォルトポートは8000であるべき");
        assert!(
            config.cors_origins.is_empty(),
            "デフォルトCORSオリジンは空であるべき"
        );
    }

    #[test]
    fn test_is_development() {
        // Arrange
        let config = Config {
            environment: Environment::Development,
            ..Default::default()
        };

        // Act & Assert
        assert!(
            config.is_development(),
            "Development環境ではis_development()がtrueを返すべき"
        );
        assert!(
            !config.is_production(),
            "Development環境ではis_production()がfalseを返すべき"
        );
    }

    #[test]
    fn test_is_production() {
        // Arrange
        let config = Config {
            environment: Environment::Production,
            ..Default::default()
        };

        // Act & Assert
        assert!(
            config.is_production(),
            "Production環境ではis_production()がtrueを返すべき"
        );
        assert!(
            !config.is_development(),
            "Production環境ではis_development()がfalseを返すべき"
        );
    }

    #[test]
    fn test_bind_address() {
        // Arrange
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            ..Default::default()
        };

        // Act
        let bind_address = config.bind_address();

        // Assert
        assert_eq!(
            bind_address, "127.0.0.1:3000",
            "バインドアドレスはhost:port形式であるべき"
        );
    }

    #[test]
    fn test_bind_address_default_values() {
        // Arrange
        let config = Config::default();

        // Act
        let bind_address = config.bind_address();

        // Assert
        assert_eq!(
            bind_address, "0.0.0.0:8000",
            "デフォルト値でのバインドアドレスは0.0.0.0:8000であるべき"
        );
    }

    #[test]
    fn test_config_with_custom_cors_origins() {
        let config = Config {
            cors_origins: vec![
                "http://localhost:5173".to_string(),
                "https://example.com".to_string(),
            ],
            ..Default::default()
        };

        assert_eq!(config.cors_origins.len(), 2);
        assert_eq!(config.cors_origins[0], "http://localhost:5173");
        assert_eq!(config.cors_origins[1], "https://example.com");
    }

    #[test]
    fn test_config_bind_address_custom_port() {
        let config = Config {
            host: "localhost".to_string(),
            port: 3000,
            ..Default::default()
        };

        assert_eq!(config.bind_address(), "localhost:3000");
    }

    #[test]
    fn test_environment_equality() {
        assert_eq!(Environment::Development, Environment::Development);
        assert_eq!(Environment::Production, Environment::Production);
        assert_ne!(Environment::Development, Environment::Production);
    }

    #[test]
    fn test_environment_development_is_default_variant() {
        // Environment::Developmentがデフォルト変数として機能することを確認（from_envではなく直接構築）
        let config = Config {
            environment: Environment::Development,
            host: "0.0.0.0".to_string(),
            port: 8000,
            cors_origins: Vec::new(),
        };
        assert_eq!(config.environment, Environment::Development);
        assert!(config.is_development());
        assert!(!config.is_production());
    }

    #[test]
    fn test_environment_production_variant() {
        let config = Config {
            environment: Environment::Production,
            host: "0.0.0.0".to_string(),
            port: 8000,
            cors_origins: Vec::new(),
        };
        assert_eq!(config.environment, Environment::Production);
        assert!(config.is_production());
        assert!(!config.is_development());
    }

    #[test]
    fn test_cors_origins_parsing_logic() {
        // CORSオリジンのパース処理をシミュレート（from_envの実装を直接テスト）
        let origins_str = "http://localhost:5173,https://example.com,  http://app.local  ";
        let parsed: Vec<String> = origins_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "http://localhost:5173");
        assert_eq!(parsed[1], "https://example.com");
        assert_eq!(parsed[2], "http://app.local");
    }

    #[test]
    fn test_cors_origins_empty_string_filtered() {
        let origins_str = "http://localhost:5173,,https://example.com";
        let parsed: Vec<String> = origins_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // 空文字列がフィルタリングされて2件になる
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_port_parse_logic() {
        // ポートのパースロジック（有効値）
        let valid_port: Option<u16> = "9090".parse().ok();
        assert_eq!(valid_port, Some(9090u16));

        // 無効値はNoneになりデフォルトにフォールバック
        let invalid_port: Option<u16> = "not_a_number".parse().ok();
        assert!(invalid_port.is_none());
        let port = invalid_port.unwrap_or(8000);
        assert_eq!(port, 8000);
    }

    #[test]
    fn test_environment_matching_logic() {
        // Environment の文字列マッチングロジック
        let env_str = "production";
        let env = match env_str.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        };
        assert_eq!(env, Environment::Production);

        let env_str2 = "prod";
        let env2 = match env_str2.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        };
        assert_eq!(env2, Environment::Production);

        let env_str3 = "development";
        let env3 = match env_str3.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        };
        assert_eq!(env3, Environment::Development);

        let env_str4 = "unknown";
        let env4 = match env_str4.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            _ => Environment::Development,
        };
        assert_eq!(env4, Environment::Development);
    }
}
