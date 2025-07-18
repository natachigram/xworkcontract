use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::helpers::{
    ensure_not_paused, get_future_timestamp, validate_budget, validate_duration,
    convert_category_to_id, convert_skills_to_ids, calculate_reward_range, 
    calculate_difficulty_from_skills, estimate_hours_from_reward_and_difficulty,
};
use crate::hash_utils::{
    create_content_hash, create_bounty_content_bundle, create_bounty_submission_content_bundle,
};
use crate::job_management::calculate_platform_fee;
use crate::msg::{BountiesResponse, BountyResponse, BountySubmissionsResponse, WinnerSelection};
use crate::security::{check_rate_limit, reentrancy_guard, RateLimitAction};
use crate::state::{
    BountySubmissionStatus, BountyStatus, Bounty, BountySubmission, RewardTier,
    BOUNTIES, BOUNTY_SUBMISSIONS, BOUNTY_SUBMISSIONS_BY_BOUNTY, ESCROWS, EscrowState,
    DisputeStatus, CONFIG, NEXT_BOUNTY_ID, NEXT_BOUNTY_SUBMISSION_ID, CONTENT_HASHES,
    HASH_TO_ENTITY, ENTITY_TO_HASH, BOUNTIES_BY_CATEGORY, BOUNTIES_BY_REWARD_RANGE,
    BOUNTIES_BY_SKILL, BOUNTIES_BY_DIFFICULTY, ACTIVE_BOUNTIES, FEATURED_BOUNTIES,
};
use crate::hash_utils::ContentHash;
use crate::{apply_security_checks, build_success_response, validate_content_inputs};
use cosmwasm_std::{
    coins, BankMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, Uint128,
};

/// Create a new bounty
#[allow(clippy::too_many_arguments)]
pub fn execute_create_bounty(
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
    reward_distribution: Vec<crate::msg::RewardTierInput>,
    documents: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CreateBounty);

    // Load configuration
    let config = CONFIG.load(deps.storage)?;

    // Validate inputs
    validate_content_inputs!(&title, &description);
    validate_budget(total_reward)?;
    validate_duration(submission_deadline_days, config.max_job_duration_days)?;
    validate_string_field(&category, "Category", 1, 50)?;
    validate_collection_size(&skills_required, "Skills required", 1, 20)?;
    validate_collection_size(&requirements, "Requirements", 1, 10)?;

    if max_winners == 0 || max_winners > 100 {
        return Err(ContractError::InvalidInput {
            error: "Max winners must be between 1 and 100".to_string(),
        });
    }

    // Validate payment
    if info.funds.len() != 1 || info.funds[0].amount != total_reward {
        return Err(ContractError::InvalidFunds {});
    }

    // Get next bounty ID
    let bounty_id = NEXT_BOUNTY_ID.load(deps.storage)?;
    NEXT_BOUNTY_ID.save(deps.storage, &(bounty_id + 1))?;

    // 🔥 Create off-chain content bundle
    let documents_vec = documents.unwrap_or_default();
    let (off_chain_bundle, content_hash_str) = create_bounty_content_bundle(
        bounty_id,
        &title,
        &description,
        &requirements,
        &documents_vec,
        &category,
        &skills_required,
        env.block.time.seconds(),
    )?;

    // 📄 Create content hash metadata
    let content_hash = create_content_hash(
        &serde_json::to_string(&off_chain_bundle)
            .map_err(|e| ContractError::InvalidInput {
                error: format!("Failed to serialize off-chain bundle: {}", e),
            })?,
        "bounty_content",
        env.block.time.seconds(),
    )?;

    // 🗄️ Store hash mappings for retrieval
    let entity_key = format!("bounty_{}", bounty_id);
    CONTENT_HASHES.save(deps.storage, &content_hash_str, &content_hash)?;
    HASH_TO_ENTITY.save(deps.storage, &content_hash_str, &entity_key)?;
    ENTITY_TO_HASH.save(deps.storage, &entity_key, &content_hash_str)?;

    // 📊 Calculate metadata for efficient searching
    let category_id = convert_category_to_id(&category);
    let skill_tags = convert_skills_to_ids(&skills_required);
    let reward_range = calculate_reward_range(total_reward);
    let difficulty_level = calculate_difficulty_from_skills(&skills_required);
    let estimated_hours = estimate_hours_from_reward_and_difficulty(total_reward, difficulty_level);

    // Convert reward distribution
    let mut reward_tiers = Vec::new();
    for (i, tier_input) in reward_distribution.iter().enumerate() {
        reward_tiers.push(RewardTier {
            position: (i + 1) as u64,
            percentage: tier_input.percentage,
            amount: total_reward.multiply_ratio(tier_input.percentage, 100u64),
        });
    }

    let bounty = Bounty {
        id: bounty_id,
        poster: info.sender.clone(),
        total_reward,
        submission_deadline: get_future_timestamp(env.block.time, submission_deadline_days),
        review_period_days,
        max_winners,
        reward_distribution: reward_tiers,
        status: BountyStatus::Open,
        created_at: env.block.time,
        updated_at: env.block.time,
        total_submissions: 0,
        selected_winners: vec![],
        escrow_id: None,
        
        // 🌐 Off-chain content reference
        content_hash,
        
        // 📊 On-chain metadata for efficient searching
        category_id,
        skill_tags,
        reward_range,
        difficulty_level,
        estimated_hours,
        is_featured: false,
    };

    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // 🔍 Update search indexes for efficient querying
    update_bounty_search_indexes(deps.storage, &bounty)?;

    // Create escrow using EscrowState schema
    let escrow_id = format!("bounty_{}", bounty_id);
    let escrow = crate::state::EscrowState {
        id: escrow_id.clone(),
        job_id: 0, // Not applicable for bounties, but required field
        client: info.sender.clone(),
        freelancer: info.sender.clone(), // Placeholder until winner is selected
        amount: total_reward,
        platform_fee: calculate_platform_fee(total_reward, config.platform_fee_percent),
        funded_at: env.block.time,
        released: false,
        dispute_status: crate::state::DisputeStatus::None,
        dispute_raised_at: None,
        dispute_deadline: None,
    };

    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    Ok(build_success_response!(
        "create_bounty",
        bounty_id,
        &info.sender,
        "total_reward" => total_reward.to_string(),
        "category" => category,
        "content_hash" => content_hash_str,
        "escrow_id" => escrow_id
    ))
}

/// Edit an existing bounty
#[allow(clippy::too_many_arguments)]
pub fn execute_edit_bounty(
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
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::EditBounty);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;
    validate_bounty_status_for_operation(&bounty.status, &[BountyStatus::Open], "edit")?;

    let _config = CONFIG.load(deps.storage)?;

    // If any content fields are being updated, we need to create a new content bundle
    let content_needs_update = title.is_some() || description.is_some() || 
                              requirements.is_some() || documents.is_some();

    if content_needs_update {
        // Get current content hash to retrieve existing content
        let entity_key = format!("bounty_{}", bounty_id);
        let _current_hash = ENTITY_TO_HASH.load(deps.storage, &entity_key)?;
        let _current_content_hash = CONTENT_HASHES.load(deps.storage, &_current_hash)?;
        
        // For now, use the provided values or reasonable defaults
        // In a real implementation, you'd retrieve and merge with existing content
        let final_title = title.unwrap_or_else(|| "Updated Bounty".to_string());
        let final_description = description.unwrap_or_else(|| "Updated description".to_string());
        let final_requirements = requirements.unwrap_or_default();
        let final_documents = documents.unwrap_or_default();

        // Validate updated content
        validate_content_inputs!(&final_title, &final_description);
        
        // 🔥 Create new off-chain content bundle
        let (off_chain_bundle, new_content_hash_str) = create_bounty_content_bundle(
            bounty_id,
            &final_title,
            &final_description,
            &final_requirements,
            &final_documents,
            "Other", // Default category for now
            &vec![], // Default skills for now
            env.block.time.seconds(),
        )?;

        // 📄 Create new content hash metadata
        let new_content_hash = create_content_hash(
            &serde_json::to_string(&off_chain_bundle)
                .map_err(|e| ContractError::InvalidInput {
                    error: format!("Failed to serialize updated off-chain bundle: {}", e),
                })?,
            "bounty_content",
            env.block.time.seconds(),
        )?;

        // 🗄️ Update hash mappings
        CONTENT_HASHES.save(deps.storage, &new_content_hash_str, &new_content_hash)?;
        HASH_TO_ENTITY.save(deps.storage, &new_content_hash_str, &entity_key)?;
        ENTITY_TO_HASH.save(deps.storage, &entity_key, &new_content_hash_str)?;

        // Update bounty's content hash
        bounty.content_hash = new_content_hash;
    }

    // Update non-content fields
    if let Some(new_deadline_days) = submission_deadline_days {
        bounty.submission_deadline = get_future_timestamp(env.block.time, new_deadline_days);
    }

    if let Some(new_review_period) = review_period_days {
        bounty.review_period_days = new_review_period;
    }

    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // 🔍 Update search indexes if metadata changed
    if content_needs_update {
        update_bounty_search_indexes(deps.storage, &bounty)?;
    }

    Ok(build_success_response!(
        "edit_bounty",
        bounty_id,
        &info.sender
    ))
}

/// Cancel a bounty
pub fn execute_cancel_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CancelBounty);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;
    validate_bounty_status_for_operation(&bounty.status, &[BountyStatus::Open], "cancel")?;

    // Update bounty status
    bounty.status = BountyStatus::Cancelled;
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // Release escrow
    let escrow_id = format!("bounty_{}", bounty_id);
    if let Ok(mut escrow) = ESCROWS.load(deps.storage, &escrow_id) {
        escrow.released = true;

        ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    }

    let mut response = build_success_response!("cancel_bounty", bounty_id, &info.sender);

    // Add bank message to return funds
    response = response.add_message(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(bounty.total_reward.u128(), "uusdc"),
    });

    Ok(response)
}

/// Submit to a bounty
#[allow(clippy::too_many_arguments)]
pub fn execute_submit_to_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
    title: String,
    description: String,
    deliverables: Vec<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::SubmitToBounty);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_bounty_status_for_operation(&bounty.status, &[BountyStatus::Open], "submit to")?;

    if env.block.time >= bounty.submission_deadline {
        return Err(ContractError::InvalidInput {
            error: "Bounty has expired".to_string(),
        });
    }

    if bounty.total_submissions >= 100 {
        return Err(ContractError::InvalidInput {
            error: "Maximum submissions reached".to_string(),
        });
    }

    // Validate inputs
    validate_content_inputs!(&title, &description);
    
    if deliverables.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "At least one deliverable must be provided".to_string(),
        });
    }

    // Check if user already submitted
    let existing_submissions: Vec<_> = BOUNTY_SUBMISSIONS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            if let Ok((_, submission)) = item {
                if submission.bounty_id == bounty_id && submission.submitter == info.sender {
                    Some(submission)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    if !existing_submissions.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "You already have a submission for this bounty".to_string(),
        });
    }

    // Get next submission ID
    let submission_id = NEXT_BOUNTY_SUBMISSION_ID.load(deps.storage)?;
    NEXT_BOUNTY_SUBMISSION_ID.save(deps.storage, &(submission_id + 1))?;

    // 🔥 Create off-chain content bundle for submission
    let (off_chain_bundle, content_hash_str) = create_bounty_submission_content_bundle(
        submission_id,
        &title,
        &description,
        &deliverables,
        None, // No review notes initially
        env.block.time.seconds(),
    )?;

    // 📄 Create content hash metadata
    let content_hash = create_content_hash(
        &serde_json::to_string(&off_chain_bundle)
            .map_err(|e| ContractError::InvalidInput {
                error: format!("Failed to serialize submission off-chain bundle: {}", e),
            })?,
        "bounty_submission_content",
        env.block.time.seconds(),
    )?;

    // 🗄️ Store hash mappings for retrieval
    let entity_key = format!("bounty_submission_{}", submission_id);
    CONTENT_HASHES.save(deps.storage, &content_hash_str, &content_hash)?;
    HASH_TO_ENTITY.save(deps.storage, &content_hash_str, &entity_key)?;
    ENTITY_TO_HASH.save(deps.storage, &entity_key, &content_hash_str)?;

    // 📊 Calculate submission metadata
    let deliverable_count = deliverables.len() as u8;
    let submission_type = if deliverables.is_empty() {
        5 // Other
    } else {
        determine_submission_type(&deliverables[0])
    };

    // Create submission
    let submission = BountySubmission {
        id: submission_id,
        bounty_id,
        submitter: info.sender.clone(),
        submitted_at: env.block.time,
        status: BountySubmissionStatus::Submitted,
        score: None,
        winner_position: None,
        
        // 🌐 Off-chain content reference
        content_hash,
        
        // 📊 On-chain metadata for efficient searching
        deliverable_count,
        submission_type,
        estimated_completion_hours: 0, // Default, can be updated later
    };

    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    // Update bounty submission count
    bounty.total_submissions += 1; // Use total_submissions instead of submission_count
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    Ok(build_success_response!(
        "submit_to_bounty",
        submission_id,
        &info.sender,
        "bounty_id" => bounty_id.to_string()
    ))
}

/// Review a bounty submission
pub fn execute_review_bounty_submission(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_id: u64,
    status: BountySubmissionStatus,
    reviewer_notes: Option<String>,
    score: Option<u8>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::ReviewBountySubmission);

    // Load and validate submission
    let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
    let bounty = BOUNTIES.load(deps.storage, submission.bounty_id)?;

    validate_user_authorization(&bounty.poster, &info.sender)?;

    // Validate inputs
    validate_optional_string_field(&reviewer_notes, "Reviewer notes", 1000)?;

    if let Some(score_val) = score {
        if score_val > 100 {
            return Err(ContractError::InvalidInput {
                error: "Score must be between 0 and 100".to_string(),
            });
        }
    }

    // Update submission
    let status_str = format!("{:?}", status);
    submission.status = status;
    submission.score = score.map(|s| s as u8); // Convert u32 to u8

    // If reviewer notes are provided, we need to update the content hash
    if reviewer_notes.is_some() {
        // Get current content hash and update with review notes
        let entity_key = format!("bounty_submission_{}", submission_id);
        let _current_hash = ENTITY_TO_HASH.load(deps.storage, &entity_key)?;
        
        // For now, create a new content bundle with review notes
        // In a real implementation, you'd retrieve and merge existing content
        let (updated_bundle, new_hash_str) = create_bounty_submission_content_bundle(
            submission_id,
            "Reviewed Submission", // Default title
            "Updated with review notes", // Default description
            &vec!["submission_url".to_string()], // Default deliverables
            reviewer_notes.as_deref(),
            env.block.time.seconds(),
        )?;

        // Create new content hash
        let new_content_hash = create_content_hash(
            &serde_json::to_string(&updated_bundle)
                .map_err(|e| ContractError::InvalidInput {
                    error: format!("Failed to serialize review bundle: {}", e),
                })?,
            "bounty_submission_content",
            env.block.time.seconds(),
        )?;

        // Update hash mappings
        CONTENT_HASHES.save(deps.storage, &new_hash_str, &new_content_hash)?;
        HASH_TO_ENTITY.save(deps.storage, &new_hash_str, &entity_key)?;
        ENTITY_TO_HASH.save(deps.storage, &entity_key, &new_hash_str)?;

        // Update submission's content hash
        submission.content_hash = new_content_hash;
    }

    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    Ok(build_success_response!(
        "review_bounty_submission",
        submission_id,
        &info.sender,
        "status" => status_str
    ))
}

/// Select bounty winners
pub fn execute_select_bounty_winners(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
    winner_selections: Vec<WinnerSelection>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::SelectBountyWinners);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;
    validate_bounty_status_for_operation(
        &bounty.status,
        &[BountyStatus::Open],
        "select winners for",
    )?;

    // Validate inputs
    if winner_selections.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "Must select at least one winner".to_string(),
        });
    }

    if winner_selections.len() > bounty.max_winners as usize {
        return Err(ContractError::InvalidInput {
            error: "Too many winners selected".to_string(),
        });
    }

    // Validate all submissions exist and belong to this bounty
    let mut winner_addresses = Vec::new();
    let mut reward_distribution = Vec::new();
    let mut total_distributed = Uint128::zero();

    for selection in &winner_selections {
        let submission = BOUNTY_SUBMISSIONS.load(deps.storage, selection.submission_id)?;
        if submission.bounty_id != bounty_id {
            return Err(ContractError::InvalidInput {
                error: format!(
                    "Submission {} does not belong to this bounty",
                    selection.submission_id
                ),
            });
        }
        
        // Calculate reward based on position
        let position = selection.position;
        let reward = if position > 0 && position <= bounty.reward_distribution.len() as u64 {
            bounty.reward_distribution[(position - 1) as usize].amount
        } else {
            Uint128::zero()
        };
        
        winner_addresses.push(submission.submitter.clone());
        reward_distribution.push(reward);
        total_distributed += reward;
    }

    // Update bounty status
    bounty.status = BountyStatus::Completed;
    bounty.selected_winners = winner_selections.iter().map(|s| s.submission_id).collect();
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // Update winning submissions
    for selection in &winner_selections {
        let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, selection.submission_id)?;
        submission.status = BountySubmissionStatus::Winner;
        submission.winner_position = Some(selection.position);
        BOUNTY_SUBMISSIONS.save(deps.storage, selection.submission_id, &submission)?;
    }

    // Create response with bank messages for winners
    let mut response = Response::new()
        .add_attribute("method", "select_bounty_winners")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("winners_count", winner_selections.len().to_string())
        .add_attribute("total_distributed", total_distributed.to_string());

    for (i, &reward) in reward_distribution.iter().enumerate() {
        if reward > Uint128::zero() {
            response = response.add_message(BankMsg::Send {
                to_address: winner_addresses[i].to_string(),
                amount: coins(reward.u128(), "uusdc"),
            });
        }
    }

    Ok(response)
}

/// Edit a bounty submission
pub fn execute_edit_bounty_submission(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_id: u64,
    title: Option<String>,
    description: Option<String>,
    deliverables: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::EditBountySubmission);

    // Load and validate submission
    let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
    
    // Only the submitter can edit their submission
    if submission.submitter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Only allow editing if submission is not yet reviewed
    if submission.status != BountySubmissionStatus::Submitted {
        return Err(ContractError::InvalidInput {
            error: "Can only edit submissions that are still pending review".to_string(),
        });
    }

    // Create new content hash with updated fields
    let content_hash = create_bounty_submission_content_bundle(
        submission_id,
        &title.unwrap_or_else(|| format!("Submission {}", submission_id)),
        &description.unwrap_or_else(|| "Updated submission".to_string()),
        &deliverables.unwrap_or_default(),
        None,
        env.block.time.seconds(),
    )?;

    // Update submission
    let hash_str = content_hash.1.clone();
    submission.content_hash = ContentHash {
        hash: hash_str.clone(),
        data_type: "bounty_submission".to_string(),
        size_bytes: hash_str.len() as u64,
        timestamp: env.block.time.seconds(),
    };
    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    Ok(Response::new()
        .add_attribute("method", "edit_bounty_submission")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("content_hash", &hash_str)
        .add_attribute("editor", info.sender.to_string()))
}

/// Withdraw a bounty submission
pub fn execute_withdraw_bounty_submission(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::WithdrawBountySubmission);

    // Load and validate submission
    let mut submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
    
    // Only the submitter can withdraw their submission
    if submission.submitter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Can't withdraw if already reviewed or selected as winner
    if submission.status == BountySubmissionStatus::Winner {
        return Err(ContractError::InvalidInput {
            error: "Cannot withdraw a winning submission".to_string(),
        });
    }

    // Update submission status
    submission.status = BountySubmissionStatus::Withdrawn;
    BOUNTY_SUBMISSIONS.save(deps.storage, submission_id, &submission)?;

    // Remove from bounty submissions index
    let bounty_id = submission.bounty_id;
    let mut bounty_submissions = BOUNTY_SUBMISSIONS_BY_BOUNTY
        .load(deps.storage, bounty_id)
        .unwrap_or_default();
    bounty_submissions.retain(|&id| id != submission_id);
    BOUNTY_SUBMISSIONS_BY_BOUNTY.save(deps.storage, bounty_id, &bounty_submissions)?;

    Ok(Response::new()
        .add_attribute("method", "withdraw_bounty_submission")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("submitter", info.sender.to_string()))
}

/// Create an escrow for a bounty
pub fn execute_create_bounty_escrow(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CreateBountyEscrow);

    // Load and validate bounty
    let bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;

    // Create escrow ID
    let escrow_id = format!("bounty_{}", bounty_id);

    // Check if escrow already exists
    if ESCROWS.has(deps.storage, &escrow_id) {
        return Err(ContractError::InvalidInput {
            error: "Escrow already exists for this bounty".to_string(),
        });
    }

    // Create escrow
    let escrow = EscrowState {
        id: escrow_id.clone(),
        job_id: bounty_id, // Using bounty_id as job_id for compatibility
        client: bounty.poster.clone(),
        freelancer: bounty.poster.clone(), // Will be updated when winner is selected
        amount: bounty.total_reward,
        platform_fee: Uint128::zero(),
        funded_at: env.block.time,
        released: false,
        dispute_status: DisputeStatus::None,
        dispute_raised_at: None,
        dispute_deadline: None,
    };

    ESCROWS.save(deps.storage, &escrow_id, &escrow)?;

    Ok(Response::new()
        .add_attribute("method", "create_bounty_escrow")
        .add_attribute("bounty_id", bounty_id.to_string())
        .add_attribute("escrow_id", escrow_id)
        .add_attribute("amount", bounty.total_reward.to_string()))
}

/// Release rewards for a completed bounty
pub fn execute_release_bounty_rewards(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bounty_id: u64,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::ReleaseBountyRewards);

    // Load and validate bounty
    let bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;

    // Check if bounty is completed
    if bounty.status != BountyStatus::Completed {
        return Err(ContractError::InvalidInput {
            error: "Bounty must be completed to release rewards".to_string(),
        });
    }

    // Check if winners have been selected
    if bounty.selected_winners.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "No winners selected for this bounty".to_string(),
        });
    }

    // Release escrow
    let escrow_id = format!("bounty_{}", bounty_id);
    if let Ok(mut escrow) = ESCROWS.load(deps.storage, &escrow_id) {
        escrow.released = true;
        ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    }

    // Create bank messages for winners
    let mut response = Response::new()
        .add_attribute("method", "release_bounty_rewards")
        .add_attribute("bounty_id", bounty_id.to_string());

    // Distribute rewards to winners
    for (i, &submission_id) in bounty.selected_winners.iter().enumerate() {
        if let Ok(submission) = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id) {
            if let Some(reward_tier) = bounty.reward_distribution.get(i) {
                response = response.add_message(BankMsg::Send {
                    to_address: submission.submitter.to_string(),
                    amount: coins(reward_tier.amount.u128(), "uusdc"),
                });
            }
        }
    }

    Ok(response)
}

// Query functions

/// Query a specific bounty
pub fn query_bounty(deps: Deps, bounty_id: u64) -> StdResult<BountyResponse> {
    let bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    Ok(BountyResponse { bounty })
}

/// Query bounties with pagination and filtering
pub fn query_bounties(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    category: Option<String>,
    status: Option<BountyStatus>,
    creator: Option<String>,
) -> StdResult<BountiesResponse> {
    let limit = limit.unwrap_or(50).min(100) as usize;
    let mut bounties = Vec::new();

    let creator_addr = if let Some(c) = creator {
        Some(deps.api.addr_validate(&c)?)
    } else {
        None
    };

    let items: StdResult<Vec<_>> = BOUNTIES
        .range(deps.storage, None, None, Order::Descending)
        .collect();

    if let Ok(bounty_pairs) = items {
        for (id, bounty) in bounty_pairs {
            // Apply start_after filter
            if let Some(start_after_id) = start_after {
                if id >= start_after_id {
                    continue;
                }
            }

            // Apply filters
            let mut include = true;

            if let Some(ref filter_category) = category {
                let category_id = convert_category_to_id(filter_category);
                if bounty.category_id != category_id {
                    include = false;
                }
            }

            if let Some(ref filter_status) = status {
                if &bounty.status != filter_status {
                    include = false;
                }
            }

            if let Some(ref filter_creator) = creator_addr {
                if bounty.poster != *filter_creator {
                    include = false;
                }
            }

            if include {
                bounties.push(bounty);
                if bounties.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(BountiesResponse { bounties })
}

/// Query bounty submissions
pub fn query_bounty_submissions(
    deps: Deps,
    bounty_id: u64,
    status: Option<BountySubmissionStatus>,
) -> StdResult<BountySubmissionsResponse> {
    let submissions: Vec<_> = BOUNTY_SUBMISSIONS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, submission)) = item {
                if submission.bounty_id == bounty_id {
                    if let Some(ref filter_status) = status {
                        if &submission.status == filter_status {
                            Some(submission)
                        } else {
                            None
                        }
                    } else {
                        Some(submission)
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(BountySubmissionsResponse { submissions })
}

/// Query user's bounty submissions
pub fn query_user_bounty_submissions(
    deps: Deps,
    user: String,
    status: Option<BountySubmissionStatus>,
) -> StdResult<BountySubmissionsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;

    let submissions: Vec<_> = BOUNTY_SUBMISSIONS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, submission)) = item {
                if submission.submitter == user_addr {
                    if let Some(ref filter_status) = status {
                        if &submission.status == filter_status {
                            Some(submission)
                        } else {
                            None
                        }
                    } else {
                        Some(submission)
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(BountySubmissionsResponse { submissions })
}

/// Helper function to update bounty search indexes
fn update_bounty_search_indexes(storage: &mut dyn cosmwasm_std::Storage, bounty: &Bounty) -> Result<(), ContractError> {
    // Update category index
    let mut category_bounties = BOUNTIES_BY_CATEGORY
        .may_load(storage, bounty.category_id)?
        .unwrap_or_default();
    category_bounties.push(bounty.id);
    BOUNTIES_BY_CATEGORY.save(storage, bounty.category_id, &category_bounties)?;

    // Update reward range index
    let mut reward_range_bounties = BOUNTIES_BY_REWARD_RANGE
        .may_load(storage, bounty.reward_range)?
        .unwrap_or_default();
    reward_range_bounties.push(bounty.id);
    BOUNTIES_BY_REWARD_RANGE.save(storage, bounty.reward_range, &reward_range_bounties)?;

    // Update skill indexes
    for &skill_id in &bounty.skill_tags {
        let mut skill_bounties = BOUNTIES_BY_SKILL
            .may_load(storage, skill_id)?
            .unwrap_or_default();
        skill_bounties.push(bounty.id);
        BOUNTIES_BY_SKILL.save(storage, skill_id, &skill_bounties)?;
    }

    // Update difficulty index
    let mut difficulty_bounties = BOUNTIES_BY_DIFFICULTY
        .may_load(storage, bounty.difficulty_level)?
        .unwrap_or_default();
    difficulty_bounties.push(bounty.id);
    BOUNTIES_BY_DIFFICULTY.save(storage, bounty.difficulty_level, &difficulty_bounties)?;

    // Update active bounties index
    if bounty.status == BountyStatus::Open {
        ACTIVE_BOUNTIES.save(storage, bounty.id, &true)?;
    }

    // Update featured bounties index
    if bounty.is_featured {
        FEATURED_BOUNTIES.save(storage, bounty.id, &true)?;
    }

    Ok(())
}

/// Helper function to determine submission type from URL
fn determine_submission_type(url: &str) -> u8 {
    let url_lower = url.to_lowercase();
    
    if url_lower.contains("github.com") || url_lower.contains("gitlab.com") || url_lower.ends_with(".git") {
        2 // Code
    } else if url_lower.contains("figma.com") || url_lower.contains("dribbble.com") || url_lower.contains("behance.net") {
        3 // Design
    } else if url_lower.contains("youtube.com") || url_lower.contains("vimeo.com") || url_lower.ends_with(".mp4") || url_lower.ends_with(".mov") {
        4 // Video
    } else if url_lower.ends_with(".pdf") || url_lower.ends_with(".doc") || url_lower.ends_with(".docx") {
        1 // Document
    } else {
        5 // Other
    }
}
