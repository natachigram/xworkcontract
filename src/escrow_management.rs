use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::helpers::ensure_not_paused;
use crate::job_management::calculate_platform_fee;
use crate::msg::{EscrowResponse, EscrowsResponse};
use crate::security::{check_rate_limit, reentrancy_guard, RateLimitAction};
use crate::state::{DisputeStatus, EscrowState, EscrowStatus, BOUNTIES, ESCROWS, JOBS};
use crate::{apply_security_checks, build_success_response};
use cosmwasm_std::{
    coins, Addr, BankMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult,
    Uint128,
};

/// Create escrow for job or bounty
#[allow(clippy::too_many_arguments)]
pub fn execute_create_escrow(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: String,
    job_id: Option<u64>,
    bounty_id: Option<u64>,
    amount: Uint128,
    recipient: Option<Addr>,
    _conditions: Vec<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CreateEscrow);

    // Validate inputs
    if job_id.is_none() && bounty_id.is_none() {
        return Err(ContractError::InvalidInput {
            error: "Either job_id or bounty_id must be provided".to_string(),
        });
    }

    if job_id.is_some() && bounty_id.is_some() {
        return Err(ContractError::InvalidInput {
            error: "Cannot provide both job_id and bounty_id".to_string(),
        });
    }

    // Validate payment
    if info.funds.len() != 1 || info.funds[0].amount != amount {
        return Err(ContractError::InvalidFunds {});
    }

    // Check if escrow already exists
    if ESCROWS.may_load(deps.storage, &escrow_id)?.is_some() {
        return Err(ContractError::InvalidInput {
            error: "Escrow already exists".to_string(),
        });
    }

    // Validate job or bounty exists
    if let Some(job_id_val) = job_id {
        JOBS.load(deps.storage, job_id_val)?;
    }

    if let Some(bounty_id_val) = bounty_id {
        BOUNTIES.load(deps.storage, bounty_id_val)?;
    }

    // Create escrow using EscrowState schema
    let escrow = EscrowState {
        id: escrow_id.clone(),
        job_id: job_id.unwrap_or(0), // Convert Option<u64> to u64, default 0 for bounties
        client: info.sender.clone(), // Use 'client' instead of 'depositor'
        freelancer: recipient.unwrap_or_else(|| info.sender.clone()), // Use 'freelancer' instead of 'recipient'
        amount,
        platform_fee: calculate_platform_fee(amount, 5), // Use reasonable default fee
        funded_at: env.block.time,
        released: false, // Use boolean instead of status enum
        dispute_status: DisputeStatus::None,
        dispute_raised_at: None,
        dispute_deadline: None,
    };

    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    Ok(build_success_response!(
        "create_escrow",
        0u64, // Using 0 as placeholder since escrow uses string ID
        &info.sender,
        "escrow_id" => escrow_id,
        "amount" => amount.to_string()
    ))
}

/// Release escrow funds
pub fn execute_release_escrow(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: String,
    recipient: Addr,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::ReleaseEscrow);

    // Load and validate escrow
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;

    // Check authorization - only client can release
    validate_user_authorization(&escrow.client, &info.sender)?;

    // Check escrow status
    if escrow.released {
        return Err(ContractError::InvalidInput {
            error: "Escrow has already been released".to_string(),
        });
    }

    // Update escrow
    escrow.released = true; // Use boolean instead of status
    escrow.freelancer = recipient.clone(); // Update freelancer field
                                           // Note: released_at field doesn't exist in EscrowState schema
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    let mut response = build_success_response!(
        "release_escrow",
        0u64,
        &info.sender,
        "escrow_id" => escrow_id,
        "recipient" => recipient.to_string(),
        "amount" => escrow.amount.to_string()
    );

    // Add bank message to release funds
    response = response.add_message(BankMsg::Send {
        to_address: recipient.to_string(),
        amount: coins(escrow.amount.u128(), "uusdc"),
    });

    Ok(response)
}

/// Refund escrow funds back to depositor
pub fn execute_refund_escrow(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: String,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::RefundEscrow);

    // Load and validate escrow
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;

    // Check authorization - only depositor can refund
    validate_user_authorization(&escrow.client, &info.sender)?;

    // Check escrow status
    if !escrow.released {
        return Err(ContractError::InvalidInput {
            error: "Escrow is not in pending status".to_string(),
        });
    }

    // Update escrow
    escrow.released = true;
    // escrow.updated_at = Some(env.block.time); // updated_at field doesn't exist
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    let mut response = build_success_response!(
        "refund_escrow",
        0u64,
        &info.sender,
        "escrow_id" => escrow_id,
        "amount" => escrow.amount.to_string()
    );

    // Add bank message to refund funds
    response = response.add_message(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(escrow.amount.u128(), "uusdc"),
    });

    Ok(response)
}

// Query functions

/// Query a specific escrow
pub fn query_escrow(deps: Deps, escrow_id: String) -> StdResult<EscrowResponse> {
    let escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    Ok(EscrowResponse { escrow })
}

/// Query job escrow
pub fn query_job_escrow(deps: Deps, job_id: u64) -> StdResult<EscrowResponse> {
    let escrow_id = format!("job_{}", job_id);
    query_escrow(deps, escrow_id)
}

/// Query bounty escrow
pub fn query_bounty_escrow(deps: Deps, bounty_id: u64) -> StdResult<EscrowResponse> {
    let escrow_id = format!("bounty_{}", bounty_id);
    query_escrow(deps, escrow_id)
}

/// Query user's escrows (as depositor)
pub fn query_user_escrows(
    deps: Deps,
    user: String,
    status: Option<EscrowStatus>,
) -> StdResult<EscrowsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;

    let escrows: Vec<_> = ESCROWS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, escrow)) = item {
                if escrow.client == user_addr {
                    if let Some(ref filter_status) = status {
                        // Convert bool released field to EscrowStatus for comparison
                        let escrow_status = if escrow.released {
                            EscrowStatus::Released
                        } else {
                            EscrowStatus::Pending
                        };
                        if escrow_status == *filter_status {
                            Some(escrow)
                        } else {
                            None
                        }
                    } else {
                        Some(escrow)
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(EscrowsResponse { escrows })
}

/// Query escrows with pagination and filtering
pub fn query_escrows(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    status: Option<EscrowStatus>,
    depositor: Option<String>,
) -> StdResult<EscrowsResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize;
    let mut escrows = Vec::new();

    let depositor_addr = if let Some(d) = depositor {
        Some(deps.api.addr_validate(&d)?)
    } else {
        None
    };

    let start_bound = start_after
        .as_ref()
        .map(|s| cw_storage_plus::Bound::exclusive(s.as_str()));
    let items: StdResult<Vec<_>> = ESCROWS
        .range(deps.storage, start_bound, None, Order::Ascending)
        .collect();

    if let Ok(escrow_pairs) = items {
        for (_, escrow) in escrow_pairs {
            // Apply filters
            let mut include = true;

            if let Some(ref filter_status) = status {
                // Convert bool released field to EscrowStatus for comparison
                let escrow_status = if escrow.released {
                    EscrowStatus::Released
                } else {
                    EscrowStatus::Pending
                };
                if escrow_status != *filter_status {
                    include = false;
                }
            }

            if let Some(ref filter_depositor) = depositor_addr {
                if escrow.client != *filter_depositor {
                    include = false;
                }
            }

            if include {
                escrows.push(escrow);
                if escrows.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(EscrowsResponse { escrows })
}
