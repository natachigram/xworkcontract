use cosmwasm_std::StdError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Data structure for content stored off-chain with hash reference
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct ContentHash {
    pub hash: String,      // Base64 encoded SHA256 hash
    pub data_type: String, // Type identifier (e.g., "job_content", "user_profile")
    pub size_bytes: u64,   // Size of original content for reference
    pub timestamp: u64,    // When this content was created
}

/// Bundle of data that should be stored off-chain
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OffChainBundle {
    pub id: String,               // Unique identifier linking to on-chain record
    pub content_type: String,     // Type of content bundle
    pub data: OffChainData,       // The actual data
    pub metadata: BundleMetadata, // Additional metadata
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OffChainData {
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BundleMetadata {
    pub created_at: u64,
    pub updated_at: u64,
    pub version: u32,
    pub entity_id: String, // ID of the parent entity (job_id, user_addr, etc.)
    pub entity_type: String, // "job", "proposal", "user", etc.
}

/// Generate SHA256 hash from content and encode as base64
pub fn generate_content_hash(content: &str) -> Result<String, StdError> {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    use base64::{engine::general_purpose, Engine as _};
    Ok(general_purpose::STANDARD.encode(result))
}

/// Create a content hash structure from data
pub fn create_content_hash(
    content: &str,
    data_type: &str,
    timestamp: u64,
) -> Result<ContentHash, StdError> {
    Ok(ContentHash {
        hash: generate_content_hash(content)?,
        data_type: data_type.to_string(),
        size_bytes: content.len() as u64,
        timestamp,
    })
}

/// Create an off-chain bundle from structured data
pub fn create_off_chain_bundle(
    entity_id: String,
    entity_type: String,
    content_type: String,
    fields: std::collections::HashMap<String, serde_json::Value>,
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let bundle = OffChainBundle {
        id: format!("{}_{}", entity_type, entity_id),
        content_type: content_type.clone(),
        data: OffChainData { fields },
        metadata: BundleMetadata {
            created_at: timestamp,
            updated_at: timestamp,
            version: 1,
            entity_id,
            entity_type,
        },
    };

    // Generate hash of the entire bundle
    let bundle_json = serde_json::to_string(&bundle)
        .map_err(|e| StdError::generic_err(format!("Serialization failed: {}", e)))?;

    let hash = generate_content_hash(&bundle_json)?;

    Ok((bundle, hash))
}

/// Verify that a hash matches the given content
pub fn verify_content_hash(content: &str, expected_hash: &str) -> Result<bool, StdError> {
    let computed_hash = generate_content_hash(content)?;
    Ok(computed_hash == expected_hash)
}

/// Helper to create job content bundle
pub fn create_job_content_bundle(
    job_id: u64,
    title: &str,
    description: &str,
    company: Option<&str>,
    location: Option<&str>,
    category: &str,
    skills_required: &[String],
    documents: &[String],
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "title".to_string(),
        serde_json::Value::String(title.to_string()),
    );
    fields.insert(
        "description".to_string(),
        serde_json::Value::String(description.to_string()),
    );
    fields.insert(
        "category".to_string(),
        serde_json::Value::String(category.to_string()),
    );
    fields.insert(
        "skills_required".to_string(),
        serde_json::to_value(skills_required)
            .map_err(|e| StdError::generic_err(format!("Skills serialization failed: {}", e)))?,
    );
    fields.insert(
        "documents".to_string(),
        serde_json::to_value(documents)
            .map_err(|e| StdError::generic_err(format!("Documents serialization failed: {}", e)))?,
    );

    if let Some(comp) = company {
        fields.insert(
            "company".to_string(),
            serde_json::Value::String(comp.to_string()),
        );
    }
    if let Some(loc) = location {
        fields.insert(
            "location".to_string(),
            serde_json::Value::String(loc.to_string()),
        );
    }

    create_off_chain_bundle(
        job_id.to_string(),
        "job".to_string(),
        "job_content".to_string(),
        fields,
        timestamp,
    )
}

/// Helper to create proposal content bundle
pub fn create_proposal_content_bundle(
    proposal_id: u64,
    cover_letter: &str,
    milestones: &[serde_json::Value],
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "cover_letter".to_string(),
        serde_json::Value::String(cover_letter.to_string()),
    );
    fields.insert(
        "milestones".to_string(),
        serde_json::Value::Array(milestones.to_vec()),
    );

    create_off_chain_bundle(
        proposal_id.to_string(),
        "proposal".to_string(),
        "proposal_content".to_string(),
        fields,
        timestamp,
    )
}

/// Helper to create bounty content bundle
pub fn create_bounty_content_bundle(
    bounty_id: u64,
    title: &str,
    description: &str,
    requirements: &[String],
    documents: &[String],
    category: &str,
    skills_required: &[String],
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "title".to_string(),
        serde_json::Value::String(title.to_string()),
    );
    fields.insert(
        "description".to_string(),
        serde_json::Value::String(description.to_string()),
    );
    fields.insert(
        "category".to_string(),
        serde_json::Value::String(category.to_string()),
    );
    fields.insert(
        "requirements".to_string(),
        serde_json::to_value(requirements).map_err(|e| {
            StdError::generic_err(format!("Requirements serialization failed: {}", e))
        })?,
    );
    fields.insert(
        "documents".to_string(),
        serde_json::to_value(documents)
            .map_err(|e| StdError::generic_err(format!("Documents serialization failed: {}", e)))?,
    );
    fields.insert(
        "skills_required".to_string(),
        serde_json::to_value(skills_required)
            .map_err(|e| StdError::generic_err(format!("Skills serialization failed: {}", e)))?,
    );

    create_off_chain_bundle(
        bounty_id.to_string(),
        "bounty".to_string(),
        "bounty_content".to_string(),
        fields,
        timestamp,
    )
}

/// Helper to create bounty submission content bundle
pub fn create_bounty_submission_content_bundle(
    submission_id: u64,
    title: &str,
    description: &str,
    deliverables: &[String],
    review_notes: Option<&str>,
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "title".to_string(),
        serde_json::Value::String(title.to_string()),
    );
    fields.insert(
        "description".to_string(),
        serde_json::Value::String(description.to_string()),
    );
    fields.insert(
        "deliverables".to_string(),
        serde_json::to_value(deliverables).map_err(|e| {
            StdError::generic_err(format!("Deliverables serialization failed: {}", e))
        })?,
    );

    if let Some(notes) = review_notes {
        fields.insert(
            "review_notes".to_string(),
            serde_json::Value::String(notes.to_string()),
        );
    }

    create_off_chain_bundle(
        submission_id.to_string(),
        "bounty_submission".to_string(),
        "bounty_submission_content".to_string(),
        fields,
        timestamp,
    )
}

/// Helper to create user profile content bundle
pub fn create_user_profile_bundle(
    user_addr: &str,
    display_name: Option<&str>,
    bio: Option<&str>,
    skills: &[String],
    portfolio_links: &[String],
    timestamp: u64,
) -> Result<(OffChainBundle, String), StdError> {
    let mut fields = std::collections::HashMap::new();

    if let Some(name) = display_name {
        fields.insert(
            "display_name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }
    if let Some(bio_text) = bio {
        fields.insert(
            "bio".to_string(),
            serde_json::Value::String(bio_text.to_string()),
        );
    }

    fields.insert(
        "skills".to_string(),
        serde_json::to_value(skills)
            .map_err(|e| StdError::generic_err(format!("Skills serialization failed: {}", e)))?,
    );
    fields.insert(
        "portfolio_links".to_string(),
        serde_json::to_value(portfolio_links)
            .map_err(|e| StdError::generic_err(format!("Portfolio serialization failed: {}", e)))?,
    );

    create_off_chain_bundle(
        user_addr.to_string(),
        "user".to_string(),
        "user_profile".to_string(),
        fields,
        timestamp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_content_hash() {
        let content = "Hello, World!";
        let hash = generate_content_hash(content).unwrap();
        assert!(!hash.is_empty());

        // Hash should be consistent
        let hash2 = generate_content_hash(content).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_verify_content_hash() {
        let content = "Test content";
        let hash = generate_content_hash(content).unwrap();
        assert!(verify_content_hash(content, &hash).unwrap());
        assert!(!verify_content_hash("Different content", &hash).unwrap());
    }

    #[test]
    fn test_create_job_content_bundle() {
        let skills = vec!["Rust".to_string(), "Blockchain".to_string()];
        let documents = vec!["doc1.pdf".to_string()];

        let (bundle, hash) = create_job_content_bundle(
            1,
            "Test Job",
            "This is a test job",
            Some("Test Company"),
            Some("Remote"),
            "Development",
            &skills,
            &documents,
            1640000000,
        )
        .unwrap();

        assert_eq!(bundle.metadata.entity_type, "job");
        assert_eq!(bundle.metadata.entity_id, "1");
        assert!(!hash.is_empty());
    }
}
