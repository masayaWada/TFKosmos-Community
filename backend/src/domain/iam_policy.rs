use serde::{Deserialize, Serialize};
use serde_json::Value;

/// IAMポリシードキュメントの構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamPolicyDocument {
    #[serde(rename = "Version")]
    pub version: Option<String>,

    #[serde(rename = "Statement")]
    pub statements: Vec<PolicyStatement>,
}

/// IAMポリシーのStatementブロック
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStatement {
    #[serde(rename = "Sid", skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,

    #[serde(rename = "Effect")]
    pub effect: String,

    #[serde(rename = "Action", skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionList>,

    #[serde(rename = "Resource", skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceList>,

    #[serde(rename = "Principal", skip_serializing_if = "Option::is_none")]
    pub principal: Option<Value>,

    #[serde(rename = "Condition", skip_serializing_if = "Option::is_none")]
    pub condition: Option<Value>,

    #[serde(rename = "NotAction", skip_serializing_if = "Option::is_none")]
    pub not_action: Option<ActionList>,

    #[serde(rename = "NotResource", skip_serializing_if = "Option::is_none")]
    pub not_resource: Option<ResourceList>,
}

/// ActionまたはNotActionは文字列または配列
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionList {
    Single(String),
    Multiple(Vec<String>),
}

impl ActionList {
    /// 常に配列として取得
    #[allow(dead_code)]
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            ActionList::Single(s) => vec![s.clone()],
            ActionList::Multiple(v) => v.clone(),
        }
    }
}

/// ResourceまたはNotResourceは文字列または配列
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceList {
    Single(String),
    Multiple(Vec<String>),
}

impl ResourceList {
    /// 常に配列として取得
    #[allow(dead_code)]
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            ResourceList::Single(s) => vec![s.clone()],
            ResourceList::Multiple(v) => v.clone(),
        }
    }
}

impl IamPolicyDocument {
    /// JSON文字列からパース
    #[allow(dead_code)]
    pub fn from_json_str(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_policy_document() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Sid": "AllowS3Access",
                    "Effect": "Allow",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::my-bucket/*"
                },
                {
                    "Effect": "Allow",
                    "Action": ["s3:ListBucket", "s3:PutObject"],
                    "Resource": ["arn:aws:s3:::my-bucket", "arn:aws:s3:::my-bucket/*"]
                }
            ]
        }"#;

        let doc = IamPolicyDocument::from_json_str(json).unwrap();
        assert_eq!(doc.version, Some("2012-10-17".to_string()));
        assert_eq!(doc.statements.len(), 2);

        assert_eq!(doc.statements[0].sid, Some("AllowS3Access".to_string()));
        assert_eq!(doc.statements[0].effect, "Allow");
        assert!(matches!(
            &doc.statements[0].action,
            Some(ActionList::Single(_))
        ));

        assert_eq!(doc.statements[1].sid, None);
        assert!(matches!(
            &doc.statements[1].action,
            Some(ActionList::Multiple(_))
        ));
    }

    #[test]
    fn test_action_list_as_vec() {
        let single = ActionList::Single("s3:GetObject".to_string());
        assert_eq!(single.as_vec(), vec!["s3:GetObject".to_string()]);

        let multiple = ActionList::Multiple(vec![
            "s3:ListBucket".to_string(),
            "s3:PutObject".to_string(),
        ]);
        assert_eq!(
            multiple.as_vec(),
            vec!["s3:ListBucket".to_string(), "s3:PutObject".to_string()]
        );
    }
}
