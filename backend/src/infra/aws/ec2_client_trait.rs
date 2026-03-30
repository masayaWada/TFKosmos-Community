//! EC2/VPCクライアント操作の抽象化トレイト

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// EC2インスタンス情報
#[derive(Debug, Clone)]
pub struct Ec2InstanceInfo {
    pub instance_id: String,
    pub instance_type: Option<String>,
    pub state: Option<String>,
    pub ami_id: Option<String>,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub key_name: Option<String>,
    pub iam_instance_profile: Option<String>,
    pub security_groups: Vec<String>,
    pub tags: HashMap<String, String>,
}

/// VPC情報
#[derive(Debug, Clone)]
pub struct VpcInfo {
    pub vpc_id: String,
    pub cidr_block: Option<String>,
    pub state: Option<String>,
    pub enable_dns_support: bool,
    pub enable_dns_hostnames: bool,
    pub instance_tenancy: Option<String>,
    pub tags: HashMap<String, String>,
}

/// サブネット情報
#[derive(Debug, Clone)]
pub struct SubnetInfo {
    pub subnet_id: String,
    pub vpc_id: String,
    pub cidr_block: Option<String>,
    pub availability_zone: Option<String>,
    pub map_public_ip_on_launch: bool,
    pub tags: HashMap<String, String>,
}

/// ルートテーブル情報
#[derive(Debug, Clone)]
pub struct RouteTableInfo {
    pub route_table_id: String,
    pub vpc_id: String,
    pub routes: Vec<RouteInfo>,
    pub associations: Vec<RouteTableAssociationInfo>,
    pub tags: HashMap<String, String>,
}

/// ルート情報
#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub destination_cidr_block: Option<String>,
    pub gateway_id: Option<String>,
    pub nat_gateway_id: Option<String>,
    #[allow(dead_code)]
    pub network_interface_id: Option<String>,
    #[allow(dead_code)]
    pub transit_gateway_id: Option<String>,
    #[allow(dead_code)]
    pub vpc_peering_connection_id: Option<String>,
}

/// ルートテーブル関連付け情報
#[derive(Debug, Clone)]
pub struct RouteTableAssociationInfo {
    pub route_table_association_id: String,
    pub subnet_id: Option<String>,
    pub main: bool,
}

/// セキュリティグループ情報
#[derive(Debug, Clone)]
pub struct SecurityGroupInfo {
    pub group_id: String,
    pub group_name: String,
    pub description: Option<String>,
    pub vpc_id: String,
    pub ingress_rules: Vec<SecurityGroupRuleInfo>,
    pub egress_rules: Vec<SecurityGroupRuleInfo>,
    pub tags: HashMap<String, String>,
}

/// セキュリティグループルール情報
#[derive(Debug, Clone)]
pub struct SecurityGroupRuleInfo {
    pub protocol: String,
    pub from_port: Option<i32>,
    pub to_port: Option<i32>,
    pub cidr_blocks: Vec<String>,
    pub source_security_group_id: Option<String>,
    pub description: Option<String>,
}

/// ネットワークACL情報
#[derive(Debug, Clone)]
pub struct NetworkAclInfo {
    pub network_acl_id: String,
    pub vpc_id: String,
    pub is_default: bool,
    pub entries: Vec<NetworkAclEntryInfo>,
    pub associations: Vec<String>,
    pub tags: HashMap<String, String>,
}

/// ネットワークACLエントリ情報
#[derive(Debug, Clone)]
pub struct NetworkAclEntryInfo {
    pub rule_number: i32,
    pub protocol: String,
    pub rule_action: String,
    pub egress: bool,
    pub cidr_block: Option<String>,
    pub from_port: Option<i32>,
    pub to_port: Option<i32>,
}

/// Internet Gateway情報
#[derive(Debug, Clone)]
pub struct InternetGatewayInfo {
    pub internet_gateway_id: String,
    pub attachments: Vec<IgwAttachmentInfo>,
    pub tags: HashMap<String, String>,
}

/// IGWアタッチメント情報
#[derive(Debug, Clone)]
pub struct IgwAttachmentInfo {
    pub vpc_id: String,
    pub state: String,
}

/// NAT Gateway情報
#[derive(Debug, Clone)]
pub struct NatGatewayInfo {
    pub nat_gateway_id: String,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub state: Option<String>,
    pub connectivity_type: Option<String>,
    pub allocation_id: Option<String>,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub tags: HashMap<String, String>,
}

/// Elastic IP情報
#[derive(Debug, Clone)]
pub struct ElasticIpInfo {
    pub allocation_id: String,
    pub public_ip: Option<String>,
    pub association_id: Option<String>,
    pub instance_id: Option<String>,
    pub network_interface_id: Option<String>,
    pub domain: Option<String>,
    pub tags: HashMap<String, String>,
}

/// EC2/VPCクライアント操作を抽象化するトレイト
#[async_trait]
pub trait Ec2ClientOps: Send + Sync {
    /// EC2インスタンス一覧を取得
    async fn describe_instances(&self) -> Result<Vec<Ec2InstanceInfo>>;

    /// VPC一覧を取得
    async fn describe_vpcs(&self) -> Result<Vec<VpcInfo>>;

    /// サブネット一覧を取得
    async fn describe_subnets(&self) -> Result<Vec<SubnetInfo>>;

    /// ルートテーブル一覧を取得
    async fn describe_route_tables(&self) -> Result<Vec<RouteTableInfo>>;

    /// セキュリティグループ一覧を取得
    async fn describe_security_groups(&self) -> Result<Vec<SecurityGroupInfo>>;

    /// ネットワークACL一覧を取得
    async fn describe_network_acls(&self) -> Result<Vec<NetworkAclInfo>>;

    /// Internet Gateway一覧を取得
    async fn describe_internet_gateways(&self) -> Result<Vec<InternetGatewayInfo>>;

    /// NAT Gateway一覧を取得
    async fn describe_nat_gateways(&self) -> Result<Vec<NatGatewayInfo>>;

    /// Elastic IP一覧を取得
    async fn describe_addresses(&self) -> Result<Vec<ElasticIpInfo>>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    mock! {
        pub Ec2Client {}

        #[async_trait]
        impl Ec2ClientOps for Ec2Client {
            async fn describe_instances(&self) -> Result<Vec<Ec2InstanceInfo>>;
            async fn describe_vpcs(&self) -> Result<Vec<VpcInfo>>;
            async fn describe_subnets(&self) -> Result<Vec<SubnetInfo>>;
            async fn describe_route_tables(&self) -> Result<Vec<RouteTableInfo>>;
            async fn describe_security_groups(&self) -> Result<Vec<SecurityGroupInfo>>;
            async fn describe_network_acls(&self) -> Result<Vec<NetworkAclInfo>>;
            async fn describe_internet_gateways(&self) -> Result<Vec<InternetGatewayInfo>>;
            async fn describe_nat_gateways(&self) -> Result<Vec<NatGatewayInfo>>;
            async fn describe_addresses(&self) -> Result<Vec<ElasticIpInfo>>;
        }
    }
}
