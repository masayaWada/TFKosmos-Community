//! AWS SDK Lambda���ライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_lambda::Client as LambdaClient;
use std::collections::HashMap;

use super::lambda_client_trait::{LambdaClientOps, LambdaFunctionInfo, LambdaLayerInfo};

/// AWS SDK Lambdaクライアントをラップした本番実装
pub struct RealLambdaClient {
    client: LambdaClient,
}

impl RealLambdaClient {
    pub fn new(client: LambdaClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LambdaClientOps for RealLambdaClient {
    async fn list_functions(&self) -> Result<Vec<LambdaFunctionInfo>> {
        let mut functions = Vec::new();
        let mut marker: Option<String> = None;

        loop {
            let mut req = self.client.list_functions();
            if let Some(m) = &marker {
                req = req.marker(m.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to list Lambda functions: {}", e))?;

            for func in output.functions() {
                let tags = match self
                    .client
                    .list_tags()
                    .resource(func.function_arn().unwrap_or_default())
                    .send()
                    .await
                {
                    Ok(t) => t
                        .tags()
                        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                        .unwrap_or_default(),
                    Err(_) => HashMap::new(),
                };

                let environment = func
                    .environment()
                    .and_then(|e| e.variables())
                    .map(|vars| vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                let vpc_config = func.vpc_config();

                functions.push(LambdaFunctionInfo {
                    function_name: func.function_name().unwrap_or_default().to_string(),
                    function_arn: func.function_arn().map(|s| s.to_string()),
                    runtime: func.runtime().map(|r| r.as_str().to_string()),
                    handler: func.handler().map(|s| s.to_string()),
                    role: func.role().map(|s| s.to_string()),
                    description: func.description().map(|s| s.to_string()),
                    memory_size: func.memory_size(),
                    timeout: func.timeout(),
                    code_size: Some(func.code_size()),
                    last_modified: func.last_modified().map(|s| s.to_string()),
                    environment,
                    vpc_subnet_ids: vpc_config
                        .map(|v| v.subnet_ids().iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    vpc_security_group_ids: vpc_config
                        .map(|v| {
                            v.security_group_ids()
                                .iter()
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                    layers: func
                        .layers()
                        .iter()
                        .filter_map(|l| l.arn().map(|s| s.to_string()))
                        .collect(),
                    tags,
                });
            }

            marker = output.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }

        Ok(functions)
    }

    async fn list_layers(&self) -> Result<Vec<LambdaLayerInfo>> {
        let mut layers = Vec::new();
        let mut marker: Option<String> = None;

        loop {
            let mut req = self.client.list_layers();
            if let Some(m) = &marker {
                req = req.marker(m.clone());
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to list Lambda layers: {}", e))?;

            for layer in output.layers() {
                let latest = layer.latest_matching_version();
                layers.push(LambdaLayerInfo {
                    layer_name: layer.layer_name().unwrap_or_default().to_string(),
                    layer_arn: layer.layer_arn().map(|s| s.to_string()),
                    latest_version: latest.map(|v| v.version()),
                    description: latest.and_then(|v| v.description().map(|s| s.to_string())),
                    compatible_runtimes: latest
                        .map(|v| {
                            v.compatible_runtimes()
                                .iter()
                                .map(|r| r.as_str().to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                });
            }

            marker = output.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }

        Ok(layers)
    }
}
