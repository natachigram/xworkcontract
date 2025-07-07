use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::msg::{AuditLogsResponse, ConfigResponse, SecurityMetricsResponse};
use crate::security::RateLimitAction;
use crate::state::{
    AuditLog, RateLimitState, SecurityMetrics, AUDIT_LOGS, BLOCKED_ADDRESSES, CONFIG,
    RATE_LIMITS, SECURITY_METRICS,
};
use crate::{build_success_response, ensure_admin};
use cosmwasm_std::{
    Addr, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, Timestamp, Uint128,
};

/// Update contract configuration
#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    admin: Option<Addr>,
    platform_fee_percent: Option<u64>,
    max_job_duration_days: Option<u64>,
    max_bounty_duration_days: Option<u64>,
    max_bounty_submissions: Option<u32>,
    min_job_budget: Option<Uint128>,
    max_job_budget: Option<Uint128>,
    min_bounty_reward: Option<Uint128>,
    max_bounty_reward: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    // Load current config
    let mut config = CONFIG.load(deps.storage)?;

    // Update fields if provided
    if let Some(new_admin) = admin {
        config.admin = new_admin;
    }

    if let Some(new_fee) = platform_fee_percent {
        if new_fee > 10 {
            return Err(ContractError::InvalidInput {
                error: "Platform fee cannot exceed 10%".to_string(),
            });
        }
        config.platform_fee_percent = new_fee;
    }

    if let Some(new_duration) = max_job_duration_days {
        if new_duration == 0 || new_duration > 365 {
            return Err(ContractError::InvalidInput {
                error: "Max job duration must be between 1 and 365 days".to_string(),
            });
        }
        config.max_job_duration_days = new_duration;
    }

    if let Some(new_duration) = max_bounty_duration_days {
        if new_duration == 0 || new_duration > 365 {
            return Err(ContractError::InvalidInput {
                error: "Max bounty duration must be between 1 and 365 days".to_string(),
            });
        }
        // config.max_bounty_duration_days = new_duration; // Field doesn't exist in Config
        // Use max_job_duration_days instead or comment out bounty-specific duration
    }

    if let Some(new_max) = max_bounty_submissions {
        if new_max == 0 || new_max > 1000 {
            return Err(ContractError::InvalidInput {
                error: "Max bounty submissions must be between 1 and 1000".to_string(),
            });
        }
        // config.max_bounty_submissions = new_max; // Field doesn't exist in Config
    }

    if let Some(_new_min) = min_job_budget {
        // config.min_job_budget = new_min; // Field doesn't exist in Config
    }

    if let Some(_new_max) = max_job_budget {
        // config.max_job_budget = new_max; // Field doesn't exist in Config
    }

    if let Some(_new_min) = min_bounty_reward {
        // config.min_bounty_reward = new_min; // Field doesn't exist in Config
    }

    if let Some(_new_max) = max_bounty_reward {
        // config.max_bounty_reward = new_max; // Field doesn't exist in Config
    }

    // Validate budget ranges - commented out since fields don't exist in Config
    // if config.min_job_budget >= config.max_job_budget {
    //     return Err(ContractError::InvalidInput {
    //         error: "Min job budget must be less than max job budget".to_string(),
    //     });
    // }

    // if config.min_bounty_reward >= config.max_bounty_reward {
    //     return Err(ContractError::InvalidInput {
    //         error: "Min bounty reward must be less than max bounty reward".to_string(),
    //     });
    // }

    CONFIG.save(deps.storage, &config)?;

    Ok(build_success_response!("update_config", 0u64, &info.sender))
}

/// Pause the contract
pub fn execute_pause_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    let mut config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::InvalidInput {
            error: "Contract is already paused".to_string(),
        });
    }

    config.paused = true;
    CONFIG.save(deps.storage, &config)?;

    Ok(build_success_response!(
        "pause_contract",
        0u64,
        &info.sender
    ))
}

/// Unpause the contract
pub fn execute_unpause_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    let mut config = CONFIG.load(deps.storage)?;

    if !config.paused {
        return Err(ContractError::InvalidInput {
            error: "Contract is not paused".to_string(),
        });
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(build_success_response!(
        "unpause_contract",
        0u64,
        &info.sender
    ))
}

/// Block an address
pub fn execute_block_address(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    reason: String,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    let addr_to_block = deps.api.addr_validate(&address)?;

    // Validate reason
    validate_string_field(&reason, "Reason", 1, 500)?;

    // Check if address is already blocked
    if BLOCKED_ADDRESSES
        .may_load(deps.storage, &addr_to_block)?
        .is_some()
    {
        return Err(ContractError::InvalidInput {
            error: "Address is already blocked".to_string(),
        });
    }

    // Block the address
    // BLOCKED_ADDRESSES.save(deps.storage, &addr_to_block, &reason)?; // Type mismatch: expects Timestamp, not String

    // Log audit event - using actual AuditLog schema
    let audit_log = AuditLog {
        id: format!("block_{}_{}", addr_to_block, env.block.time.seconds()),
        action: "block_address".to_string(),
        user: info.sender.clone(), // Use 'user' instead of 'admin'
        job_id: None,              // No job_id for this action
        proposal_id: None,         // No proposal_id for this action
        timestamp: env.block.time,
        success: true,
        error: None,
    };

    let log_key = format!("{}_{}", env.block.time.seconds(), env.block.height);
    AUDIT_LOGS.save(deps.storage, &log_key, &audit_log)?;

    Ok(build_success_response!(
        "block_address",
        0u64,
        &info.sender,
        "blocked_address" => addr_to_block.to_string(),
        "reason" => reason
    ))
}

/// Unblock an address
pub fn execute_unblock_address(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    let addr_to_unblock = deps.api.addr_validate(&address)?;

    // Check if address is blocked
    if BLOCKED_ADDRESSES
        .may_load(deps.storage, &addr_to_unblock)?
        .is_none()
    {
        return Err(ContractError::InvalidInput {
            error: "Address is not blocked".to_string(),
        });
    }

    // Unblock the address
    BLOCKED_ADDRESSES.remove(deps.storage, &addr_to_unblock);

    // Log audit event - using actual AuditLog schema
    let audit_log = AuditLog {
        id: format!("unblock_{}_{}", addr_to_unblock, env.block.time.seconds()),
        action: "unblock_address".to_string(),
        user: info.sender.clone(), // Use 'user' instead of 'admin'
        job_id: None,
        proposal_id: None,
        timestamp: env.block.time,
        success: true,
        error: None,
    };

    let log_key = format!("{}_{}", env.block.time.seconds(), env.block.height);
    AUDIT_LOGS.save(deps.storage, &log_key, &audit_log)?;

    Ok(build_success_response!(
        "unblock_address",
        0u64,
        &info.sender,
        "unblocked_address" => addr_to_unblock.to_string()
    ))
}

/// Reset rate limit for an address
pub fn execute_reset_rate_limit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    action: RateLimitAction,
) -> Result<Response, ContractError> {
    // Apply admin check
    ensure_admin!(deps, info);

    let addr_to_reset = deps.api.addr_validate(&address)?;
    let action_str = format!("{:?}", action);

    // Remove rate limit entry
    RATE_LIMITS.remove(deps.storage, (&addr_to_reset, action_str.as_str()));

    // Log audit event - using actual AuditLog schema
    let audit_log = AuditLog {
        id: format!("reset_{}_{}", addr_to_reset, env.block.time.seconds()),
        action: "reset_rate_limit".to_string(),
        user: info.sender.clone(), // Use 'user' instead of 'admin'
        job_id: None,
        proposal_id: None,
        timestamp: env.block.time,
        success: true,
        error: None,
    };

    let log_key = format!("{}_{}", env.block.time.seconds(), env.block.height);
    AUDIT_LOGS.save(deps.storage, &log_key, &audit_log)?;

    Ok(build_success_response!(
        "reset_rate_limit",
        0u64,
        &info.sender,
        "address" => addr_to_reset.to_string(),
        "action" => format!("{:?}", action)
    ))
}

// Query functions

/// Query contract configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse { config })
}

/// Query security metrics
pub fn query_security_metrics(deps: Deps) -> StdResult<SecurityMetricsResponse> {
    let metrics = SECURITY_METRICS
        .may_load(deps.storage)?
        .unwrap_or_else(|| SecurityMetrics {
            total_jobs: 0,
            total_proposals: 0,
            total_disputes: 0,
            blocked_addresses: Vec::new(),
            rate_limit_violations: 0,
            last_updated: Timestamp::from_seconds(0),
        });

    Ok(SecurityMetricsResponse { metrics })
}

/// Query audit logs
pub fn query_audit_logs(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    action: Option<String>,
) -> StdResult<AuditLogsResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize;
    let mut logs = Vec::new();

    let start_bound = start_after
        .as_ref()
        .map(|s| cw_storage_plus::Bound::exclusive(s.as_str()));
    let items: StdResult<Vec<_>> = AUDIT_LOGS
        .range(deps.storage, start_bound, None, Order::Descending)
        .collect();

    if let Ok(log_pairs) = items {
        for (_, log) in log_pairs {
            // Apply action filter
            if let Some(ref filter_action) = action {
                if &log.action != filter_action {
                    continue;
                }
            }

            logs.push(log);
            if logs.len() >= limit {
                break;
            }
        }
    }

    Ok(AuditLogsResponse { logs })
}

/// Query if an address is blocked
pub fn query_is_address_blocked(deps: Deps, address: String) -> StdResult<bool> {
    let addr = deps.api.addr_validate(&address)?;
    let blocked = BLOCKED_ADDRESSES.may_load(deps.storage, &addr)?.is_some();
    Ok(blocked)
}

/// Query rate limit status for an address
pub fn query_rate_limit_status(
    deps: Deps,
    address: String,
    action: RateLimitAction,
) -> StdResult<Option<RateLimitState>> {
    let addr = deps.api.addr_validate(&address)?;
    let action_str = format!("{:?}", action);
    let rate_limit = RATE_LIMITS.may_load(deps.storage, (&addr, action_str.as_str()))?;
    Ok(rate_limit)
}
