use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::helpers::{ensure_not_paused};
use crate::msg::{UserStatsResponse, RatingsResponse, UserProfileResponse};
use crate::security::{RateLimitAction, check_rate_limit, reentrancy_guard};
use crate::state::{
    UserProfile, UserStats, Rating, USER_PROFILES, USER_STATS, RATINGS, JOBS,
};
use crate::{apply_security_checks, build_success_response};
use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, StdResult, 
    Uint128, Order, Deps, Addr, Decimal,
};

/// Update user profile
#[allow(clippy::too_many_arguments)]
pub fn execute_update_user_profile(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    name: Option<String>,
    bio: Option<String>,
    skills: Option<Vec<String>>,
    _location: Option<String>,
    _website: Option<String>,
    portfolio_url: Option<String>,
    _hourly_rate: Option<Uint128>,
    _availability: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::UpdateProfile);

    // Load or create user profile
    let mut profile = USER_PROFILES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_else(|| UserProfile {
            display_name: None,
            bio: None,
            skills: vec![],
            portfolio_links: vec![],
            created_at: Some(env.block.time),
            updated_at: Some(env.block.time),
        });

    // Update fields if provided
    if let Some(new_display_name) = name {
        validate_string_field(&new_display_name, "Display name", 1, 100)?;
        profile.display_name = Some(new_display_name);
    }

    if let Some(new_bio) = bio {
        validate_string_field(&new_bio, "Bio", 1, 1000)?;
        profile.bio = Some(new_bio);
    }

    if let Some(new_skills) = skills {
        validate_collection_size(&new_skills, "Skills", 0, 50)?;
        profile.skills = new_skills;
    }

    // Update portfolio links if provided  
    if let Some(new_portfolio_url) = portfolio_url {
        profile.portfolio_links = vec![new_portfolio_url];
    }

    profile.updated_at = Some(env.block.time);
    USER_PROFILES.save(deps.storage, &info.sender, &profile)?;

    Ok(build_success_response!("update_user_profile", 0u64, &info.sender))
}

/// Submit a rating for a user
pub fn execute_submit_rating(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    job_id: u64,
    rated_user: String,
    rating: u32,
    comment: Option<String>,
) -> Result<Response, ContractError> {
    // Apply security checks
    apply_security_checks!(deps, env, info, RateLimitAction::SubmitRating);

    let rated_user_addr = deps.api.addr_validate(&rated_user)?;

    // Load and validate job
    let job = JOBS.load(deps.storage, job_id)?;

    // Validate that the rater is involved in the job
    let can_rate = job.poster == info.sender || 
                   job.assigned_freelancer.as_ref() == Some(&info.sender);

    if !can_rate {
        return Err(ContractError::Unauthorized {});
    }

    // Validate that the rated user is involved in the job
    let can_be_rated = job.poster == rated_user_addr || 
                       job.assigned_freelancer.as_ref() == Some(&rated_user_addr);

    if !can_be_rated {
        return Err(ContractError::InvalidInput {
            error: "Rated user is not involved in this job".to_string(),
        });
    }

    // Cannot rate yourself
    if info.sender == rated_user_addr {
        return Err(ContractError::InvalidInput {
            error: "Cannot rate yourself".to_string(),
        });
    }

    // Validate rating value
    if rating == 0 || rating > 5 {
        return Err(ContractError::InvalidInput {
            error: "Rating must be between 1 and 5".to_string(),
        });
    }

    // Validate comment
    validate_optional_string_field(&comment, "Comment", 1000)?;

    // Check if rating already exists
    let rating_key = format!("{}_{}", job_id, info.sender);
    if RATINGS.may_load(deps.storage, &rating_key)?.is_some() {
        return Err(ContractError::InvalidInput {
            error: "You have already rated this job".to_string(),
        });
    }

    // Create rating
    let new_rating = Rating {
        id: rating_key.clone(),
        job_id,
        rater: info.sender.clone(),
        rated: rated_user_addr.clone(),
        rating: rating.try_into().map_err(|_| ContractError::InvalidInput {
            error: "Rating must be between 1 and 5".to_string(),
        })?,
        comment: comment.unwrap_or_default(),
        created_at: env.block.time,
    };

    RATINGS.save(deps.storage, &rating_key, &new_rating)?;

    // Update user stats
    let mut stats = USER_STATS
        .may_load(deps.storage, &rated_user_addr)?
        .unwrap_or_else(|| UserStats {
            total_jobs_posted: 0,
            total_jobs_completed: 0,
            total_earned: Uint128::zero(),
            total_spent: Uint128::zero(),
            average_rating: Decimal::zero(),
            total_ratings: 0,
            completion_rate: Decimal::zero(),
            display_name: None,
        });

    // Recalculate average rating
    let new_total_ratings = stats.total_ratings + 1;
    let current_sum = stats.average_rating * Decimal::from_atomics(stats.total_ratings, 0).unwrap();
    let new_rating_decimal = Decimal::from_atomics(rating, 0).unwrap();
    let new_average = (current_sum + new_rating_decimal) / Decimal::from_atomics(new_total_ratings, 0).unwrap();

    stats.average_rating = new_average;
    stats.total_ratings = new_total_ratings;
    USER_STATS.save(deps.storage, &rated_user_addr, &stats)?;

    Ok(build_success_response!(
        "submit_rating",
        job_id,
        &info.sender,
        "rated_user" => rated_user_addr.to_string(),
        "rating" => rating.to_string()
    ))
}

// Query functions

/// Query user profile
pub fn query_user_profile(deps: Deps, user: String) -> StdResult<UserProfileResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let profile = USER_PROFILES.may_load(deps.storage, &user_addr)?;
    Ok(UserProfileResponse { 
        profile: profile.unwrap_or_default() 
    })
}

/// Query user statistics
pub fn query_user_stats(deps: Deps, user: String) -> StdResult<UserStatsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let stats = USER_STATS.may_load(deps.storage, &user_addr)?;
    Ok(UserStatsResponse { 
        stats: stats.unwrap_or_default() 
    })
}

/// Query user ratings
pub fn query_user_ratings(deps: Deps, user: String) -> StdResult<RatingsResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    
    let ratings: Vec<_> = RATINGS
        .range(deps.storage, None, None, Order::Descending)
        .filter_map(|item| {
            if let Ok((_, rating)) = item {
                if rating.rated == user_addr {
                    Some(rating)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(RatingsResponse { ratings })
}

/// Query job rating
pub fn query_job_rating(deps: Deps, job_id: u64, rater: String) -> StdResult<Rating> {
    let rater_addr = deps.api.addr_validate(&rater)?;
    let rating_key = format!("{}_{}", job_id, rater_addr);
    let rating = RATINGS.load(deps.storage, &rating_key)?;
    Ok(rating)
}

/// Calculate and update user statistics
pub fn update_user_job_stats(
    deps: DepsMut,
    user: &Addr,
    job_completed: bool,
    amount_earned: Option<Uint128>,
    amount_spent: Option<Uint128>,
) -> Result<(), ContractError> {
    let mut stats = USER_STATS
        .may_load(deps.storage, user)?
        .unwrap_or_else(|| UserStats {
            total_jobs_posted: 0,
            total_jobs_completed: 0,
            total_earned: Uint128::zero(),
            total_spent: Uint128::zero(),
            average_rating: Decimal::zero(),
            total_ratings: 0,
            completion_rate: Decimal::zero(),
            display_name: None,
        });

    if job_completed {
        stats.total_jobs_completed += 1;
    }

    if let Some(earned) = amount_earned {
        stats.total_earned += earned;
    }

    if let Some(spent) = amount_spent {
        stats.total_spent += spent;
    }

    USER_STATS.save(deps.storage, user, &stats)?;
    Ok(())
}

/// Calculate and update user bounty statistics
pub fn update_user_bounty_stats(
    deps: DepsMut,
    user: &Addr,
    bounty_created: bool,
    bounty_won: bool,
    amount_earned: Option<Uint128>,
    amount_spent: Option<Uint128>,
) -> Result<(), ContractError> {
    let mut stats = USER_STATS
        .may_load(deps.storage, user)?
        .unwrap_or_else(|| UserStats {
            total_jobs_posted: 0,
            total_jobs_completed: 0,
            total_earned: Uint128::zero(),
            total_spent: Uint128::zero(),
            average_rating: Decimal::zero(),
            total_ratings: 0,
            completion_rate: Decimal::zero(),
            display_name: None,
        });

    // Note: bounty-specific stats not available in current UserStats schema
    // These would need to be tracked separately if needed
    if bounty_created {
        // Could increment total_jobs_posted as a generic "items posted" counter
        stats.total_jobs_posted += 1;
    }

    if bounty_won {
        // Could increment total_jobs_completed as a generic "items completed" counter  
        stats.total_jobs_completed += 1;
    }

    if let Some(earned) = amount_earned {
        stats.total_earned += earned;
    }

    if let Some(spent) = amount_spent {
        stats.total_spent += spent;
    }

    USER_STATS.save(deps.storage, user, &stats)?;
    Ok(())
}

/// Update user profile reputation
pub fn update_user_reputation(
    deps: DepsMut,
    user: &Addr,
    _reputation_change: i32,
) -> Result<(), ContractError> {
    // Note: reputation_score field not available in current UserProfile schema
    // This would need to be handled via UserStats.average_rating or a separate structure
    if USER_PROFILES.may_load(deps.storage, user)?.is_some() {
        // For now, just ensure the user profile exists
        // Reputation could be tracked via average_rating in UserStats
    }

    Ok(())
}
