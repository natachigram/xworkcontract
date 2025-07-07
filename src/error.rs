use cosmwasm_std::{DivideByZeroError, OverflowError, StdError};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Math overflow: {0}")]
    Overflow(#[from] OverflowError),

    #[error("Division by zero: {0}")]
    DivideByZero(#[from] DivideByZeroError),

    #[error("Payment error: {0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid input: {error}")]
    InvalidInput { error: String },

    #[error("Job not found")]
    JobNotFound {},

    #[error("Proposal not found")]
    ProposalNotFound {},

    #[error("Escrow not found")]
    EscrowNotFound {},

    #[error("Insufficient funds: expected {expected}, got {actual}")]
    InsufficientFunds { expected: String, actual: String },

    #[error("Invalid funds provided")]
    InvalidFunds {},

    #[error("Job status error: {msg}")]
    JobStatusError { msg: String },

    #[error("Escrow already exists for job {job_id}")]
    EscrowAlreadyExists { job_id: u64 },

    #[error("Escrow not funded for job {job_id}")]
    EscrowNotFunded { job_id: u64 },

    #[error("Payment error: {msg}")]
    PaymentError { msg: String },

    #[error("Rating error: {msg}")]
    RatingError { msg: String },

    #[error("Invalid deadline: deadline must be in the future")]
    InvalidDeadline {},

    #[error("Job expired")]
    JobExpired {},

    #[error("Dispute period still active")]
    DisputePeriodActive {},

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("Platform fee too high: maximum {max}%")]
    PlatformFeeTooHigh { max: u64 },

    #[error("Rating already submitted")]
    RatingAlreadySubmitted {},

    #[error("Cannot rate own work")]
    CannotRateOwnWork {},

    #[error("Escrow amount too low: minimum {min}")]
    EscrowAmountTooLow { min: String },

    #[error("Milestone not found")]
    MilestoneNotFound {},

    #[error("All milestones must be completed")]
    MilestonesNotCompleted {},

    // Security-specific errors
    #[error("Reentrancy attack detected")]
    ReentrancyAttack {},

    #[error("Rate limit exceeded for {action}: maximum {limit} per day")]
    RateLimitExceeded { action: String, limit: u64 },

    #[error("Access denied: insufficient permissions")]
    AccessDenied {},

    #[error("Dispute already exists")]
    DisputeAlreadyExists {},

    #[error("Invalid escrow state transition")]
    InvalidEscrowStateTransition {},

    #[error("Milestone already completed")]
    MilestoneAlreadyCompleted {},

    #[error("Cannot modify completed job")]
    CannotModifyCompletedJob {},

    #[error("Proposal deadline exceeded")]
    ProposalDeadlineExceeded {},

    #[error("Emergency stop activated")]
    EmergencyStop {},

    #[error("Invalid signature")]
    InvalidSignature {},

    #[error("Nonce already used")]
    NonceAlreadyUsed {},

    #[error("Feature not yet implemented")]
    NotImplemented {},

    // CW20 related errors
    #[error("CW20 error: {msg}")]
    Cw20Error { msg: String },

    #[error("Token transfer failed")]
    TokenTransferFailed {},
}
