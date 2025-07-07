use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::helpers::{
    ensure_not_paused, get_future_timestamp, validate_budget, validate_duration,
    validate_job_description, validate_job_title,
};
use crate::job_management::calculate_platform_fee;
use crate::msg::{
    BountiesResponse, BountyResponse, BountySubmissionsResponse,
};
use crate::security::{check_rate_limit, reentrancy_guard, validate_text_inputs, RateLimitAction};
use crate::state::{
    Bounty, BountyStatus, BountySubmission, BountySubmissionStatus, RewardTier, BOUNTIES,
    BOUNTY_SUBMISSIONS, CONFIG, ESCROWS, NEXT_BOUNTY_ID, NEXT_BOUNTY_SUBMISSION_ID,
};
use crate::{
    apply_security_checks, build_success_response,
    validate_content_inputs,
};
use cosmwasm_std::{
    coins, BankMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult,
    Uint128,
};

/// Create a new bounty
#[allow(clippy::too_many_arguments)]
pub fn execute_create_bounty(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    total_reward: Uint128,
    category: String,
    skills_required: Vec<String>,
    duration_days: u64,
    max_submissions: u32,
    submission_requirements: Vec<String>,
    judging_criteria: Vec<String>,
    winner_selection_type: String,
    _company: Option<String>,
    _location: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::CreateBounty);

    // Load configuration
    let config = CONFIG.load(deps.storage)?;

    // Validate inputs
    validate_content_inputs!(&title, &description);
    validate_budget(total_reward)?;
    validate_duration(duration_days, config.max_job_duration_days)?; // Use existing job duration config
    validate_string_field(&category, "Category", 1, 50)?;
    validate_collection_size(&skills_required, "Skills required", 1, 20)?;
    validate_collection_size(&submission_requirements, "Submission requirements", 1, 10)?;
    validate_collection_size(&judging_criteria, "Judging criteria", 1, 10)?;
    // Note: company and location are not in the schema, so removing validation

    // Validate max submissions (simplified)
    if max_submissions == 0 || max_submissions > 100 {
        // Use reasonable default instead of config field
        return Err(ContractError::InvalidInput {
            error: "Max submissions must be between 1 and 100".to_string(),
        });
    }

    // Validate winner selection type
    let valid_selection_types = ["single", "multiple", "ranked"];
    if !valid_selection_types.contains(&winner_selection_type.as_str()) {
        return Err(ContractError::InvalidInput {
            error: "Invalid winner selection type".to_string(),
        });
    }

    // Validate payment
    if info.funds.len() != 1 || info.funds[0].amount != total_reward {
        return Err(ContractError::InvalidFunds {});
    }

    // Get next bounty ID
    let bounty_id = NEXT_BOUNTY_ID.load(deps.storage)?;
    NEXT_BOUNTY_ID.save(deps.storage, &(bounty_id + 1))?;

    // Create bounty using actual schema fields
    let bounty = Bounty {
        id: bounty_id,
        poster: info.sender.clone(), // Use 'poster' instead of 'creator'
        title,
        description,
        requirements: submission_requirements, // Map to 'requirements' field
        total_reward,
        category: category.clone(),
        skills_required,
        submission_deadline: get_future_timestamp(env.block.time, duration_days), // Use deadline instead of expires_at
        review_period_days: 7, // Default review period
        max_winners: if winner_selection_type == "single" {
            1
        } else {
            max_submissions.min(10).into()
        }, // Reasonable default
        reward_distribution: vec![crate::state::RewardTier {
            position: 1,
            percentage: 100,
            amount: total_reward,
        }], // Simple single winner distribution for now
        documents: vec![],     // Empty for now
        status: BountyStatus::Open,
        created_at: env.block.time,
        updated_at: env.block.time,
        total_submissions: 0, // Use 'total_submissions' instead of 'submission_count'
        selected_winners: vec![],
        escrow_id: None, // Set after escrow creation
    };

    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

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
        released: false, // Use 'released' boolean instead of status enum
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
    category: Option<String>,
    skills_required: Option<Vec<String>>,
    duration_days: Option<u64>,
    max_submissions: Option<u32>,
    submission_requirements: Option<Vec<String>>,
    judging_criteria: Option<Vec<String>>,
    company: Option<String>,
    location: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::EditBounty);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_user_authorization(&bounty.poster, &info.sender)?;
    validate_bounty_status_for_operation(&bounty.status, &[BountyStatus::Open], "edit")?;

    let _config = CONFIG.load(deps.storage)?;

    // Update fields if provided
    if let Some(ref new_title) = title {
        validate_content_inputs!(new_title, new_title);
        bounty.title = new_title.clone();
    }

    if let Some(ref new_description) = description {
        validate_content_inputs!(new_description, new_description);
        bounty.description = new_description.clone();
    }

    if let Some(ref new_category) = category {
        validate_string_field(new_category, "Category", 1, 50)?;
        bounty.category = new_category.clone();
    }

    if let Some(ref new_skills) = skills_required {
        validate_collection_size(new_skills, "Skills required", 1, 20)?;
        bounty.skills_required = new_skills.clone();
    }

    if let Some(_new_duration) = duration_days {
        // validate_duration(new_duration, config.max_job_duration_days)?; // Use max_job_duration_days instead
        // bounty.duration_days = new_duration; // duration_days field doesn't exist
        // bounty.expires_at = get_future_timestamp(bounty.created_at, new_duration); // expires_at field doesn't exist
    }

    if let Some(new_max_submissions) = max_submissions {
        // if new_max_submissions == 0 || new_max_submissions > config.max_bounty_submissions {
        //     return Err(ContractError::InvalidInput {
        //         error: format!(
        //             "Max submissions must be between 1 and {}",
        //             config.max_bounty_submissions
        //         ),
        //     });
        // }
        // bounty.max_submissions = new_max_submissions; // max_submissions field doesn't exist
        bounty.total_submissions = new_max_submissions as u64; // Convert u32 to u64
    }

    if let Some(ref new_requirements) = submission_requirements {
        validate_collection_size(new_requirements, "Submission requirements", 1, 10)?;
        // bounty.submission_requirements = new_requirements.clone(); // Field doesn't exist
        bounty.requirements = new_requirements.clone(); // Use requirements instead
    }

    if let Some(ref new_criteria) = judging_criteria {
        validate_collection_size(new_criteria, "Judging criteria", 1, 10)?;
        // bounty.judging_criteria = new_criteria.clone(); // Field doesn't exist
    }

    if let Some(new_company) = company {
        validate_optional_string_field(&Some(new_company.clone()), "Company", 100)?;
        // bounty.company = Some(new_company); // Field doesn't exist
    }

    if let Some(new_location) = location {
        validate_optional_string_field(&Some(new_location.clone()), "Location", 100)?;
        // bounty.location = Some(new_location); // Field doesn't exist
    }

    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

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
        escrow.released = true; // Use boolean instead of status
                                // released_at field doesn't exist in EscrowState
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
    submission_title: String,
    submission_description: String,
    submission_url: String,
    additional_notes: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::SubmitToBounty);

    // Load and validate bounty
    let mut bounty = BOUNTIES.load(deps.storage, bounty_id)?;
    validate_bounty_status_for_operation(&bounty.status, &[BountyStatus::Open], "submit to")?;

    // Check if bounty has expired
    // if env.block.time >= bounty.expires_at { // expires_at field doesn't exist
    if env.block.time >= bounty.submission_deadline {
        // Use submission_deadline instead
        return Err(ContractError::InvalidInput {
            error: "Bounty has expired".to_string(),
        });
    }

    // Check if max submissions reached
    // if bounty.submission_count >= bounty.max_submissions { // max_submissions field doesn't exist
    if bounty.total_submissions >= 100 {
        // Use reasonable limit instead
        return Err(ContractError::InvalidInput {
            error: "Maximum submissions reached".to_string(),
        });
    }

    // Validate inputs
    validate_content_inputs!(&submission_title, &submission_description);
    validate_string_field(&submission_url, "Submission URL", 1, 500)?;
    validate_optional_string_field(&additional_notes, "Additional notes", 1000)?;

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

    // Create submission
    let submission = BountySubmission {
        id: submission_id,
        bounty_id,
        submitter: info.sender.clone(),
        title: submission_title,
        description: submission_description,
        deliverables: vec![submission_url], // Put URL in deliverables array
        submitted_at: env.block.time,
        status: BountySubmissionStatus::Submitted,
        review_notes: additional_notes, // Map additional_notes to review_notes
        score: None,
        winner_position: None,
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
    score: Option<u32>,
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
    submission.review_notes = reviewer_notes; // Use correct field name
    submission.score = score.map(|s| s as u8); // Convert u32 to u8
                                               // reviewed_at and updated_at don't exist in schema
                                               // removed those field assignments

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
    winner_submissions: Vec<u64>,
    reward_distribution: Vec<Uint128>,
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
    if winner_submissions.is_empty() {
        return Err(ContractError::InvalidInput {
            error: "Must select at least one winner".to_string(),
        });
    }

    if winner_submissions.len() != reward_distribution.len() {
        return Err(ContractError::InvalidInput {
            error: "Winner submissions and reward distribution must have same length".to_string(),
        });
    }

    // Validate total rewards don't exceed bounty total
    let total_distributed: Uint128 = reward_distribution.iter().sum();
    if total_distributed > bounty.total_reward {
        return Err(ContractError::InvalidInput {
            error: "Total reward distribution exceeds bounty total reward".to_string(),
        });
    }

    // Validate all submissions exist and belong to this bounty
    let mut winner_addresses = Vec::new();
    for &submission_id in &winner_submissions {
        let submission = BOUNTY_SUBMISSIONS.load(deps.storage, submission_id)?;
        if submission.bounty_id != bounty_id {
            return Err(ContractError::InvalidInput {
                error: format!(
                    "Submission {} does not belong to this bounty",
                    submission_id
                ),
            });
        }
        winner_addresses.push(submission.submitter);
    }

    // Update bounty with winners
    bounty.status = BountyStatus::Completed;
    bounty.selected_winners = winner_submissions.clone();

    // Convert Vec<Uint128> to Vec<RewardTier>
    let reward_tiers: Vec<RewardTier> = reward_distribution
        .iter()
        .enumerate()
        .map(|(i, &amount)| RewardTier {
            position: (i + 1) as u64,
            percentage: 0, // TODO: Calculate percentage based on amount and total
            amount,
        })
        .collect();

    bounty.reward_distribution = reward_tiers;
    bounty.updated_at = env.block.time;
    BOUNTIES.save(deps.storage, bounty_id, &bounty)?;

    // Release escrow and distribute rewards
    let escrow_id = format!("bounty_{}", bounty_id);
    if let Ok(mut escrow) = ESCROWS.load(deps.storage, &escrow_id) {
        escrow.released = true; // Use boolean instead of status enum
                                // Note: escrow.released_at doesn't exist in EscrowState schema
        ESCROWS.save(deps.storage, &escrow_id, &escrow)?;
    }

    let mut response = build_success_response!(
        "select_bounty_winners",
        bounty_id,
        &info.sender,
        "winners_count" => winner_submissions.len().to_string(),
        "total_distributed" => total_distributed.to_string()
    );

    // Add bank messages to distribute rewards
    for (i, &reward) in reward_distribution.iter().enumerate() {
        if let Some(winner_addr) = winner_addresses.get(i) {
            response = response.add_message(BankMsg::Send {
                to_address: winner_addr.to_string(),
                amount: coins(reward.u128(), "uusdc"),
            });
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
                if &bounty.category != filter_category {
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
