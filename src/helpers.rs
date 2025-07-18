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
                    // Map category string to ID for comparison  
                    let category_match = category.as_ref().map_or(true, |c| {
                        let category_id = match c.to_lowercase().as_str() {
                            "web development" => 1,
                            "mobile development" => 2,
                            "design" => 3,
                            "writing" => 4,
                            "marketing" => 5,
                            _ => 99, // Other
                        };
                        job.category_id == category_id
                    });
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

// Category and skill conversion helpers for bounty optimization
pub fn convert_category_to_id(category: &str) -> u8 {
    match category.to_lowercase().as_str() {
        "web development" | "web" => 1,
        "mobile development" | "mobile" => 2,
        "design" | "ui/ux" => 3,
        "writing" | "content writing" => 4,
        "marketing" | "digital marketing" => 5,
        "blockchain" | "smart contracts" => 6,
        "data science" | "machine learning" => 7,
        "devops" | "infrastructure" => 8,
        "testing" | "qa" => 9,
        "video editing" | "video production" => 10,
        _ => 99, // Other
    }
}

pub fn convert_skill_to_id(skill: &str) -> u8 {
    match skill.to_lowercase().as_str() {
        "rust" => 1,
        "javascript" | "js" => 2,
        "typescript" | "ts" => 3,
        "python" => 4,
        "react" => 5,
        "vue" => 6,
        "angular" => 7,
        "nodejs" | "node" => 8,
        "solidity" => 9,
        "cosmwasm" => 10,
        "html" => 11,
        "css" => 12,
        "figma" => 13,
        "photoshop" => 14,
        "illustrator" => 15,
        "after effects" => 16,
        "premiere pro" => 17,
        "copywriting" => 18,
        "seo" => 19,
        "social media" => 20,
        "aws" => 21,
        "docker" => 22,
        "kubernetes" => 23,
        "postgresql" => 24,
        "mongodb" => 25,
        _ => 99, // Other
    }
}

pub fn convert_skills_to_ids(skills: &[String]) -> Vec<u8> {
    skills.iter()
        .map(|skill| convert_skill_to_id(skill))
        .collect()
}

pub fn calculate_reward_range(reward: Uint128) -> u8 {
    let amount = reward.u128();
    if amount < 100_000_000 { // < $100 (assuming 6 decimal places)
        1 // Low
    } else if amount < 1_000_000_000 { // < $1000
        2 // Medium
    } else {
        3 // High
    }
}

pub fn calculate_difficulty_from_skills(skills: &[String]) -> u8 {
    let advanced_skills = ["rust", "solidity", "cosmwasm", "machine learning", "blockchain", "kubernetes"];
    let intermediate_skills = ["typescript", "react", "vue", "angular", "nodejs", "python"];
    
    let has_advanced = skills.iter().any(|skill| 
        advanced_skills.contains(&skill.to_lowercase().as_str())
    );
    let has_intermediate = skills.iter().any(|skill| 
        intermediate_skills.contains(&skill.to_lowercase().as_str())
    );
    
    if has_advanced {
        3 // Expert
    } else if has_intermediate {
        2 // Intermediate
    } else {
        1 // Entry
    }
}

pub fn estimate_hours_from_reward_and_difficulty(reward: Uint128, difficulty: u8) -> u16 {
    let amount = reward.u128();
    let hourly_rate = match difficulty {
        1 => 15_000_000,  // $15/hour for entry level
        2 => 30_000_000,  // $30/hour for intermediate
        3 => 60_000_000,  // $60/hour for expert
        _ => 25_000_000,  // $25/hour default
    };
    
    ((amount / hourly_rate) as u16).max(1).min(500) // Cap at 500 hours
}
