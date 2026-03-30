//! AWS VPCリソーススキャナー

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

use crate::infra::aws::ec2_client_trait::Ec2ClientOps;
use crate::models::ScanConfig;

/// AWS VPCスキャナー（EC2 SDKを使用）
pub struct AwsVpcScanner<C: Ec2ClientOps> {
    config: ScanConfig,
    ec2_client: Arc<C>,
}

impl<C: Ec2ClientOps> AwsVpcScanner<C> {
    /// 本番用・テスト用共通：クライアントを指定してスキャナーを作成
    pub fn new(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            ec2_client: client,
        }
    }

    #[cfg(test)]
    pub fn new_with_client(config: ScanConfig, client: Arc<C>) -> Self {
        Self {
            config,
            ec2_client: client,
        }
    }

    /// VPCリソースをスキャンし結果をresultsに追加
    pub async fn scan_into(
        &self,
        results: &mut serde_json::Map<String, Value>,
        progress_callback: &(dyn Fn(u32, String) + Send + Sync),
        completed_targets: &AtomicUsize,
        total_targets: usize,
    ) -> Result<()> {
        let scan_targets = &self.config.scan_targets;

        // VPCs
        if scan_targets.get("vpcs").copied().unwrap_or(false) {
            debug!("VPCsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "VPCsのスキャン中...".to_string(),
            );
            let vpcs = self.scan_vpcs().await?;
            let count = vpcs.len();
            results.insert("vpcs".to_string(), Value::Array(vpcs));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("VPCsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("vpcs".to_string(), Value::Array(Vec::new()));
        }

        // Subnets
        if scan_targets.get("subnets").copied().unwrap_or(false) {
            debug!("Subnetsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "Subnetsのスキャン中...".to_string(),
            );
            let subnets = self.scan_subnets().await?;
            let count = subnets.len();
            results.insert("subnets".to_string(), Value::Array(subnets));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("Subnetsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("subnets".to_string(), Value::Array(Vec::new()));
        }

        // Route Tables
        if scan_targets.get("route_tables").copied().unwrap_or(false) {
            debug!("Route Tablesのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "Route Tablesのスキャン中...".to_string(),
            );
            let rts = self.scan_route_tables().await?;
            let count = rts.len();
            results.insert("route_tables".to_string(), Value::Array(rts));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("Route Tablesのスキャン完了: {}件", count),
            );
        } else {
            results.insert("route_tables".to_string(), Value::Array(Vec::new()));
        }

        // Security Groups
        if scan_targets
            .get("security_groups")
            .copied()
            .unwrap_or(false)
        {
            debug!("Security Groupsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                "Security Groupsのスキャン中...".to_string(),
            );
            let sgs = self.scan_security_groups().await?;
            let count = sgs.len();
            results.insert("security_groups".to_string(), Value::Array(sgs));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets) as u32,
                format!("Security Groupsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("security_groups".to_string(), Value::Array(Vec::new()));
        }

        // Network ACLs
        if scan_targets.get("network_acls").copied().unwrap_or(false) {
            debug!("Network ACLsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Network ACLsのスキャン中...".to_string(),
            );
            let nacls = self.scan_network_acls().await?;
            let count = nacls.len();
            results.insert("network_acls".to_string(), Value::Array(nacls));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Network ACLsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("network_acls".to_string(), Value::Array(Vec::new()));
        }

        // Internet Gateways
        if scan_targets
            .get("internet_gateways")
            .copied()
            .unwrap_or(false)
        {
            debug!("Internet Gatewaysのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Internet Gatewaysのスキャン中...".to_string(),
            );
            let igws = self.scan_internet_gateways().await?;
            let count = igws.len();
            results.insert("internet_gateways".to_string(), Value::Array(igws));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Internet Gatewaysのスキャン完了: {}件", count),
            );
        } else {
            results.insert("internet_gateways".to_string(), Value::Array(Vec::new()));
        }

        // NAT Gateways
        if scan_targets.get("nat_gateways").copied().unwrap_or(false) {
            debug!("NAT Gatewaysのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "NAT Gatewaysのスキャン中...".to_string(),
            );
            let nat_gws = self.scan_nat_gateways().await?;
            let count = nat_gws.len();
            results.insert("nat_gateways".to_string(), Value::Array(nat_gws));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("NAT Gatewaysのスキャン完了: {}件", count),
            );
        } else {
            results.insert("nat_gateways".to_string(), Value::Array(Vec::new()));
        }

        // Elastic IPs
        if scan_targets.get("elastic_ips").copied().unwrap_or(false) {
            debug!("Elastic IPsのスキャンを開始");
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                "Elastic IPsのスキャン中...".to_string(),
            );
            let eips = self.scan_elastic_ips().await?;
            let count = eips.len();
            results.insert("elastic_ips".to_string(), Value::Array(eips));
            completed_targets.fetch_add(1, Ordering::Relaxed);
            progress_callback(
                (completed_targets.load(Ordering::Relaxed) * 100 / total_targets.max(1)) as u32,
                format!("Elastic IPsのスキャン完了: {}件", count),
            );
        } else {
            results.insert("elastic_ips".to_string(), Value::Array(Vec::new()));
        }

        Ok(())
    }

    async fn scan_vpcs(&self) -> Result<Vec<Value>> {
        let vpcs_info = self.ec2_client.describe_vpcs().await?;
        let vpcs: Vec<Value> = vpcs_info
            .into_iter()
            .map(|v| {
                let mut j = json!({
                    "vpc_id": v.vpc_id,
                    "enable_dns_support": v.enable_dns_support,
                    "enable_dns_hostnames": v.enable_dns_hostnames,
                });
                if let Some(cidr) = &v.cidr_block {
                    j["cidr_block"] = json!(cidr);
                }
                if let Some(state) = &v.state {
                    j["state"] = json!(state);
                }
                if let Some(tenancy) = &v.instance_tenancy {
                    j["instance_tenancy"] = json!(tenancy);
                }
                if !v.tags.is_empty() {
                    j["tags"] = json!(v.tags);
                }
                j
            })
            .collect();
        info!(count = vpcs.len(), "VPCスキャン完了");
        Ok(vpcs)
    }

    async fn scan_subnets(&self) -> Result<Vec<Value>> {
        let subnets_info = self.ec2_client.describe_subnets().await?;
        let subnets: Vec<Value> = subnets_info
            .into_iter()
            .map(|s| {
                let mut j = json!({
                    "subnet_id": s.subnet_id,
                    "vpc_id": s.vpc_id,
                    "map_public_ip_on_launch": s.map_public_ip_on_launch,
                });
                if let Some(cidr) = &s.cidr_block {
                    j["cidr_block"] = json!(cidr);
                }
                if let Some(az) = &s.availability_zone {
                    j["availability_zone"] = json!(az);
                }
                if !s.tags.is_empty() {
                    j["tags"] = json!(s.tags);
                }
                j
            })
            .collect();
        Ok(subnets)
    }

    async fn scan_route_tables(&self) -> Result<Vec<Value>> {
        let rts_info = self.ec2_client.describe_route_tables().await?;
        let rts: Vec<Value> = rts_info
            .into_iter()
            .map(|rt| {
                let routes: Vec<Value> = rt
                    .routes
                    .iter()
                    .map(|r| {
                        let mut j = json!({});
                        if let Some(v) = &r.destination_cidr_block {
                            j["destination_cidr_block"] = json!(v);
                        }
                        if let Some(v) = &r.gateway_id {
                            j["gateway_id"] = json!(v);
                        }
                        if let Some(v) = &r.nat_gateway_id {
                            j["nat_gateway_id"] = json!(v);
                        }
                        j
                    })
                    .collect();

                let associations: Vec<Value> = rt
                    .associations
                    .iter()
                    .map(|a| {
                        json!({
                            "route_table_association_id": a.route_table_association_id,
                            "subnet_id": a.subnet_id,
                            "main": a.main,
                        })
                    })
                    .collect();

                let mut j = json!({
                    "route_table_id": rt.route_table_id,
                    "vpc_id": rt.vpc_id,
                    "routes": routes,
                    "associations": associations,
                });
                if !rt.tags.is_empty() {
                    j["tags"] = json!(rt.tags);
                }
                j
            })
            .collect();
        Ok(rts)
    }

    async fn scan_security_groups(&self) -> Result<Vec<Value>> {
        let sgs_info = self.ec2_client.describe_security_groups().await?;
        let sgs: Vec<Value> = sgs_info
            .into_iter()
            .map(|sg| {
                let ingress: Vec<Value> = sg
                    .ingress_rules
                    .iter()
                    .map(|r| {
                        let mut j = json!({
                            "protocol": r.protocol,
                        });
                        if let Some(v) = r.from_port {
                            j["from_port"] = json!(v);
                        }
                        if let Some(v) = r.to_port {
                            j["to_port"] = json!(v);
                        }
                        if !r.cidr_blocks.is_empty() {
                            j["cidr_blocks"] = json!(r.cidr_blocks);
                        }
                        if let Some(v) = &r.source_security_group_id {
                            j["source_security_group_id"] = json!(v);
                        }
                        if let Some(v) = &r.description {
                            j["description"] = json!(v);
                        }
                        j
                    })
                    .collect();

                let egress: Vec<Value> = sg
                    .egress_rules
                    .iter()
                    .map(|r| {
                        let mut j = json!({ "protocol": r.protocol });
                        if let Some(v) = r.from_port {
                            j["from_port"] = json!(v);
                        }
                        if let Some(v) = r.to_port {
                            j["to_port"] = json!(v);
                        }
                        if !r.cidr_blocks.is_empty() {
                            j["cidr_blocks"] = json!(r.cidr_blocks);
                        }
                        j
                    })
                    .collect();

                let mut j = json!({
                    "group_id": sg.group_id,
                    "group_name": sg.group_name,
                    "vpc_id": sg.vpc_id,
                    "ingress_rules": ingress,
                    "egress_rules": egress,
                });
                if let Some(v) = &sg.description {
                    j["description"] = json!(v);
                }
                if !sg.tags.is_empty() {
                    j["tags"] = json!(sg.tags);
                }
                j
            })
            .collect();
        Ok(sgs)
    }

    async fn scan_network_acls(&self) -> Result<Vec<Value>> {
        let nacls_info = self.ec2_client.describe_network_acls().await?;
        let nacls: Vec<Value> = nacls_info
            .into_iter()
            .map(|nacl| {
                let entries: Vec<Value> = nacl
                    .entries
                    .iter()
                    .map(|e| {
                        let mut j = json!({
                            "rule_number": e.rule_number,
                            "protocol": e.protocol,
                            "rule_action": e.rule_action,
                            "egress": e.egress,
                        });
                        if let Some(v) = &e.cidr_block {
                            j["cidr_block"] = json!(v);
                        }
                        if let Some(v) = e.from_port {
                            j["from_port"] = json!(v);
                        }
                        if let Some(v) = e.to_port {
                            j["to_port"] = json!(v);
                        }
                        j
                    })
                    .collect();

                let mut j = json!({
                    "network_acl_id": nacl.network_acl_id,
                    "vpc_id": nacl.vpc_id,
                    "is_default": nacl.is_default,
                    "entries": entries,
                });
                if !nacl.associations.is_empty() {
                    j["associations"] = json!(nacl.associations);
                }
                if !nacl.tags.is_empty() {
                    j["tags"] = json!(nacl.tags);
                }
                j
            })
            .collect();
        Ok(nacls)
    }

    async fn scan_internet_gateways(&self) -> Result<Vec<Value>> {
        let igws_info = self.ec2_client.describe_internet_gateways().await?;
        let igws: Vec<Value> = igws_info
            .into_iter()
            .map(|igw| {
                let attachments: Vec<Value> = igw
                    .attachments
                    .iter()
                    .map(|a| {
                        json!({
                            "vpc_id": a.vpc_id,
                            "state": a.state,
                        })
                    })
                    .collect();

                let mut j = json!({
                    "internet_gateway_id": igw.internet_gateway_id,
                });
                if !attachments.is_empty() {
                    j["attachments"] = json!(attachments);
                }
                if !igw.tags.is_empty() {
                    j["tags"] = json!(igw.tags);
                }
                j
            })
            .collect();
        info!(count = igws.len(), "Internet Gatewaysスキャン完了");
        Ok(igws)
    }

    async fn scan_nat_gateways(&self) -> Result<Vec<Value>> {
        let nat_gws_info = self.ec2_client.describe_nat_gateways().await?;
        let nat_gws: Vec<Value> = nat_gws_info
            .into_iter()
            .map(|ngw| {
                let mut j = json!({
                    "nat_gateway_id": ngw.nat_gateway_id,
                });
                if let Some(v) = &ngw.vpc_id {
                    j["vpc_id"] = json!(v);
                }
                if let Some(v) = &ngw.subnet_id {
                    j["subnet_id"] = json!(v);
                }
                if let Some(v) = &ngw.state {
                    j["state"] = json!(v);
                }
                if let Some(v) = &ngw.connectivity_type {
                    j["connectivity_type"] = json!(v);
                }
                if let Some(v) = &ngw.allocation_id {
                    j["allocation_id"] = json!(v);
                }
                if let Some(v) = &ngw.public_ip {
                    j["public_ip"] = json!(v);
                }
                if let Some(v) = &ngw.private_ip {
                    j["private_ip"] = json!(v);
                }
                if !ngw.tags.is_empty() {
                    j["tags"] = json!(ngw.tags);
                }
                j
            })
            .collect();
        info!(count = nat_gws.len(), "NAT Gatewaysスキャン完了");
        Ok(nat_gws)
    }

    async fn scan_elastic_ips(&self) -> Result<Vec<Value>> {
        let eips_info = self.ec2_client.describe_addresses().await?;
        let eips: Vec<Value> = eips_info
            .into_iter()
            .map(|eip| {
                let mut j = json!({
                    "allocation_id": eip.allocation_id,
                });
                if let Some(v) = &eip.public_ip {
                    j["public_ip"] = json!(v);
                }
                if let Some(v) = &eip.association_id {
                    j["association_id"] = json!(v);
                }
                if let Some(v) = &eip.instance_id {
                    j["instance_id"] = json!(v);
                }
                if let Some(v) = &eip.network_interface_id {
                    j["network_interface_id"] = json!(v);
                }
                if let Some(v) = &eip.domain {
                    j["domain"] = json!(v);
                }
                if !eip.tags.is_empty() {
                    j["tags"] = json!(eip.tags);
                }
                j
            })
            .collect();
        info!(count = eips.len(), "Elastic IPsスキャン完了");
        Ok(eips)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::aws::ec2_client_trait::mock::MockEc2Client;
    use crate::infra::aws::ec2_client_trait::{
        SecurityGroupInfo, SecurityGroupRuleInfo, SubnetInfo, VpcInfo,
    };
    use std::collections::HashMap;

    fn make_test_config(targets: Vec<&str>) -> ScanConfig {
        let mut scan_targets = HashMap::new();
        for t in targets {
            scan_targets.insert(t.to_string(), true);
        }
        ScanConfig {
            provider: "aws".to_string(),
            account_id: None,
            profile: None,
            assume_role_arn: None,
            assume_role_session_name: None,
            subscription_id: None,
            tenant_id: None,
            auth_method: None,
            service_principal_config: None,
            scope_type: None,
            scope_value: None,
            scan_targets,
            filters: HashMap::new(),
            include_tags: true,
        }
    }

    #[tokio::test]
    async fn test_scan_vpcs() {
        let mut mock = MockEc2Client::new();
        mock.expect_describe_vpcs().returning(|| {
            Ok(vec![VpcInfo {
                vpc_id: "vpc-12345".to_string(),
                cidr_block: Some("10.0.0.0/16".to_string()),
                state: Some("available".to_string()),
                enable_dns_support: true,
                enable_dns_hostnames: true,
                instance_tenancy: Some("default".to_string()),
                tags: HashMap::from([("Name".to_string(), "main-vpc".to_string())]),
            }])
        });

        let config = make_test_config(vec!["vpcs"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let vpcs = results.get("vpcs").unwrap().as_array().unwrap();
        assert_eq!(vpcs.len(), 1);
        assert_eq!(vpcs[0]["vpc_id"], "vpc-12345");
    }

    #[tokio::test]
    async fn test_scan_subnets() {
        let mut mock = MockEc2Client::new();
        mock.expect_describe_subnets().returning(|| {
            Ok(vec![SubnetInfo {
                subnet_id: "subnet-12345".to_string(),
                vpc_id: "vpc-12345".to_string(),
                cidr_block: Some("10.0.1.0/24".to_string()),
                availability_zone: Some("ap-northeast-1a".to_string()),
                map_public_ip_on_launch: false,
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["subnets"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let subnets = results.get("subnets").unwrap().as_array().unwrap();
        assert_eq!(subnets.len(), 1);
        assert_eq!(subnets[0]["subnet_id"], "subnet-12345");
    }

    #[tokio::test]
    async fn test_scan_security_groups() {
        let mut mock = MockEc2Client::new();
        mock.expect_describe_security_groups().returning(|| {
            Ok(vec![SecurityGroupInfo {
                group_id: "sg-12345".to_string(),
                group_name: "web-sg".to_string(),
                description: Some("Web SG".to_string()),
                vpc_id: "vpc-12345".to_string(),
                ingress_rules: vec![SecurityGroupRuleInfo {
                    protocol: "tcp".to_string(),
                    from_port: Some(443),
                    to_port: Some(443),
                    cidr_blocks: vec!["0.0.0.0/0".to_string()],
                    source_security_group_id: None,
                    description: Some("HTTPS".to_string()),
                }],
                egress_rules: vec![],
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["security_groups"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let sgs = results.get("security_groups").unwrap().as_array().unwrap();
        assert_eq!(sgs.len(), 1);
        assert_eq!(sgs[0]["group_name"], "web-sg");
    }

    #[tokio::test]
    async fn test_scan_route_tables() {
        use crate::infra::aws::ec2_client_trait::{
            RouteInfo, RouteTableAssociationInfo, RouteTableInfo,
        };

        let mut mock = MockEc2Client::new();
        mock.expect_describe_route_tables().returning(|| {
            Ok(vec![RouteTableInfo {
                route_table_id: "rtb-12345".to_string(),
                vpc_id: "vpc-12345".to_string(),
                routes: vec![RouteInfo {
                    destination_cidr_block: Some("0.0.0.0/0".to_string()),
                    gateway_id: Some("igw-12345".to_string()),
                    nat_gateway_id: None,
                    network_interface_id: None,
                    transit_gateway_id: None,
                    vpc_peering_connection_id: None,
                }],
                associations: vec![RouteTableAssociationInfo {
                    route_table_association_id: "rtbassoc-12345".to_string(),
                    subnet_id: Some("subnet-12345".to_string()),
                    main: false,
                }],
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["route_tables"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let rts = results.get("route_tables").unwrap().as_array().unwrap();
        assert_eq!(rts.len(), 1);
        assert_eq!(rts[0]["route_table_id"], "rtb-12345");
        assert_eq!(rts[0]["vpc_id"], "vpc-12345");
    }

    #[tokio::test]
    async fn test_scan_network_acls() {
        use crate::infra::aws::ec2_client_trait::{NetworkAclEntryInfo, NetworkAclInfo};

        let mut mock = MockEc2Client::new();
        mock.expect_describe_network_acls().returning(|| {
            Ok(vec![NetworkAclInfo {
                network_acl_id: "acl-12345".to_string(),
                vpc_id: "vpc-12345".to_string(),
                is_default: true,
                entries: vec![NetworkAclEntryInfo {
                    rule_number: 100,
                    protocol: "-1".to_string(),
                    rule_action: "allow".to_string(),
                    egress: false,
                    cidr_block: Some("0.0.0.0/0".to_string()),
                    from_port: None,
                    to_port: None,
                }],
                associations: vec!["subnet-12345".to_string()],
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["network_acls"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let nacls = results.get("network_acls").unwrap().as_array().unwrap();
        assert_eq!(nacls.len(), 1);
        assert_eq!(nacls[0]["network_acl_id"], "acl-12345");
        assert_eq!(nacls[0]["is_default"], true);
    }

    #[tokio::test]
    async fn test_scan_internet_gateways() {
        use crate::infra::aws::ec2_client_trait::{IgwAttachmentInfo, InternetGatewayInfo};

        let mut mock = MockEc2Client::new();
        mock.expect_describe_internet_gateways().returning(|| {
            Ok(vec![InternetGatewayInfo {
                internet_gateway_id: "igw-12345".to_string(),
                attachments: vec![IgwAttachmentInfo {
                    vpc_id: "vpc-12345".to_string(),
                    state: "available".to_string(),
                }],
                tags: HashMap::from([("Name".to_string(), "main-igw".to_string())]),
            }])
        });

        let config = make_test_config(vec!["internet_gateways"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let igws = results
            .get("internet_gateways")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(igws.len(), 1);
        assert_eq!(igws[0]["internet_gateway_id"], "igw-12345");
        // アタッチメントが含まれることを確認
        assert!(igws[0]["attachments"].is_array());
    }

    #[tokio::test]
    async fn test_scan_nat_gateways() {
        use crate::infra::aws::ec2_client_trait::NatGatewayInfo;

        let mut mock = MockEc2Client::new();
        mock.expect_describe_nat_gateways().returning(|| {
            Ok(vec![NatGatewayInfo {
                nat_gateway_id: "nat-12345".to_string(),
                vpc_id: Some("vpc-12345".to_string()),
                subnet_id: Some("subnet-12345".to_string()),
                state: Some("available".to_string()),
                connectivity_type: Some("public".to_string()),
                allocation_id: Some("eipalloc-12345".to_string()),
                public_ip: Some("1.2.3.4".to_string()),
                private_ip: Some("10.0.0.5".to_string()),
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["nat_gateways"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let nat_gws = results.get("nat_gateways").unwrap().as_array().unwrap();
        assert_eq!(nat_gws.len(), 1);
        assert_eq!(nat_gws[0]["nat_gateway_id"], "nat-12345");
        assert_eq!(nat_gws[0]["state"], "available");
        assert_eq!(nat_gws[0]["connectivity_type"], "public");
        assert_eq!(nat_gws[0]["public_ip"], "1.2.3.4");
    }

    #[tokio::test]
    async fn test_scan_elastic_ips() {
        use crate::infra::aws::ec2_client_trait::ElasticIpInfo;

        let mut mock = MockEc2Client::new();
        mock.expect_describe_addresses().returning(|| {
            Ok(vec![ElasticIpInfo {
                allocation_id: "eipalloc-12345".to_string(),
                public_ip: Some("1.2.3.4".to_string()),
                association_id: Some("eipassoc-12345".to_string()),
                instance_id: Some("i-12345".to_string()),
                network_interface_id: None,
                domain: Some("vpc".to_string()),
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["elastic_ips"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        let eips = results.get("elastic_ips").unwrap().as_array().unwrap();
        assert_eq!(eips.len(), 1);
        assert_eq!(eips[0]["allocation_id"], "eipalloc-12345");
        assert_eq!(eips[0]["public_ip"], "1.2.3.4");
        assert_eq!(eips[0]["domain"], "vpc");
    }

    #[tokio::test]
    async fn test_scan_disabled_targets_return_empty_arrays() {
        // スキャンターゲットが false の場合、空の配列が返ることを確認
        let mock = MockEc2Client::new();

        let config = make_test_config(vec![]); // 全ターゲットを無効
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 1)
            .await
            .unwrap();

        // 全リソースが空配列で返ることを確認
        assert_eq!(results.get("vpcs").unwrap().as_array().unwrap().len(), 0);
        assert_eq!(results.get("subnets").unwrap().as_array().unwrap().len(), 0);
        assert_eq!(
            results
                .get("internet_gateways")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            results
                .get("nat_gateways")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            results
                .get("elastic_ips")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn test_scan_multiple_targets() {
        use crate::infra::aws::ec2_client_trait::{InternetGatewayInfo, NatGatewayInfo};

        let mut mock = MockEc2Client::new();
        mock.expect_describe_vpcs().returning(|| {
            Ok(vec![VpcInfo {
                vpc_id: "vpc-multi".to_string(),
                cidr_block: Some("10.0.0.0/16".to_string()),
                state: Some("available".to_string()),
                enable_dns_support: true,
                enable_dns_hostnames: false,
                instance_tenancy: None,
                tags: HashMap::new(),
            }])
        });
        mock.expect_describe_internet_gateways().returning(|| {
            Ok(vec![InternetGatewayInfo {
                internet_gateway_id: "igw-multi".to_string(),
                attachments: vec![],
                tags: HashMap::new(),
            }])
        });
        mock.expect_describe_nat_gateways().returning(|| {
            Ok(vec![NatGatewayInfo {
                nat_gateway_id: "nat-multi".to_string(),
                vpc_id: None,
                subnet_id: None,
                state: None,
                connectivity_type: None,
                allocation_id: None,
                public_ip: None,
                private_ip: None,
                tags: HashMap::new(),
            }])
        });

        let config = make_test_config(vec!["vpcs", "internet_gateways", "nat_gateways"]);
        let scanner = AwsVpcScanner::new_with_client(config, Arc::new(mock));
        let mut results = serde_json::Map::new();
        let completed = AtomicUsize::new(0);

        scanner
            .scan_into(&mut results, &|_, _| {}, &completed, 3)
            .await
            .unwrap();

        assert_eq!(results.get("vpcs").unwrap().as_array().unwrap().len(), 1);
        assert_eq!(
            results
                .get("internet_gateways")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            results
                .get("nat_gateways")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
