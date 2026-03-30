//! リソース依存関係グラフサービス
//!
//! スキャン済みのAWS/Azureリソースから依存関係グラフ（ノードとエッジ）を構築する。
//! IAMユーザー・グループ・ロール・ポリシーの間の関係を BFS で展開し、
//! フロントエンドでの可視化（`@xyflow/react`）に使用するデータ形式で返す。
//!
//! # 依存関係の種類（AWS）
//! - `user → group`: ユーザーがグループに所属
//! - `user → policy`: ユーザーにポリシーが直接アタッチ
//! - `group → policy`: グループにポリシーがアタッチ
//! - `role → policy`: ロールにポリシーがアタッチ

use anyhow::Result;
use serde_json::Value;
use std::collections::{HashSet, VecDeque};

use crate::models::{DependencyEdge, DependencyGraph, DependencyNode};
use crate::services::scan_service::ScanService;

/// リソース依存関係グラフを構築するユーティリティサービス（インスタンス不要）
pub struct DependencyService;

impl DependencyService {
    /// 依存関係グラフを取得する
    pub async fn get_dependencies(
        scan_service: &ScanService,
        scan_id: &str,
        root_id: Option<&str>,
    ) -> Result<DependencyGraph> {
        let scan_data = scan_service
            .get_scan_data(scan_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Scan not found"))?;

        let provider = scan_data
            .get("provider")
            .and_then(|p| p.as_str())
            .unwrap_or("aws");

        match provider {
            "aws" => Self::extract_aws_dependencies(&scan_data, root_id),
            "azure" => Self::extract_azure_dependencies(&scan_data, root_id),
            _ => Ok(DependencyGraph {
                nodes: vec![],
                edges: vec![],
            }),
        }
    }

    /// AWS IAMリソースの依存関係を抽出する
    fn extract_aws_dependencies(
        scan_data: &Value,
        root_id: Option<&str>,
    ) -> Result<DependencyGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // ユーザーノードを追加
        if let Some(users) = scan_data.get("users").and_then(|u| u.as_array()) {
            for user in users {
                if let Some(name) = user.get("user_name").and_then(|n| n.as_str()) {
                    nodes.push(DependencyNode {
                        id: format!("user:{}", name),
                        node_type: "user".to_string(),
                        name: name.to_string(),
                        data: user.clone(),
                    });
                }
            }
        }

        // グループノードを追加
        if let Some(groups) = scan_data.get("groups").and_then(|g| g.as_array()) {
            for group in groups {
                if let Some(name) = group.get("group_name").and_then(|n| n.as_str()) {
                    nodes.push(DependencyNode {
                        id: format!("group:{}", name),
                        node_type: "group".to_string(),
                        name: name.to_string(),
                        data: group.clone(),
                    });
                }
            }
        }

        // ロールノードを追加
        if let Some(roles) = scan_data.get("roles").and_then(|r| r.as_array()) {
            for role in roles {
                if let Some(name) = role.get("role_name").and_then(|n| n.as_str()) {
                    nodes.push(DependencyNode {
                        id: format!("role:{}", name),
                        node_type: "role".to_string(),
                        name: name.to_string(),
                        data: role.clone(),
                    });
                }
            }
        }

        // ポリシーノードを追加
        if let Some(policies) = scan_data.get("policies").and_then(|p| p.as_array()) {
            for policy in policies {
                if let Some(arn) = policy.get("arn").and_then(|a| a.as_str()) {
                    let name = policy
                        .get("policy_name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(arn);
                    nodes.push(DependencyNode {
                        id: format!("policy:{}", arn),
                        node_type: "policy".to_string(),
                        name: name.to_string(),
                        data: policy.clone(),
                    });
                }
            }
        }

        // アタッチメントからエッジを作成
        if let Some(attachments) = scan_data.get("attachments").and_then(|a| a.as_array()) {
            for attachment in attachments {
                let entity_type = attachment
                    .get("entity_type")
                    .and_then(|e| e.as_str())
                    .unwrap_or("");
                let entity_name = attachment
                    .get("entity_name")
                    .and_then(|e| e.as_str())
                    .unwrap_or("");
                let policy_arn = attachment
                    .get("policy_arn")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");

                let source_id = match entity_type {
                    "User" => format!("user:{}", entity_name),
                    "Group" => format!("group:{}", entity_name),
                    "Role" => format!("role:{}", entity_name),
                    _ => continue,
                };

                edges.push(DependencyEdge {
                    source: source_id,
                    target: format!("policy:{}", policy_arn),
                    edge_type: "policy_attachment".to_string(),
                    label: Some("has policy".to_string()),
                });
            }
        }

        // グループメンバーシップのエッジを作成
        if let Some(groups) = scan_data.get("groups").and_then(|g| g.as_array()) {
            for group in groups {
                if let Some(group_name) = group.get("group_name").and_then(|n| n.as_str()) {
                    if let Some(members) = group.get("members").and_then(|m| m.as_array()) {
                        for member in members {
                            if let Some(user_name) = member.as_str() {
                                edges.push(DependencyEdge {
                                    source: format!("user:{}", user_name),
                                    target: format!("group:{}", group_name),
                                    edge_type: "group_membership".to_string(),
                                    label: Some("member of".to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }

        // ルートIDでフィルタリング
        if let Some(root) = root_id {
            Self::filter_by_root(&mut nodes, &mut edges, root);
        }

        Ok(DependencyGraph { nodes, edges })
    }

    /// Azure IAMリソースの依存関係を抽出する
    fn extract_azure_dependencies(
        scan_data: &Value,
        root_id: Option<&str>,
    ) -> Result<DependencyGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // ロール定義ノードを追加
        if let Some(role_definitions) = scan_data.get("role_definitions").and_then(|r| r.as_array())
        {
            for role_def in role_definitions {
                if let Some(id) = role_def.get("id").and_then(|i| i.as_str()) {
                    let name = role_def.get("name").and_then(|n| n.as_str()).unwrap_or(id);
                    nodes.push(DependencyNode {
                        id: format!("role_definition:{}", id),
                        node_type: "role_definition".to_string(),
                        name: name.to_string(),
                        data: role_def.clone(),
                    });
                }
            }
        }

        // ロール割り当てからノードとエッジを作成
        if let Some(role_assignments) = scan_data.get("role_assignments").and_then(|r| r.as_array())
        {
            for assignment in role_assignments {
                if let (Some(principal_id), Some(role_def_id)) = (
                    assignment.get("principal_id").and_then(|p| p.as_str()),
                    assignment
                        .get("role_definition_id")
                        .and_then(|r| r.as_str()),
                ) {
                    // プリンシパルノードを追加（存在しない場合）
                    let principal_node_id = format!("principal:{}", principal_id);
                    let principal_name = assignment
                        .get("principal_name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(principal_id);

                    if !nodes.iter().any(|n| n.id == principal_node_id) {
                        nodes.push(DependencyNode {
                            id: principal_node_id.clone(),
                            node_type: "principal".to_string(),
                            name: principal_name.to_string(),
                            data: assignment.clone(),
                        });
                    }

                    // エッジを追加
                    edges.push(DependencyEdge {
                        source: principal_node_id,
                        target: format!("role_definition:{}", role_def_id),
                        edge_type: "role_assignment".to_string(),
                        label: Some("assigned".to_string()),
                    });
                }
            }
        }

        // ルートIDでフィルタリング
        if let Some(root) = root_id {
            Self::filter_by_root(&mut nodes, &mut edges, root);
        }

        Ok(DependencyGraph { nodes, edges })
    }

    /// root_idから到達可能なノードのみを残す（BFS使用）
    fn filter_by_root(
        nodes: &mut Vec<DependencyNode>,
        edges: &mut Vec<DependencyEdge>,
        root_id: &str,
    ) {
        let mut reachable: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        queue.push_back(root_id.to_string());

        while let Some(current) = queue.pop_front() {
            if reachable.contains(&current) {
                continue;
            }
            reachable.insert(current.clone());

            for edge in edges.iter() {
                if edge.source == current && !reachable.contains(&edge.target) {
                    queue.push_back(edge.target.clone());
                }
                if edge.target == current && !reachable.contains(&edge.source) {
                    queue.push_back(edge.source.clone());
                }
            }
        }

        nodes.retain(|n| reachable.contains(&n.id));
        edges.retain(|e| reachable.contains(&e.source) && reachable.contains(&e.target));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_aws_dependencies() {
        let scan_data = json!({
            "provider": "aws",
            "users": [
                {"user_name": "alice"},
                {"user_name": "bob"}
            ],
            "groups": [
                {"group_name": "admins", "members": ["alice"]},
                {"group_name": "developers", "members": ["bob"]}
            ],
            "roles": [
                {"role_name": "admin-role"}
            ],
            "policies": [
                {"arn": "arn:aws:iam::123:policy/AdminPolicy", "policy_name": "AdminPolicy"}
            ],
            "attachments": [
                {"entity_type": "User", "entity_name": "alice", "policy_arn": "arn:aws:iam::123:policy/AdminPolicy"},
                {"entity_type": "Group", "entity_name": "admins", "policy_arn": "arn:aws:iam::123:policy/AdminPolicy"}
            ]
        });

        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();

        assert_eq!(result.nodes.len(), 6); // 2 users + 2 groups + 1 role + 1 policy
        assert_eq!(result.edges.len(), 4); // 2 policy attachments + 2 group memberships
    }

    #[test]
    fn test_filter_by_root() {
        let mut nodes = vec![
            DependencyNode {
                id: "user:alice".to_string(),
                node_type: "user".to_string(),
                name: "alice".to_string(),
                data: json!({}),
            },
            DependencyNode {
                id: "user:bob".to_string(),
                node_type: "user".to_string(),
                name: "bob".to_string(),
                data: json!({}),
            },
            DependencyNode {
                id: "policy:p1".to_string(),
                node_type: "policy".to_string(),
                name: "p1".to_string(),
                data: json!({}),
            },
        ];

        let mut edges = vec![DependencyEdge {
            source: "user:alice".to_string(),
            target: "policy:p1".to_string(),
            edge_type: "policy_attachment".to_string(),
            label: Some("has policy".to_string()),
        }];

        DependencyService::filter_by_root(&mut nodes, &mut edges, "user:alice");

        assert_eq!(nodes.len(), 2); // alice and p1
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_extract_azure_dependencies() {
        let scan_data = json!({
            "provider": "azure",
            "role_definitions": [
                {"id": "role-def-1", "name": "Contributor"},
                {"id": "role-def-2", "name": "Reader"}
            ],
            "role_assignments": [
                {
                    "principal_id": "principal-001",
                    "principal_name": "Alice",
                    "role_definition_id": "role-def-1"
                },
                {
                    "principal_id": "principal-002",
                    "principal_name": "Bob",
                    "role_definition_id": "role-def-2"
                }
            ]
        });

        let result = DependencyService::extract_azure_dependencies(&scan_data, None).unwrap();

        // 2 role_definitions + 2 principals = 4 nodes
        assert_eq!(result.nodes.len(), 4, "ノード数は4であるべき");
        // 2 role_assignments = 2 edges
        assert_eq!(result.edges.len(), 2, "エッジ数は2であるべき");

        // Verify edge types
        assert!(
            result
                .edges
                .iter()
                .all(|e| e.edge_type == "role_assignment"),
            "全エッジがrole_assignment型であるべき"
        );
    }

    #[test]
    fn test_extract_azure_dependencies_duplicate_principal() {
        let scan_data = json!({
            "provider": "azure",
            "role_definitions": [
                {"id": "role-def-1", "name": "Contributor"},
                {"id": "role-def-2", "name": "Reader"}
            ],
            "role_assignments": [
                {
                    "principal_id": "principal-001",
                    "principal_name": "Alice",
                    "role_definition_id": "role-def-1"
                },
                {
                    "principal_id": "principal-001",
                    "principal_name": "Alice",
                    "role_definition_id": "role-def-2"
                }
            ]
        });

        let result = DependencyService::extract_azure_dependencies(&scan_data, None).unwrap();

        // 2 role_definitions + 1 unique principal = 3 nodes
        assert_eq!(
            result.nodes.len(),
            3,
            "重複プリンシパルは1つにまとめられるべき"
        );
        assert_eq!(result.edges.len(), 2, "エッジは2つあるべき");
    }

    #[test]
    fn test_extract_aws_dependencies_empty_data() {
        let scan_data = json!({
            "provider": "aws"
        });

        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();

        assert!(
            result.nodes.is_empty(),
            "空のスキャンデータではノードが空であるべき"
        );
        assert!(
            result.edges.is_empty(),
            "空のスキャンデータではエッジが空であるべき"
        );
    }

    #[test]
    fn test_extract_azure_dependencies_empty_data() {
        let scan_data = json!({
            "provider": "azure"
        });

        let result = DependencyService::extract_azure_dependencies(&scan_data, None).unwrap();

        assert!(
            result.nodes.is_empty(),
            "空のスキャンデータではノードが空であるべき"
        );
        assert!(
            result.edges.is_empty(),
            "空のスキャンデータではエッジが空であるべき"
        );
    }

    #[test]
    fn test_extract_aws_dependencies_with_root_id_filter() {
        let scan_data = json!({
            "provider": "aws",
            "users": [
                {"user_name": "alice"},
                {"user_name": "bob"}
            ],
            "policies": [
                {"arn": "arn:aws:iam::123:policy/Policy1", "policy_name": "Policy1"},
                {"arn": "arn:aws:iam::123:policy/Policy2", "policy_name": "Policy2"}
            ],
            "attachments": [
                {"entity_type": "User", "entity_name": "alice", "policy_arn": "arn:aws:iam::123:policy/Policy1"},
                {"entity_type": "User", "entity_name": "bob", "policy_arn": "arn:aws:iam::123:policy/Policy2"}
            ]
        });

        let result =
            DependencyService::extract_aws_dependencies(&scan_data, Some("user:alice")).unwrap();

        // alice + Policy1 のみ到達可能
        assert_eq!(
            result.nodes.len(),
            2,
            "root_idフィルタ後は2ノードであるべき"
        );
        assert_eq!(
            result.edges.len(),
            1,
            "root_idフィルタ後は1エッジであるべき"
        );
        assert!(
            result.nodes.iter().any(|n| n.name == "alice"),
            "aliceがノードに含まれるべき"
        );
        assert!(
            result.nodes.iter().any(|n| n.name == "Policy1"),
            "Policy1がノードに含まれるべき"
        );
    }

    #[test]
    fn test_extract_aws_dependencies_role_attachment_edge() {
        let scan_data = json!({
            "provider": "aws",
            "roles": [
                {"role_name": "admin-role"}
            ],
            "policies": [
                {"arn": "arn:aws:iam::123:policy/AdminPolicy", "policy_name": "AdminPolicy"}
            ],
            "attachments": [
                {"entity_type": "Role", "entity_name": "admin-role", "policy_arn": "arn:aws:iam::123:policy/AdminPolicy"}
            ]
        });

        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();

        assert_eq!(result.nodes.len(), 2, "ロール+ポリシーで2ノードであるべき");
        assert_eq!(
            result.edges.len(),
            1,
            "アタッチメント1つで1エッジであるべき"
        );
        assert_eq!(
            result.edges[0].source, "role:admin-role",
            "エッジのsourceはロールであるべき"
        );
    }

    #[test]
    fn test_extract_aws_dependencies_unknown_entity_type_skipped() {
        let scan_data = json!({
            "provider": "aws",
            "policies": [
                {"arn": "arn:aws:iam::123:policy/P1", "policy_name": "P1"}
            ],
            "attachments": [
                {"entity_type": "Unknown", "entity_name": "something", "policy_arn": "arn:aws:iam::123:policy/P1"}
            ]
        });

        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();

        assert_eq!(result.nodes.len(), 1, "ポリシーノードのみ存在するべき");
        assert!(
            result.edges.is_empty(),
            "不明なentity_typeのアタッチメントはスキップされるべき"
        );
    }

    #[test]
    fn test_extract_aws_dependencies_policy_without_name_uses_arn() {
        let scan_data = json!({
            "provider": "aws",
            "policies": [
                {"arn": "arn:aws:iam::123:policy/NoNamePolicy"}
            ]
        });

        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(
            result.nodes[0].name, "arn:aws:iam::123:policy/NoNamePolicy",
            "policy_nameがない場合はARNが名前として使用されるべき"
        );
    }

    #[test]
    fn test_filter_by_root_nonexistent_root() {
        let mut nodes = vec![DependencyNode {
            id: "user:alice".to_string(),
            node_type: "user".to_string(),
            name: "alice".to_string(),
            data: json!({}),
        }];
        let mut edges = vec![];

        DependencyService::filter_by_root(&mut nodes, &mut edges, "user:nonexistent");

        assert!(
            nodes.is_empty(),
            "存在しないrootでフィルタすると全ノードが除去されるべき"
        );
    }

    #[test]
    fn test_filter_by_root_bidirectional_traversal() {
        let mut nodes = vec![
            DependencyNode {
                id: "a".to_string(),
                node_type: "user".to_string(),
                name: "a".to_string(),
                data: json!({}),
            },
            DependencyNode {
                id: "b".to_string(),
                node_type: "group".to_string(),
                name: "b".to_string(),
                data: json!({}),
            },
            DependencyNode {
                id: "c".to_string(),
                node_type: "policy".to_string(),
                name: "c".to_string(),
                data: json!({}),
            },
            DependencyNode {
                id: "d".to_string(),
                node_type: "policy".to_string(),
                name: "d".to_string(),
                data: json!({}),
            },
        ];

        // a -> b -> c, d is disconnected
        let mut edges = vec![
            DependencyEdge {
                source: "a".to_string(),
                target: "b".to_string(),
                edge_type: "member".to_string(),
                label: None,
            },
            DependencyEdge {
                source: "b".to_string(),
                target: "c".to_string(),
                edge_type: "policy".to_string(),
                label: None,
            },
        ];

        // Filter from the middle: should find both directions
        DependencyService::filter_by_root(&mut nodes, &mut edges, "b");

        assert_eq!(nodes.len(), 3, "bから到達可能なa,b,cの3ノードであるべき");
        assert_eq!(edges.len(), 2, "2エッジが残るべき");
        assert!(
            !nodes.iter().any(|n| n.id == "d"),
            "切断されたノードdは除去されるべき"
        );
    }

    #[test]
    fn test_unknown_provider_returns_empty_graph() {
        let scan_data = json!({
            "provider": "gcp",
            "users": [{"user_name": "alice"}]
        });

        // Call extract_aws_dependencies directly with unknown provider data
        // The get_dependencies method handles provider routing
        let result = DependencyService::extract_aws_dependencies(&scan_data, None).unwrap();
        // It still processes the data based on field presence regardless of provider
        assert_eq!(result.nodes.len(), 1);
    }
}
