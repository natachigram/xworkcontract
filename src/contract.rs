use crate::error::ContractError;
use crate::escrow::{
    create_escrow_cw20, create_escrow_native, raise_dispute, refund_escrow, release_escrow,
    resolve_dispute,
};
use crate::helpers::{
    ensure_not_paused, get_future_timestamp, query_jobs_paginated, query_user_proposals,
    validate_budget, validate_deadline, validate_duration, validate_job_description,
    validate_job_title,
};
use crate::job_management::{execute_edit_job, execute_edit_proposal, execute_submit_proposal};
use crate::msg::{
    BountiesResponse, BountyResponse, BountySubmissionResponse, BountySubmissionsResponse,
    ConfigResponse, DisputeResponse, DisputesResponse, EscrowResponse, ExecuteMsg, InstantiateMsg,
    JobResponse, JobsResponse, MilestoneInput, PlatformStatsResponse, ProposalResponse,
    ProposalsResponse, QueryMsg, RatingsResponse, RewardTierInput, UserStatsResponse,
    WinnerSelection,
};
use crate::security::{
    check_rate_limit, reentrancy_guard, validate_job_duration, validate_text_inputs,
    RateLimitAction,
};
use crate::state::{
    Bounty, BountyStatus, BountySubmission, BountySubmissionStatus, Config, Job, JobStatus,
    Milestone, Rating, RewardTier, BLOCKED_ADDRESSES, BOUNTIES, BOUNTY_COUNTER, BOUNTY_SUBMISSIONS,
    BOUNTY_SUBMISSIONS_BY_BOUNTY, BOUNTY_SUBMISSION_COUNTER, CONFIG, DISPUTES, ESCROWS, JOBS,
    JOB_COUNTER, JOB_PROPOSALS, PROPOSALS, PROPOSAL_COUNTER, RATE_LIMITS, RATINGS,
    USER_BOUNTY_SUBMISSIONS, USER_PROPOSALS, USER_STATS,
};
use crate::user_management::execute_update_user_profile;

use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:xworks-freelance-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = match msg.admin {
        Some(admin_str) => deps.api.addr_validate(&admin_str)?,
        None => info.sender.clone(),
    };

    let platform_fee_percent = msg.platform_fee_percent.unwrap_or(5u64);
    if platform_fee_percent > 10 {
        return Err(ContractError::PlatformFeeTooHigh { max: 10 });
    }

    let config = Config {
        admin: admin.clone(),
        platform_fee_percent,
        min_escrow_amount: msg.min_escrow_amount.unwrap_or(Uint128::new(1000)),
        dispute_period_days: msg.dispute_period_days.unwrap_or(7u64),
        max_job_duration_days: msg.max_job_duration_days.unwrap_or(365u64),
        paused: false,
    };

    CONFIG.save(deps.storage, &config)?;
    JOB_COUNTER.save(deps.storage, &0)?;
    PROPOSAL_COUNTER.save(deps.storage, &0)?;

    // Initialize all the NEXT_* counters used by other modules
    use crate::state::{
        NEXT_BOUNTY_ID, NEXT_BOUNTY_SUBMISSION_ID, NEXT_ESCROW_ID, NEXT_JOB_ID, NEXT_PROPOSAL_ID,
    };
    NEXT_JOB_ID.save(deps.storage, &0)?;
    NEXT_PROPOSAL_ID.save(deps.storage, &0)?;
    NEXT_ESCROW_ID.save(deps.storage, &0)?;
    NEXT_BOUNTY_ID.save(deps.storage, &0)?;
    NEXT_BOUNTY_SUBMISSION_ID.save(deps.storage, &0)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", admin.to_string())
        .add_attribute("platform_fee", platform_fee_percent.to_string())
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Job Management
        ExecuteMsg::PostJob {
            title,
            description,
            budget,
            category,
            skills_required,
            duration_days,
            documents,
            milestones,
        } => execute_post_job(
            deps,
            env,
            info,
            title,
            description,
            budget,
            category,
            skills_required,
            duration_days,
            documents,
            milestones,
        ),

        ExecuteMsg::EditJob {
            job_id,
            title,
            description,
            budget,
            category,
            skills_required,
            duration_days,
            documents,
            milestones,
        } => execute_edit_job(
            deps,
            env,
            info,
            job_id,
            title,
            description,
            budget,
            category,
            skills_required,
            duration_days,
            documents,
            milestones,
        ),

        ExecuteMsg::UpdateUserProfile {
            name,
            bio,
            skills,
            location,
            website,
            portfolio_url,
            hourly_rate,
            availability,
        } => execute_update_user_profile(
            deps,
            env,
            info,
            name,
            bio,
            skills,
            location,
            website,
            portfolio_url,
            hourly_rate,
            availability,
        ),

        ExecuteMsg::DeleteJob { job_id } => {
            crate::job_management::execute_delete_job(deps, env, info, job_id)
        }
        ExecuteMsg::CancelJob { job_id } => {
            crate::job_management::execute_cancel_job(deps, env, info, job_id)
        }

        // Proposal Management
        ExecuteMsg::SubmitProposal {
            job_id,
            cover_letter,
            delivery_time_days,
            milestones,
        } => execute_submit_proposal(
            deps,
            env,
            info,
            job_id,
            cover_letter,
            delivery_time_days,
            milestones,
        ),

        ExecuteMsg::EditProposal {
            proposal_id,
            cover_letter,
            delivery_time_days,
            milestones,
        } => execute_edit_proposal(
            deps,
            env,
            info,
            proposal_id,
            cover_letter,
            delivery_time_days,
            milestones,
        ),

        ExecuteMsg::WithdrawProposal { proposal_id } => {
            execute_withdraw_proposal(deps, env, info, proposal_id)
        }

        ExecuteMsg::AcceptProposal {
            job_id,
            proposal_id,
        } => execute_accept_proposal(deps, env, info, job_id, proposal_id),

        // Escrow Management
        ExecuteMsg::CreateEscrow { job_id } => {
            // Legacy support - create native escrow with job budget
            let _job = JOBS.load(deps.storage, job_id)?;
            create_escrow_native(deps, env, info, job_id)
        }
        ExecuteMsg::FundEscrow { escrow_id: _ } => Err(ContractError::InvalidInput {
            error: "FundEscrow is deprecated. Use CreateEscrowNative or CreateEscrowCw20 instead"
                .to_string(),
        }),
        ExecuteMsg::ReleaseEscrow { escrow_id } => release_escrow(deps, env, info, escrow_id),
        ExecuteMsg::RefundEscrow { escrow_id } => refund_escrow(deps, env, info, escrow_id),

        // Work Management
        ExecuteMsg::CompleteJob { job_id } => execute_complete_job(deps, env, info, job_id),
        ExecuteMsg::CompleteMilestone {
            job_id,
            milestone_id,
        } => execute_complete_milestone(deps, env, info, job_id, milestone_id),
        ExecuteMsg::ApproveMilestone {
            job_id,
            milestone_id,
        } => execute_approve_milestone(deps, env, info, job_id, milestone_id),

        // Rating System
        ExecuteMsg::SubmitRating {
            job_id,
            rating,
            comment,
        } => execute_submit_rating(deps, env, info, job_id, rating, comment),

        // Dispute Management
        ExecuteMsg::RaiseDispute {
            job_id,
            reason,
            evidence,
        } => raise_dispute(deps, env, info, job_id, reason, evidence),
        ExecuteMsg::ResolveDispute {
            dispute_id,
            resolution,
            release_to_freelancer,
        } => resolve_dispute(
            deps,
            env,
            info,
            dispute_id,
            resolution,
            release_to_freelancer,
        ),

        // Admin Functions
        ExecuteMsg::UpdateConfig {
            admin,
            platform_fee_percent,
            min_escrow_amount,
            dispute_period_days,
            max_job_duration_days,
        } => execute_update_config(
            deps,
            env,
            info,
            admin,
            platform_fee_percent,
            min_escrow_amount,
            dispute_period_days,
            max_job_duration_days,
        ),
        ExecuteMsg::PauseContract {} => execute_pause_contract(deps, env, info),
        ExecuteMsg::UnpauseContract {} => execute_unpause_contract(deps, env, info),

        // New escrow functions
        ExecuteMsg::CreateEscrowNative { job_id, amount: _ } => {
            create_escrow_native(deps, env, info, job_id)
        }
        ExecuteMsg::CreateEscrowCw20 {
            job_id: _,
            token_address: _,
            amount,
        } => create_escrow_cw20(deps, env, info, amount, cosmwasm_std::Binary::default()),

        // Security functions
        ExecuteMsg::BlockAddress { address, reason } => {
            execute_block_address(deps, env, info, address, reason)
        }
        ExecuteMsg::UnblockAddress { address } => execute_unblock_address(deps, env, info, address),
        ExecuteMsg::ResetRateLimit { address } => {
            execute_reset_rate_limit(deps, env, info, address)
        }

        // Bounty Management
        ExecuteMsg::CreateBounty {
            title,
            description,
            requirements,
            total_reward,
            category,
            skills_required,
            submission_deadline_days,
            review_period_days,
            max_winners,
            reward_distribution,
            documents,
        } => execute_create_bounty(
            deps,
            env,
            info,
            title,
            description,
            requirements,
            total_reward,
            category,
            skills_required,
            submission_deadline_days,
            review_period_days,
            max_winners,
            reward_distribution,
            documents,
        ),
        ExecuteMsg::EditBounty {
            bounty_id,
            title,
            description,
            requirements,
            submission_deadline_days,
            review_period_days,
            documents,
        } => execute_edit_bounty(
            deps,
            env,
            info,
            bounty_id,
            title,
            description,
            requirements,
            submission_deadline_days,
            review_period_days,
            documents,
        ),
        ExecuteMsg::CancelBounty { bounty_id } => execute_cancel_bounty(deps, env, info, bounty_id),
        ExecuteMsg::SubmitToBounty {
            bounty_id,
            title,
            description,
            deliverables,
        } => execute_submit_to_bounty(deps, env, info, bounty_id, title, description, deliverables),
        ExecuteMsg::EditBountySubmission {
            submission_id,
            title,
            description,
            deliverables,
        } => execute_edit_bounty_submission(
            deps,
            env,
            info,
            submission_id,
            title,
            description,
            deliverables,
        ),
        ExecuteMsg::WithdrawBountySubmission { submission_id } => {
            execute_withdraw_bounty_submission(deps, env, info, submission_id)
        }
        ExecuteMsg::ReviewBountySubmission {
            submission_id,
            status,
            review_notes,
            score,
        } => execute_review_bounty_submission(
            deps,
            env,
            info,
            submission_id,
            status,
            review_notes,
            score,
        ),
        ExecuteMsg::SelectBountyWinners {
            bounty_id,
            winner_submissions,
        } => execute_select_bounty_winners(deps, env, info, bounty_id, winner_submissions),
        ExecuteMsg::CreateBountyEscrow { bounty_id } => {
            execute_create_bounty_escrow(deps, env, info, bounty_id)
        }
        ExecuteMsg::ReleaseBountyRewards { bounty_id } => {
            execute_release_bounty_rewards(deps, env, info, bounty_id)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_post_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    budget: Uint128,
    category: String,
    skills_required: Vec<String>,
    duration_days: u64,
    documents: Option<Vec<String>>,
    milestones: Option<Vec<MilestoneInput>>,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    check_rate_limit(deps.branch(), &env, &info.sender, RateLimitAction::PostJob)?;

    // Input validation and sanitization
    validate_text_inputs(&title, &description, None, None)?;
    validate_job_duration(duration_days)?;
    ensure_not_paused(deps.as_ref())?;

    let config = CONFIG.load(deps.storage)?;

    // Validate inputs
    validate_job_title(&title)?;
    validate_job_description(&description)?;
    validate_budget(budget)?;
    validate_duration(duration_days, config.max_job_duration_days)?;

    if category.is_empty() || category.len() > 50 {
        return Err(ContractError::InvalidInput {
            error: "Category must be between 1-50 characters".to_string(),
        });
    }

    if skills_required.is_empty() || skills_required.len() > 20 {
        return Err(ContractError::InvalidInput {
            error: "Must specify 1-20 skills".to_string(),
        });
    }

    // Get and increment job counter
    let job_id = JOB_COUNTER.load(deps.storage)?;
    JOB_COUNTER.save(deps.storage, &(job_id + 1))?;

    let deadline = get_future_timestamp(env.block.time, duration_days);

    // Process milestones
    let mut job_milestones = Vec::new();
    if let Some(milestone_inputs) = milestones {
        let mut total_milestone_amount = Uint128::zero();

        for (idx, milestone_input) in milestone_inputs.iter().enumerate() {
            if milestone_input.title.is_empty() || milestone_input.title.len() > 100 {
                return Err(ContractError::InvalidInput {
                    error: "Milestone title must be between 1-100 characters".to_string(),
                });
            }

            if milestone_input.description.len() > 500 {
                return Err(ContractError::InvalidInput {
                    error: "Milestone description must be max 500 characters".to_string(),
                });
            }

            total_milestone_amount = total_milestone_amount.checked_add(milestone_input.amount)?;

            let milestone_deadline =
                get_future_timestamp(env.block.time, milestone_input.deadline_days);
            validate_deadline(milestone_deadline, env.block.time)?;

            job_milestones.push(Milestone {
                id: idx as u64,
                title: milestone_input.title.clone(),
                description: milestone_input.description.clone(),
                amount: milestone_input.amount,
                deadline: milestone_deadline,
                completed: false,
                completed_at: None,
            });
        }

        // Ensure milestone amounts sum to total budget
        if total_milestone_amount != budget {
            return Err(ContractError::InvalidInput {
                error: "Sum of milestone amounts must equal total budget".to_string(),
            });
        }
    }

    // Create and save job
    let job = Job {
        id: job_id,
        poster: info.sender.clone(),
        title,
        description,
        budget,
        category,
        skills_required,
        duration_days,
        documents: documents.unwrap_or_default(),
        status: JobStatus::Open,
        assigned_freelancer: None,
        created_at: env.block.time,
        updated_at: env.block.time,
        deadline,
        milestones: job_milestones,
        escrow_id: None,
        total_proposals: 0,
        company: None,
        location: None,
    };

    JOBS.save(deps.storage, job_id, &job)?;

    // Initialize empty proposals list for this job
    JOB_PROPOSALS.save(deps.storage, job_id, &Vec::new())?;

    // Update user stats
    let mut user_stats = USER_STATS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    user_stats.total_jobs_posted += 1;
    USER_STATS.save(deps.storage, &info.sender, &user_stats)?;

    Ok(Response::new()
        .add_attribute("method", "post_job")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("poster", info.sender.to_string())
        .add_attribute("budget", budget.to_string())
        .add_attribute("milestones_count", job.milestones.len().to_string()))
}

fn execute_withdraw_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Load and validate proposal
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;

    if proposal.freelancer != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Check if job is still open
    let job = JOBS.load(deps.storage, proposal.job_id)?;
    if job.status != JobStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Cannot withdraw proposal for non-open job".to_string(),
        });
    }

    // Remove proposal from storage
    PROPOSALS.remove(deps.storage, proposal_id);

    // Remove from job proposals list
    let mut job_proposals = JOB_PROPOSALS.load(deps.storage, proposal.job_id)?;
    job_proposals.retain(|&id| id != proposal_id);
    JOB_PROPOSALS.save(deps.storage, proposal.job_id, &job_proposals)?;

    // Remove from user proposals list
    let mut user_proposals = USER_PROPOSALS.load(deps.storage, &info.sender)?;
    user_proposals.retain(|&id| id != proposal_id);
    USER_PROPOSALS.save(deps.storage, &info.sender, &user_proposals)?;

    // Update job proposal count
    let mut job = JOBS.load(deps.storage, proposal.job_id)?;
    job.total_proposals = job.total_proposals.saturating_sub(1);
    job.updated_at = env.block.time;
    JOBS.save(deps.storage, proposal.job_id, &job)?;

    Ok(Response::new()
        .add_attribute("method", "withdraw_proposal")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("job_id", proposal.job_id.to_string())
        .add_attribute("freelancer", info.sender.to_string()))
}

fn execute_accept_proposal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Load and validate job
    let mut job = JOBS.load(deps.storage, job_id)?;

    if job.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if job.status != JobStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Job is not open for acceptance".to_string(),
        });
    }

    // Load and validate proposal
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;

    if proposal.job_id != job_id {
        return Err(ContractError::InvalidInput {
            error: "Proposal does not belong to this job".to_string(),
        });
    }

    // Update job status and assign freelancer
    job.status = JobStatus::InProgress;
    job.assigned_freelancer = Some(proposal.freelancer.clone());
    job.updated_at = env.block.time;

    JOBS.save(deps.storage, job_id, &job)?;

    // Update user stats
    let mut freelancer_stats = USER_STATS
        .may_load(deps.storage, &proposal.freelancer)?
        .unwrap_or_default();
    freelancer_stats.total_jobs_completed += 1;
    USER_STATS.save(deps.storage, &proposal.freelancer, &freelancer_stats)?;

    Ok(Response::new()
        .add_attribute("method", "accept_proposal")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("freelancer", proposal.freelancer.to_string())
        .add_attribute("delivery_time_days", proposal.delivery_time_days.to_string()))
}

fn execute_complete_job(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Load and validate job
    let mut job = JOBS.load(deps.storage, job_id)?;

    // Only assigned freelancer can mark job as complete
    if job.assigned_freelancer.is_none() || job.assigned_freelancer.as_ref() != Some(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    if job.status != JobStatus::InProgress {
        return Err(ContractError::InvalidInput {
            error: "Job is not in progress".to_string(),
        });
    }

    // Check if all milestones are completed (if any)
    if !job.milestones.is_empty() {
        let all_completed = job.milestones.iter().all(|m| m.completed);
        if !all_completed {
            return Err(ContractError::InvalidInput {
                error: "All milestones must be completed before job completion".to_string(),
            });
        }
    }

    // Update job status
    job.status = JobStatus::Completed;
    job.updated_at = env.block.time;

    JOBS.save(deps.storage, job_id, &job)?;

    // Update freelancer stats
    if let Some(freelancer) = &job.assigned_freelancer {
        let mut freelancer_stats = USER_STATS
            .may_load(deps.storage, freelancer)?
            .unwrap_or_default();
        freelancer_stats.total_earned = freelancer_stats.total_earned.checked_add(job.budget)?;
        freelancer_stats.completion_rate = Decimal::from_ratio(
            freelancer_stats.total_jobs_completed,
            freelancer_stats.total_jobs_completed + 1,
        );
        USER_STATS.save(deps.storage, freelancer, &freelancer_stats)?;
    }

    // Update poster stats
    let mut poster_stats = USER_STATS
        .may_load(deps.storage, &job.poster)?
        .unwrap_or_default();
    poster_stats.total_spent = poster_stats.total_spent.checked_add(job.budget)?;
    USER_STATS.save(deps.storage, &job.poster, &poster_stats)?;

    let mut response = Response::new()
        .add_attribute("method", "complete_job")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("freelancer", info.sender.to_string())
        .add_attribute("budget", job.budget.to_string());

    // Automatically release escrow if it exists
    if let Some(escrow_id) = &job.escrow_id {
        // Load escrow and check if it can be released
        if let Ok(escrow) = ESCROWS.load(deps.storage, escrow_id) {
            if !escrow.released && escrow.dispute_status == crate::state::DisputeStatus::None {
                // Auto-release escrow to freelancer upon job completion
                let config = CONFIG.load(deps.storage)?;

                // Generate payment messages
                let freelancer_payment = cosmwasm_std::BankMsg::Send {
                    to_address: escrow.freelancer.to_string(),
                    amount: vec![cosmwasm_std::Coin {
                        denom: "uxion".to_string(),
                        amount: escrow.amount,
                    }],
                };

                let platform_fee_payment = cosmwasm_std::BankMsg::Send {
                    to_address: config.admin.to_string(),
                    amount: vec![cosmwasm_std::Coin {
                        denom: "uxion".to_string(),
                        amount: escrow.platform_fee,
                    }],
                };

                // Mark escrow as released
                let mut updated_escrow = escrow;
                updated_escrow.released = true;
                ESCROWS.save(deps.storage, escrow_id, &updated_escrow)?;

                // Add payment messages and attributes
                response = response
                    .add_message(freelancer_payment)
                    .add_message(platform_fee_payment)
                    .add_attribute("escrow_released", "true")
                    .add_attribute("escrow_id", escrow_id)
                    .add_attribute("payment_amount", updated_escrow.amount.to_string())
                    .add_attribute("platform_fee", updated_escrow.platform_fee.to_string());
            }
        }
    }

    Ok(response)
}

fn execute_complete_milestone(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    milestone_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Load and validate job
    let mut job = JOBS.load(deps.storage, job_id)?;

    // Only assigned freelancer can complete milestones
    if job.assigned_freelancer.is_none() || job.assigned_freelancer.as_ref() != Some(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    if job.status != JobStatus::InProgress {
        return Err(ContractError::InvalidInput {
            error: "Job is not in progress".to_string(),
        });
    }

    // Find and update milestone
    let milestone_index = job
        .milestones
        .iter()
        .position(|m| m.id == milestone_id)
        .ok_or_else(|| ContractError::InvalidInput {
            error: "Milestone not found".to_string(),
        })?;

    if job.milestones[milestone_index].completed {
        return Err(ContractError::InvalidInput {
            error: "Milestone already completed".to_string(),
        });
    }

    // Mark milestone as completed
    job.milestones[milestone_index].completed = true;
    job.milestones[milestone_index].completed_at = Some(env.block.time);
    job.updated_at = env.block.time;

    JOBS.save(deps.storage, job_id, &job)?;

    Ok(Response::new()
        .add_attribute("method", "complete_milestone")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("milestone_id", milestone_id.to_string())
        .add_attribute("freelancer", info.sender.to_string())
        .add_attribute(
            "milestone_amount",
            job.milestones[milestone_index].amount.to_string(),
        ))
}

fn execute_approve_milestone(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    milestone_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Load and validate job
    let mut job = JOBS.load(deps.storage, job_id)?;

    // Only job poster can approve milestones
    if job.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if job.status != JobStatus::InProgress {
        return Err(ContractError::InvalidInput {
            error: "Job is not in progress".to_string(),
        });
    }

    // Find milestone
    let milestone_index = job
        .milestones
        .iter()
        .position(|m| m.id == milestone_id)
        .ok_or_else(|| ContractError::InvalidInput {
            error: "Milestone not found".to_string(),
        })?;

    if !job.milestones[milestone_index].completed {
        return Err(ContractError::InvalidInput {
            error: "Milestone must be completed before approval".to_string(),
        });
    }

    let milestone_amount = job.milestones[milestone_index].amount;

    // In a real implementation, this would trigger escrow release for the milestone amount
    let mut response = Response::new()
        .add_attribute("method", "approve_milestone")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("milestone_id", milestone_id.to_string())
        .add_attribute("approver", info.sender.to_string())
        .add_attribute("milestone_amount", milestone_amount.to_string());

    // If escrow exists, release milestone payment
    if let Some(escrow_id) = &job.escrow_id {
        response = response.add_attribute("escrow_partial_release", escrow_id);
        // Note: In a real implementation, you'd call partial escrow release here
    }

    job.updated_at = env.block.time;
    JOBS.save(deps.storage, job_id, &job)?;

    Ok(response)
}

fn execute_submit_rating(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    rating: u8,
    comment: String,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    ensure_not_paused(deps.as_ref())?;

    // Input validation
    if !(1..=5).contains(&rating) {
        return Err(ContractError::InvalidInput {
            error: "Rating must be between 1 and 5".to_string(),
        });
    }

    if comment.len() > 500 {
        return Err(ContractError::InvalidInput {
            error: "Comment must be max 500 characters".to_string(),
        });
    }

    // Load and validate job
    let job = JOBS.load(deps.storage, job_id)?;

    if job.status != JobStatus::Completed {
        return Err(ContractError::InvalidInput {
            error: "Job must be completed to submit rating".to_string(),
        });
    }

    // Determine who is being rated
    let (rated_user, is_poster_rating) = if job.poster == info.sender {
        // Job poster is rating the freelancer
        if let Some(freelancer) = &job.assigned_freelancer {
            (freelancer.clone(), true)
        } else {
            return Err(ContractError::InvalidInput {
                error: "No freelancer assigned to rate".to_string(),
            });
        }
    } else if job.assigned_freelancer.as_ref() == Some(&info.sender) {
        // Freelancer is rating the job poster
        (job.poster.clone(), false)
    } else {
        return Err(ContractError::Unauthorized {});
    };

    // Check if rating already exists
    let rating_key = format!("{}_{}", job_id, info.sender);
    if RATINGS.may_load(deps.storage, &rating_key)?.is_some() {
        return Err(ContractError::InvalidInput {
            error: "Rating already submitted for this job".to_string(),
        });
    }

    // Create and save rating
    let rating_record = Rating {
        id: rating_key.clone(),
        job_id,
        rater: info.sender.clone(),
        rated: rated_user.clone(),
        rating,
        comment: comment.clone(),
        created_at: env.block.time,
    };

    RATINGS.save(deps.storage, &rating_key, &rating_record)?;

    // Update rated user's stats
    let mut user_stats = USER_STATS
        .may_load(deps.storage, &rated_user)?
        .unwrap_or_default();

    let new_total_ratings = user_stats.total_ratings + 1;
    let new_average = (user_stats.average_rating
        * Decimal::from_ratio(user_stats.total_ratings, 1u128)
        + Decimal::from_ratio(rating as u128, 1u128))
        / Decimal::from_ratio(new_total_ratings, 1u128);

    user_stats.average_rating = new_average;
    user_stats.total_ratings = new_total_ratings;

    USER_STATS.save(deps.storage, &rated_user, &user_stats)?;

    let rating_type = if is_poster_rating {
        "freelancer"
    } else {
        "poster"
    };

    Ok(Response::new()
        .add_attribute("method", "submit_rating")
        .add_attribute("job_id", job_id.to_string())
        .add_attribute("rater", info.sender.to_string())
        .add_attribute("rated", rated_user.to_string())
        .add_attribute("rating", rating.to_string())
        .add_attribute("rating_type", rating_type))
}

#[allow(clippy::too_many_arguments)]
fn execute_update_config(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    admin: Option<String>,
    platform_fee_percent: Option<u64>,
    min_escrow_amount: Option<Uint128>,
    dispute_period_days: Option<u64>,
    max_job_duration_days: Option<u64>,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;

    let mut config = CONFIG.load(deps.storage)?;

    // Only admin can update config
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Update fields if provided
    if let Some(new_admin) = admin {
        config.admin = deps.api.addr_validate(&new_admin)?;
    }

    if let Some(fee_percent) = platform_fee_percent {
        if fee_percent > 10 {
            return Err(ContractError::PlatformFeeTooHigh { max: 10 });
        }
        config.platform_fee_percent = fee_percent;
    }

    if let Some(min_amount) = min_escrow_amount {
        config.min_escrow_amount = min_amount;
    }

    if let Some(dispute_days) = dispute_period_days {
        config.dispute_period_days = dispute_days;
    }

    if let Some(max_duration) = max_job_duration_days {
        config.max_job_duration_days = max_duration;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "update_config")
        .add_attribute("admin", config.admin.to_string())
        .add_attribute(
            "platform_fee_percent",
            config.platform_fee_percent.to_string(),
        ))
}

fn execute_pause_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Only admin can pause contract
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    config.paused = true;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "pause_contract")
        .add_attribute("admin", info.sender.to_string()))
}

fn execute_unpause_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Only admin can unpause contract
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "unpause_contract")
        .add_attribute("admin", info.sender.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetJob { job_id } => to_json_binary(&query_job(deps, job_id)?),
        QueryMsg::GetJobs {
            start_after,
            limit,
            category,
            status,
            poster,
        } => to_json_binary(&query_jobs(
            deps,
            start_after,
            limit,
            category,
            status,
            poster,
        )?),
        QueryMsg::GetAllJobs { limit, category } => {
            to_json_binary(&query_all_jobs(deps, limit, category)?)
        }
        QueryMsg::GetUserJobs { user, status } => {
            to_json_binary(&query_user_jobs(deps, user, status)?)
        }
        QueryMsg::GetProposal { proposal_id } => {
            to_json_binary(&query_proposal(deps, proposal_id)?)
        }
        QueryMsg::GetJobProposals { job_id } => to_json_binary(&query_job_proposals(deps, job_id)?),
        QueryMsg::GetUserProposals {
            user,
            start_after,
            limit,
        } => to_json_binary(&query_user_proposals_query(deps, user, start_after, limit)?),
        QueryMsg::GetEscrow { escrow_id } => to_json_binary(&query_escrow(deps, escrow_id)?),
        QueryMsg::GetJobEscrow { job_id } => to_json_binary(&query_job_escrow(deps, job_id)?),
        QueryMsg::GetUserRatings { user } => to_json_binary(&query_user_ratings(deps, user)?),
        QueryMsg::GetJobRating { job_id, rater } => {
            to_json_binary(&query_job_rating(deps, job_id, rater)?)
        }
        QueryMsg::GetUserStats { user } => to_json_binary(&query_user_stats(deps, user)?),
        QueryMsg::GetPlatformStats {} => to_json_binary(&query_platform_stats(deps)?),
        QueryMsg::GetDispute { dispute_id } => to_json_binary(&query_dispute(deps, dispute_id)?),
        QueryMsg::GetJobDisputes { job_id } => to_json_binary(&query_job_disputes(deps, job_id)?),
        QueryMsg::GetUserDisputes { user } => to_json_binary(&query_user_disputes(deps, user)?),
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        // Security queries
        QueryMsg::GetSecurityMetrics {} => to_json_binary(&query_security_metrics(deps)?),
        QueryMsg::GetAuditLogs {
            start_after,
            limit,
            action_filter,
        } => to_json_binary(&query_audit_logs(deps, start_after, limit, action_filter)?),
        QueryMsg::IsAddressBlocked { address } => {
            to_json_binary(&query_is_address_blocked(deps, address)?)
        }
        QueryMsg::GetRateLimitStatus { address } => {
            to_json_binary(&query_rate_limit_status(deps, address)?)
        }

        // Bounty Queries
        QueryMsg::GetBounty { bounty_id } => to_json_binary(&query_bounty(deps, bounty_id)?),
        QueryMsg::GetBounties {
            start_after,
            limit,
            category,
            status,
            poster,
        } => to_json_binary(&query_bounties(
            deps,
            start_after,
            limit,
            category,
            status,
            poster,
        )?),
        QueryMsg::GetAllBounties { limit, category } => {
            to_json_binary(&query_all_bounties(deps, limit, category)?)
        }
        QueryMsg::GetUserBounties { user, status } => {
            to_json_binary(&query_user_bounties(deps, user, status)?)
        }
        QueryMsg::GetBountySubmission { submission_id } => {
            to_json_binary(&query_bounty_submission(deps, submission_id)?)
        }
        QueryMsg::GetBountySubmissions { bounty_id, status } => {
            to_json_binary(&query_bounty_submissions(deps, bounty_id, status)?)
        }
        QueryMsg::GetUserBountySubmissions {
            user,
            start_after,
            limit,
        } => to_json_binary(&query_user_bounty_submissions(
            deps,
            user,
            start_after,
            limit,
        )?),
    }
}

// Query function implementations
fn query_job(deps: Deps, job_id: u64) -> StdResult<JobResponse> {
    let job = JOBS.load(deps.storage, job_id)?;
    Ok(JobResponse { job })
}

fn query_all_jobs(
    deps: Deps,
    limit: Option<u32>,
    category: Option<String>,
) -> StdResult<JobsResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize; // Max 100 jobs for frontend
    let mut jobs = Vec::new();

    let jobs_result: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, cosmwasm_std::Order::Descending) // Most recent first
        .collect();

    if let Ok(job_pairs) = jobs_result {
        for (_, job) in job_pairs {
            // Only show open jobs for landing page
            if job.status == JobStatus::Open {
                // Filter by category if specified
                if let Some(ref cat) = category {
                    if job.category.to_lowercase() != cat.to_lowercase() {
                        continue;
                    }
                }

                jobs.push(job);

                if jobs.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(JobsResponse { jobs })
}

fn query_jobs(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    category: Option<String>,
    status: Option<JobStatus>,
    poster: Option<String>,
) -> StdResult<JobsResponse> {
    let poster_addr = if let Some(poster_str) = poster {
        Some(deps.api.addr_validate(&poster_str)?)
    } else {
        None
    };

    let jobs = query_jobs_paginated(
        deps.storage,
        start_after,
        limit,
        category,
        status,
        poster_addr,
    )?;

    Ok(JobsResponse { jobs })
}

fn query_user_jobs(deps: Deps, user: String, status: Option<JobStatus>) -> StdResult<JobsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let jobs = query_jobs_paginated(deps.storage, None, None, None, status, Some(user_addr))?;

    Ok(JobsResponse { jobs })
}

fn query_proposal(deps: Deps, proposal_id: u64) -> StdResult<ProposalResponse> {
    let proposal = PROPOSALS.load(deps.storage, proposal_id)?;
    Ok(ProposalResponse { proposal })
}

fn query_job_proposals(deps: Deps, job_id: u64) -> StdResult<ProposalsResponse> {
    let proposal_ids = JOB_PROPOSALS.load(deps.storage, job_id)?;
    let mut proposals = Vec::new();

    for proposal_id in proposal_ids {
        if let Ok(proposal) = PROPOSALS.load(deps.storage, proposal_id) {
            proposals.push(proposal);
        }
    }

    Ok(ProposalsResponse { proposals })
}

fn query_user_proposals_query(
    deps: Deps,
    user: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<ProposalsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let proposals = query_user_proposals(deps.storage, &user_addr, start_after, limit)?;
    Ok(ProposalsResponse { proposals })
}

fn query_escrow(deps: Deps, escrow_id: String) -> StdResult<EscrowResponse> {
    let escrow = ESCROWS.load(deps.storage, &escrow_id)?;
    Ok(EscrowResponse { escrow })
}

fn query_job_escrow(deps: Deps, job_id: u64) -> StdResult<EscrowResponse> {
    let job = JOBS.load(deps.storage, job_id)?;
    if let Some(escrow_id) = job.escrow_id {
        let escrow = ESCROWS.load(deps.storage, &escrow_id)?;
        Ok(EscrowResponse { escrow })
    } else {
        Err(cosmwasm_std::StdError::not_found(
            "Escrow not found for job",
        ))
    }
}

// Query functions implementation
fn query_user_ratings(deps: Deps, user: String) -> StdResult<RatingsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let mut ratings = Vec::new();

    // Iterate through all ratings and find ones where the user is either rater or rated
    let all_ratings: StdResult<Vec<_>> = RATINGS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect();

    if let Ok(rating_pairs) = all_ratings {
        for (_, rating) in rating_pairs {
            if rating.rater == user_addr || rating.rated == user_addr {
                ratings.push(rating);
            }
        }
    }

    Ok(RatingsResponse { ratings })
}

fn query_job_rating(deps: Deps, job_id: u64, rater: String) -> StdResult<Rating> {
    let rater_addr = deps.api.addr_validate(&rater)?;
    let rating_key = format!("{}_{}", job_id, rater_addr);
    let rating = RATINGS.load(deps.storage, &rating_key)?;
    Ok(rating)
}

fn query_user_stats(deps: Deps, user: String) -> StdResult<UserStatsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let stats = USER_STATS
        .may_load(deps.storage, &user_addr)?
        .unwrap_or_default();
    Ok(UserStatsResponse { stats })
}

fn query_platform_stats(deps: Deps) -> StdResult<PlatformStatsResponse> {
    let mut total_jobs = 0u64;
    let mut active_jobs = 0u64;
    let mut completed_jobs = 0u64;
    let mut total_volume = Uint128::zero();

    // Efficiently process jobs using iterator without collecting all into memory
    for (_, job) in JOBS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .flatten()
    {
        total_jobs += 1;
        total_volume = total_volume.checked_add(job.budget)?;

        match job.status {
            JobStatus::Open | JobStatus::InProgress => active_jobs += 1,
            JobStatus::Completed => completed_jobs += 1,
            _ => {}
        }
    }

    // Count unique users efficiently
    let total_users = USER_STATS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .count() as u64;

    // Calculate platform fees (approximate based on volume and fee percentage)
    let config = CONFIG.load(deps.storage)?;
    let platform_fees_collected = total_volume.multiply_ratio(config.platform_fee_percent, 100u64);

    Ok(PlatformStatsResponse {
        total_jobs,
        active_jobs,
        completed_jobs,
        total_users,
        total_volume,
        platform_fees_collected,
    })
}

fn query_dispute(deps: Deps, dispute_id: String) -> StdResult<DisputeResponse> {
    let dispute = DISPUTES.load(deps.storage, &dispute_id)?;
    Ok(DisputeResponse { dispute })
}

fn query_job_disputes(deps: Deps, job_id: u64) -> StdResult<DisputesResponse> {
    let mut disputes = Vec::new();

    let disputes_result: StdResult<Vec<_>> = DISPUTES
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect();

    if let Ok(dispute_pairs) = disputes_result {
        for (_, dispute) in dispute_pairs {
            if dispute.job_id == job_id {
                disputes.push(dispute);
            }
        }
    }

    Ok(DisputesResponse { disputes })
}

fn query_user_disputes(deps: Deps, user: String) -> StdResult<DisputesResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let mut disputes = Vec::new();

    let disputes_result: StdResult<Vec<_>> = DISPUTES
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect();

    if let Ok(dispute_pairs) = disputes_result {
        for (_, dispute) in dispute_pairs {
            if dispute.raised_by == user_addr {
                disputes.push(dispute);
            }
        }
    }

    Ok(DisputesResponse { disputes })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse { config })
}

// Security execute functions
fn execute_block_address(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    reason: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only admin can block addresses
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr_to_block = deps.api.addr_validate(&address)?;
    BLOCKED_ADDRESSES.save(deps.storage, &addr_to_block, &env.block.time)?;

    Ok(Response::new()
        .add_attribute("method", "block_address")
        .add_attribute("blocked_address", address)
        .add_attribute("reason", reason)
        .add_attribute("admin", info.sender.to_string()))
}

fn execute_unblock_address(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only admin can unblock addresses
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr_to_unblock = deps.api.addr_validate(&address)?;
    BLOCKED_ADDRESSES.remove(deps.storage, &addr_to_unblock);

    Ok(Response::new()
        .add_attribute("method", "unblock_address")
        .add_attribute("unblocked_address", address)
        .add_attribute("admin", info.sender.to_string()))
}

fn execute_reset_rate_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only admin can reset rate limits
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr_to_reset = deps.api.addr_validate(&address)?;

    // Remove all rate limit entries for this address
    let rate_limit_keys: StdResult<Vec<_>> = RATE_LIMITS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .filter(|item| {
            if let Ok((key, _)) = item {
                key.0 == addr_to_reset
            } else {
                false
            }
        })
        .collect();

    if let Ok(keys) = rate_limit_keys {
        for (key, _) in keys {
            RATE_LIMITS.remove(deps.storage, (&key.0, &key.1));
        }
    }

    Ok(Response::new()
        .add_attribute("method", "reset_rate_limit")
        .add_attribute("reset_address", address)
        .add_attribute("admin", info.sender.to_string()))
}

// Security query functions
fn query_security_metrics(deps: Deps) -> StdResult<crate::msg::SecurityMetricsResponse> {
    // Get basic metrics from storage
    let mut total_jobs = 0u64;
    let mut total_proposals = 0u64;
    let mut total_disputes = 0u64;

    // Count jobs
    if let Ok(jobs) = JOBS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()
    {
        total_jobs = jobs.len() as u64;
    }

    // Count proposals
    if let Ok(proposals) = PROPOSALS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()
    {
        total_proposals = proposals.len() as u64;
    }

    // Count disputes
    if let Ok(disputes) = DISPUTES
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()
    {
        total_disputes = disputes.len() as u64;
    }

    // Get blocked addresses
    let blocked_addresses = if let Ok(blocked) = BLOCKED_ADDRESSES
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()
    {
        blocked.into_iter().map(|(addr, _)| addr).collect()
    } else {
        Vec::new()
    };

    let metrics = crate::state::SecurityMetrics {
        total_jobs,
        total_proposals,
        total_disputes,
        blocked_addresses,
        rate_limit_violations: 0, // This would be tracked in a real implementation
        last_updated: cosmwasm_std::Timestamp::from_seconds(0), // Current time would be passed from env
    };

    Ok(crate::msg::SecurityMetricsResponse { metrics })
}

fn query_audit_logs(
    _deps: Deps,
    _start_after: Option<String>,
    _limit: Option<u32>,
    _action_filter: Option<String>,
) -> StdResult<crate::msg::AuditLogsResponse> {
    // Basic implementation - in a real system this would query actual audit logs
    Ok(crate::msg::AuditLogsResponse {
        logs: Vec::new(), // Would return actual audit logs from storage
    })
}

fn query_is_address_blocked(
    deps: Deps,
    address: String,
) -> StdResult<crate::msg::AddressBlockedResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let is_blocked = BLOCKED_ADDRESSES.may_load(deps.storage, &addr)?.is_some();
    Ok(crate::msg::AddressBlockedResponse {
        is_blocked,
        reason: None, // Could be enhanced to store and return the blocking reason
    })
}

fn query_rate_limit_status(
    deps: Deps,
    address: String,
) -> StdResult<crate::msg::RateLimitStatusResponse> {
    let addr = deps.api.addr_validate(&address)?;

    // Get rate limit state from the enhanced security system
    let current_time = cosmwasm_std::Timestamp::from_seconds(0); // Use a default timestamp
    let rate_limit = crate::security::USER_RATE_LIMITS
        .may_load(deps.storage, &addr)?
        .unwrap_or(crate::security::RateLimit {
            daily_jobs: 0,
            daily_proposals: 0,
            daily_bounties: 0,
            daily_disputes: 0,
            daily_escrows: 0,
            daily_admin_actions: 0,
            last_reset: current_time,
        });

    Ok(crate::msg::RateLimitStatusResponse {
        current_count: rate_limit.daily_jobs, // Use jobs as primary metric
        limit: 5,                             // MAX_JOBS_PER_USER_PER_DAY
        window_start: rate_limit.last_reset,
        is_limited: rate_limit.daily_jobs >= 5,
    })
}

// ========================================
// BOUNTY EXECUTION FUNCTIONS
// ========================================

#[allow(clippy::too_many_arguments)]
fn execute_create_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    requirements: Vec<String>,
    total_reward: Uint128,
    category: String,
    skills_required: Vec<String>,
    submission_deadline_days: u64,
    review_period_days: u64,
    max_winners: u64,
    reward_distribution: Vec<RewardTierInput>,
    documents: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // Security checks
    reentrancy_guard(deps.branch())?;
    check_rate_limit(deps.branch(), &env, &info.sender, RateLimitAction::PostJob)?;
    ensure_not_paused(deps.as_ref())?;

    if skills_required.is_empty() || skills_required.len() > 20 {
        return Err(ContractError::InvalidInput {
            error: "Skills required must have 1-20 items".to_string(),
        });
    }

    if requirements.is_empty() || requirements.len() > 10 {
        return Err(ContractError::InvalidInput {
            error: "Requirements must have 1-10 items".to_string(),
        });
    }

    if submission_deadline_days == 0 || submission_deadline_days > 365 {
        return Err(ContractError::InvalidInput {
            error: "Submission deadline must be between 1-365 days".to_string(),
        });
    }

    if review_period_days == 0 || review_period_days > 30 {
        return Err(ContractError::InvalidInput {
            error: "Review period must be between 1-30 days".to_string(),
        });
    }

    if max_winners == 0 || max_winners > 50 {
        return Err(ContractError::InvalidInput {
            error: "Max winners must be between 1-50".to_string(),
        });
    }

    // Validate reward distribution
    if reward_distribution.len() != max_winners as usize {
        return Err(ContractError::InvalidInput {
            error: "Reward distribution must match max winners count".to_string(),
        });
    }

    let mut total_percentage = 0u64;
    let mut reward_tiers: Vec<RewardTier> = Vec::new();

    for (i, tier_input) in reward_distribution.iter().enumerate() {
        if tier_input.position != (i + 1) as u64 {
            return Err(ContractError::InvalidInput {
                error: "Reward positions must be sequential starting from 1".to_string(),
            });
        }

        if tier_input.percentage == 0 || tier_input.percentage > 100 {
            return Err(ContractError::InvalidInput {
                error: "Each reward percentage must be between 1-100".to_string(),
            });
        }

        total_percentage += tier_input.percentage;

        let amount = total_reward * Uint128::from(tier_input.percentage) / Uint128::from(100u64);
        reward_tiers.push(RewardTier {
            position: tier_input.position,
            percentage: tier_input.percentage,
            amount,
        });
    }

    if total_percentage != 100 {
        return Err(ContractError::InvalidInput {
            error: "Total reward percentage must equal 100".to_string(),
        });
    }

    // Get and increment bounty counter
    let bounty_id = BOUNTY_COUNTER.load(deps.storage).unwrap_or(0);
    BOUNTY_COUNTER.save(deps.storage, &(bounty_id + 1))?;

    // Calculate deadlines
    let submission_deadline = get_future_timestamp(env.block.time, submission_deadline_days);

    // Create bounty
    let bounty = Bounty {
        id: bounty_id,
        poster: info.sender.clone(),
        title: title.clone(),
        description: description.clone(),
        requirements,
        total_reward,
        category: category.clone(),
        skills_required,
        submission_deadline,
        review_period_days,
        max_winners,
        reward_distribution: reward_tiers,
        documents: documents.unwrap_or_default(),
        status: BountyStatus::Open,
        created_at: env.block.time,
        updated_at: env.block.time,
        total_submissions: 0,
        selected_winners: Vec::new(),
        escrow_id: None,
    };

    // Save bounty
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_attribute("method", "create_bounty")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("poster", info.sender.to_string())
        .add_attribute("title", title)
        .add_attribute("total_reward", total_reward.to_string())
        .add_attribute("category", category)
        .add_attribute("max_winners", max_winners.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn execute_edit_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
    title: Option<String>,
    description: Option<String>,
    requirements: Option<Vec<String>>,
    submission_deadline_days: Option<u64>,
    review_period_days: Option<u64>,
    documents: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Only bounty poster can edit
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only edit open bounties
    if bounty.status != BountyStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Can only edit open bounties".to_string(),
        });
    }

    // Check if bounty has submissions - if so, limit what can be edited
    if bounty.total_submissions > 0
        && (title.is_some() || requirements.is_some() || submission_deadline_days.is_some())
    {
        return Err(ContractError::InvalidInput {
            error: "Cannot change title, requirements, or deadline when submissions exist"
                .to_string(),
        });
    }

    // Update fields if provided
    if let Some(new_title) = title {
        validate_job_title(&new_title)?;
        bounty.title = new_title;
    }

    if let Some(new_description) = description {
        validate_job_description(&new_description)?;
        bounty.description = new_description;
    }

    if let Some(new_requirements) = requirements {
        if new_requirements.is_empty() || new_requirements.len() > 10 {
            return Err(ContractError::InvalidInput {
                error: "Requirements must have 1-10 items".to_string(),
            });
        }
        bounty.requirements = new_requirements;
    }

    if let Some(deadline_days) = submission_deadline_days {
        if deadline_days == 0 || deadline_days > 365 {
            return Err(ContractError::InvalidInput {
                error: "Submission deadline must be between 1-365 days".to_string(),
            });
        }
        bounty.submission_deadline = get_future_timestamp(env.block.time, deadline_days);
    }

    if let Some(review_days) = review_period_days {
        if review_days == 0 || review_days > 30 {
            return Err(ContractError::InvalidInput {
                error: "Review period must be between 1-30 days".to_string(),
            });
        }
        bounty.review_period_days = review_days;
    }

    if let Some(new_documents) = documents {
        bounty.documents = new_documents;
    }

    bounty.updated_at = env.block.time;

    // Save updated bounty
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_attribute("method", "edit_bounty")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("editor", info.sender.to_string()))
}

fn execute_cancel_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Only bounty poster can cancel
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only cancel open bounties
    if bounty.status != BountyStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Can only cancel open bounties".to_string(),
        });
    }

    // Update status
    bounty.status = BountyStatus::Cancelled;
    bounty.updated_at = env.block.time;

    // Save updated bounty
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_attribute("method", "cancel_bounty")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("poster", info.sender.to_string()))
}

fn execute_submit_to_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
    title: String,
    description: String,
    deliverables: Vec<String>,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;
    check_rate_limit(
        deps.branch(),
        &env,
        &info.sender,
        RateLimitAction::SubmitProposal,
    )?; // Use same rate limit as proposals

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Can only submit to open bounties
    if bounty.status != BountyStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Can only submit to open bounties".to_string(),
        });
    }

    // Check deadline
    if env.block.time >= bounty.submission_deadline {
        return Err(ContractError::InvalidInput {
            error: "Submission deadline has passed".to_string(),
        });
    }

    // Validate inputs
    if title.is_empty() || title.len() > 100 {
        return Err(ContractError::InvalidInput {
            error: "Title must be 1-100 characters".to_string(),
        });
    }

    if description.is_empty() || description.len() > 2000 {
        return Err(ContractError::InvalidInput {
            error: "Description must be 1-2000 characters".to_string(),
        });
    }

    if deliverables.is_empty() || deliverables.len() > 10 {
        return Err(ContractError::InvalidInput {
            error: "Must provide 1-10 deliverable URLs".to_string(),
        });
    }

    // Check if user already submitted
    let user_submissions = USER_BOUNTY_SUBMISSIONS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    for submission_id in user_submissions.iter() {
        let submission = BOUNTY_SUBMISSIONS.load(deps.storage, *submission_id)?;
        if submission.bounty_id == bounty_id
            && submission.status != BountySubmissionStatus::Rejected
        {
            return Err(ContractError::InvalidInput {
                error: "You have already submitted to this bounty".to_string(),
            });
        }
    }

    // Get and increment submission counter
    let submission_id = BOUNTY_SUBMISSION_COUNTER.load(deps.storage).unwrap_or(0);
    BOUNTY_SUBMISSION_COUNTER.save(deps.storage, &(submission_id + 1))?;

    // Create submission
    let submission = BountySubmission {
        id: submission_id,
        bounty_id,
        submitter: info.sender.clone(),
        title: title.clone(),
        description: description.clone(),
        deliverables,
        submitted_at: env.block.time,
        status: BountySubmissionStatus::Submitted,
        review_notes: None,
        score: None,
        winner_position: None,
    };

    // Save submission
    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    // Update bounty submission count
    bounty.total_submissions += 1;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // Update bounty submissions mapping
    let mut bounty_submissions = BOUNTY_SUBMISSIONS_BY_BOUNTY
        .may_load(deps.storage, bounty_id)?
        .unwrap_or_default();
    bounty_submissions.push(submission_id);
    BOUNTY_SUBMISSIONS_BY_BOUNTY.save(deps.storage, bounty_id, &bounty_submissions)?;

    // Update user submissions mapping
    let mut user_submissions = USER_BOUNTY_SUBMISSIONS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    user_submissions.push(submission_id);
    USER_BOUNTY_SUBMISSIONS.save(deps.storage, &info.sender, &user_submissions)?;

    Ok(Response::new()
        .add_attribute("method", "submit_to_bounty")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("submitter", info.sender.to_string())
        .add_attribute("title", title))
}

fn execute_edit_bounty_submission(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_id: u64,
    title: Option<String>,
    description: Option<String>,
    deliverables: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate submission
    let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;

    // Only submitter can edit
    if submission.submitter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only edit submitted or rejected submissions
    if submission.status != BountySubmissionStatus::Submitted
        && submission.status != BountySubmissionStatus::Rejected
    {
        return Err(ContractError::InvalidInput {
            error: "Can only edit submitted or rejected submissions".to_string(),
        });
    }

    // Check if bounty deadline has passed
    let bounty = BOUNTIES.load(deps.storage, submission.bounty_id)?;
    if env.block.time >= bounty.submission_deadline {
        return Err(ContractError::InvalidInput {
            error: "Cannot edit submission after deadline".to_string(),
        });
    }

    // Update fields if provided
    if let Some(new_title) = title {
        if new_title.is_empty() || new_title.len() > 100 {
            return Err(ContractError::InvalidInput {
                error: "Title must be 1-100 characters".to_string(),
            });
        }
        submission.title = new_title;
    }

    if let Some(new_description) = description {
        if new_description.is_empty() || new_description.len() > 2000 {
            return Err(ContractError::InvalidInput {
                error: "Description must be 1-2000 characters".to_string(),
            });
        }
        submission.description = new_description;
    }

    if let Some(new_deliverables) = deliverables {
        if new_deliverables.is_empty() || new_deliverables.len() > 10 {
            return Err(ContractError::InvalidInput {
                error: "Must provide 1-10 deliverable URLs".to_string(),
            });
        }
        submission.deliverables = new_deliverables;
    }

    // Reset status to submitted if it was rejected
    if submission.status == BountySubmissionStatus::Rejected {
        submission.status = BountySubmissionStatus::Submitted;
        submission.review_notes = None;
        submission.score = None;
    }

    // Save updated submission
    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    Ok(Response::new()
        .add_attribute("method", "edit_bounty_submission")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("submitter", info.sender.to_string()))
}

fn execute_withdraw_bounty_submission(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    submission_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate submission
    let submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;

    // Only submitter can withdraw
    if submission.submitter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only withdraw submitted submissions
    if submission.status != BountySubmissionStatus::Submitted {
        return Err(ContractError::InvalidInput {
            error: "Can only withdraw submitted submissions".to_string(),
        });
    }

    // Remove submission from storage
    BOUNTY_SUBMISSIONS.remove(deps.storage, submission_id);

    // Update bounty submission count
    let mut bounty = BOUNTIES.load(deps.storage, submission.bounty_id)?;
    bounty.total_submissions = bounty.total_submissions.saturating_sub(1);
    BOUNTIES.save(deps.storage, submission.bounty_id, &bounty)?;

    // Update bounty submissions mapping
    let mut bounty_submissions = BOUNTY_SUBMISSIONS_BY_BOUNTY
        .may_load(deps.storage, submission.bounty_id)?
        .unwrap_or_default();
    bounty_submissions.retain(|&id| id != submission_id);
    BOUNTY_SUBMISSIONS_BY_BOUNTY.save(deps.storage, submission.bounty_id, &bounty_submissions)?;

    // Update user submissions mapping
    let mut user_submissions = USER_BOUNTY_SUBMISSIONS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    user_submissions.retain(|&id| id != submission_id);
    USER_BOUNTY_SUBMISSIONS.save(deps.storage, &info.sender, &user_submissions)?;

    Ok(Response::new()
        .add_attribute("method", "withdraw_bounty_submission")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("submitter", info.sender.to_string()))
}

fn execute_review_bounty_submission(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    submission_id: u64,
    status: BountySubmissionStatus,
    review_notes: Option<String>,
    score: Option<u8>,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate submission
    let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
    let bounty = BOUNTIES.load(deps.storage, submission.bounty_id)?;

    // Only bounty poster can review
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only review submitted submissions
    if submission.status != BountySubmissionStatus::Submitted {
        return Err(ContractError::InvalidInput {
            error: "Can only review submitted submissions".to_string(),
        });
    }

    // Validate status change
    match status {
        BountySubmissionStatus::UnderReview
        | BountySubmissionStatus::Approved
        | BountySubmissionStatus::Rejected => {}
        _ => {
            return Err(ContractError::InvalidInput {
                error: "Invalid review status".to_string(),
            })
        }
    }

    // Validate score if provided
    if let Some(score_val) = score {
        if score_val == 0 || score_val > 10 {
            return Err(ContractError::InvalidInput {
                error: "Score must be between 1-10".to_string(),
            });
        }
    }

    // Validate review notes length
    if let Some(ref notes) = review_notes {
        if notes.len() > 500 {
            return Err(ContractError::InvalidInput {
                error: "Review notes must be max 500 characters".to_string(),
            });
        }
    }

    // Update submission
    submission.status = status.clone();
    submission.review_notes = review_notes;
    submission.score = score;

    // Save updated submission
    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    Ok(Response::new()
        .add_attribute("method", "review_bounty_submission")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("bounty_id", submission.bounty_id.to_string())
        .add_attribute("reviewer", info.sender.to_string())
        .add_attribute("status", format!("{:?}", status))
        .add_attribute("score", score.map_or("none".to_string(), |s| s.to_string())))
}

fn execute_select_bounty_winners(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
    winner_submissions: Vec<WinnerSelection>,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Only bounty poster can select winners
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only select winners for open bounties
    if bounty.status != BountyStatus::Open {
        return Err(ContractError::InvalidInput {
            error: "Can only select winners for open bounties".to_string(),
        });
    }

    // Check if submission deadline has passed
    if env.block.time < bounty.submission_deadline {
        return Err(ContractError::InvalidInput {
            error: "Cannot select winners before submission deadline".to_string(),
        });
    }

    // Validate winner selections
    if winner_submissions.is_empty() || winner_submissions.len() > bounty.max_winners as usize {
        return Err(ContractError::InvalidInput {
            error: format!("Must select 1-{} winners", bounty.max_winners),
        });
    }

    // Validate winner positions and submissions
    let mut selected_positions = std::collections::HashSet::new();
    let mut selected_submission_ids = std::collections::HashSet::new();

    for winner in &winner_submissions {
        // Check position is valid
        if winner.position == 0 || winner.position > bounty.max_winners {
            return Err(ContractError::InvalidInput {
                error: format!("Position must be between 1-{}", bounty.max_winners),
            });
        }

        // Check position is unique
        if !selected_positions.insert(winner.position) {
            return Err(ContractError::InvalidInput {
                error: "Each position can only have one winner".to_string(),
            });
        }

        // Check submission exists and belongs to this bounty
        let submission = BOUNTY_SUBMISSIONS.load(deps.storage, winner.submission_id)?;
        if submission.bounty_id != bounty_id {
            return Err(ContractError::InvalidInput {
                error: "Submission does not belong to this bounty".to_string(),
            });
        }

        // Check submission is approved
        if submission.status != BountySubmissionStatus::Approved {
            return Err(ContractError::InvalidInput {
                error: "Only approved submissions can be selected as winners".to_string(),
            });
        }

        // Check submission is unique
        if !selected_submission_ids.insert(winner.submission_id) {
            return Err(ContractError::InvalidInput {
                error: "Each submission can only win one position".to_string(),
            });
        }
    }

    // Update winning submissions
    for winner in &winner_submissions {
        let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, winner.submission_id)?;
        submission.status = BountySubmissionStatus::Winner;
        submission.winner_position = Some(winner.position);
        BOUNTY_SUBMISSIONS.save(deps.storage, winner.submission_id, &submission)?;
    }

    // Update bounty
    bounty.status = BountyStatus::InReview;
    bounty.selected_winners = winner_submissions.iter().map(|w| w.submission_id).collect();
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_attribute("method", "select_bounty_winners")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("selector", info.sender.to_string())
        .add_attribute("winners_count", winner_submissions.len().to_string()))
}

fn execute_create_bounty_escrow(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Only bounty poster can create escrow
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only create escrow for bounties in review or with selected winners
    if bounty.status != BountyStatus::InReview {
        return Err(ContractError::InvalidInput {
            error: "Can only create escrow for bounties with selected winners".to_string(),
        });
    }

    // Check if escrow already exists
    if bounty.escrow_id.is_some() {
        return Err(ContractError::InvalidInput {
            error: "Escrow already exists for this bounty".to_string(),
        });
    }

    // Check if winners are selected
    if bounty.selected_winners.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "No winners selected for this bounty".to_string(),
        });
    }

    // Validate payment amount
    let config = CONFIG.load(deps.storage)?;
    let platform_fee =
        bounty.total_reward * Uint128::from(config.platform_fee_percent) / Uint128::from(100u64);
    let total_required = bounty.total_reward + platform_fee;

    let payment = info.funds.iter().find(|coin| coin.denom == "uxion");
    match payment {
        Some(coin) => {
            if coin.amount < total_required {
                return Err(ContractError::InsufficientFunds {
                    expected: total_required.to_string(),
                    actual: coin.amount.to_string(),
                });
            }
        }
        None => {
            return Err(ContractError::InsufficientFunds {
                expected: total_required.to_string(),
                actual: "0".to_string(),
            });
        }
    }

    // Create escrow ID
    let escrow_id = format!("bounty_{}_{}", bounty_id, env.block.time.seconds());

    // Update bounty
    bounty.escrow_id = Some(escrow_id.clone());
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_attribute("method", "create_bounty_escrow")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("escrow_id", escrow_id)
        .add_attribute("total_amount", total_required.to_string())
        .add_attribute("platform_fee", platform_fee.to_string())
        .add_attribute("poster", info.sender.to_string()))
}

fn execute_release_bounty_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Security checks
    ensure_not_paused(deps.as_ref())?;
    reentrancy_guard(deps.branch())?;

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;

    // Only bounty poster can release rewards
    if bounty.poster != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can only release rewards for bounties in review
    if bounty.status != BountyStatus::InReview {
        return Err(ContractError::InvalidInput {
            error: "Can only release rewards for bounties in review".to_string(),
        });
    }

    // Check if escrow exists
    if bounty.escrow_id.is_none() {
        return Err(ContractError::InvalidInput {
            error: "No escrow found for this bounty".to_string(),
        });
    }

    // Check if winners are selected
    if bounty.selected_winners.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "No winners selected for this bounty".to_string(),
        });
    }

    // Create payout messages for winners
    let mut messages = Vec::new();
    let mut total_paid = Uint128::zero();

    for winner_submission_id in &bounty.selected_winners {
        let submission = BOUNTY_SUBMISSIONS.load(deps.storage, *winner_submission_id)?;

        if let Some(position) = submission.winner_position {
            // Find the reward tier for this position
            if let Some(reward_tier) = bounty
                .reward_distribution
                .iter()
                .find(|tier| tier.position == position)
            {
                let payout_msg = cosmwasm_std::BankMsg::Send {
                    to_address: submission.submitter.to_string(),
                    amount: vec![cosmwasm_std::Coin {
                        denom: "uxion".to_string(),
                        amount: reward_tier.amount,
                    }],
                };
                messages.push(cosmwasm_std::SubMsg::new(payout_msg));
                total_paid += reward_tier.amount;
            }
        }
    }

    // Update bounty status
    bounty.status = BountyStatus::Completed;
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(Response::new()
        .add_submessages(messages)
        .add_attribute("method", "release_bounty_rewards")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("total_paid", total_paid.to_string())
        .add_attribute("winners_count", bounty.selected_winners.len().to_string())
        .add_attribute("poster", info.sender.to_string()))
}

// ========================================
// BOUNTY QUERY FUNCTIONS
// ========================================

fn query_bounty(deps: Deps, bounty_id: u64) -> StdResult<BountyResponse> {
    let bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    Ok(BountyResponse { bounty })
}

fn query_bounties(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    category: Option<String>,
    status: Option<BountyStatus>,
    poster: Option<String>,
) -> StdResult<BountiesResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize;
    let start = start_after.map(Bound::exclusive);

    let bounties: StdResult<Vec<Bounty>> = BOUNTIES
        .range(deps.storage, start, None, cosmwasm_std::Order::Descending)
        .take(limit)
        .map(|item| item.map(|(_, bounty)| bounty))
        .filter(|result| {
            if let Ok(bounty) = result {
                // Filter by category
                if let Some(ref cat) = category {
                    if bounty.category != *cat {
                        return false;
                    }
                }

                // Filter by status
                if let Some(ref stat) = status {
                    if bounty.status != *stat {
                        return false;
                    }
                }

                // Filter by poster
                if let Some(ref post) = poster {
                    if bounty.poster.as_str() != post {
                        return false;
                    }
                }

                true
            } else {
                true
            }
        })
        .collect();

    Ok(BountiesResponse {
        bounties: bounties?,
    })
}

fn query_all_bounties(
    deps: Deps,
    limit: Option<u32>,
    category: Option<String>,
) -> StdResult<BountiesResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize;

    let bounties: StdResult<Vec<Bounty>> = BOUNTIES
        .range(deps.storage, None, None, cosmwasm_std::Order::Descending)
        .take(limit)
        .map(|item| item.map(|(_, bounty)| bounty))
        .filter(|result| {
            if let Ok(bounty) = result {
                // Only show open bounties
                if bounty.status != BountyStatus::Open {
                    return false;
                }

                // Filter by category
                if let Some(ref cat) = category {
                    if bounty.category != *cat {
                        return false;
                    }
                }

                true
            } else {
                true
            }
        })
        .collect();

    Ok(BountiesResponse {
        bounties: bounties?,
    })
}

fn query_user_bounties(
    deps: Deps,
    user: String,
    status: Option<BountyStatus>,
) -> StdResult<BountiesResponse> {
    let user_addr = deps.api.addr_validate(&user)?;

    let bounties: StdResult<Vec<Bounty>> = BOUNTIES
        .range(deps.storage, None, None, cosmwasm_std::Order::Descending)
        .map(|item| item.map(|(_, bounty)| bounty))
        .filter(|result| {
            if let Ok(bounty) = result {
                // Filter by poster
                if bounty.poster != user_addr {
                    return false;
                }

                // Filter by status
                if let Some(ref stat) = status {
                    if bounty.status != *stat {
                        return false;
                    }
                }

                true
            } else {
                true
            }
        })
        .collect();

    Ok(BountiesResponse {
        bounties: bounties?,
    })
}

fn query_bounty_submission(deps: Deps, submission_id: u64) -> StdResult<BountySubmissionResponse> {
    let submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
    Ok(BountySubmissionResponse { submission })
}

fn query_bounty_submissions(
    deps: Deps,
    bounty_id: u64,
    status: Option<BountySubmissionStatus>,
) -> StdResult<BountySubmissionsResponse> {
    let submission_ids = BOUNTY_SUBMISSIONS_BY_BOUNTY
        .may_load(deps.storage, bounty_id)?
        .unwrap_or_default();

    let submissions: StdResult<Vec<BountySubmission>> = submission_ids
        .into_iter()
        .map(|id| BOUNTY_SUBMISSIONS.load(deps.storage, id))
        .filter(|result| {
            if let Ok(submission) = result {
                // Filter by status
                if let Some(ref stat) = status {
                    submission.status == *stat
                } else {
                    true
                }
            } else {
                true
            }
        })
        .collect();

    Ok(BountySubmissionsResponse {
        submissions: submissions?,
    })
}

fn query_user_bounty_submissions(
    deps: Deps,
    user: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<BountySubmissionsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let limit = limit.unwrap_or(50).min(100) as usize;

    let submission_ids = USER_BOUNTY_SUBMISSIONS
        .may_load(deps.storage, &user_addr)?
        .unwrap_or_default();

    let mut filtered_ids = submission_ids;

    // Apply start_after filter
    if let Some(after_id) = start_after {
        filtered_ids.retain(|&id| id > after_id);
    }

    // Sort and limit
    filtered_ids.sort_by(|a, b| b.cmp(a)); // Descending order
    filtered_ids.truncate(limit);

    let submissions: StdResult<Vec<BountySubmission>> = filtered_ids
        .into_iter()
        .map(|id| BOUNTY_SUBMISSIONS.load(deps.storage, id))
        .collect();

    Ok(BountySubmissionsResponse {
        submissions: submissions?,
    })
}
