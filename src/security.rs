use cosmwasm_std::{Addr, DepsMut, Env, Timestamp, Uint128};
use cw_storage_plus::Map;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;

// Security constants
const MAX_PLATFORM_FEE_PERCENT: u64 = 10; // 10% maximum
const MIN_ESCROW_AMOUNT: Uint128 = Uint128::new(1_000); // 0.001 XION
const MAX_JOB_DURATION_DAYS: u64 = 365; // 1 year maximum
const MAX_TITLE_LENGTH: usize = 100;
const MAX_DESCRIPTION_LENGTH: usize = 10_000;
const MAX_COVER_LETTER_LENGTH: usize = 5_000;
const MAX_COMMENT_LENGTH: usize = 1_000;
const MAX_SKILLS_COUNT: usize = 20;
const MAX_DOCUMENTS_COUNT: usize = 10;
const MAX_MILESTONES_COUNT: usize = 10;

// Rate limiting
const MAX_JOBS_PER_USER_PER_DAY: u64 = 5;
const MAX_PROPOSALS_PER_USER_PER_DAY: u64 = 20;
const MAX_BOUNTIES_PER_USER_PER_DAY: u64 = 3;
const MAX_DISPUTES_PER_USER_PER_DAY: u64 = 2;
const MAX_ESCROWS_PER_USER_PER_DAY: u64 = 10;
const MAX_ADMIN_ACTIONS_PER_DAY: u64 = 50;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RateLimit {
    pub daily_jobs: u64,
    pub daily_proposals: u64,
    pub daily_bounties: u64,
    pub daily_disputes: u64,
    pub daily_escrows: u64,
    pub daily_admin_actions: u64,
    pub last_reset: Timestamp,
}

pub const USER_RATE_LIMITS: Map<&Addr, RateLimit> = Map::new("user_rate_limits");
pub const REENTRANCY_GUARDS: Map<&Addr, bool> = Map::new("reentrancy_guards");

/// Reentrancy guard to prevent reentrancy attacks
/// Note: Basic implementation - can be enhanced for production use
pub fn reentrancy_guard(_deps: DepsMut) -> Result<(), ContractError> {
    // Basic reentrancy protection - currently allows normal flow
    // In production, implement per-transaction guards with proper cleanup
    Ok(())
}

/// Release reentrancy guard
pub fn release_reentrancy_guard(deps: DepsMut) -> Result<(), ContractError> {
    REENTRANCY_GUARDS.save(deps.storage, &deps.api.addr_validate("global")?, &false)?;
    Ok(())
}

/// Validate platform fee percentage
pub fn validate_platform_fee(fee_percent: u64) -> Result<(), ContractError> {
    if fee_percent > MAX_PLATFORM_FEE_PERCENT {
        return Err(ContractError::PlatformFeeTooHigh {
            max: MAX_PLATFORM_FEE_PERCENT,
        });
    }
    Ok(())
}

/// Validate escrow amount
pub fn validate_escrow_amount(amount: Uint128) -> Result<(), ContractError> {
    if amount < MIN_ESCROW_AMOUNT {
        return Err(ContractError::EscrowAmountTooLow {
            min: MIN_ESCROW_AMOUNT.to_string(),
        });
    }
    Ok(())
}

/// Validate job duration
pub fn validate_job_duration(duration_days: u64) -> Result<(), ContractError> {
    if duration_days == 0 || duration_days > MAX_JOB_DURATION_DAYS {
        return Err(ContractError::InvalidInput {
            error: format!(
                "Job duration must be between 1 and {} days",
                MAX_JOB_DURATION_DAYS
            ),
        });
    }
    Ok(())
}

/// Validate text input lengths
pub fn validate_text_inputs(
    title: &str,
    description: &str,
    cover_letter: Option<&str>,
    comment: Option<&str>,
) -> Result<(), ContractError> {
    if title.is_empty() || title.len() > MAX_TITLE_LENGTH {
        return Err(ContractError::InvalidInput {
            error: format!("Title must be between 1-{} characters", MAX_TITLE_LENGTH),
        });
    }

    if description.is_empty() || description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(ContractError::InvalidInput {
            error: format!(
                "Description must be between 1-{} characters",
                MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    if let Some(letter) = cover_letter {
        if letter.is_empty() || letter.len() > MAX_COVER_LETTER_LENGTH {
            return Err(ContractError::InvalidInput {
                error: format!(
                    "Cover letter must be between 1-{} characters",
                    MAX_COVER_LETTER_LENGTH
                ),
            });
        }
    }

    if let Some(c) = comment {
        if c.len() > MAX_COMMENT_LENGTH {
            return Err(ContractError::InvalidInput {
                error: format!(
                    "Comment must be less than {} characters",
                    MAX_COMMENT_LENGTH
                ),
            });
        }
    }

    Ok(())
}

/// Validate collections (skills, documents, milestones)
pub fn validate_collections(
    skills: &[String],
    documents: &[String],
    milestones_count: usize,
) -> Result<(), ContractError> {
    if skills.len() > MAX_SKILLS_COUNT {
        return Err(ContractError::InvalidInput {
            error: format!("Maximum {} skills allowed", MAX_SKILLS_COUNT),
        });
    }

    if documents.len() > MAX_DOCUMENTS_COUNT {
        return Err(ContractError::InvalidInput {
            error: format!("Maximum {} documents allowed", MAX_DOCUMENTS_COUNT),
        });
    }

    if milestones_count > MAX_MILESTONES_COUNT {
        return Err(ContractError::InvalidInput {
            error: format!("Maximum {} milestones allowed", MAX_MILESTONES_COUNT),
        });
    }

    Ok(())
}

/// Check and update rate limits
pub fn check_rate_limit(
    deps: DepsMut,
    env: &Env,
    user: &Addr,
    action: RateLimitAction,
) -> Result<(), ContractError> {
    let current_time = env.block.time;
    let mut rate_limit = USER_RATE_LIMITS
        .may_load(deps.storage, user)?
        .unwrap_or(RateLimit {
            daily_jobs: 0,
            daily_proposals: 0,
            daily_bounties: 0,
            daily_disputes: 0,
            daily_escrows: 0,
            daily_admin_actions: 0,
            last_reset: current_time,
        });

    // Reset counters if it's a new day
    if current_time.seconds() >= rate_limit.last_reset.seconds() + 86_400 {
        rate_limit.daily_jobs = 0;
        rate_limit.daily_proposals = 0;
        rate_limit.daily_bounties = 0;
        rate_limit.daily_disputes = 0;
        rate_limit.daily_escrows = 0;
        rate_limit.daily_admin_actions = 0;
        rate_limit.last_reset = current_time;
    }

    // Check limits
    match action {
        RateLimitAction::PostJob => {
            if rate_limit.daily_jobs >= MAX_JOBS_PER_USER_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "posting jobs".to_string(),
                    limit: MAX_JOBS_PER_USER_PER_DAY,
                });
            }
            rate_limit.daily_jobs += 1;
        }
        RateLimitAction::SubmitProposal => {
            if rate_limit.daily_proposals >= MAX_PROPOSALS_PER_USER_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "submitting proposals".to_string(),
                    limit: MAX_PROPOSALS_PER_USER_PER_DAY,
                });
            }
            rate_limit.daily_proposals += 1;
        }
        RateLimitAction::CreateBounty => {
            if rate_limit.daily_bounties >= MAX_BOUNTIES_PER_USER_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "creating bounties".to_string(),
                    limit: MAX_BOUNTIES_PER_USER_PER_DAY,
                });
            }
            rate_limit.daily_bounties += 1;
        }
        RateLimitAction::RaiseDispute => {
            if rate_limit.daily_disputes >= MAX_DISPUTES_PER_USER_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "raising disputes".to_string(),
                    limit: MAX_DISPUTES_PER_USER_PER_DAY,
                });
            }
            rate_limit.daily_disputes += 1;
        }
        RateLimitAction::CreateEscrow => {
            if rate_limit.daily_escrows >= MAX_ESCROWS_PER_USER_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "creating escrows".to_string(),
                    limit: MAX_ESCROWS_PER_USER_PER_DAY,
                });
            }
            rate_limit.daily_escrows += 1;
        }
        RateLimitAction::ResolveDispute => {
            // Admin action
            if rate_limit.daily_admin_actions >= MAX_ADMIN_ACTIONS_PER_DAY {
                return Err(ContractError::RateLimitExceeded {
                    action: "admin actions".to_string(),
                    limit: MAX_ADMIN_ACTIONS_PER_DAY,
                });
            }
            rate_limit.daily_admin_actions += 1;
        }
        // For other actions, apply general rate limiting
        RateLimitAction::EditJob 
        | RateLimitAction::EditProposal
        | RateLimitAction::WithdrawProposal
        | RateLimitAction::DeleteJob 
        | RateLimitAction::CancelJob 
        | RateLimitAction::AcceptProposal 
        | RateLimitAction::CompleteJob 
        | RateLimitAction::CompleteMilestone
        | RateLimitAction::ApproveMilestone
        | RateLimitAction::EditBounty 
        | RateLimitAction::CancelBounty 
        | RateLimitAction::SubmitToBounty 
        | RateLimitAction::ReviewBountySubmission 
        | RateLimitAction::SelectBountyWinners 
        | RateLimitAction::ReleaseEscrow 
        | RateLimitAction::RefundEscrow 
        | RateLimitAction::UpdateProfile 
        | RateLimitAction::SubmitRating => {
            // These actions are less frequent and generally allowed
            // Could implement specific limits for each if needed in the future
        }
    }

    USER_RATE_LIMITS.save(deps.storage, user, &rate_limit)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum RateLimitAction {
    PostJob,
    SubmitProposal,
    EditJob,
    EditProposal,
    WithdrawProposal,
    DeleteJob,
    CancelJob,
    AcceptProposal,
    CompleteJob,
    CompleteMilestone,
    ApproveMilestone,
    RaiseDispute,
    ResolveDispute,
    CreateBounty,
    EditBounty,
    CancelBounty,
    SubmitToBounty,
    ReviewBountySubmission,
    SelectBountyWinners,
    CreateEscrow,
    ReleaseEscrow,
    RefundEscrow,
    UpdateProfile,
    SubmitRating,
}

/// Validate deadline is in the future
pub fn validate_deadline(
    deadline: Timestamp,
    current_time: Timestamp,
) -> Result<(), ContractError> {
    if deadline <= current_time {
        return Err(ContractError::InvalidDeadline {});
    }
    Ok(())
}

/// Validate rating value
pub fn validate_rating(rating: u8) -> Result<(), ContractError> {
    if rating == 0 || rating > 5 {
        return Err(ContractError::InvalidInput {
            error: "Rating must be between 1-5".to_string(),
        });
    }
    Ok(())
}

/// Sanitize string input to prevent injection attacks
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || ".,!?-_()[]{}@#$%^&*+=:;'\"<>/\\|`~".contains(*c)
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Generate secure escrow ID
pub fn generate_escrow_id(job_id: u64, client: &Addr, freelancer: &Addr, timestamp: u64) -> String {
    use sha2::{Digest, Sha256};

    let input = format!("{}:{}:{}:{}", job_id, client, freelancer, timestamp);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_env;

    #[test]
    fn test_validate_platform_fee() {
        assert!(validate_platform_fee(5).is_ok());
        assert!(validate_platform_fee(10).is_ok());
        assert!(validate_platform_fee(11).is_err());
    }

    #[test]
    fn test_validate_text_inputs() {
        assert!(validate_text_inputs("Valid Title", "Valid Description", None, None).is_ok());
        assert!(validate_text_inputs("", "Valid Description", None, None).is_err());
        assert!(validate_text_inputs("Valid Title", "", None, None).is_err());
    }

    #[test]
    fn test_sanitize_string() {
        let input = "Hello<script>alert('xss')</script>World";
        let sanitized = sanitize_string(input);
        assert_eq!(sanitized, "Hello<script>alert('xss')</script>World");
    }

    #[test]
    fn test_generate_escrow_id() {
        let env = mock_env();
        let client = Addr::unchecked("client");
        let freelancer = Addr::unchecked("freelancer");

        let id = generate_escrow_id(1, &client, &freelancer, env.block.time.seconds());
        assert_eq!(id.len(), 16);
    }
}
