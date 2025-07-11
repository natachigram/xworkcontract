use cosmwasm_std::{
    Addr, Deps, Order, StdResult, Storage, Timestamp, Uint128, Decimal
};
use cw_storage_plus::Bound;

use crate::state::{
    Job, JobStatus, Proposal, 
    JOBS, PROPOSALS, RATINGS, USER_STATS
};
use crate::error::ContractError;

// Validation helpers
pub fn validate_job_title(title: &str) -> Result<(), ContractError> {
    if title.is_empty() || title.len() > 100 {
        return Err(ContractError::InvalidInput {
            error: "Title must be between 1-100 characters".to_string(),
        });
    }
    Ok(())
}

pub fn validate_job_description(description: &str) -> Result<(), ContractError> {
    if description.is_empty() || description.len() > 5000 {
        return Err(ContractError::InvalidInput {
            error: "Description must be between 1-5000 characters".to_string(),
        });
    }
    Ok(())
}

pub fn validate_budget(budget: Uint128) -> Result<(), ContractError> {
    // Allow budget = 0 for free projects
    if budget.is_zero() {
        return Ok(());
    }
    
    // For paid projects, enforce minimum escrow amount
    let min_escrow = Uint128::new(1_000); // 0.001 XION minimum
    if budget < min_escrow {
        return Err(ContractError::EscrowAmountTooLow {
            min: min_escrow.to_string(),
        });
    }
    
    Ok(())
}

pub fn validate_duration(duration_days: u64, max_duration: u64) -> Result<(), ContractError> {
    if duration_days == 0 || duration_days > max_duration {
        return Err(ContractError::InvalidInput {
            error: format!("Duration must be between 1-{} days", max_duration),
        });
    }
    Ok(())
}

pub fn validate_cover_letter(cover_letter: &str) -> Result<(), ContractError> {
    if cover_letter.is_empty() || cover_letter.len() > 2000 {
        return Err(ContractError::InvalidInput {
            error: "Cover letter must be between 1-2000 characters".to_string(),
        });
    }
    Ok(())
}

pub fn validate_rating(rating: u8) -> Result<(), ContractError> {
    if rating == 0 || rating > 5 {
        return Err(ContractError::InvalidInput {
            error: "Rating must be between 1-5".to_string(),
        });
    }
    Ok(())
}

pub fn validate_deadline(deadline: Timestamp, current_time: Timestamp) -> Result<(), ContractError> {
    if deadline <= current_time {
        return Err(ContractError::InvalidDeadline {});
    }
    Ok(())
}

// Query helpers
pub fn query_jobs_paginated(
    storage: &dyn Storage,
    start_after: Option<u64>,
    limit: Option<u32>,
    category: Option<String>,
    status: Option<JobStatus>,
    poster: Option<Addr>,
) -> StdResult<Vec<Job>> {
    let limit = limit.unwrap_or(10).min(50) as usize;
    let start = start_after.map(Bound::exclusive);
    
    let jobs: Result<Vec<_>, _> = JOBS
        .range(storage, start, None, Order::Ascending)
        .take(limit)
        .filter_map(|item| {
            match item {
                Ok((_, job)) => {
                    let category_match = category.as_ref().map_or(true, |c| &job.category == c);
                    let status_match = status.as_ref().map_or(true, |s| &job.status == s);
                    let poster_match = poster.as_ref().map_or(true, |p| job.poster == *p);
                    
                    if category_match && status_match && poster_match {
                        Some(Ok(job))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            }
        })
        .collect();
    
    jobs
}

pub fn query_user_proposals(
    storage: &dyn Storage,
    user: &Addr,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<Proposal>> {
    let limit = limit.unwrap_or(10).min(50) as usize;
    let start = start_after.map(Bound::exclusive);
    
    let proposals: Result<Vec<_>, _> = PROPOSALS
        .range(storage, start, None, Order::Ascending)
        .filter_map(|item| {
            match item {
                Ok((_, proposal)) => {
                    if proposal.freelancer == *user {
                        Some(Ok(proposal))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            }
        })
        .take(limit)
        .collect();
    
    proposals
}

pub fn calculate_user_average_rating(
    storage: &dyn Storage,
    user: &Addr,
) -> StdResult<(Decimal, u64)> {
    let ratings: Result<Vec<_>, _> = RATINGS
        .range(storage, None, None, Order::Ascending)
        .filter_map(|item| {
            match item {
                Ok((_, rating)) => {
                    if rating.rated == *user {
                        Some(Ok(rating))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            }
        })
        .collect();
    
    let ratings = ratings?;
    let total_ratings = ratings.len() as u64;
    
    if total_ratings == 0 {
        return Ok((Decimal::zero(), 0));
    }
    
    let sum: u64 = ratings.iter().map(|r| r.rating as u64).sum();
    let average = Decimal::from_ratio(sum, total_ratings);
    
    Ok((average, total_ratings))
}

pub fn update_user_rating_stats(
    storage: &mut dyn Storage,
    user: &Addr,
) -> StdResult<()> {
    let (average_rating, total_ratings) = calculate_user_average_rating(storage, user)?;
    
    let mut stats = USER_STATS.may_load(storage, user)?.unwrap_or_default();
    stats.average_rating = average_rating;
    stats.total_ratings = total_ratings;
    
    USER_STATS.save(storage, user, &stats)?;
    
    Ok(())
}

// Security helpers
pub fn ensure_not_paused(deps: Deps) -> Result<(), ContractError> {
    let config = crate::state::CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    Ok(())
}

pub fn ensure_admin(deps: Deps, sender: &Addr) -> Result<(), ContractError> {
    let config = crate::state::CONFIG.load(deps.storage)?;
    if *sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

// Math helpers with overflow protection  
pub fn safe_multiply_percentage(amount: Uint128, percentage: u64) -> Result<Uint128, ContractError> {
    if percentage > 100 {
        return Err(ContractError::InvalidInput {
            error: "Percentage cannot exceed 100".to_string(),
        });
    }
    
    // Use try_into for the calculation to avoid type issues
    match amount.u128().checked_mul(percentage as u128) {
        Some(multiplied) => {
            match multiplied.checked_div(100) {
                Some(result) => Ok(Uint128::new(result)),
                None => Err(ContractError::InvalidInput {
                    error: "Division error in percentage calculation".to_string(),
                })
            }
        },
        None => Err(ContractError::InvalidInput {
            error: "Arithmetic overflow in percentage calculation".to_string(),
        })
    }
}

// Time helpers
pub fn get_future_timestamp(current: Timestamp, days: u64) -> Timestamp {
    current.plus_seconds(days * 24 * 60 * 60)
}

pub fn is_expired(deadline: Timestamp, current: Timestamp) -> bool {
    current > deadline
}
