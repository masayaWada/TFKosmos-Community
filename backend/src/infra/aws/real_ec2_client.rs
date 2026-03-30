//! AWS SDK EC2クライアントの本番実装

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_ec2::Client as Ec2Client;
use std::collections::HashMap;

use super::ec2_client_trait::{
    Ec2ClientOps, Ec2InstanceInfo, ElasticIpInfo, IgwAttachmentInfo, InternetGatewayInfo,
    NatGatewayInfo, NetworkAclEntryInfo, NetworkAclInfo, RouteInfo, RouteTableAssociationInfo,
    RouteTableInfo, SecurityGroupInfo, SecurityGroupRuleInfo, SubnetInfo, VpcInfo,
};

/// AWS SDK EC2クライアントをラップした本番実装
pub struct RealEc2Client {
    client: Ec2Client,
}

impl RealEc2Client {
    pub fn new(client: Ec2Client) -> Self {
        Self { client }
    }
}

fn extract_tags(tags: &[aws_sdk_ec2::types::Tag]) -> HashMap<String, String> {
    tags.iter()
        .filter_map(|t| {
            let key = t.key()?.to_string();
            let value = t.value()?.to_string();
            Some((key, value))
        })
        .collect()
}

#[async_trait]
impl Ec2ClientOps for RealEc2Client {
    async fn describe_instances(&self) -> Result<Vec<Ec2InstanceInfo>> {
        let mut instances = Vec::new();
        let mut paginator = self.client.describe_instances().into_paginator().send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(|e| anyhow!("Failed to describe instances: {}", e))?;
            for reservation in page.reservations() {
                for inst in reservation.instances() {
                    instances.push(Ec2InstanceInfo {
                        instance_id: inst.instance_id().unwrap_or_default().to_string(),
                        instance_type: inst.instance_type().map(|t| t.as_str().to_string()),
                        state: inst
                            .state()
                            .and_then(|s| s.name())
                            .map(|n| n.as_str().to_string()),
                        ami_id: inst.image_id().map(|s| s.to_string()),
                        vpc_id: inst.vpc_id().map(|s| s.to_string()),
                        subnet_id: inst.subnet_id().map(|s| s.to_string()),
                        private_ip: inst.private_ip_address().map(|s| s.to_string()),
                        public_ip: inst.public_ip_address().map(|s| s.to_string()),
                        key_name: inst.key_name().map(|s| s.to_string()),
                        iam_instance_profile: inst
                            .iam_instance_profile()
                            .and_then(|p| p.arn())
                            .map(|s| s.to_string()),
                        security_groups: inst
                            .security_groups()
                            .iter()
                            .filter_map(|sg| sg.group_id().map(|s| s.to_string()))
                            .collect(),
                        tags: extract_tags(inst.tags()),
                    });
                }
            }
        }

        Ok(instances)
    }

    async fn describe_vpcs(&self) -> Result<Vec<VpcInfo>> {
        let output = self
            .client
            .describe_vpcs()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe VPCs: {}", e))?;

        let vpcs = output
            .vpcs()
            .iter()
            .map(|vpc| {
                VpcInfo {
                    vpc_id: vpc.vpc_id().unwrap_or_default().to_string(),
                    cidr_block: vpc.cidr_block().map(|s| s.to_string()),
                    state: vpc.state().map(|s| s.as_str().to_string()),
                    // DNS属性は別途APIで取得が必要だが、デフォルト値を使用
                    enable_dns_support: true,
                    enable_dns_hostnames: false,
                    instance_tenancy: vpc.instance_tenancy().map(|t| t.as_str().to_string()),
                    tags: extract_tags(vpc.tags()),
                }
            })
            .collect();

        Ok(vpcs)
    }

    async fn describe_subnets(&self) -> Result<Vec<SubnetInfo>> {
        let output = self
            .client
            .describe_subnets()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe subnets: {}", e))?;

        let subnets = output
            .subnets()
            .iter()
            .map(|s| SubnetInfo {
                subnet_id: s.subnet_id().unwrap_or_default().to_string(),
                vpc_id: s.vpc_id().unwrap_or_default().to_string(),
                cidr_block: s.cidr_block().map(|c| c.to_string()),
                availability_zone: s.availability_zone().map(|a| a.to_string()),
                map_public_ip_on_launch: s.map_public_ip_on_launch().unwrap_or(false),
                tags: extract_tags(s.tags()),
            })
            .collect();

        Ok(subnets)
    }

    async fn describe_route_tables(&self) -> Result<Vec<RouteTableInfo>> {
        let output = self
            .client
            .describe_route_tables()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe route tables: {}", e))?;

        let tables = output
            .route_tables()
            .iter()
            .map(|rt| {
                let routes = rt
                    .routes()
                    .iter()
                    .map(|r| RouteInfo {
                        destination_cidr_block: r.destination_cidr_block().map(|s| s.to_string()),
                        gateway_id: r.gateway_id().map(|s| s.to_string()),
                        nat_gateway_id: r.nat_gateway_id().map(|s| s.to_string()),
                        network_interface_id: r.network_interface_id().map(|s| s.to_string()),
                        transit_gateway_id: r.transit_gateway_id().map(|s| s.to_string()),
                        vpc_peering_connection_id: r
                            .vpc_peering_connection_id()
                            .map(|s| s.to_string()),
                    })
                    .collect();

                let associations = rt
                    .associations()
                    .iter()
                    .map(|a| RouteTableAssociationInfo {
                        route_table_association_id: a
                            .route_table_association_id()
                            .unwrap_or_default()
                            .to_string(),
                        subnet_id: a.subnet_id().map(|s| s.to_string()),
                        main: a.main().unwrap_or(false),
                    })
                    .collect();

                RouteTableInfo {
                    route_table_id: rt.route_table_id().unwrap_or_default().to_string(),
                    vpc_id: rt.vpc_id().unwrap_or_default().to_string(),
                    routes,
                    associations,
                    tags: extract_tags(rt.tags()),
                }
            })
            .collect();

        Ok(tables)
    }

    async fn describe_security_groups(&self) -> Result<Vec<SecurityGroupInfo>> {
        let output = self
            .client
            .describe_security_groups()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe security groups: {}", e))?;

        let groups = output
            .security_groups()
            .iter()
            .map(|sg| {
                let ingress_rules = sg
                    .ip_permissions()
                    .iter()
                    .map(|r| SecurityGroupRuleInfo {
                        protocol: r.ip_protocol().unwrap_or_default().to_string(),
                        from_port: r.from_port(),
                        to_port: r.to_port(),
                        cidr_blocks: r
                            .ip_ranges()
                            .iter()
                            .filter_map(|ir| ir.cidr_ip().map(|s| s.to_string()))
                            .collect(),
                        source_security_group_id: r
                            .user_id_group_pairs()
                            .first()
                            .and_then(|p| p.group_id().map(|s| s.to_string())),
                        description: r
                            .ip_ranges()
                            .first()
                            .and_then(|ir| ir.description().map(|s| s.to_string())),
                    })
                    .collect();

                let egress_rules = sg
                    .ip_permissions_egress()
                    .iter()
                    .map(|r| SecurityGroupRuleInfo {
                        protocol: r.ip_protocol().unwrap_or_default().to_string(),
                        from_port: r.from_port(),
                        to_port: r.to_port(),
                        cidr_blocks: r
                            .ip_ranges()
                            .iter()
                            .filter_map(|ir| ir.cidr_ip().map(|s| s.to_string()))
                            .collect(),
                        source_security_group_id: r
                            .user_id_group_pairs()
                            .first()
                            .and_then(|p| p.group_id().map(|s| s.to_string())),
                        description: r
                            .ip_ranges()
                            .first()
                            .and_then(|ir| ir.description().map(|s| s.to_string())),
                    })
                    .collect();

                SecurityGroupInfo {
                    group_id: sg.group_id().unwrap_or_default().to_string(),
                    group_name: sg.group_name().unwrap_or_default().to_string(),
                    description: sg.description().map(|s| s.to_string()),
                    vpc_id: sg.vpc_id().unwrap_or_default().to_string(),
                    ingress_rules,
                    egress_rules,
                    tags: extract_tags(sg.tags()),
                }
            })
            .collect();

        Ok(groups)
    }

    async fn describe_network_acls(&self) -> Result<Vec<NetworkAclInfo>> {
        let output = self
            .client
            .describe_network_acls()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe network ACLs: {}", e))?;

        let acls = output
            .network_acls()
            .iter()
            .map(|acl| {
                let entries = acl
                    .entries()
                    .iter()
                    .map(|e| NetworkAclEntryInfo {
                        rule_number: e.rule_number().unwrap_or(0),
                        protocol: e.protocol().unwrap_or_default().to_string(),
                        rule_action: e
                            .rule_action()
                            .map(|a| a.as_str().to_string())
                            .unwrap_or_default(),
                        egress: e.egress().unwrap_or(false),
                        cidr_block: e.cidr_block().map(|s| s.to_string()),
                        from_port: e.port_range().map(|p| p.from().unwrap_or(0)),
                        to_port: e.port_range().map(|p| p.to().unwrap_or(0)),
                    })
                    .collect();

                let associations = acl
                    .associations()
                    .iter()
                    .filter_map(|a| a.subnet_id().map(|s| s.to_string()))
                    .collect();

                NetworkAclInfo {
                    network_acl_id: acl.network_acl_id().unwrap_or_default().to_string(),
                    vpc_id: acl.vpc_id().unwrap_or_default().to_string(),
                    is_default: acl.is_default().unwrap_or(false),
                    entries,
                    associations,
                    tags: extract_tags(acl.tags()),
                }
            })
            .collect();

        Ok(acls)
    }

    async fn describe_internet_gateways(&self) -> Result<Vec<InternetGatewayInfo>> {
        let output = self
            .client
            .describe_internet_gateways()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe internet gateways: {}", e))?;

        let igws = output
            .internet_gateways()
            .iter()
            .map(|igw| {
                let attachments = igw
                    .attachments()
                    .iter()
                    .map(|a| IgwAttachmentInfo {
                        vpc_id: a.vpc_id().unwrap_or_default().to_string(),
                        state: a
                            .state()
                            .map(|s| s.as_str().to_string())
                            .unwrap_or_default(),
                    })
                    .collect();

                InternetGatewayInfo {
                    internet_gateway_id: igw.internet_gateway_id().unwrap_or_default().to_string(),
                    attachments,
                    tags: extract_tags(igw.tags()),
                }
            })
            .collect();

        Ok(igws)
    }

    async fn describe_nat_gateways(&self) -> Result<Vec<NatGatewayInfo>> {
        let output = self
            .client
            .describe_nat_gateways()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe NAT gateways: {}", e))?;

        let nat_gws = output
            .nat_gateways()
            .iter()
            .map(|ngw| {
                let addr = ngw.nat_gateway_addresses().first();
                NatGatewayInfo {
                    nat_gateway_id: ngw.nat_gateway_id().unwrap_or_default().to_string(),
                    vpc_id: ngw.vpc_id().map(|s| s.to_string()),
                    subnet_id: ngw.subnet_id().map(|s| s.to_string()),
                    state: ngw.state().map(|s| s.as_str().to_string()),
                    connectivity_type: ngw.connectivity_type().map(|c| c.as_str().to_string()),
                    allocation_id: addr.and_then(|a| a.allocation_id().map(|s| s.to_string())),
                    public_ip: addr.and_then(|a| a.public_ip().map(|s| s.to_string())),
                    private_ip: addr.and_then(|a| a.private_ip().map(|s| s.to_string())),
                    tags: extract_tags(ngw.tags()),
                }
            })
            .collect();

        Ok(nat_gws)
    }

    async fn describe_addresses(&self) -> Result<Vec<ElasticIpInfo>> {
        let output = self
            .client
            .describe_addresses()
            .send()
            .await
            .map_err(|e| anyhow!("Failed to describe addresses: {}", e))?;

        let eips = output
            .addresses()
            .iter()
            .map(|addr| ElasticIpInfo {
                allocation_id: addr.allocation_id().unwrap_or_default().to_string(),
                public_ip: addr.public_ip().map(|s| s.to_string()),
                association_id: addr.association_id().map(|s| s.to_string()),
                instance_id: addr.instance_id().map(|s| s.to_string()),
                network_interface_id: addr.network_interface_id().map(|s| s.to_string()),
                domain: addr.domain().map(|d| d.as_str().to_string()),
                tags: extract_tags(addr.tags()),
            })
            .collect();

        Ok(eips)
    }
}
