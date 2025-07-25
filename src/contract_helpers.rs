use crate::error::ContractError;
use crate::helpers::{
    query_jobs_paginated, validate_budget, validate_duration,
};
use crate::msg::JobsResponse;
use crate::state::{BountyStatus, JobStatus};
use cosmwasm_std::{attr, Addr, Attribute, StdResult, Uint128};

// Helper macros and functions to reduce code duplication

/// Macro to apply standard security checks for execute functions
#[macro_export]
macro_rules! apply_security_checks {
    ($deps:expr, $env:expr, $info:expr, $rate_limit_action:expr) => {
        ensure_not_paused($deps.as_ref())?;
        reentrancy_guard($deps.branch())?;
        check_rate_limit($deps.branch(), &$env, &$info.sender, $rate_limit_action)?;
    };
}

/// Macro to apply basic security checks without rate limiting
#[macro_export]
macro_rules! apply_basic_security_checks {
    ($deps:expr) => {
        ensure_not_paused($deps.as_ref())?;
        reentrancy_guard($deps.branch())?;
    };
}

/// Macro to apply admin-only checks
#[macro_export]
macro_rules! ensure_admin {
    ($deps:expr, $info:expr) => {
        let config = CONFIG.load($deps.storage)?;
        if config.admin != $info.sender {
            return Err(ContractError::Unauthorized {});
        }
    };
}

/// Macro to validate content inputs with basic checks
#[macro_export]
macro_rules! validate_content_inputs {
    ($title:expr, $description:expr) => {
        if $title.is_empty() {
            return Err(ContractError::InvalidInput {
                error: "Title cannot be empty".to_string(),
            });
        }
        if $title.len() > 200 {
            return Err(ContractError::InvalidInput {
                error: "Title too long".to_string(),
            });
        }
        if $description.is_empty() {
            return Err(ContractError::InvalidInput {
                error: "Description cannot be empty".to_string(),
            });
        }
        if $description.len() > 5000 {
            return Err(ContractError::InvalidInput {
                error: "Description too long".to_string(),
            });
        }
    };
}

/// Macro for building standard responses
#[macro_export]
macro_rules! build_success_response {
    ($method:expr, $id:expr, $user:expr) => {
        Response::new()
            .add_attributes(build_response_attributes($method, $id, $user, vec![]))
    };
    ($method:expr, $id:expr, $user:expr, $($key:expr => $value:expr),*) => {
        Response::new()
            .add_attributes(build_response_attributes($method, $id, $user, vec![$(($key, $value.to_string())),*]))
    };
}

// Advanced validation helpers
pub fn validate_string_field(
    value: &str,
    field_name: &str,
    min_length: usize,
    max_length: usize,
) -> Result<(), ContractError> {
    if value.is_empty() || value.len() < min_length || value.len() > max_length {
        return Err(ContractError::InvalidInput {
            error: format!(
                "{} must be between {}-{} characters",
                field_name, min_length, max_length
            ),
        });
    }
    Ok(())
}

pub fn validate_optional_string_field(
    field: &Option<String>,
    field_name: &str,
    max_length: usize,
) -> Result<(), ContractError> {
    if let Some(ref value) = field {
        validate_string_field(value, field_name, 1, max_length)?;
    }
    Ok(())
}

pub fn validate_collection_size<T>(
    collection: &[T],
    field_name: &str,
    min_count: usize,
    max_count: usize,
) -> Result<(), ContractError> {
    if collection.len() < min_count || collection.len() > max_count {
        return Err(ContractError::InvalidInput {
            error: format!("{} must have {}-{} items", field_name, min_count, max_count),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn validate_job_creation_inputs(
    title: &str,
    description: &str,
    budget: Uint128,
    category: &str,
    skills_required: &[String],
    duration_days: u64,
    company: &Option<String>,
    location: &Option<String>,
    max_duration_days: u64,
) -> Result<(), ContractError> {
    validate_content_inputs!(title, description);
    validate_budget(budget)?;
    validate_duration(duration_days, max_duration_days)?;
    validate_string_field(category, "Category", 1, 50)?;
    validate_collection_size(skills_required, "Skills required", 1, 20)?;
    validate_optional_string_field(company, "Company name", 100)?;
    validate_optional_string_field(location, "Location", 100)?;
    Ok(())
}



// Helper function to build standard job/bounty query responses
pub fn build_jobs_response(
    storage: &dyn cosmwasm_std::Storage,
    start_after: Option<u64>,
    limit: Option<u32>,
    _category: Option<String>,
    status: Option<JobStatus>,
    poster: Option<Addr>,
) -> StdResult<JobsResponse> {
    let jobs = query_jobs_paginated(storage, start_after, limit, status, poster)?;
    Ok(JobsResponse { jobs })
}

/// Helper function to validate user authorization for job/bounty operations
pub fn validate_user_authorization(job_poster: &Addr, user: &Addr) -> Result<(), ContractError> {
    if job_poster != user {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

/// Helper function to validate job status for operations
pub fn validate_job_status_for_operation(
    status: &JobStatus,
    allowed_statuses: &[JobStatus],
    operation: &str,
) -> Result<(), ContractError> {
    if !allowed_statuses.contains(status) {
        return Err(ContractError::InvalidInput {
            error: format!("Cannot {} job with status {:?}", operation, status),
        });
    }
    Ok(())
}

/// Helper function to validate bounty status for operations
pub fn validate_bounty_status_for_operation(
    status: &BountyStatus,
    allowed_statuses: &[BountyStatus],
    operation: &str,
) -> Result<(), ContractError> {
    if !allowed_statuses.contains(status) {
        return Err(ContractError::InvalidInput {
            error: format!("Cannot {} bounty with status {:?}", operation, status),
        });
    }
    Ok(())
}

/// Helper function to build standard response attributes
pub fn build_response_attributes(
    method: &str,
    id: u64,
    user: &Addr,
    additional_attrs: Vec<(&str, String)>,
) -> Vec<Attribute> {
    let mut attrs = vec![
        attr("method", method),
        attr("id", id.to_string()),
        attr("user", user.to_string()),
    ];

    for (key, value) in additional_attrs {
        attrs.push(attr(key, value));
    }

    attrs
}

/// Helper function to count items with a filter
pub fn count_items_with_filter<T, F>(
    storage: &dyn cosmwasm_std::Storage,
    map: &cw_storage_plus::Map<cosmwasm_std::Addr, T>,
    _filter: F,
) -> StdResult<u64>
where
    F: Fn(&T) -> bool,
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let count = map
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .count() as u64;
    Ok(count)
}
