use anyhow::{Context, Result};
use aws_config::SdkConfig;
use aws_sdk_cloudwatch::Client as CloudWatchClient;
use aws_sdk_dynamodb::Client as DynamoDBClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_elasticloadbalancingv2::Client as ELBClient;
use aws_sdk_iam::Client as IamClient;
use aws_sdk_lambda::Client as LambdaClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sts::Client as StsClient;

use crate::models::ConnectionTestResponse;

pub struct AwsClientFactory;

impl AwsClientFactory {
    pub async fn create_config(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        _assume_role_session_name: Option<String>,
    ) -> Result<SdkConfig> {
        // aws loginで設定された認証情報を使用する場合、AWS CLIコマンド経由で認証情報を取得
        // これは、aws-configがlogin_sessionを直接サポートしていないため
        let profile_name = profile.as_deref().unwrap_or("default");

        // AWS CLIコマンドで認証情報を取得（aws loginで設定された認証情報を使用）
        // 環境変数が設定されていない場合のみ実行
        if std::env::var("AWS_ACCESS_KEY_ID").is_err()
            || std::env::var("AWS_SECRET_ACCESS_KEY").is_err()
        {
            let credentials_output = tokio::process::Command::new("aws")
                .args([
                    "configure",
                    "export-credentials",
                    "--profile",
                    profile_name,
                    "--format",
                    "env",
                ])
                .output()
                .await;

            // 環境変数を設定（成功した場合）
            if let Ok(output) = credentials_output {
                if output.status.success() {
                    let env_vars = String::from_utf8_lossy(&output.stdout);
                    for line in env_vars.lines() {
                        if let Some((key, value)) = line.split_once('=') {
                            let key = key.trim().trim_start_matches("export ");
                            let value = value.trim().trim_matches('"');
                            if !key.is_empty() && !value.is_empty() {
                                std::env::set_var(key, value);
                            }
                        }
                    }
                }
            }
        }

        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

        if let Some(profile_name) = &profile {
            config_loader = config_loader.profile_name(profile_name);
        }

        let config = config_loader.load().await;

        // Handle assume role if provided
        if let Some(_role_arn) = assume_role_arn {
            // In a real implementation, use STS to assume the role
            // For now, return the base config
            // TODO: Implement assume role logic
        }

        Ok(config)
    }

    pub async fn create_iam_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<IamClient> {
        let config = Self::create_config(
            profile.clone(),
            assume_role_arn.clone(),
            assume_role_session_name.clone(),
        )
        .await
        .with_context(|| {
            format!(
                "Failed to load AWS configuration. Profile: {:?}, Assume Role ARN: {:?}. \
                    Please ensure AWS credentials are configured. \
                    If using 'aws login', make sure the authentication is complete.",
                profile, assume_role_arn
            )
        })?;

        // Verify credentials by testing with STS
        let sts_client = StsClient::new(&config);
        if let Err(e) = sts_client.get_caller_identity().send().await {
            return Err(anyhow::anyhow!(
                "Failed to verify AWS credentials. Profile: {:?}, Error: {}. \
                Please ensure you have run 'aws login' or configured AWS credentials properly. \
                You can test your credentials by running: aws sts get-caller-identity",
                profile,
                e
            ));
        }

        Ok(IamClient::new(&config))
    }

    pub async fn create_s3_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<S3Client> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(S3Client::new(&config))
    }

    pub async fn create_ec2_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<Ec2Client> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(Ec2Client::new(&config))
    }

    pub async fn create_dynamodb_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<DynamoDBClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(DynamoDBClient::new(&config))
    }

    pub async fn create_cloudwatch_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<CloudWatchClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(CloudWatchClient::new(&config))
    }

    pub async fn create_sns_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<SnsClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(SnsClient::new(&config))
    }

    pub async fn create_elb_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<ELBClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(ELBClient::new(&config))
    }

    pub async fn create_lambda_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<LambdaClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(LambdaClient::new(&config))
    }

    pub async fn create_rds_client(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<RdsClient> {
        let config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        Ok(RdsClient::new(&config))
    }

    pub async fn test_connection(
        profile: Option<String>,
        assume_role_arn: Option<String>,
        assume_role_session_name: Option<String>,
    ) -> Result<ConnectionTestResponse> {
        let sts_config =
            Self::create_config(profile, assume_role_arn, assume_role_session_name).await?;
        let sts_client = StsClient::new(&sts_config);

        match sts_client.get_caller_identity().send().await {
            Ok(_response) => {
                Ok(ConnectionTestResponse {
                    success: true,
                    message: Some("Connection successful".to_string()),
                    account_id: None,
                    user_arn: None,
                    subscription_name: None, // AWSでは使用しない
                })
            }
            Err(e) => Ok(ConnectionTestResponse {
                success: false,
                message: Some(format!("Connection failed: {}", e)),
                account_id: None,
                user_arn: None,
                subscription_name: None,
            }),
        }
    }
}
