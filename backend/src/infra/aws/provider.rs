//! AWS プロバイダーの CloudProviderScanner 実装

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;

use crate::infra::aws::client_factory::AwsClientFactory;
use crate::infra::aws::cloudwatch_scanner::AwsCloudWatchSNSScanner;
use crate::infra::aws::dynamodb_scanner::AwsDynamoDBScanner;
use crate::infra::aws::ec2_scanner::AwsEc2Scanner;
use crate::infra::aws::elb_scanner::AwsELBScanner;
use crate::infra::aws::lambda_scanner::AwsLambdaScanner;
use crate::infra::aws::rds_scanner::AwsRdsScanner;
use crate::infra::aws::real_cloudwatch_client::RealCloudWatchClient;
use crate::infra::aws::real_dynamodb_client::RealDynamoDBClient;
use crate::infra::aws::real_ec2_client::RealEc2Client;
use crate::infra::aws::real_elb_client::RealELBClient;
use crate::infra::aws::real_lambda_client::RealLambdaClient;
use crate::infra::aws::real_rds_client::RealRdsClient;
use crate::infra::aws::real_s3_client::RealS3Client;
use crate::infra::aws::real_sns_client::RealSNSClient;
use crate::infra::aws::s3_scanner::AwsS3Scanner;
use crate::infra::aws::scanner::AwsIamScanner;
use crate::infra::aws::vpc_scanner::AwsVpcScanner;
use crate::infra::provider_trait::{CloudProviderScanner, ResourceTemplate};
use crate::models::ScanConfig;

/// AWS プロバイダー
pub struct AwsProvider;

#[async_trait]
impl CloudProviderScanner for AwsProvider {
    fn provider_name(&self) -> &'static str {
        "aws"
    }

    fn get_templates(&self) -> Vec<ResourceTemplate> {
        vec![
            // IAM
            ResourceTemplate {
                resource_type: "users",
                template_path: "aws/iam_user.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "groups",
                template_path: "aws/iam_group.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "roles",
                template_path: "aws/iam_role.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "policies",
                template_path: "aws/iam_policy.tf.j2",
                provider: "aws",
            },
            // S3
            ResourceTemplate {
                resource_type: "buckets",
                template_path: "aws/s3_bucket.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "bucket_policies",
                template_path: "aws/s3_bucket_policy.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "lifecycle_rules",
                template_path: "aws/s3_lifecycle.tf.j2",
                provider: "aws",
            },
            // EC2
            ResourceTemplate {
                resource_type: "instances",
                template_path: "aws/ec2_instance.tf.j2",
                provider: "aws",
            },
            // VPC
            ResourceTemplate {
                resource_type: "vpcs",
                template_path: "aws/vpc.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "subnets",
                template_path: "aws/subnet.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "route_tables",
                template_path: "aws/route_table.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "security_groups",
                template_path: "aws/security_group.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "network_acls",
                template_path: "aws/network_acl.tf.j2",
                provider: "aws",
            },
            // VPC (IGW/NAT/EIP)
            ResourceTemplate {
                resource_type: "internet_gateways",
                template_path: "aws/internet_gateway.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "nat_gateways",
                template_path: "aws/nat_gateway.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "elastic_ips",
                template_path: "aws/eip.tf.j2",
                provider: "aws",
            },
            // RDS
            ResourceTemplate {
                resource_type: "db_instances",
                template_path: "aws/rds_instance.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "db_subnet_groups",
                template_path: "aws/rds_subnet_group.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "db_parameter_groups",
                template_path: "aws/rds_parameter_group.tf.j2",
                provider: "aws",
            },
            // Lambda
            ResourceTemplate {
                resource_type: "functions",
                template_path: "aws/lambda_function.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "lambda_layers",
                template_path: "aws/lambda_layer.tf.j2",
                provider: "aws",
            },
            // DynamoDB
            ResourceTemplate {
                resource_type: "dynamodb_tables",
                template_path: "aws/dynamodb_table.tf.j2",
                provider: "aws",
            },
            // ELB/ALB
            ResourceTemplate {
                resource_type: "load_balancers",
                template_path: "aws/alb.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "lb_listeners",
                template_path: "aws/alb_listener.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "lb_target_groups",
                template_path: "aws/alb_target_group.tf.j2",
                provider: "aws",
            },
            // CloudWatch/SNS
            ResourceTemplate {
                resource_type: "cloudwatch_alarms",
                template_path: "aws/cloudwatch_alarm.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "sns_topics",
                template_path: "aws/sns_topic.tf.j2",
                provider: "aws",
            },
            ResourceTemplate {
                resource_type: "sns_subscriptions",
                template_path: "aws/sns_subscription.tf.j2",
                provider: "aws",
            },
        ]
    }

    async fn scan(
        &self,
        config: ScanConfig,
        callback: Box<dyn Fn(u32, String) + Send + Sync>,
    ) -> Result<serde_json::Value> {
        // IAMスキャンで基本のresultsを取得（他スキャナーより先に実行）
        let callback = Arc::new(callback);
        let scanner = AwsIamScanner::new(config.clone()).await?;
        let cb_clone: Box<dyn Fn(u32, String) + Send + Sync> = {
            let cb = callback.clone();
            Box::new(move |p, m| cb(p, m))
        };
        let iam_result = scanner.scan(cb_clone).await?;

        let mut results = match iam_result {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };

        // scan_targetsに非IAMリソースが含まれているか確認
        let has_s3_targets = config
            .scan_targets
            .keys()
            .any(|k| ["buckets", "bucket_policies", "lifecycle_rules"].contains(&k.as_str()))
            && config.scan_targets.values().any(|&v| v);
        let has_ec2_targets = config
            .scan_targets
            .get("instances")
            .copied()
            .unwrap_or(false);
        let has_vpc_targets = config.scan_targets.keys().any(|k| {
            [
                "vpcs",
                "subnets",
                "route_tables",
                "security_groups",
                "network_acls",
                "internet_gateways",
                "nat_gateways",
                "elastic_ips",
            ]
            .contains(&k.as_str())
        }) && config.scan_targets.values().any(|&v| v);
        let has_rds_targets = config.scan_targets.keys().any(|k| {
            ["db_instances", "db_subnet_groups", "db_parameter_groups"].contains(&k.as_str())
        }) && config.scan_targets.values().any(|&v| v);
        let has_lambda_targets = config
            .scan_targets
            .keys()
            .any(|k| ["functions", "lambda_layers"].contains(&k.as_str()))
            && config.scan_targets.values().any(|&v| v);
        let has_dynamodb_targets = config
            .scan_targets
            .get("dynamodb_tables")
            .copied()
            .unwrap_or(false);
        let has_elb_targets = config
            .scan_targets
            .keys()
            .any(|k| ["load_balancers", "lb_target_groups"].contains(&k.as_str()))
            && config.scan_targets.values().any(|&v| v);
        let has_cw_sns_targets = config.scan_targets.keys().any(|k| {
            ["cloudwatch_alarms", "sns_topics", "sns_subscriptions"].contains(&k.as_str())
        }) && config.scan_targets.values().any(|&v| v);

        // 非IAMリソースのスキャン対象数を計算
        let non_iam_total: usize = config
            .scan_targets
            .iter()
            .filter(|(k, &v)| v && !["users", "groups", "roles", "policies"].contains(&k.as_str()))
            .count();

        // 共有プログレスカウンター（全グループで共有）
        let completed = Arc::new(AtomicUsize::new(0));

        // 同時実行するAWS APIコール数を制限するセマフォ
        // AWS APIのレートリミットに対する保護として、最大10並列に制限
        let semaphore = Arc::new(Semaphore::new(10));

        // 各スキャナーグループを並列実行
        // グループ間は独立（異なるSDKクライアントを使用）
        // EC2+VPCはクライアントを共有するため同一グループ内で逐次実行

        let s3_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_s3_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("S3スキャンを開始");
                let s3_client = AwsClientFactory::create_s3_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("S3クライアントの作成に失敗")?;

                let s3_scanner = AwsS3Scanner::new(config, Arc::new(RealS3Client::new(s3_client)));
                let mut map = serde_json::Map::new();
                s3_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        let ec2_vpc_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_ec2_targets && !has_vpc_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                let ec2_client = AwsClientFactory::create_ec2_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("EC2クライアントの作成に失敗")?;

                let ec2_client = Arc::new(RealEc2Client::new(ec2_client));
                let mut map = serde_json::Map::new();

                if has_ec2_targets {
                    info!("EC2スキャンを開始");
                    let ec2_scanner = AwsEc2Scanner::new(config.clone(), ec2_client.clone());
                    ec2_scanner
                        .scan_into(&mut map, &**cb, &completed, non_iam_total)
                        .await?;
                }

                if has_vpc_targets {
                    info!("VPCスキャンを開始");
                    let vpc_scanner = AwsVpcScanner::new(config, ec2_client);
                    vpc_scanner
                        .scan_into(&mut map, &**cb, &completed, non_iam_total)
                        .await?;
                }
                Ok(map)
            }
        };

        let rds_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_rds_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("RDSスキャンを開始");
                let rds_client = AwsClientFactory::create_rds_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("RDSクライアントの作成に失敗")?;

                let rds_scanner =
                    AwsRdsScanner::new(config, Arc::new(RealRdsClient::new(rds_client)));
                let mut map = serde_json::Map::new();
                rds_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        let lambda_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_lambda_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("Lambdaスキャンを開始");
                let lambda_client = AwsClientFactory::create_lambda_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("Lambdaクライアントの作成に失敗")?;

                let lambda_scanner =
                    AwsLambdaScanner::new(config, Arc::new(RealLambdaClient::new(lambda_client)));
                let mut map = serde_json::Map::new();
                lambda_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        let dynamodb_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_dynamodb_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("DynamoDBスキャンを開始");
                let dynamodb_client = AwsClientFactory::create_dynamodb_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("DynamoDBクライアントの作成に失敗")?;

                let dynamodb_scanner = AwsDynamoDBScanner::new(
                    config,
                    Arc::new(RealDynamoDBClient::new(dynamodb_client)),
                );
                let mut map = serde_json::Map::new();
                dynamodb_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        let elb_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_elb_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("ELB/ALBスキャンを開始");
                let elb_client = AwsClientFactory::create_elb_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("ELBクライアントの作成に失敗")?;

                let elb_scanner =
                    AwsELBScanner::new(config, Arc::new(RealELBClient::new(elb_client)));
                let mut map = serde_json::Map::new();
                elb_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        let cw_sns_future = {
            let config = config.clone();
            let cb = callback.clone();
            let completed = completed.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                if !has_cw_sns_targets {
                    return Ok::<_, anyhow::Error>(serde_json::Map::new());
                }
                info!("CloudWatch/SNSスキャンを開始");
                let cw_client = AwsClientFactory::create_cloudwatch_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("CloudWatchクライアントの作成に失敗")?;

                let sns_client = AwsClientFactory::create_sns_client(
                    config.profile.clone(),
                    config.assume_role_arn.clone(),
                    config.assume_role_session_name.clone(),
                )
                .await
                .context("SNSクライアントの作成に失敗")?;

                let cw_sns_scanner = AwsCloudWatchSNSScanner::new(
                    config,
                    Arc::new(RealCloudWatchClient::new(cw_client)),
                    Arc::new(RealSNSClient::new(sns_client)),
                );
                let mut map = serde_json::Map::new();
                cw_sns_scanner
                    .scan_into(&mut map, &**cb, &completed, non_iam_total)
                    .await?;
                Ok(map)
            }
        };

        // 全グループを並列実行
        let (s3_map, ec2_vpc_map, rds_map, lambda_map, dynamodb_map, elb_map, cw_sns_map) = tokio::try_join!(
            s3_future,
            ec2_vpc_future,
            rds_future,
            lambda_future,
            dynamodb_future,
            elb_future,
            cw_sns_future,
        )?;

        // 各グループの結果をIAM結果にマージ
        for map in [
            s3_map,
            ec2_vpc_map,
            rds_map,
            lambda_map,
            dynamodb_map,
            elb_map,
            cw_sns_map,
        ] {
            results.extend(map);
        }

        Ok(Value::Object(results))
    }
}
