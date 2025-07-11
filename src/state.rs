use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum JobStatus {
    Open,
    InProgress,
    Completed,
    Cancelled,
    Disputed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum DisputeStatus {
    None,
    Raised,
    UnderReview,
    Resolved,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Job {
    pub id: u64,
    pub poster: Addr,
    pub title: String,
    pub description: String,
    pub budget: Uint128,
    pub category: String,
    pub skills_required: Vec<String>,
    pub duration_days: u64,
    pub documents: Vec<String>, // URLs or IPFS hashes
    pub status: JobStatus,
    pub assigned_freelancer: Option<Addr>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deadline: Timestamp,
    pub milestones: Vec<Milestone>,
    pub escrow_id: Option<String>,
    pub total_proposals: u64,
    // New fields for UI requirements
    pub company: Option<String>,  // Company name (optional)
    pub location: Option<String>, // Job location (optional)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Milestone {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub amount: Uint128,
    pub deadline: Timestamp,
    pub completed: bool,
    pub completed_at: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Proposal {
    pub id: u64,
    pub freelancer: Addr,
    pub job_id: u64,
    pub cover_letter_hash: String, // IPFS hash for cover letter (max 750 chars off-chain)
    pub resume_hash: String,       // IPFS hash for resume (PDF, max 5MB off-chain)
    pub delivery_time_days: u64,
    pub contact_preference: String, // Contact preference field
    pub agreed_to_terms: bool,      // Agreement to budget and time
    pub agreed_to_escrow: bool,     // Acknowledgment of crypto escrow payment
    pub submitted_at: Timestamp,
    pub milestones: Vec<ProposalMilestone>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ProposalMilestone {
    pub title: String,
    pub description: String,
    pub amount: Uint128,
    pub deadline_days: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EscrowState {
    pub id: String,
    pub job_id: u64,
    pub client: Addr,
    pub freelancer: Addr,
    pub amount: Uint128,
    pub platform_fee: Uint128,
    pub funded_at: Timestamp,
    pub released: bool,
    pub dispute_status: DisputeStatus,
    pub dispute_raised_at: Option<Timestamp>,
    pub dispute_deadline: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub platform_fee_percent: u64, // Max 10%
    pub min_escrow_amount: Uint128,
    pub dispute_period_days: u64,   // Default 7 days
    pub max_job_duration_days: u64, // Default 365 days
    pub paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Rating {
    pub id: String,
    pub job_id: u64,
    pub rater: Addr,
    pub rated: Addr,
    pub rating: u8, // 1-5 stars
    pub comment: String,
    pub created_at: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserStats {
    pub total_jobs_posted: u64,
    pub total_jobs_completed: u64,
    pub total_earned: Uint128,
    pub total_spent: Uint128,
    pub average_rating: Decimal,
    pub total_ratings: u64,
    pub completion_rate: Decimal,
    // New field for UI display
    pub display_name: Option<String>, // Optional display name for freelancers
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Dispute {
    pub id: String,
    pub job_id: u64,
    pub raised_by: Addr,
    pub reason: String,
    pub evidence: Vec<String>,
    pub status: DisputeStatus,
    pub created_at: Timestamp,
    pub resolved_at: Option<Timestamp>,
    pub resolution: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ProposalStatus {
    Submitted,
    Accepted,
    Rejected,
    Withdrawn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum EscrowStatus {
    Pending,
    Funded,
    Released,
    Refunded,
    Disputed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum BountySubmissionStatus {
    Submitted,
    UnderReview,
    Approved,
    Rejected,
    Winner,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserProfile {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub skills: Vec<String>,
    pub portfolio_links: Vec<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Escrow {
    pub id: String,
    pub job_id: Option<u64>,
    pub bounty_id: Option<u64>,
    pub client: Addr,
    pub freelancer: Addr,
    pub amount: Uint128,
    pub platform_fee: Uint128,
    pub status: EscrowStatus,
    pub created_at: Timestamp,
    pub funded_at: Option<Timestamp>,
    pub released_at: Option<Timestamp>,
    pub dispute_status: DisputeStatus,
    pub dispute_raised_at: Option<Timestamp>,
    pub dispute_deadline: Option<Timestamp>,
}

// Security-related structures
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SecurityMetrics {
    pub total_jobs: u64,
    pub total_proposals: u64,
    pub total_disputes: u64,
    pub blocked_addresses: Vec<Addr>,
    pub rate_limit_violations: u64,
    pub last_updated: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RateLimitState {
    pub user: Addr,
    pub action_type: String, // "post_job", "submit_proposal", etc.
    pub count: u64,
    pub window_start: Timestamp,
    pub last_action: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuditLog {
    pub id: String,
    pub action: String,
    pub user: Addr,
    pub job_id: Option<u64>,
    pub proposal_id: Option<u64>,
    pub timestamp: Timestamp,
    pub success: bool,
    pub error: Option<String>,
}

// Bounty-related structures
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum BountyStatus {
    Open,
    InReview,
    Completed,
    Cancelled,
    Expired,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Bounty {
    pub id: u64,
    pub poster: Addr,
    pub title: String,
    pub description: String,
    pub requirements: Vec<String>, // Specific deliverables/requirements
    pub total_reward: Uint128,
    pub category: String,
    pub skills_required: Vec<String>,
    pub submission_deadline: Timestamp,
    pub review_period_days: u64, // Days to review submissions after deadline
    pub max_winners: u64,        // Maximum number of winners (1 for single winner, >1 for multiple)
    pub reward_distribution: Vec<RewardTier>, // How rewards are split among winners
    pub documents: Vec<String>,  // Reference materials, briefs, etc.
    pub status: BountyStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub total_submissions: u64,
    pub selected_winners: Vec<u64>, // Submission IDs that won
    pub escrow_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardTier {
    pub position: u64,   // 1st place, 2nd place, etc.
    pub percentage: u64, // Percentage of total reward (sum should be 100)
    pub amount: Uint128, // Calculated amount based on percentage
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BountySubmission {
    pub id: u64,
    pub bounty_id: u64,
    pub submitter: Addr,
    pub title: String,
    pub description: String,
    pub deliverables: Vec<String>, // URLs to work, GitHub repos, documents, etc.
    pub submitted_at: Timestamp,
    pub status: BountySubmissionStatus,
    pub review_notes: Option<String>, // Admin/poster notes during review
    pub score: Option<u8>,            // 1-10 score for ranking submissions
    pub winner_position: Option<u64>, // If winner, what position (1st, 2nd, etc.)
}

// Enhanced storage keys with security features
pub const JOBS: Map<u64, Job> = Map::new("jobs");
pub const PROPOSALS: Map<u64, Proposal> = Map::new("proposals");
pub const JOB_PROPOSALS: Map<u64, Vec<u64>> = Map::new("job_proposals"); // job_id -> proposal_ids
pub const USER_PROPOSALS: Map<&Addr, Vec<u64>> = Map::new("user_proposals"); // user -> proposal_ids
pub const USER_JOB_PROPOSALS: Map<(&Addr, u64), u64> = Map::new("user_job_proposals"); // (user, job_id) -> proposal_id to prevent duplicates
pub const JOB_COUNTER: Item<u64> = Item::new("job_counter");
pub const PROPOSAL_COUNTER: Item<u64> = Item::new("proposal_counter");
pub const ESCROWS: Map<&str, EscrowState> = Map::new("escrows");

// Bounty storage
pub const BOUNTIES: Map<u64, Bounty> = Map::new("bounties");
pub const BOUNTY_SUBMISSIONS: Map<u64, BountySubmission> = Map::new("bounty_submissions");
pub const BOUNTY_SUBMISSIONS_BY_BOUNTY: Map<u64, Vec<u64>> =
    Map::new("bounty_submissions_by_bounty");
pub const USER_BOUNTY_SUBMISSIONS: Map<&Addr, Vec<u64>> = Map::new("user_bounty_submissions");
pub const BOUNTY_COUNTER: Item<u64> = Item::new("bounty_counter");
pub const BOUNTY_SUBMISSION_COUNTER: Item<u64> = Item::new("bounty_submission_counter");

pub const CONFIG: Item<Config> = Item::new("config");
pub const RATINGS: Map<&str, Rating> = Map::new("ratings"); // job_id_rater_address
pub const USER_STATS: Map<&Addr, UserStats> = Map::new("user_stats");
pub const DISPUTES: Map<&str, Dispute> = Map::new("disputes");

// Missing ID counters
pub const NEXT_JOB_ID: Item<u64> = Item::new("next_job_id");
pub const NEXT_PROPOSAL_ID: Item<u64> = Item::new("next_proposal_id");
pub const NEXT_ESCROW_ID: Item<u64> = Item::new("next_escrow_id");
pub const NEXT_BOUNTY_ID: Item<u64> = Item::new("next_bounty_id");
pub const NEXT_BOUNTY_SUBMISSION_ID: Item<u64> = Item::new("next_bounty_submission_id");

// User profiles storage
pub const USER_PROFILES: Map<&Addr, UserProfile> = Map::new("user_profiles");

// Security-related storage
pub const SECURITY_METRICS: Item<SecurityMetrics> = Item::new("security_metrics");
pub const RATE_LIMITS: Map<(&Addr, &str), RateLimitState> = Map::new("rate_limits");
pub const AUDIT_LOGS: Map<&str, AuditLog> = Map::new("audit_logs");
pub const REENTRANCY_GUARDS: Map<&Addr, bool> = Map::new("reentrancy_guards");
pub const BLOCKED_ADDRESSES: Map<&Addr, Timestamp> = Map::new("blocked_addresses");
