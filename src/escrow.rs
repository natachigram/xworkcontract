use cosmwasm_std::{
    Addr, BankMsg, Coin, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg, to_json_binary, Binary, Decimal
};
use cw_utils::must_pay;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::security::{
    reentrancy_guard, generate_escrow_id
};
use crate::state::{
    EscrowState, DisputeStatus, Dispute,
    AuditLog, ESCROWS, CONFIG, DISPUTES, AUDIT_LOGS,
    JOBS, USER_STATS
};

const DISPUTE_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60; // 7 days
const XION_DENOM: &str = "uxion";

#[derive(serde::Deserialize)]
struct EscrowHookMsg {
    job_id: u64,
}

// Enhanced escrow creation with CW20 support and security
pub fn create_escrow_native(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
) -> Result<Response, ContractError> {
    // Security check - reentrancy guard
    reentrancy_guard(deps.branch())?;
    
    let result = create_escrow_internal(deps.branch(), env.clone(), info.clone(), job_id, None, None);
    
    // Log the action
    let log_id = generate_escrow_id(job_id, &info.sender, &info.sender, env.block.time.seconds());
    let audit_log = AuditLog {
        id: log_id.clone(),
        action: "create_escrow".to_string(),
        user: info.sender,
        job_id: Some(job_id),
        proposal_id: None,
        timestamp: env.block.time,
        success: result.is_ok(),
        error: result.as_ref().err().map(|e| e.to_string()),
    };
    AUDIT_LOGS.save(deps.storage, &log_id, &audit_log)?;
    
    result
}

// CW20 token escrow support
pub fn create_escrow_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let hook_msg: EscrowHookMsg = cosmwasm_std::from_json(&msg)?;
    let token_contract = info.sender.clone();
    
    create_escrow_internal(deps, env, info, hook_msg.job_id, Some(amount), Some(token_contract))
}

pub fn create_escrow_internal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    cw20_amount: Option<Uint128>,
    token_contract: Option<Addr>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    // Check if contract is paused
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }

    let job = JOBS.load(deps.storage, job_id)?;
    
    // Cannot create escrow for free projects
    if job.budget.is_zero() {
        return Err(ContractError::InvalidInput {
            error: "Cannot create escrow for free projects".to_string(),
        });
    }
    
    // Only job poster can create escrow
    if job.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    
    // Job must be in progress
    if job.status != crate::state::JobStatus::InProgress {
        return Err(ContractError::InvalidInput {
            error: "Job must be in progress to create escrow".to_string(),
        });
    }
    
    // Check if escrow already exists
    if job.escrow_id.is_some() {
        return Err(ContractError::EscrowAlreadyExists { job_id });
    }
    
    // Validate payment amount
    let payment_amount = if let Some(amount) = cw20_amount {
        amount
    } else {
        must_pay(&info, XION_DENOM)?
    };
    
    if payment_amount < job.budget {
        return Err(ContractError::InsufficientFunds {
            expected: job.budget.to_string(),
            actual: payment_amount.to_string(),
        });
    }
    
    if payment_amount < config.min_escrow_amount {
        return Err(ContractError::EscrowAmountTooLow {
            min: config.min_escrow_amount.to_string(),
        });
    }
    
    // Calculate platform fee (max 10%)
    let platform_fee = payment_amount
        .checked_mul(Uint128::from(config.platform_fee_percent))?
        .checked_div(Uint128::from(100u128))?;
    let freelancer_amount = payment_amount.checked_sub(platform_fee)?;
    
    // Generate unique escrow ID
    let escrow_id = format!("escrow_{}_{}", job_id, env.block.time.seconds());
    
    // Create escrow state
    let escrow = EscrowState {
        id: escrow_id.clone(),
        job_id,
        client: job.poster.clone(),
        freelancer: job.assigned_freelancer.clone().unwrap(),
        amount: freelancer_amount,
        platform_fee,
        funded_at: env.block.time,
        released: false,
        dispute_status: DisputeStatus::None,
        dispute_raised_at: None,
        dispute_deadline: None,
    };
    
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    // Update job with escrow ID
    let mut updated_job = job;
    updated_job.escrow_id = Some(escrow_id.clone());
    updated_job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &updated_job)?;
    
    // Send CW20 tokens to escrow contract if applicable
    if let Some(amount) = cw20_amount {
        let msg = Cw20ExecuteMsg::Transfer {
            recipient: escrow_id.clone(),
            amount,
        };
        let transfer_msg = WasmMsg::Execute {
            contract_addr: token_contract.unwrap().to_string(),
            msg: to_json_binary(&msg)?,
            funds: vec![],
        };
        
        return Ok(Response::new()
            .add_message(transfer_msg)
            .add_attribute("method", "create_escrow")
            .add_attribute("job_id", job_id.to_string())
            .add_attribute("escrow_id", escrow_id)
            .add_attribute("amount", payment_amount.to_string())
            .add_attribute("platform_fee", platform_fee.to_string()));
    }
    
    Ok(Response::new()
        .add_attribute("method", "create_escrow")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("escrow_id", escrow_id)
        .add_attribute("amount", payment_amount.to_string())
        .add_attribute("platform_fee", platform_fee.to_string()))
}

pub fn release_escrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    let job = JOBS.load(deps.storage, escrow.job_id)?;
    
    // Check if contract is paused
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    
    // Check authorization
    let can_release = info.sender == escrow.client || 
        (job.status == crate::state::JobStatus::Completed && 
         env.block.time.seconds() > (escrow.funded_at.seconds() + DISPUTE_PERIOD_SECONDS));
    
    if !can_release {
        return Err(ContractError::Unauthorized {});
    }
    
    if escrow.released {
        return Err(ContractError::InvalidInput {
            error: "Escrow already released".to_string(),
        });
    }
    
    // Check if dispute is active
    if escrow.dispute_status == DisputeStatus::Raised || 
       escrow.dispute_status == DisputeStatus::UnderReview {
        return Err(ContractError::DisputePeriodActive {});
    }
    
    escrow.released = true;
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    let freelancer_msg = BankMsg::Send {
        to_address: escrow.freelancer.to_string(),
        amount: vec![Coin {
            denom: XION_DENOM.to_string(),
            amount: escrow.amount,
        }],
    };
    
    let platform_msg = BankMsg::Send {
        to_address: config.admin.to_string(),
        amount: vec![Coin {
            denom: XION_DENOM.to_string(),
            amount: escrow.platform_fee,
        }],
    };
    
    // Update user stats
    update_user_stats_on_completion(deps.storage, &escrow.client, &escrow.freelancer, escrow.amount)?;
    
    Ok(Response::new()
        .add_message(freelancer_msg)
        .add_message(platform_msg)
        .add_attribute("method", "release_escrow")
        .add_attribute("escrow_id", escrow_id)
        .add_attribute("amount", escrow.amount.to_string()))
}

pub fn refund_escrow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    escrow_id: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    
    // Only admin can refund (for dispute resolution)
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    
    if escrow.released {
        return Err(ContractError::InvalidInput {
            error: "Escrow already released".to_string(),
        });
    }
    
    escrow.released = true;
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    let total_amount = escrow.amount.checked_add(escrow.platform_fee)?;
    let refund_msg = BankMsg::Send {
        to_address: escrow.client.to_string(),
        amount: vec![Coin {
            denom: XION_DENOM.to_string(),
            amount: total_amount,
        }],
    };
    
    Ok(Response::new()
        .add_message(refund_msg)
        .add_attribute("method", "refund_escrow")
        .add_attribute("escrow_id", escrow_id)
        .add_attribute("amount", total_amount.to_string()))
}

pub fn raise_dispute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    reason: String,
    evidence: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let job = JOBS.load(deps.storage, job_id)?;
    
    // Check if contract is paused
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    
    // Only client or freelancer can raise dispute
    if info.sender != job.poster && 
       Some(info.sender.clone()) != job.assigned_freelancer {
        return Err(ContractError::Unauthorized {});
    }
    
    // Job must be in progress or completed
    if job.status != crate::state::JobStatus::InProgress && 
       job.status != crate::state::JobStatus::Completed {
        return Err(ContractError::InvalidInput {
            error: "Can only dispute active or completed jobs".to_string(),
        });
    }
    
    // Check if escrow exists
    let escrow_id = job.escrow_id.clone().ok_or(ContractError::EscrowNotFound {})?;
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    
    // Check if dispute already exists
    if escrow.dispute_status != DisputeStatus::None {
        return Err(ContractError::InvalidInput {
            error: "Dispute already exists for this job".to_string(),
        });
    }
    
    // Validate inputs
    if reason.is_empty() || reason.len() > 1000 {
        return Err(ContractError::InvalidInput {
            error: "Dispute reason must be between 1-1000 characters".to_string(),
        });
    }
    
    // Create dispute
    let dispute_id = format!("dispute_{}_{}", job_id, env.block.time.seconds());
    let dispute_deadline = env.block.time.plus_seconds(config.dispute_period_days * 24 * 60 * 60);
    
    let dispute = Dispute {
        id: dispute_id.clone(),
        job_id,
        raised_by: info.sender.clone(),
        reason,
        evidence,
        status: DisputeStatus::Raised,
        created_at: env.block.time,
        resolved_at: None,
        resolution: None,
    };
    
    DISPUTES.save(deps.storage, &dispute_id, &dispute)?;
    
    // Update escrow status
    escrow.dispute_status = DisputeStatus::Raised;
    escrow.dispute_raised_at = Some(env.block.time);
    escrow.dispute_deadline = Some(dispute_deadline);
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    // Update job status
    let mut updated_job = job;
    updated_job.status = crate::state::JobStatus::Disputed;
    updated_job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &updated_job)?;
    
    Ok(Response::new()
        .add_attribute("method", "raise_dispute")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("dispute_id", dispute_id)
        .add_attribute("raised_by", info.sender.to_string()))
}

pub fn resolve_dispute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dispute_id: String,
    resolution: String,
    release_to_freelancer: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    // Only admin can resolve disputes
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    
    let mut dispute = DISPUTES.load(deps.storage, &dispute_id)?;
    
    if dispute.status != DisputeStatus::Raised && 
       dispute.status != DisputeStatus::UnderReview {
        return Err(ContractError::InvalidInput {
            error: "Dispute already resolved".to_string(),
        });
    }
    
    // Validate resolution
    if resolution.is_empty() || resolution.len() > 2000 {
        return Err(ContractError::InvalidInput {
            error: "Resolution must be between 1-2000 characters".to_string(),
        });
    }
    
    // Update dispute
    dispute.status = DisputeStatus::Resolved;
    dispute.resolved_at = Some(env.block.time);
    dispute.resolution = Some(resolution.clone());
    DISPUTES.save(deps.storage, &dispute_id, &dispute)?;
    
    // Get job and escrow
    let mut job = JOBS.load(deps.storage, dispute.job_id)?;
    let escrow_id = job.escrow_id.clone().ok_or(ContractError::EscrowNotFound {})?;
    let mut escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    
    // Update escrow and job status
    escrow.dispute_status = DisputeStatus::Resolved;
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    job.status = if release_to_freelancer {
        crate::state::JobStatus::Completed
    } else {
        crate::state::JobStatus::Cancelled
    };
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, dispute.job_id, &job)?;
    
    // Release funds based on resolution
    let mut response = Response::new()
        .add_attribute("method", "resolve_dispute")
        .add_attribute("dispute_id", dispute_id)
        .add_attribute("resolution", resolution)
        .add_attribute("release_to_freelancer", release_to_freelancer.to_string());
    
    if release_to_freelancer {
        // Release to freelancer
        response = response.add_message(BankMsg::Send {
            to_address: escrow.freelancer.to_string(),
            amount: vec![Coin {
                denom: XION_DENOM.to_string(),
                amount: escrow.amount,
            }],
        });
        
        // Platform fee to admin
        response = response.add_message(BankMsg::Send {
            to_address: config.admin.to_string(),
            amount: vec![Coin {
                denom: XION_DENOM.to_string(),
                amount: escrow.platform_fee,
            }],
        });
        
        // Update user stats for successful completion
        update_user_stats_on_completion(deps.storage, &escrow.client, &escrow.freelancer, escrow.amount)?;
    } else {
        // Refund to client (minus platform fee for dispute resolution)
        let refund_amount = escrow.amount;
        response = response.add_message(BankMsg::Send {
            to_address: escrow.client.to_string(),
            amount: vec![Coin {
                denom: XION_DENOM.to_string(),
                amount: refund_amount,
            }],
        });
        
        // Platform fee to admin
        response = response.add_message(BankMsg::Send {
            to_address: config.admin.to_string(),
            amount: vec![Coin {
                denom: XION_DENOM.to_string(),
                amount: escrow.platform_fee,
            }],
        });
    }
    
    // Mark escrow as released
    escrow.released = true;
    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    
    Ok(response)
}

// Helper function to update user statistics
fn update_user_stats_on_completion(
    storage: &mut dyn cosmwasm_std::Storage,
    client: &Addr,
    freelancer: &Addr,
    amount: Uint128,
) -> StdResult<()> {
    // Update client stats
    let mut client_stats = USER_STATS.may_load(storage, client)?.unwrap_or_default();
    client_stats.total_spent = client_stats.total_spent.checked_add(amount)?;
    USER_STATS.save(storage, client, &client_stats)?;
    
    // Update freelancer stats
    let mut freelancer_stats = USER_STATS.may_load(storage, freelancer)?.unwrap_or_default();
    freelancer_stats.total_earned = freelancer_stats.total_earned.checked_add(amount)?;
    freelancer_stats.total_jobs_completed = freelancer_stats.total_jobs_completed.checked_add(1)
        .ok_or_else(|| cosmwasm_std::StdError::overflow(cosmwasm_std::OverflowError::new(
            cosmwasm_std::OverflowOperation::Add, 
            "jobs completed counter", 
            "overflow"
        )))?;
    
    // Calculate completion rate
    if freelancer_stats.total_jobs_posted > 0 {
        freelancer_stats.completion_rate = Decimal::from_ratio(
            freelancer_stats.total_jobs_completed, 
            freelancer_stats.total_jobs_posted
        );
    }
    
    USER_STATS.save(storage, freelancer, &freelancer_stats)?;
    
    Ok(())
}

// Backward compatibility alias
pub fn create_escrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
) -> Result<Response, ContractError> {
    create_escrow_native(deps, env, info, job_id)
}
