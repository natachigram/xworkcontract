use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::hash_utils::{
    create_content_hash, create_job_content_bundle, create_proposal_content_bundle,
};
use crate::helpers::{ensure_not_paused, get_future_timestamp, validate_budget, validate_duration};
use crate::msg::{JobResponse, JobsResponse, MilestoneInput, ProposalResponse, ProposalsResponse};
use crate::security::{check_rate_limit, reentrancy_guard, RateLimitAction};
use crate::state::{
    ContactPreference, Job, JobStatus, Proposal, ProposalMilestone, ProposalStatus, Rating, CONFIG,
    CONTENT_HASHES, DISPUTES, ENTITY_TO_HASH, ESCROWS, HASH_TO_ENTITY, JOBS, JOB_PROPOSALS,
    NEXT_JOB_ID, NEXT_PROPOSAL_ID, PROPOSALS, RATINGS,
};
// Import macros explicitly
use crate::{apply_security_checks, build_success_response, ensure_admin, validate_content_inputs};
// Remove the explicit crate prefixes for macros
use cosmwasm_std::{
    coins, Addr, BankMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, Uint128,
};

/// Helper function to calculate platform fee
pub fn calculate_platform_fee(amount: Uint128, fee_percent: u64) -> Uint128 {
    amount * Uint128::from(fee_percent) / Uint128::from(100u64)
}

/// 🎯 Create a new job posting with hybrid on-chain/off-chain storage
#[allow(clippy::too_many_arguments)]
pub fn execute_post_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    budget: Uint128,
    category: String,
    skills_required: Vec<String>,
    duration_days: u64,
    company: Option<String>,
    location: Option<String>,
    documents: Option<Vec<String>>,
    _milestones: Option<Vec<MilestoneInput>>,
    _experience_level: u8,
    _is_remote: bool,
    _urgency_level: u8,
    off_chain_storage_key: String,
) -> Result<Response, ContractError> {
    // 🔒 Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::PostJob);

    // 📋 Load configuration and validate
    let config = CONFIG.load(deps.storage)?;
    validate_content_inputs!(&title, &description);
    validate_budget(budget)?;
    validate_duration(duration_days, config.max_job_duration_days)?;

    // 💰 Validate payment
    if budget.is_zero() {
        if !info.funds.is_empty() {
            return Err(ContractError::InvalidFunds {});
        }
    } else {
        if info.funds.len() != 1 || info.funds[0].amount != budget {
            return Err(ContractError::InvalidFunds {});
        }
    }

    // 🆔 Generate job ID
    let job_id = NEXT_JOB_ID.load(deps.storage)?;
    NEXT_JOB_ID.save(deps.storage, &(job_id + 1))?;

    // 🌐 Create off-chain content bundle
    let documents_vec = documents.unwrap_or_default();
    let (off_chain_bundle, content_hash_str) = create_job_content_bundle(
        job_id,
        &title,
        &description,
        company.as_deref(),
        location.as_deref(),
        &category,
        &skills_required,
        &documents_vec,
        env.block.time.seconds(),
    )?;

    // 📄 Create content hash metadata
    let content_hash = create_content_hash(
        &serde_json::to_string(&off_chain_bundle).map_err(|e| ContractError::InvalidInput {
            error: format!("Serialization error: {}", e),
        })?,
        "job_content",
        env.block.time.seconds(),
    )?;

    // 🗄️ Store hash mappings for retrieval
    let entity_key = format!("job_{}", job_id);
    CONTENT_HASHES.save(deps.storage, &content_hash_str, &content_hash)?;
    HASH_TO_ENTITY.save(deps.storage, &content_hash_str, &entity_key)?;
    ENTITY_TO_HASH.save(deps.storage, &entity_key, &content_hash_str)?;

    // 🎯 Create optimized on-chain job record
    let job = Job {
        id: job_id,
        poster: info.sender.clone(),
        budget,
        duration_days,
        status: JobStatus::Open,
        assigned_freelancer: None,
        created_at: env.block.time,
        updated_at: env.block.time,
        deadline: get_future_timestamp(env.block.time, duration_days),
        escrow_id: Some(format!("job_{}", job_id)),
        total_proposals: 0,
        content_hash,
    };

    JOBS.save(deps.storage, job_id, &job)?;

    //  Create escrow
    let escrow_id = format!("job_{}", job_id);
    let escrow = crate::state::EscrowState {
        id: escrow_id.clone(),
        job_id,
        client: info.sender.clone(),
        freelancer: Addr::unchecked(""), // Will be set when job is assigned
        amount: budget,
        platform_fee: calculate_platform_fee(budget, config.platform_fee_percent),
        funded_at: env.block.time,
        released: false,
        dispute_status: crate::state::DisputeStatus::None,
        dispute_raised_at: None,
        dispute_deadline: None,
    };

    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    // 🎉 Return success response with essential info
    Ok(build_success_response!(
        "post_job",
        job_id,
        &info.sender,
        "budget" => budget.to_string(),
        "content_hash" => content_hash_str,
        "off_chain_key" => off_chain_storage_key,
        "escrow_id" => escrow_id
    ))
}

#[allow(clippy::too_many_arguments)]
/// 🎯 Submit a proposal with hybrid on-chain/off-chain storage
#[allow(clippy::too_many_arguments)]
pub fn execute_submit_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    cover_letter: String,
    delivery_time_days: u64,
    contact_preference: ContactPreference,
    agreed_to_terms: bool,
    agreed_to_escrow: bool,
    milestones: Option<Vec<crate::state::ProposalMilestone>>,
    _portfolio_samples: Option<Vec<String>>,
    _off_chain_storage_key: String,
) -> Result<Response, ContractError> {
    // 🔒 Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::SubmitProposal);

    // 📋 Load and validate job
    let mut job = JOBS
        .load(deps.storage, job_id)
        .map_err(|_| ContractError::JobNotFound {})?;
    validate_job_status_for_operation(&job.status, &[JobStatus::Open], "submit proposal to")?;

    // 🔍 Validate inputs
    validate_content_inputs!(&cover_letter, &cover_letter);

    let config = CONFIG.load(deps.storage)?;
    validate_duration(delivery_time_days, config.max_job_duration_days)?;

    // ❌ Check if user already has a proposal for this job
    let existing_proposals: Vec<_> = PROPOSALS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            if let Ok((_, proposal)) = item {
                if proposal.job_id == job_id && proposal.freelancer == info.sender {
                    Some(proposal)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    if !existing_proposals.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "You already have a proposal for this job".to_string(),
        });
    }

    // 🆔 Generate proposal ID
    let proposal_id = NEXT_PROPOSAL_ID.load(deps.storage)?;
    NEXT_PROPOSAL_ID.save(deps.storage, &(proposal_id + 1))?;

    // 🌐 Create off-chain content bundle
    let milestones_json = milestones
        .as_ref()
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
        .unwrap_or(serde_json::Value::Null);

    let (off_chain_bundle, content_hash_str) = create_proposal_content_bundle(
        proposal_id,
        &cover_letter,
        &[milestones_json],
        env.block.time.seconds(),
    )?;

    // 📄 Create content hash metadata
    let content_hash = create_content_hash(
        &serde_json::to_string(&off_chain_bundle).map_err(|e| ContractError::InvalidInput {
            error: format!("Serialization error: {}", e),
        })?,
        "proposal_content",
        env.block.time.seconds(),
    )?;

    // ️ Store hash mappings
    let entity_key = format!("proposal_{}", proposal_id);
    CONTENT_HASHES.save(deps.storage, &content_hash_str, &content_hash)?;
    HASH_TO_ENTITY.save(deps.storage, &content_hash_str, &entity_key)?;
    ENTITY_TO_HASH.save(deps.storage, &entity_key, &content_hash_str)?;

    // 🎯 Create optimized on-chain proposal record
    let proposal = Proposal {
        id: proposal_id,
        freelancer: info.sender.clone(),
        job_id,
        delivery_time_days,
        contact_preference,
        agreed_to_terms,
        agreed_to_escrow,
        submitted_at: env.block.time,
        content_hash,
    };

    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    // Update job proposals mapping
    let mut job_proposals = JOB_PROPOSALS
        .may_load(deps.storage, job_id)?
        .unwrap_or_default();
    job_proposals.push(proposal_id);
    JOB_PROPOSALS.save(deps.storage, job_id, &job_proposals)?;

    // Update job proposal count
    job.total_proposals += 1;
    JOBS.save(deps.storage, job_id, &job)?;

    Ok(build_success_response!(
        "submit_proposal",
        proposal_id,
        &info.sender,
        "job_id" => job_id.to_string(),
        "delivery_time_days" => delivery_time_days.to_string()
    ))
}

/// 🎯 Edit an existing job with hybrid on-chain/off-chain storage
#[allow(clippy::too_many_arguments)]
pub fn execute_edit_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    title: Option<String>,
    description: Option<String>,
    budget: Option<Uint128>,
    category: Option<String>,
    skills_required: Option<Vec<String>>,
    duration_days: Option<u64>,
    documents: Option<Vec<String>>,
    milestones: Option<Vec<MilestoneInput>>,
    off_chain_storage_key: String,
) -> Result<Response, ContractError> {
    // 🔒 Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::EditJob);

    // 📋 Load and validate job
    let mut job = JOBS
        .load(deps.storage, job_id)
        .map_err(|_| ContractError::JobNotFound {})?;
    validate_user_authorization(&job.poster, &info.sender)?;
    validate_job_status_for_operation(&job.status, &[JobStatus::Open], "edit")?;

    let config = CONFIG.load(deps.storage)?;

    // 🔄 Track what changed for off-chain updates
    let mut content_changed = false;
    let mut metadata_changed = false;

    // ⚡ Update on-chain metadata fields if they changed
    if let Some(new_budget) = budget {
        validate_budget(new_budget)?;
        if job.budget != new_budget {
            job.budget = new_budget;
            metadata_changed = true;
        }
    }

    if let Some(new_duration) = duration_days {
        validate_duration(new_duration, config.max_job_duration_days)?;
        if job.duration_days != new_duration {
            job.duration_days = new_duration;
            job.deadline = get_future_timestamp(env.block.time, new_duration);
            metadata_changed = true;
        }
    }

    // 🏷️ Update category and skills if changed
    // 🌐 If content fields changed, create new off-chain bundle
    if title.is_some()
        || description.is_some()
        || category.is_some()
        || skills_required.is_some()
        || documents.is_some()
        || milestones.is_some()
    {
        content_changed = true;
    }

    if content_changed {
        // 📦 Need to fetch existing content and update it
        // In production, this would fetch from off-chain storage using the current hash
        // For now, we'll create a new bundle with provided values

        let final_title = title.unwrap_or_else(|| "Updated Job".to_string()); // In real app, fetch from off-chain
        let final_description = description.unwrap_or_else(|| "Updated Description".to_string());
        let final_category = category.unwrap_or_else(|| "General".to_string());
        let final_skills = skills_required.unwrap_or_default();
        let final_documents = documents.unwrap_or_default();

        // 🔍 Validate content inputs
        validate_content_inputs!(&final_title, &final_description);

        // 🌐 Create new off-chain content bundle
        let (off_chain_bundle, content_hash_str) = create_job_content_bundle(
            job_id,
            &final_title,
            &final_description,
            None, // company - would be fetched from existing data
            None, // location - would be fetched from existing data
            &final_category,
            &final_skills,
            &final_documents,
            env.block.time.seconds(),
        )?;

        // 📄 Create new content hash
        let content_hash = create_content_hash(
            &serde_json::to_string(&off_chain_bundle).map_err(|e| ContractError::InvalidInput {
                error: format!("Serialization error: {}", e),
            })?,
            "job_content",
            env.block.time.seconds(),
        )?;

        // 🗄️ Update hash mappings
        let entity_key = format!("job_{}", job_id);

        // Remove old hash mapping
        if let Ok(old_hash) = ENTITY_TO_HASH.load(deps.storage, &entity_key) {
            CONTENT_HASHES.remove(deps.storage, &old_hash);
            HASH_TO_ENTITY.remove(deps.storage, &old_hash);
        }

        // Add new hash mappings
        CONTENT_HASHES.save(deps.storage, &content_hash_str, &content_hash)?;
        HASH_TO_ENTITY.save(deps.storage, &content_hash_str, &entity_key)?;
        ENTITY_TO_HASH.save(deps.storage, &entity_key, &content_hash_str)?;

        // Update job content hash
        job.content_hash = content_hash;
    }

    // 🕒 Update timestamp and save
    if metadata_changed || content_changed {
        job.updated_at = env.block.time;
        JOBS.save(deps.storage, job_id, &job)?;
    }

    Ok(build_success_response!(
        "edit_job",
        job_id,
        &info.sender,
        "content_changed" => content_changed.to_string(),
        "metadata_changed" => metadata_changed.to_string(),
        "off_chain_key" => off_chain_storage_key
    ))
}

/// Delete a job
pub fn execute_delete_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::DeleteJob);

    // Load and validate job
    let job = JOBS
        .load(deps.storage, job_id)
        .map_err(|_| ContractError::JobNotFound {})?;
    validate_user_authorization(&job.poster, &info.sender)?;
    validate_job_status_for_operation(&job.status, &[JobStatus::Open], "delete")?;

    // Check if job has proposals
    if job.total_proposals > 0 {
        return Err(ContractError::InvalidInput {
            error: "Cannot delete job with existing proposals".to_string(),
        });
    }

    // Remove job
    JOBS.remove(deps.storage, job_id);

    // Release escrow
    let escrow_id = format!("job_{}", job_id);
    if let Ok(mut escrow) = ESCROWS.load(deps.storage, &escrow_id) {
        // Note: The EscrowState struct doesn't have a status field, so we only update released
        escrow.released = true;
        ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    }

    let mut response = build_success_response!("delete_job", job_id, &info.sender);

    // Add bank message to return funds
    response = response.add_message(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(job.budget.u128(), "uusdc"),
    });

    Ok(response)
}

/// Cancel a job
pub fn execute_cancel_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CancelJob);

    // Load and validate job
    let mut job = JOBS
        .load(deps.storage, job_id)
        .map_err(|_| ContractError::JobNotFound {})?;
    validate_user_authorization(&job.poster, &info.sender)?;
    validate_job_status_for_operation(
        &job.status,
        &[JobStatus::Open, JobStatus::InProgress],
        "cancel",
    )?;

    // Update job status
    job.status = JobStatus::Cancelled;
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &job)?;

    Ok(build_success_response!("cancel_job", job_id, &info.sender))
}

/// Accept a proposal
pub fn execute_accept_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::AcceptProposal);

    // Load and validate proposal
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;
    let mut job = JOBS.load(deps.storage, proposal.job_id)?;

    validate_user_authorization(&job.poster, &info.sender)?;
    validate_job_status_for_operation(&job.status, &[JobStatus::Open], "accept proposal for")?;

    // Note: Proposal struct doesn't have status/updated_at fields, so we skip updating those
    // We only update the job to reflect that it's assigned

    // Update job status
    job.status = JobStatus::InProgress;
    job.assigned_freelancer = Some(proposal.freelancer.clone());
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, proposal.job_id, &job)?;

    // Note: We skip rejecting other proposals since Proposal struct doesn't have status/updated_at fields
    // In a real implementation, we might want to store proposal status separately or modify the struct

    Ok(build_success_response!(
        "accept_proposal",
        proposal_id,
        &info.sender,
        "job_id" => proposal.job_id.to_string(),
        "freelancer" => proposal.freelancer.to_string()
    ))
}

/// Complete a job
pub fn execute_complete_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    _completion_notes: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CompleteJob);

    // Load and validate job
    let mut job = JOBS.load(deps.storage, job_id)?;
    validate_job_status_for_operation(&job.status, &[JobStatus::InProgress], "complete")?;

    // Check if user is assigned freelancer
    if let Some(ref assigned_freelancer) = job.assigned_freelancer {
        validate_user_authorization(assigned_freelancer, &info.sender)?;
    } else {
        return Err(ContractError::InvalidInput {
            error: "Job is not assigned to anyone".to_string(),
        });
    }

    // Update job status
    job.status = JobStatus::Completed;
    // Note: Job struct doesn't have completed_at field, so we just update updated_at
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &job)?;

    // Release escrow
    let escrow_id = format!("job_{}", job_id);
    if let Ok(mut escrow) = ESCROWS.load(deps.storage, &escrow_id) {
        // Note: EscrowState struct doesn't have status, recipient, or released_at fields
        escrow.released = true;
        ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    }

    let mut response = build_success_response!(
        "complete_job",
        job_id,
        &info.sender,
        "budget" => job.budget.to_string()
    );

    // Add bank message to release funds to freelancer
    if let Some(ref freelancer) = job.assigned_freelancer {
        response = response.add_message(BankMsg::Send {
            to_address: freelancer.to_string(),
            amount: coins(job.budget.u128(), "uusdc"),
        });
    }

    Ok(response)
}

// Query functions

/// Query a specific job
pub fn query_job(deps: Deps, job_id: u64) -> StdResult<JobResponse> {
    let job = JOBS.load(deps.storage, job_id)?;
    Ok(JobResponse { job })
}

/// Query jobs with pagination and filtering
pub fn query_jobs(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    category: Option<String>,
    status: Option<JobStatus>,
    poster: Option<String>,
) -> StdResult<JobsResponse> {
    let poster_addr = if let Some(p) = poster {
        Some(deps.api.addr_validate(&p)?)
    } else {
        None
    };

    build_jobs_response(
        deps.storage,
        start_after,
        limit,
        category,
        status,
        poster_addr,
    )
}

/// Query user's jobs
pub fn query_user_jobs(
    deps: Deps,
    user: String,
    status: Option<JobStatus>,
) -> StdResult<JobsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    build_jobs_response(deps.storage, None, None, None, status, Some(user_addr))
}

/// Query a specific proposal
pub fn query_proposal(deps: Deps, proposal_id: u64) -> StdResult<ProposalResponse> {
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;
    Ok(ProposalResponse { proposal })
}

/// Query proposals for a job
pub fn query_job_proposals(deps: Deps, job_id: u64) -> StdResult<ProposalsResponse> {
    let proposals: Vec<_> = PROPOSALS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, proposal)) = item {
                if proposal.job_id == job_id {
                    Some(proposal)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(ProposalsResponse { proposals })
}

/// Query user's proposals
pub fn query_user_proposals(
    deps: Deps,
    user: String,
    _status: Option<ProposalStatus>,
) -> StdResult<ProposalsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;

    let proposals: Vec<_> = PROPOSALS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, proposal)) = item {
                if proposal.freelancer == user_addr {
                    // Note: Proposal struct doesn't have status field, so we return all proposals
                    // In a real implementation, we'd need to store status separately or modify the struct
                    Some(proposal)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    Ok(ProposalsResponse { proposals })
}

/// Query job rating
pub fn query_job_rating(deps: Deps, job_id: u64, rater: String) -> StdResult<Rating> {
    // Create the key for this specific job-rater combination
    let key = format!("{}_{}", job_id, rater);

    // Load the specific rating
    RATINGS.load(deps.storage, &key)
}

// Additional Proposal Management Functions

#[allow(clippy::too_many_arguments)]
pub fn execute_edit_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    _cover_letter: Option<String>,
    delivery_time_days: Option<u64>,
    _milestones: Option<Vec<ProposalMilestone>>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::EditProposal);

    // Load and validate proposal
    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)?;

    // Check authorization - only proposer can edit
    if proposal.freelancer != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Note: Since Proposal doesn't have status field, we assume it's editable if it exists
    // In a full implementation, you would add status field to Proposal struct

    // In hybrid architecture, updates would recreate the content bundle
    // For now, we'll update available fields and recreate content hash

    // Update delivery time if provided
    if let Some(new_delivery_time) = delivery_time_days {
        proposal.delivery_time_days = new_delivery_time;
    }

    // For content updates (cover_letter, milestones), we would need to:
    // 1. Fetch current off-chain content using content_hash
    // 2. Update the relevant fields
    // 3. Create new content bundle and hash
    // 4. Update the content_hash field

    // For now, just update timestamp to indicate proposal was modified
    proposal.submitted_at = env.block.time;

    // Save updated proposal
    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    // Build response
    let response = build_success_response!("edit_proposal", proposal_id, &info.sender);

    Ok(response)
}

pub fn execute_withdraw_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::WithdrawProposal);

    // Load and validate proposal
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;

    // Check authorization - only proposer can withdraw
    if proposal.freelancer != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Remove proposal from storage (withdrawal)
    PROPOSALS.remove(deps.storage, proposal_id);

    // Build response
    let response = build_success_response!("withdraw_proposal", proposal_id, &info.sender);

    Ok(response)
}

// Milestone Management Functions

pub fn execute_complete_milestone(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _job_id: u64,
    _milestone_id: u64,
) -> Result<Response, ContractError> {
    // In hybrid architecture, milestone completion is tracked off-chain
    // This function is deprecated but kept for API compatibility
    Err(ContractError::InvalidInput {
        error: "Milestone management is now handled off-chain".to_string(),
    })
}

pub fn execute_approve_milestone(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _job_id: u64,
    _milestone_id: u64,
) -> Result<Response, ContractError> {
    // In hybrid architecture, milestone approval is tracked off-chain
    // This function is deprecated but kept for API compatibility
    Err(ContractError::InvalidInput {
        error: "Milestone management is now handled off-chain".to_string(),
    })
}

// Dispute Management Functions

pub fn execute_raise_dispute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    reason: String,
    evidence: Vec<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::RaiseDispute);

    // Load job
    let mut job = JOBS.load(deps.storage, job_id)?;

    // Check authorization - only job poster or assigned freelancer can raise dispute
    let is_authorized =
        job.poster == info.sender || job.assigned_freelancer.as_ref() == Some(&info.sender);

    if !is_authorized {
        return Err(ContractError::Unauthorized {});
    }

    // Check job status - can only dispute in progress or completed jobs
    if !matches!(job.status, JobStatus::InProgress | JobStatus::Completed) {
        return Err(ContractError::InvalidInput {
            error: "Can only dispute jobs that are in progress or completed".to_string(),
        });
    }

    // Validate inputs
    if reason.len() < 10 || reason.len() > 1000 {
        return Err(ContractError::InvalidInput {
            error: "Dispute reason must be 10-1000 characters".to_string(),
        });
    }

    // Update job status to disputed
    job.status = JobStatus::Disputed;
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &job)?;

    // Create dispute record
    let dispute_id = format!("dispute_{}_{}", job_id, env.block.time.seconds());

    let dispute = crate::state::Dispute {
        id: dispute_id.clone(),
        job_id,
        raised_by: info.sender.clone(),
        reason,
        evidence,
        status: crate::state::DisputeStatus::Raised,
        created_at: env.block.time,
        resolved_at: None,
        resolution: None,
    };

    DISPUTES.save(deps.storage, &dispute_id, &dispute)?;

    // Update escrow to prevent release
    if let Some(ref escrow_id) = job.escrow_id {
        if let Ok(mut escrow) = ESCROWS.load(deps.storage, escrow_id) {
            escrow.dispute_status = crate::state::DisputeStatus::Raised;
            escrow.dispute_raised_at = Some(env.block.time);
            // Set dispute deadline (e.g., 7 days for resolution)
            escrow.dispute_deadline = Some(env.block.time.plus_days(7));
            ESCROWS.save(deps.storage, escrow_id, &escrow)?;
        }
    }

    // Build response
    let response = build_success_response!("raise_dispute", job_id, &info.sender)
        .add_attribute("dispute_id", dispute_id.clone())
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("reason_length", dispute.reason.len().to_string())
        .add_attribute("evidence_count", dispute.evidence.len().to_string());

    Ok(response)
}

pub fn execute_resolve_dispute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dispute_id: String,
    resolution: String,
    release_to_freelancer: bool,
) -> Result<Response, ContractError> {
    // Apply security checks and admin check
    apply_security_checks!(deps, env, info, RateLimitAction::ResolveDispute);
    ensure_admin!(deps, info);

    // Validate inputs
    if resolution.len() < 10 || resolution.len() > 2000 {
        return Err(ContractError::InvalidInput {
            error: "Resolution must be 10-2000 characters".to_string(),
        });
    }

    // Load dispute from storage
    let mut dispute = DISPUTES.load(deps.storage, &dispute_id)?;

    if dispute.status != crate::state::DisputeStatus::Raised {
        return Err(ContractError::InvalidInput {
            error: "Dispute is not in raised status".to_string(),
        });
    }

    // Load and update job
    let mut job = JOBS.load(deps.storage, dispute.job_id)?;

    if job.status != JobStatus::Disputed {
        return Err(ContractError::InvalidInput {
            error: "Job is not disputed".to_string(),
        });
    }

    // Update dispute record
    dispute.status = crate::state::DisputeStatus::Resolved;
    dispute.resolved_at = Some(env.block.time);
    dispute.resolution = Some(resolution.clone());
    DISPUTES.save(deps.storage, &dispute_id, &dispute)?;

    // Handle escrow resolution and payment
    let mut messages = Vec::new();

    if let Some(ref escrow_id) = job.escrow_id {
        if let Ok(mut escrow) = ESCROWS.load(deps.storage, escrow_id) {
            // Update escrow status
            escrow.dispute_status = crate::state::DisputeStatus::Resolved;
            escrow.released = true;

            // Create payment message based on resolution
            let recipient = if release_to_freelancer {
                job.assigned_freelancer.as_ref().unwrap_or(&job.poster)
            } else {
                &job.poster
            };

            let payment_msg = cosmwasm_std::BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![cosmwasm_std::Coin {
                    denom: "uxion".to_string(),
                    amount: escrow.amount,
                }],
            };
            messages.push(cosmwasm_std::SubMsg::new(payment_msg));

            ESCROWS.save(deps.storage, escrow_id, &escrow)?;
        }
    }

    // Resolve dispute
    job.status = JobStatus::Completed;
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, dispute.job_id, &job)?;

    // Build response
    let mut response = build_success_response!("resolve_dispute", dispute.job_id, &info.sender);

    response = response
        .add_submessages(messages)
        .add_attribute("dispute_id", dispute_id)
        .add_attribute("job_id", dispute.job_id.to_string())
        .add_attribute("release_to_freelancer", release_to_freelancer.to_string())
        .add_attribute("resolution_length", resolution.len().to_string());

    Ok(response)
}

/// Validate user authorization
fn validate_user_authorization(owner: &Addr, requester: &Addr) -> Result<(), ContractError> {
    if owner != requester {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

/// Validate job status for operations
fn validate_job_status_for_operation(
    status: &JobStatus,
    allowed_statuses: &[JobStatus],
    operation: &str,
) -> Result<(), ContractError> {
    if !allowed_statuses.contains(status) {
        return Err(ContractError::InvalidInput {
            error: format!("Cannot {} job in status {:?}", operation, status),
        });
    }
    Ok(())
}
