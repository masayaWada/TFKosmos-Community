use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::infra::generators::naming::NamingGenerator;
use crate::infra::templates::manager::TemplateManager;
use crate::models::GenerationConfig;

pub struct TerraformGenerator;

use crate::infra::provider_trait::ResourceTemplate;

impl TerraformGenerator {
    pub async fn generate(
        scan_data: &Value,
        config: &GenerationConfig,
        selected_resources: &HashMap<String, Vec<Value>>,
        output_path: &PathBuf,
    ) -> Result<Vec<String>> {
        let provider = scan_data
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("aws");

        tracing::info!(provider = %provider, "Starting generation for provider");
        tracing::debug!(output_path = ?output_path, "Output path");
        tracing::debug!(selected_resources = ?selected_resources, "Selected resources");

        // Define resource templates based on provider
        let templates = Self::get_templates_for_provider(provider);
        tracing::debug!(
            count = templates.len(),
            provider = %provider,
            "Found templates for provider"
        );

        let mut generated_files = Vec::new();

        // Process each resource type
        for template_info in templates {
            let resource_type = template_info.resource_type;

            // Get resources from scan data
            let resources = scan_data
                .get(resource_type)
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            tracing::debug!(
                resource_type = %resource_type,
                count = resources.len(),
                "Resource type: found resources"
            );

            if resources.is_empty() {
                tracing::debug!(resource_type = %resource_type, "Skipping resource type (no resources)");
                continue;
            }

            // Filter by selected resources if provided
            // If selected_resources is empty or doesn't contain this resource type, use all resources
            let resources_to_process = if selected_resources.is_empty() {
                tracing::debug!(
                    count = resources.len(),
                    resource_type = %resource_type,
                    "No selection filter provided, using all resources for type"
                );
                resources
            } else if let Some(selected) = selected_resources.get(resource_type) {
                tracing::debug!(
                    resource_type = %resource_type,
                    count = selected.len(),
                    "Filtering resources for type"
                );
                if selected.is_empty() {
                    tracing::debug!(resource_type = %resource_type, "Skipping resource type (empty selection)");
                    continue; // Skip if empty selection
                }

                // Extract selected IDs (handle both string IDs and object IDs)
                let selected_ids: Vec<String> = selected
                    .iter()
                    .filter_map(|s| {
                        // If it's a string, use it directly
                        if let Some(id_str) = s.as_str() {
                            Some(id_str.to_string())
                        } else if let Some(obj) = s.as_object() {
                            // If it's an object, try to extract ID from common fields
                            obj.get("user_name")
                                .or_else(|| obj.get("group_name"))
                                .or_else(|| obj.get("role_name"))
                                .or_else(|| obj.get("arn"))
                                .or_else(|| obj.get("id"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                tracing::debug!(
                    count = selected_ids.len(),
                    ids = ?selected_ids,
                    "Extracted selected IDs"
                );

                // Filter resources that match selected IDs
                let filtered: Vec<_> = resources
                    .iter()
                    .filter(|r| {
                        // Get resource identifier based on resource type
                        let resource_id = match resource_type {
                            "users" => r.get("user_name").and_then(|v| v.as_str()),
                            "groups" => r.get("group_name").and_then(|v| v.as_str()),
                            "roles" => r.get("role_name").and_then(|v| v.as_str()),
                            "policies" => r
                                .get("arn")
                                .or_else(|| r.get("policy_name"))
                                .and_then(|v| v.as_str()),
                            _ => r
                                .get("arn")
                                .or_else(|| r.get("id"))
                                .or_else(|| r.get("name"))
                                .and_then(|v| v.as_str()),
                        };

                        if let Some(id) = resource_id {
                            let matches = selected_ids.contains(&id.to_string());
                            if matches {
                                tracing::debug!(resource_id = %id, "Resource matches selected ID");
                            }
                            matches
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();
                tracing::debug!(
                    count = filtered.len(),
                    resource_type = %resource_type,
                    "Filtered resources for type"
                );
                filtered
            } else {
                tracing::debug!(
                    resource_type = %resource_type,
                    count = resources.len(),
                    "No selection filter for type, using all resources"
                );
                resources
            };

            if resources_to_process.is_empty() {
                tracing::debug!(resource_type = %resource_type, "Skipping resource type (no resources to process)");
                continue;
            }

            tracing::info!(
                count = resources_to_process.len(),
                resource_type = %resource_type,
                "Processing resources for type"
            );

            // Generate files based on file split rule
            match config.file_split_rule.as_str() {
                "single" => {
                    tracing::debug!(resource_type = %resource_type, "Generating single file for type");
                    let file_path = Self::generate_single_file(
                        &resources_to_process,
                        &template_info,
                        config,
                        output_path,
                    )
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to generate single file for type '{}'",
                            resource_type
                        )
                    })?;
                    tracing::info!(file = %file_path, "Generated file");
                    generated_files.push(file_path);
                }
                "by_resource_type" => {
                    tracing::debug!(resource_type = %resource_type, "Generating file by resource type");
                    let file_path = Self::generate_by_resource_type(
                        &resources_to_process,
                        &template_info,
                        config,
                        output_path,
                    )
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to generate file by resource type for type '{}'",
                            resource_type
                        )
                    })?;
                    tracing::info!(file = %file_path, "Generated file");
                    generated_files.push(file_path);
                }
                "by_resource_name" => {
                    tracing::debug!(resource_type = %resource_type, "Generating files by resource name for type");
                    let files = Self::generate_by_resource_name(
                        &resources_to_process,
                        &template_info,
                        config,
                        output_path,
                    )
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to generate files by resource name for type '{}'",
                            resource_type
                        )
                    })?;
                    tracing::info!(
                        count = files.len(),
                        resource_type = %resource_type,
                        "Generated files for type"
                    );
                    generated_files.extend(files);
                }
                _ => {
                    // Default to single file
                    tracing::warn!(
                        file_split_rule = %config.file_split_rule,
                        "Unknown file split rule, defaulting to single file"
                    );
                    let file_path = Self::generate_single_file(
                        &resources_to_process,
                        &template_info,
                        config,
                        output_path,
                    )
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to generate single file for type '{}'",
                            resource_type
                        )
                    })?;
                    tracing::info!(file = %file_path, "Generated file");
                    generated_files.push(file_path);
                }
            }
        }

        // Generate README if requested
        if config.generate_readme {
            tracing::debug!("Generating README");
            let readme_path = Self::generate_readme(config, output_path, &generated_files)
                .await
                .with_context(|| "Failed to generate README")?;
            tracing::info!(file = %readme_path, "Generated README");
            generated_files.push(readme_path);
        }

        tracing::info!(count = generated_files.len(), "Generation complete");
        if generated_files.is_empty() {
            return Err(anyhow::anyhow!(
                "No files were generated. This may be because:\n\
                1. No resources were found in the scan data\n\
                2. All resources were filtered out by selection\n\
                3. Template files could not be loaded\n\
                Please check the scan data and ensure resources exist."
            ));
        }

        Ok(generated_files)
    }

    /// プロバイダー名からテンプレート一覧を取得する
    ///
    /// 各プロバイダーの `CloudProviderScanner::get_templates()` に委譲する。
    fn get_templates_for_provider(provider: &str) -> Vec<ResourceTemplate> {
        use crate::infra::aws::provider::AwsProvider;
        use crate::infra::azure::provider::AzureProvider;
        use crate::infra::provider_trait::CloudProviderScanner;

        match provider {
            "aws" => AwsProvider.get_templates(),
            "azure" => AzureProvider.get_templates(),
            _ => vec![],
        }
    }

    async fn generate_single_file(
        resources: &[Value],
        template_info: &ResourceTemplate,
        config: &GenerationConfig,
        output_path: &Path,
    ) -> Result<String> {
        tracing::debug!(
            count = resources.len(),
            "Generating single file for resources"
        );
        let mut content = String::new();

        for (idx, resource) in resources.iter().enumerate() {
            tracing::debug!(
                index = idx + 1,
                total = resources.len(),
                "Rendering resource"
            );
            let rendered = Self::render_resource(resource, template_info, config)
                .await
                .with_context(|| {
                    format!(
                        "Failed to render resource {} of type '{}'",
                        idx + 1,
                        template_info.resource_type
                    )
                })?;
            content.push_str(&rendered);
            content.push_str("\n\n");
        }

        let file_name = format!("{}.tf", template_info.resource_type);
        let file_path = output_path.join(&file_name);

        tracing::debug!(file_path = ?file_path, bytes = content.len(), "Writing file");
        fs::write(&file_path, content)
            .with_context(|| format!("Failed to write file: {:?}", file_path))?;

        // Verify file was written
        if !file_path.exists() {
            return Err(anyhow::anyhow!("File was not created: {:?}", file_path));
        }
        let metadata = fs::metadata(&file_path)?;
        tracing::info!(file_path = ?file_path, bytes = metadata.len(), "File written successfully");

        Ok(file_name)
    }

    async fn generate_by_resource_type(
        resources: &[Value],
        template_info: &ResourceTemplate,
        config: &GenerationConfig,
        output_path: &Path,
    ) -> Result<String> {
        // Same as single file for now
        Self::generate_single_file(resources, template_info, config, output_path).await
    }

    async fn generate_by_resource_name(
        resources: &[Value],
        template_info: &ResourceTemplate,
        config: &GenerationConfig,
        output_path: &Path,
    ) -> Result<Vec<String>> {
        let mut files = Vec::new();

        for resource in resources {
            let rendered = Self::render_resource(resource, template_info, config).await?;

            // Get resource name for file name
            let resource_name = Self::get_resource_name(resource, template_info.resource_type)?;
            let file_name = format!(
                "{}_{}.tf",
                template_info.resource_type,
                NamingGenerator::apply_naming_convention(&resource_name, &config.naming_convention)
            );
            let file_path = output_path.join(&file_name);

            fs::write(&file_path, rendered)
                .with_context(|| format!("Failed to write file: {:?}", file_path))?;

            files.push(file_name);
        }

        Ok(files)
    }

    async fn render_resource(
        resource: &Value,
        template_info: &ResourceTemplate,
        config: &GenerationConfig,
    ) -> Result<String> {
        // Get resource name for Terraform resource identifier
        let resource_name = Self::get_resource_name(resource, template_info.resource_type)?;
        let terraform_resource_name =
            NamingGenerator::apply_naming_convention(&resource_name, &config.naming_convention);

        // Prepare context for template
        let mut context = serde_json::Map::new();
        context.insert(
            "resource_name".to_string(),
            Value::String(terraform_resource_name),
        );

        // Add resource data using the context keys expected by each Jinja2 template
        match template_info.resource_type {
            "users" => {
                context.insert("user".to_string(), resource.clone());
            }
            "groups" => {
                context.insert("group".to_string(), resource.clone());
            }
            "roles" => {
                context.insert("role".to_string(), resource.clone());
            }
            "policies" => {
                context.insert("policy".to_string(), resource.clone());
            }
            "buckets" => {
                context.insert("bucket".to_string(), resource.clone());
            }
            "bucket_policies" => {
                context.insert("policy".to_string(), resource.clone());
            }
            "lifecycle_rules" => {
                context.insert("rule".to_string(), resource.clone());
            }
            "instances" => {
                context.insert("instance".to_string(), resource.clone());
            }
            "vpcs" => {
                context.insert("vpc".to_string(), resource.clone());
            }
            "subnets" => {
                context.insert("subnet".to_string(), resource.clone());
            }
            "route_tables" => {
                context.insert("rt".to_string(), resource.clone());
            }
            "security_groups" => {
                context.insert("sg".to_string(), resource.clone());
            }
            "network_acls" => {
                context.insert("nacl".to_string(), resource.clone());
            }
            "functions" => {
                context.insert("function".to_string(), resource.clone());
            }
            "lambda_layers" => {
                context.insert("layer".to_string(), resource.clone());
            }
            "dynamodb_tables" => {
                context.insert("table".to_string(), resource.clone());
            }
            "internet_gateways" => {
                context.insert("igw".to_string(), resource.clone());
            }
            "nat_gateways" => {
                context.insert("nat_gw".to_string(), resource.clone());
            }
            "elastic_ips" => {
                context.insert("eip".to_string(), resource.clone());
            }
            "load_balancers" => {
                context.insert("lb".to_string(), resource.clone());
            }
            "target_groups" => {
                context.insert("tg".to_string(), resource.clone());
            }
            "listeners" => {
                context.insert("listener".to_string(), resource.clone());
            }
            "cloudwatch_alarms" => {
                context.insert("alarm".to_string(), resource.clone());
            }
            "sns_topics" => {
                context.insert("topic".to_string(), resource.clone());
            }
            "sns_subscriptions" => {
                context.insert("subscription".to_string(), resource.clone());
            }
            "db_instances" => {
                context.insert("db".to_string(), resource.clone());
            }
            "db_subnet_groups" => {
                context.insert("group".to_string(), resource.clone());
            }
            "db_parameter_groups" => {
                context.insert("pg".to_string(), resource.clone());
            }
            "role_definitions" => {
                context.insert("role_definition".to_string(), resource.clone());
            }
            "role_assignments" => {
                context.insert("role_assignment".to_string(), resource.clone());
            }
            "virtual_machines" => {
                context.insert("vm".to_string(), resource.clone());
            }
            "virtual_networks" => {
                context.insert("vnet".to_string(), resource.clone());
            }
            "network_security_groups" => {
                context.insert("nsg".to_string(), resource.clone());
            }
            "storage_accounts" => {
                context.insert("sa".to_string(), resource.clone());
            }
            "sql_databases" => {
                context.insert("db".to_string(), resource.clone());
            }
            _ => {
                context.insert("resource".to_string(), resource.clone());
            }
        }

        let context_value = Value::Object(context);

        // Render template
        tracing::debug!(template_path = %template_info.template_path, "Rendering template");
        let rendered =
            TemplateManager::render_template(template_info.template_path, &context_value)
                .await
                .with_context(|| {
                    format!("Failed to render template: {}", template_info.template_path)
                })?;
        tracing::debug!(bytes = rendered.len(), "Template rendered successfully");
        Ok(rendered)
    }

    fn get_resource_name(resource: &Value, resource_type: &str) -> Result<String> {
        fn str_field(obj: &Value, key: &str) -> Option<String> {
            obj.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
        }

        fn last_path_segment(s: &str) -> String {
            s.rsplit('/').next().unwrap_or(s).to_string()
        }

        match resource_type {
            "users" => Ok(str_field(resource, "user_name")
                .ok_or_else(|| anyhow::anyhow!("Missing user_name"))?),
            "groups" => Ok(str_field(resource, "group_name")
                .ok_or_else(|| anyhow::anyhow!("Missing group_name"))?),
            "roles" => Ok(str_field(resource, "role_name")
                .ok_or_else(|| anyhow::anyhow!("Missing role_name"))?),
            "policies" => Ok(str_field(resource, "policy_name")
                .ok_or_else(|| anyhow::anyhow!("Missing policy_name"))?),
            "buckets" => Ok(str_field(resource, "name")
                .ok_or_else(|| anyhow::anyhow!("Missing bucket name"))?),
            "bucket_policies" => Ok(str_field(resource, "bucket")
                .ok_or_else(|| anyhow::anyhow!("Missing bucket (bucket policy target)"))?),
            "lifecycle_rules" => {
                let bucket = str_field(resource, "bucket")
                    .ok_or_else(|| anyhow::anyhow!("Missing bucket for lifecycle rule"))?;
                let id = str_field(resource, "id")
                    .ok_or_else(|| anyhow::anyhow!("Missing id for lifecycle rule"))?;
                Ok(format!("{bucket}_{id}"))
            }
            "instances" => Ok(str_field(resource, "instance_id")
                .ok_or_else(|| anyhow::anyhow!("Missing instance_id"))?),
            "vpcs" => {
                Ok(str_field(resource, "vpc_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing vpc_id"))?)
            }
            "subnets" => Ok(str_field(resource, "subnet_id")
                .ok_or_else(|| anyhow::anyhow!("Missing subnet_id"))?),
            "route_tables" => Ok(str_field(resource, "route_table_id")
                .ok_or_else(|| anyhow::anyhow!("Missing route_table_id"))?),
            "security_groups" => {
                if let Some(id) = str_field(resource, "group_id") {
                    Ok(id)
                } else if let Some(name) = str_field(resource, "group_name") {
                    Ok(name)
                } else {
                    Err(anyhow::anyhow!(
                        "Missing group_id or group_name for security group"
                    ))
                }
            }
            "network_acls" => Ok(str_field(resource, "network_acl_id")
                .ok_or_else(|| anyhow::anyhow!("Missing network_acl_id"))?),
            "db_instances" => Ok(str_field(resource, "db_instance_identifier")
                .ok_or_else(|| anyhow::anyhow!("Missing db_instance_identifier"))?),
            "db_subnet_groups" => Ok(str_field(resource, "db_subnet_group_name")
                .ok_or_else(|| anyhow::anyhow!("Missing db_subnet_group_name"))?),
            "db_parameter_groups" => Ok(str_field(resource, "db_parameter_group_name")
                .ok_or_else(|| anyhow::anyhow!("Missing db_parameter_group_name"))?),
            "role_definitions" => {
                if let Some(n) = str_field(resource, "role_name") {
                    Ok(n)
                } else if let Some(id) = str_field(resource, "role_definition_id") {
                    Ok(last_path_segment(&id))
                } else {
                    Err(anyhow::anyhow!(
                        "Missing role_name or role_definition_id for role definition"
                    ))
                }
            }
            "role_assignments" => {
                if let Some(id) = str_field(resource, "assignment_id") {
                    return Ok(if id.contains('/') {
                        last_path_segment(&id)
                    } else {
                        id
                    });
                }
                let principal = str_field(resource, "principal_id").ok_or_else(|| {
                    anyhow::anyhow!("Missing assignment_id or principal_id for role assignment")
                })?;
                let scope = str_field(resource, "scope").unwrap_or_default();
                let scope_suffix = if scope.contains('/') {
                    last_path_segment(&scope)
                } else {
                    scope
                };
                Ok(format!("{principal}_{scope_suffix}"))
            }
            "virtual_machines"
            | "virtual_networks"
            | "network_security_groups"
            | "storage_accounts" => {
                if let Some(n) = str_field(resource, "name") {
                    Ok(n)
                } else if let Some(id) = str_field(resource, "id") {
                    Ok(last_path_segment(&id))
                } else {
                    Err(anyhow::anyhow!("Missing name or id"))
                }
            }
            "sql_databases" => {
                let db_name = str_field(resource, "name")
                    .ok_or_else(|| anyhow::anyhow!("Missing name for SQL database"))?;
                if let Some(server) = str_field(resource, "server_name") {
                    Ok(format!("{server}_{db_name}"))
                } else {
                    Ok(db_name)
                }
            }
            _ => {
                if let Some(name) = resource.get("name").and_then(|v| v.as_str()) {
                    Ok(name.to_string())
                } else if let Some(name) = resource.get("display_name").and_then(|v| v.as_str()) {
                    Ok(name.to_string())
                } else {
                    Err(anyhow::anyhow!("Cannot determine resource name"))
                }
            }
        }
    }

    async fn generate_readme(
        _config: &GenerationConfig,
        output_path: &Path,
        files: &[String],
    ) -> Result<String> {
        let mut readme = String::new();
        readme.push_str("# Terraform Code Generation\n\n");
        readme.push_str("This directory contains Terraform code generated by TFKosmos.\n\n");
        readme.push_str("## Generated Files\n\n");

        for file in files {
            readme.push_str(&format!("- {}\n", file));
        }

        readme.push_str("\n## Usage\n\n");
        readme.push_str("1. Review the generated Terraform files\n");
        readme.push_str("2. Run `terraform init` to initialize the Terraform working directory\n");
        readme.push_str("3. Run `terraform plan` to review the changes\n");
        readme.push_str("4. Run `terraform apply` to apply the changes\n\n");
        readme.push_str("## Import Script\n\n");
        readme.push_str(
            "Use the generated import script to import existing resources into Terraform state.\n",
        );

        let readme_path = output_path.join("README.md");
        fs::write(&readme_path, readme)
            .with_context(|| format!("Failed to write README: {:?}", readme_path))?;

        Ok("README.md".to_string())
    }

    pub async fn generate_import_script(
        scan_data: &Value,
        config: &GenerationConfig,
        selected_resources: &HashMap<String, Vec<Value>>,
        output_path: &Path,
    ) -> Result<Option<String>> {
        let provider = scan_data
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("aws");

        let mut import_commands = Vec::new();

        // Process each resource type
        let templates = Self::get_templates_for_provider(provider);
        for template_info in templates {
            let resource_type = template_info.resource_type;

            let resources = scan_data
                .get(resource_type)
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if resources.is_empty() {
                continue;
            }

            // Filter by selected resources if provided
            // If selected_resources is empty or doesn't contain this resource type, use all resources
            let resources_to_process = if selected_resources.is_empty() {
                tracing::debug!(count = resources.len(), resource_type = %resource_type, "No selection filter provided, using all resources for type");
                resources
            } else if let Some(selected) = selected_resources.get(resource_type) {
                tracing::debug!(
                    resource_type = %resource_type,
                    count = selected.len(),
                    "Filtering resources for type"
                );
                if selected.is_empty() {
                    tracing::debug!(resource_type = %resource_type, "Skipping resource type (empty selection)");
                    continue;
                }

                // Extract selected IDs (handle both string IDs and object IDs)
                let selected_ids: Vec<String> = selected
                    .iter()
                    .filter_map(|s| {
                        // If it's a string, use it directly
                        if let Some(id_str) = s.as_str() {
                            Some(id_str.to_string())
                        } else if let Some(obj) = s.as_object() {
                            // If it's an object, try to extract ID from common fields
                            obj.get("user_name")
                                .or_else(|| obj.get("group_name"))
                                .or_else(|| obj.get("role_name"))
                                .or_else(|| obj.get("arn"))
                                .or_else(|| obj.get("id"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                tracing::debug!(
                    count = selected_ids.len(),
                    ids = ?selected_ids,
                    "Extracted selected IDs"
                );

                // Filter resources that match selected IDs
                let filtered: Vec<_> = resources
                    .iter()
                    .filter(|r| {
                        // Get resource identifier based on resource type
                        let resource_id = match resource_type {
                            "users" => r.get("user_name").and_then(|v| v.as_str()),
                            "groups" => r.get("group_name").and_then(|v| v.as_str()),
                            "roles" => r.get("role_name").and_then(|v| v.as_str()),
                            "policies" => r
                                .get("arn")
                                .or_else(|| r.get("policy_name"))
                                .and_then(|v| v.as_str()),
                            _ => r
                                .get("arn")
                                .or_else(|| r.get("id"))
                                .or_else(|| r.get("name"))
                                .and_then(|v| v.as_str()),
                        };

                        if let Some(id) = resource_id {
                            selected_ids.contains(&id.to_string())
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();

                tracing::debug!(
                    count = filtered.len(),
                    resource_type = %resource_type,
                    "Filtered resources for type"
                );
                filtered
            } else {
                tracing::debug!(
                    resource_type = %resource_type,
                    count = resources.len(),
                    "No selection filter for type, using all resources"
                );
                resources
            };

            tracing::info!(
                count = resources_to_process.len(),
                resource_type = %resource_type,
                "Processing resources for type"
            );
            for resource in resources_to_process {
                match Self::generate_import_command(&resource, resource_type, provider) {
                    Ok(import_cmd) => {
                        tracing::debug!(import_cmd = %import_cmd, "Generated import command");
                        import_commands.push(import_cmd);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to generate import command for resource");
                    }
                }
            }
        }

        tracing::info!(
            count = import_commands.len(),
            "Total import commands generated"
        );
        if import_commands.is_empty() {
            tracing::debug!("No import commands generated, returning None");
            return Ok(None);
        }

        // Generate import script
        let script_content = match config.import_script_format.as_str() {
            "sh" => Self::generate_sh_import_script(&import_commands),
            "ps1" => Self::generate_ps1_import_script(&import_commands),
            _ => Self::generate_sh_import_script(&import_commands),
        };

        let script_name = match config.import_script_format.as_str() {
            "ps1" => "import.ps1",
            _ => "import.sh",
        };

        let script_path = output_path.join(script_name);
        tracing::debug!(script_path = ?script_path, bytes = script_content.len(), "Writing import script");
        fs::write(&script_path, script_content)
            .with_context(|| format!("Failed to write import script: {:?}", script_path))?;

        // Verify file was written
        if !script_path.exists() {
            return Err(anyhow::anyhow!(
                "Import script was not created: {:?}",
                script_path
            ));
        }
        let metadata = fs::metadata(&script_path)?;
        tracing::info!(script_path = ?script_path, bytes = metadata.len(), "Import script written successfully");

        // Make script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
            tracing::debug!("Import script made executable");
        }

        Ok(Some(script_name.to_string()))
    }

    fn generate_import_command(
        resource: &Value,
        resource_type: &str,
        provider: &str,
    ) -> Result<String> {
        let resource_name = Self::get_resource_name(resource, resource_type)?;
        let terraform_resource_name = NamingGenerator::to_snake_case(&resource_name);

        match (provider, resource_type) {
            ("aws", "users") => {
                let arn = resource
                    .get("arn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing ARN"))?;
                Ok(format!(
                    "terraform import aws_iam_user.{} {}",
                    terraform_resource_name, arn
                ))
            }
            ("aws", "groups") => {
                let arn = resource
                    .get("arn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing ARN"))?;
                Ok(format!(
                    "terraform import aws_iam_group.{} {}",
                    terraform_resource_name, arn
                ))
            }
            ("aws", "roles") => {
                let arn = resource
                    .get("arn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing ARN"))?;
                Ok(format!(
                    "terraform import aws_iam_role.{} {}",
                    terraform_resource_name, arn
                ))
            }
            ("aws", "policies") => {
                let arn = resource
                    .get("arn")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing ARN"))?;
                Ok(format!(
                    "terraform import aws_iam_policy.{} {}",
                    terraform_resource_name, arn
                ))
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported provider/resource type combination"
            )),
        }
    }

    fn generated_by_header() -> &'static str {
        #[cfg(not(feature = "pro"))]
        {
            "# Generated by TFKosmos Community Edition"
        }
        #[cfg(feature = "pro")]
        {
            "# Generated by TFKosmos"
        }
    }

    fn generate_sh_import_script(commands: &[String]) -> String {
        let mut script = String::new();
        script.push_str("#!/bin/bash\n");
        script.push_str("# Terraform import script\n");
        script.push_str(Self::generated_by_header());
        script.push_str("\n\n");
        script.push_str("set -e\n\n");

        for cmd in commands {
            script.push_str(&format!("{}\n", cmd));
        }

        script
    }

    fn generate_ps1_import_script(commands: &[String]) -> String {
        let mut script = String::new();
        script.push_str("# Terraform import script\n");
        script.push_str(Self::generated_by_header());
        script.push_str("\n\n");
        script.push_str("$ErrorActionPreference = \"Stop\"\n\n");

        for cmd in commands {
            script.push_str(&format!("{}\n", cmd));
        }

        script
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::TempDir;

    // ========================================
    // get_templates_for_provider のテスト
    // ========================================

    #[test]
    fn test_get_templates_for_aws() {
        let templates = TerraformGenerator::get_templates_for_provider("aws");
        assert_eq!(templates.len(), 28);

        let template_types: Vec<&str> = templates.iter().map(|t| t.resource_type).collect();
        // IAM
        assert!(template_types.contains(&"users"));
        assert!(template_types.contains(&"groups"));
        assert!(template_types.contains(&"roles"));
        assert!(template_types.contains(&"policies"));
        // S3
        assert!(template_types.contains(&"buckets"));
        assert!(template_types.contains(&"bucket_policies"));
        assert!(template_types.contains(&"lifecycle_rules"));
        // EC2
        assert!(template_types.contains(&"instances"));
        // VPC
        assert!(template_types.contains(&"vpcs"));
        assert!(template_types.contains(&"subnets"));
        assert!(template_types.contains(&"route_tables"));
        assert!(template_types.contains(&"security_groups"));
        assert!(template_types.contains(&"network_acls"));
        assert!(template_types.contains(&"internet_gateways"));
        assert!(template_types.contains(&"nat_gateways"));
        assert!(template_types.contains(&"elastic_ips"));
        // RDS
        assert!(template_types.contains(&"db_instances"));
        assert!(template_types.contains(&"db_subnet_groups"));
        assert!(template_types.contains(&"db_parameter_groups"));
        // Lambda
        assert!(template_types.contains(&"functions"));
        assert!(template_types.contains(&"lambda_layers"));
        // DynamoDB
        assert!(template_types.contains(&"dynamodb_tables"));
        // ELB/ALB
        assert!(template_types.contains(&"load_balancers"));
        assert!(template_types.contains(&"lb_listeners"));
        assert!(template_types.contains(&"lb_target_groups"));
        // CloudWatch/SNS
        assert!(template_types.contains(&"cloudwatch_alarms"));
        assert!(template_types.contains(&"sns_topics"));
        assert!(template_types.contains(&"sns_subscriptions"));
    }

    #[test]
    fn test_get_templates_for_azure() {
        let templates = TerraformGenerator::get_templates_for_provider("azure");
        assert_eq!(templates.len(), 9);

        let template_types: Vec<&str> = templates.iter().map(|t| t.resource_type).collect();
        // IAM
        assert!(template_types.contains(&"role_definitions"));
        assert!(template_types.contains(&"role_assignments"));
        // Compute
        assert!(template_types.contains(&"virtual_machines"));
        // Network
        assert!(template_types.contains(&"virtual_networks"));
        assert!(template_types.contains(&"network_security_groups"));
        // Storage
        assert!(template_types.contains(&"storage_accounts"));
        // SQL
        assert!(template_types.contains(&"sql_databases"));
        // App Service
        assert!(template_types.contains(&"app_services"));
        assert!(template_types.contains(&"function_apps"));
    }

    #[test]
    fn test_get_templates_for_unknown_provider() {
        let templates = TerraformGenerator::get_templates_for_provider("unknown");
        assert_eq!(templates.len(), 0);
    }

    // ========================================
    // get_resource_name のテスト
    // ========================================

    #[test]
    fn test_get_resource_name_user() {
        let resource = json!({
            "user_name": "test-user",
            "arn": "arn:aws:iam::123456789012:user/test-user"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "users");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-user");
    }

    #[test]
    fn test_get_resource_name_group() {
        let resource = json!({
            "group_name": "test-group",
            "arn": "arn:aws:iam::123456789012:group/test-group"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "groups");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-group");
    }

    #[test]
    fn test_get_resource_name_role() {
        let resource = json!({
            "role_name": "test-role",
            "arn": "arn:aws:iam::123456789012:role/test-role"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "roles");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-role");
    }

    #[test]
    fn test_get_resource_name_policy() {
        let resource = json!({
            "policy_name": "test-policy",
            "arn": "arn:aws:iam::123456789012:policy/test-policy"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "policies");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-policy");
    }

    #[test]
    fn test_get_resource_name_missing_field() {
        let resource = json!({
            "arn": "arn:aws:iam::123456789012:user/test-user"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "users");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing user_name"));
    }

    #[test]
    fn test_get_resource_name_generic() {
        let resource = json!({
            "name": "generic-resource"
        });

        let result = TerraformGenerator::get_resource_name(&resource, "unknown_type");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "generic-resource");
    }

    #[test]
    fn test_get_resource_name_ec2_instance() {
        let resource = json!({
            "instance_id": "i-0abc123",
            "ami_id": "ami-123",
            "instance_type": "t3.micro"
        });
        let result = TerraformGenerator::get_resource_name(&resource, "instances");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "i-0abc123");
    }

    #[test]
    fn test_get_resource_name_vpc() {
        let resource = json!({ "vpc_id": "vpc-12345", "cidr_block": "10.0.0.0/16" });
        let result = TerraformGenerator::get_resource_name(&resource, "vpcs");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "vpc-12345");
    }

    #[test]
    fn test_get_resource_name_lifecycle_rule() {
        let resource = json!({
            "bucket": "my-bucket",
            "id": "expire-old",
            "status": "Enabled"
        });
        let result = TerraformGenerator::get_resource_name(&resource, "lifecycle_rules");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-bucket_expire-old");
    }

    #[test]
    fn test_get_resource_name_sql_database_with_server() {
        let resource = json!({
            "name": "appdb",
            "server_name": "sqlsrv1"
        });
        let result = TerraformGenerator::get_resource_name(&resource, "sql_databases");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "sqlsrv1_appdb");
    }

    // ========================================
    // generate_import_command のテスト
    // ========================================

    #[test]
    fn test_generate_import_command_aws_user() {
        let resource = json!({
            "user_name": "test-user",
            "arn": "arn:aws:iam::123456789012:user/test-user"
        });

        let result = TerraformGenerator::generate_import_command(&resource, "users", "aws");
        assert!(result.is_ok());

        let import_cmd = result.unwrap();
        assert!(import_cmd.contains("terraform import"));
        assert!(import_cmd.contains("aws_iam_user.test_user"));
        assert!(import_cmd.contains("arn:aws:iam::123456789012:user/test-user"));
    }

    #[test]
    fn test_generate_import_command_aws_group() {
        let resource = json!({
            "group_name": "test-group",
            "arn": "arn:aws:iam::123456789012:group/test-group"
        });

        let result = TerraformGenerator::generate_import_command(&resource, "groups", "aws");
        assert!(result.is_ok());

        let import_cmd = result.unwrap();
        assert!(import_cmd.contains("terraform import"));
        assert!(import_cmd.contains("aws_iam_group.test_group"));
        assert!(import_cmd.contains("arn:aws:iam::123456789012:group/test-group"));
    }

    #[test]
    fn test_generate_import_command_aws_role() {
        let resource = json!({
            "role_name": "test-role",
            "arn": "arn:aws:iam::123456789012:role/test-role"
        });

        let result = TerraformGenerator::generate_import_command(&resource, "roles", "aws");
        assert!(result.is_ok());

        let import_cmd = result.unwrap();
        assert!(import_cmd.contains("terraform import"));
        assert!(import_cmd.contains("aws_iam_role.test_role"));
        assert!(import_cmd.contains("arn:aws:iam::123456789012:role/test-role"));
    }

    #[test]
    fn test_generate_import_command_aws_policy() {
        let resource = json!({
            "policy_name": "test-policy",
            "arn": "arn:aws:iam::123456789012:policy/test-policy"
        });

        let result = TerraformGenerator::generate_import_command(&resource, "policies", "aws");
        assert!(result.is_ok());

        let import_cmd = result.unwrap();
        assert!(import_cmd.contains("terraform import"));
        assert!(import_cmd.contains("aws_iam_policy.test_policy"));
        assert!(import_cmd.contains("arn:aws:iam::123456789012:policy/test-policy"));
    }

    #[test]
    fn test_generate_import_command_unsupported_provider() {
        let resource = json!({
            "user_name": "test-user",
            "arn": "arn:aws:iam::123456789012:user/test-user"
        });

        let result = TerraformGenerator::generate_import_command(&resource, "users", "gcp");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported provider/resource type combination"));
    }

    // ========================================
    // generate_sh_import_script のテスト
    // ========================================

    #[test]
    fn test_generate_sh_import_script() {
        let commands = vec![
            "terraform import aws_iam_user.test_user arn:aws:iam::123456789012:user/test-user"
                .to_string(),
            "terraform import aws_iam_group.test_group arn:aws:iam::123456789012:group/test-group"
                .to_string(),
        ];

        let script = TerraformGenerator::generate_sh_import_script(&commands);

        assert!(script.contains("#!/bin/bash"));
        assert!(script.contains("set -e"));
        assert!(script.contains("terraform import aws_iam_user.test_user"));
        assert!(script.contains("terraform import aws_iam_group.test_group"));
    }

    // ========================================
    // generate_ps1_import_script のテスト
    // ========================================

    #[test]
    fn test_generate_ps1_import_script() {
        let commands = vec![
            "terraform import aws_iam_user.test_user arn:aws:iam::123456789012:user/test-user"
                .to_string(),
            "terraform import aws_iam_group.test_group arn:aws:iam::123456789012:group/test-group"
                .to_string(),
        ];

        let script = TerraformGenerator::generate_ps1_import_script(&commands);

        assert!(script.contains("$ErrorActionPreference"));
        assert!(script.contains("terraform import aws_iam_user.test_user"));
        assert!(script.contains("terraform import aws_iam_group.test_group"));
    }

    // ========================================
    // generate_readme のテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_readme() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: true,
            selected_resources: HashMap::new(),
        };

        let files = vec!["users.tf".to_string(), "groups.tf".to_string()];

        let result = TerraformGenerator::generate_readme(&config, output_path, &files).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "README.md");

        let readme_path = output_path.join("README.md");
        assert!(readme_path.exists());

        let readme_content = std::fs::read_to_string(readme_path).unwrap();
        assert!(readme_content.contains("# Terraform Code Generation"));
        assert!(readme_content.contains("users.tf"));
        assert!(readme_content.contains("groups.tf"));
        assert!(readme_content.contains("terraform init"));
    }

    // ========================================
    // generate_import_script のテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_import_script_with_resources() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: true,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();

        let result = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            output_path,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().is_some());

        let script_name = result.unwrap().unwrap();
        assert_eq!(script_name, "import.sh");

        let script_path = output_path.join(&script_name);
        assert!(script_path.exists());

        let script_content = std::fs::read_to_string(script_path).unwrap();
        assert!(script_content.contains("#!/bin/bash"));
        assert!(script_content.contains("terraform import"));
        assert!(script_content.contains("aws_iam_user.test_user"));
    }

    #[tokio::test]
    async fn test_generate_import_script_ps1_format() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "ps1".to_string(),
            generate_readme: true,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();

        let result = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            output_path,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().is_some());

        let script_name = result.unwrap().unwrap();
        assert_eq!(script_name, "import.ps1");

        let script_path = output_path.join(&script_name);
        assert!(script_path.exists());

        let script_content = std::fs::read_to_string(script_path).unwrap();
        assert!(script_content.contains("$ErrorActionPreference"));
        assert!(script_content.contains("terraform import"));
    }

    #[tokio::test]
    async fn test_generate_import_script_no_resources() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let scan_data = json!({
            "provider": "aws",
            "users": []
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: true,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();

        let result = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            output_path,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ========================================
    // generate() with selected_resources フィルタリングのテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_with_selected_resources_filters_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "user-included",
                    "arn": "arn:aws:iam::123456789012:user/user-included",
                    "path": "/",
                    "tags": {}
                },
                {
                    "user_name": "user-excluded",
                    "arn": "arn:aws:iam::123456789012:user/user-excluded",
                    "path": "/",
                    "tags": {}
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        // Only select user-included
        let mut selected_resources: HashMap<String, Vec<Value>> = HashMap::new();
        selected_resources.insert(
            "users".to_string(),
            vec![Value::String("user-included".to_string())],
        );

        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("users.tf");
        assert!(tf_path.exists(), "users.tf should exist");
        let content = std::fs::read_to_string(&tf_path).unwrap();
        assert!(
            content.contains("user_included"),
            "user_included should be in output"
        );
        assert!(
            !content.contains("user_excluded"),
            "user_excluded should NOT be in output"
        );
    }

    #[tokio::test]
    async fn test_generate_with_empty_selection_for_type_skips_type() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user",
                    "path": "/",
                    "tags": {}
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        // Empty selection for "users" type → should skip generating users.tf
        let mut selected_resources: HashMap<String, Vec<Value>> = HashMap::new();
        selected_resources.insert("users".to_string(), vec![]);

        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        // Should fail because no files were generated
        assert!(
            result.is_err(),
            "Expected error when no resources selected, but got: {:?}",
            result.ok()
        );
        let tf_path = output_path.join("users.tf");
        assert!(
            !tf_path.exists(),
            "users.tf should NOT exist when selection is empty"
        );
    }

    // ========================================
    // generate() with file_split_rule のテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_with_split_by_resource_type() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user",
                    "path": "/",
                    "tags": {}
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "by_resource_type".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("users.tf");
        assert!(tf_path.exists(), "users.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_iam_user"),
            "Content should contain aws_iam_user"
        );
    }

    #[tokio::test]
    async fn test_generate_with_split_by_resource_name() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "alice",
                    "arn": "arn:aws:iam::123456789012:user/alice",
                    "path": "/",
                    "tags": {}
                },
                {
                    "user_name": "bob",
                    "arn": "arn:aws:iam::123456789012:user/bob",
                    "path": "/",
                    "tags": {}
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "by_resource_name".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        // Each user should have its own file
        let alice_path = output_path.join("users_alice.tf");
        let bob_path = output_path.join("users_bob.tf");
        assert!(alice_path.exists(), "users_alice.tf should exist");
        assert!(bob_path.exists(), "users_bob.tf should exist");
    }

    // ========================================
    // generate() 各AWSリソース型のテスト
    // ========================================

    #[tokio::test]
    async fn test_generate_aws_s3_buckets() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "buckets": [
                {
                    "name": "my-test-bucket",
                    "versioning": "Enabled",
                    "tags": {
                        "Environment": "test"
                    }
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("buckets.tf");
        assert!(tf_path.exists(), "buckets.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_s3_bucket"),
            "Content should contain aws_s3_bucket"
        );
        assert!(
            content.contains("my-test-bucket"),
            "Content should contain bucket name"
        );
    }

    #[tokio::test]
    async fn test_generate_aws_ec2_instances() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "instances": [
                {
                    "instance_id": "i-0abc123def456",
                    "ami_id": "ami-0abcdef1234567890",
                    "instance_type": "t3.micro",
                    "subnet_id": "subnet-12345678",
                    "key_name": "my-key-pair"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("instances.tf");
        assert!(tf_path.exists(), "instances.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_instance"),
            "Content should contain aws_instance"
        );
        assert!(
            content.contains("ami-0abcdef1234567890"),
            "Content should contain AMI ID"
        );
        assert!(
            content.contains("t3.micro"),
            "Content should contain instance type"
        );
    }

    #[tokio::test]
    async fn test_generate_aws_vpc() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "vpcs": [
                {
                    "vpc_id": "vpc-0abc12345",
                    "cidr_block": "10.0.0.0/16",
                    "enable_dns_support": true,
                    "enable_dns_hostnames": true,
                    "instance_tenancy": "default"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("vpcs.tf");
        assert!(tf_path.exists(), "vpcs.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_vpc"),
            "Content should contain aws_vpc"
        );
        assert!(
            content.contains("10.0.0.0/16"),
            "Content should contain CIDR block"
        );
    }

    #[tokio::test]
    async fn test_generate_aws_rds() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "db_instances": [
                {
                    "db_instance_identifier": "my-db-instance",
                    "db_instance_class": "db.t3.micro",
                    "engine": "mysql",
                    "engine_version": "8.0",
                    "allocated_storage": 20,
                    "storage_type": "gp2",
                    "master_username": "admin",
                    "multi_az": false,
                    "publicly_accessible": false,
                    "storage_encrypted": true
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("db_instances.tf");
        assert!(tf_path.exists(), "db_instances.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_db_instance"),
            "Content should contain aws_db_instance"
        );
        assert!(
            content.contains("my-db-instance"),
            "Content should contain DB identifier"
        );
        assert!(
            content.contains("mysql"),
            "Content should contain engine type"
        );
    }

    #[tokio::test]
    async fn test_generate_aws_lambda() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "functions": [
                {
                    "name": "my-lambda-function",
                    "function_name": "my-lambda-function",
                    "role": "arn:aws:iam::123456789012:role/lambda-role",
                    "runtime": "python3.11",
                    "handler": "index.handler",
                    "description": "My test lambda",
                    "memory_size": 128,
                    "timeout": 30
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("functions.tf");
        assert!(tf_path.exists(), "functions.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_lambda_function"),
            "Content should contain aws_lambda_function"
        );
        assert!(
            content.contains("my-lambda-function"),
            "Content should contain function name"
        );
        assert!(
            content.contains("python3.11"),
            "Content should contain runtime"
        );
    }

    #[tokio::test]
    async fn test_generate_aws_dynamodb() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "aws",
            "dynamodb_tables": [
                {
                    "name": "my-dynamodb-table",
                    "table_name": "my-dynamodb-table",
                    "billing_mode": "PAY_PER_REQUEST",
                    "key_schema": [
                        {
                            "attribute_name": "id",
                            "key_type": "HASH"
                        }
                    ],
                    "attribute_definitions": [
                        {
                            "attribute_name": "id",
                            "attribute_type": "S"
                        }
                    ]
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("dynamodb_tables.tf");
        assert!(tf_path.exists(), "dynamodb_tables.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("aws_dynamodb_table"),
            "Content should contain aws_dynamodb_table"
        );
        assert!(
            content.contains("my-dynamodb-table"),
            "Content should contain table name"
        );
        assert!(
            content.contains("PAY_PER_REQUEST"),
            "Content should contain billing mode"
        );
    }

    // ========================================
    // generate_import_script() 追加テスト
    // ========================================

    #[tokio::test]
    async fn test_generate_import_script_with_selected_resources() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "user-a",
                    "arn": "arn:aws:iam::123456789012:user/user-a"
                },
                {
                    "user_name": "user-b",
                    "arn": "arn:aws:iam::123456789012:user/user-b"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        // Only select user-a
        let mut selected_resources: HashMap<String, Vec<Value>> = HashMap::new();
        selected_resources.insert(
            "users".to_string(),
            vec![Value::String("user-a".to_string())],
        );

        let result = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            output_path,
        )
        .await;

        assert!(
            result.is_ok(),
            "generate_import_script failed: {:?}",
            result.err()
        );
        assert!(result.as_ref().unwrap().is_some());

        let script_name = result.unwrap().unwrap();
        let script_path = output_path.join(&script_name);
        let script_content = std::fs::read_to_string(script_path).unwrap();
        assert!(
            script_content.contains("aws_iam_user.user_a"),
            "Script should contain user-a import"
        );
        assert!(
            !script_content.contains("user_b"),
            "Script should NOT contain user-b import"
        );
    }

    #[tokio::test]
    async fn test_generate_import_script_default_format_is_sh() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path();

        let scan_data = json!({
            "provider": "aws",
            "users": [
                {
                    "user_name": "test-user",
                    "arn": "arn:aws:iam::123456789012:user/test-user"
                }
            ]
        });

        // Use an unknown format → should default to sh
        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "unknown_format".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result = TerraformGenerator::generate_import_script(
            &scan_data,
            &config,
            &selected_resources,
            output_path,
        )
        .await;

        assert!(
            result.is_ok(),
            "generate_import_script failed: {:?}",
            result.err()
        );
        let script_name = result.unwrap().unwrap();
        // Unknown format defaults to "import.sh"
        assert_eq!(script_name, "import.sh");

        let script_path = output_path.join(&script_name);
        let script_content = std::fs::read_to_string(script_path).unwrap();
        assert!(
            script_content.contains("#!/bin/bash"),
            "Default format should be bash script"
        );
    }

    #[tokio::test]
    async fn test_generate_azure_virtual_machines_linux_and_windows() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().to_path_buf();

        let scan_data = json!({
            "provider": "azure",
            "virtual_machines": [
                {
                    "id": "/subscriptions/x/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/linux-vm",
                    "name": "linux-vm",
                    "os_type": "Linux",
                    "location": "japaneast",
                    "resource_group": "rg",
                    "vm_size": "Standard_B2s",
                    "admin_username": "azureuser"
                },
                {
                    "id": "/subscriptions/x/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/win-vm",
                    "name": "win-vm",
                    "os_type": "Windows",
                    "location": "japaneast",
                    "resource_group": "rg",
                    "vm_size": "Standard_D2s_v5",
                    "admin_username": "azureuser"
                }
            ]
        });

        let config = GenerationConfig {
            output_path: output_path.to_str().unwrap().to_string(),
            file_split_rule: "single".to_string(),
            naming_convention: "snake_case".to_string(),
            import_script_format: "sh".to_string(),
            generate_readme: false,
            selected_resources: HashMap::new(),
        };

        let selected_resources = HashMap::new();
        let result =
            TerraformGenerator::generate(&scan_data, &config, &selected_resources, &output_path)
                .await;

        assert!(result.is_ok(), "generate failed: {:?}", result.err());
        let tf_path = output_path.join("virtual_machines.tf");
        assert!(tf_path.exists(), "virtual_machines.tf should exist");
        let content = std::fs::read_to_string(tf_path).unwrap();
        assert!(
            content.contains("azurerm_linux_virtual_machine"),
            "Linux VM should use azurerm_linux_virtual_machine"
        );
        assert!(
            content.contains("azurerm_windows_virtual_machine"),
            "Windows VM should use azurerm_windows_virtual_machine"
        );
        assert!(
            content.contains("admin_password"),
            "Windows VM block must include admin_password"
        );
    }
}
