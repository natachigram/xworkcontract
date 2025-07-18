use crate::hash_utils::ContentHash;
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

    // 🔥 ESSENTIAL DATA (KEPT ON-CHAIN)
    pub budget: Uint128,
    pub duration_days: u64,
    pub status: JobStatus,
    pub assigned_freelancer: Option<Addr>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deadline: Timestamp,
    pub escrow_id: Option<String>,
    pub total_proposals: u64,

    // 🌐 CONTENT HASH (POINTS TO OFF-CHAIN DATA)
    pub content_hash: ContentHash, // Contains: title, description, company, location, category, skills, documents

    // 📊 SEARCHABLE METADATA (MINIMAL ON-CHAIN)
    pub category_id: u8,      // Numeric category for efficient filtering
    pub skill_tags: Vec<u8>,  // Skill IDs for efficient matching
    pub budget_range: u8,     // 1=Low(<$500), 2=Mid($500-5k), 3=High(>$5k)
    pub experience_level: u8, // 1=Entry, 2=Mid, 3=Senior
    pub is_remote: bool,
    pub has_milestones: bool,
    pub urgency_level: u8, // 1=Low, 2=Medium, 3=High, 4=Urgent
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Milestone {
    pub id: u64,
    pub amount: Uint128,
    pub deadline: Timestamp,
    pub completed: bool,
    pub completed_at: Option<Timestamp>,
    // Content hash points to title, description, requirements off-chain
    pub content_hash: ContentHash,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Proposal {
    pub id: u64,
    pub freelancer: Addr,
    pub job_id: u64,

    // 🔥 ESSENTIAL DATA (KEPT ON-CHAIN)
    pub delivery_time_days: u64,
    pub contact_preference: ContactPreference,
    pub agreed_to_terms: bool,
    pub agreed_to_escrow: bool,
    pub submitted_at: Timestamp,

    // 🌐 CONTENT HASH (POINTS TO OFF-CHAIN DATA)
    pub content_hash: ContentHash, // Contains: cover_letter, milestones, portfolio

    // 📊 PROPOSAL METADATA (MINIMAL ON-CHAIN)
    pub proposal_score: u8, // Auto-calculated score 1-100
    pub has_milestones: bool,
    pub milestone_count: u8,
    pub estimated_hours: u16, // If provided by freelancer
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ContactPreference {
    Email,
    Platform,
    Phone,
    VideoCall,
    Discord,
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
    Withdrawn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserProfile {
    // 🔥 ESSENTIAL DATA (KEPT ON-CHAIN)
    pub created_at: Timestamp,
    pub updated_at: Timestamp,

    // 🌐 CONTENT HASH (POINTS TO OFF-CHAIN DATA)
    pub content_hash: ContentHash, // Contains: display_name, bio, skills, portfolio_links

    // 📊 USER STATS AND METADATA (ON-CHAIN)
    pub total_jobs_completed: u64,
    pub average_rating: Decimal,
    pub total_earned: Uint128,
    pub is_verified: bool,
    pub response_time_hours: u8, // Average response time in hours
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

    // 🔥 ESSENTIAL BUSINESS LOGIC DATA (KEPT ON-CHAIN)
    pub total_reward: Uint128,           // Contract needs for escrow/payments
    pub submission_deadline: Timestamp,   // Contract needs for deadline enforcement
    pub review_period_days: u64,         // Contract needs for review period enforcement
    pub max_winners: u64,                // Contract needs for winner selection logic
    pub reward_distribution: Vec<RewardTier>, // Contract needs for payment distribution
    pub status: BountyStatus,            // Contract needs for state management
    pub created_at: Timestamp,           // Contract needs for time-based logic
    pub updated_at: Timestamp,           // Contract needs for modification tracking
    pub total_submissions: u64,          // Contract needs for submission counting
    pub selected_winners: Vec<u64>,      // Contract needs for winner tracking
    pub escrow_id: Option<String>,       // Contract needs for escrow management

    // 🌐 ALL CONTENT OFF-CHAIN (via content_hash)
    pub content_hash: ContentHash,       // title, description, requirements, documents, skills, categories
}
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

    // 🔥 ESSENTIAL DATA (KEPT ON-CHAIN)
    pub submitted_at: Timestamp,
    pub status: BountySubmissionStatus,
    pub score: Option<u8>,            // 1-10 score for ranking submissions
    pub winner_position: Option<u64>, // If winner, what position (1st, 2nd, etc.)

    // 🌐 CONTENT HASH (POINTS TO OFF-CHAIN DATA)
    pub content_hash: ContentHash, // Contains: title, description, deliverables, review_notes

    // 📊 SUBMISSION METADATA (MINIMAL ON-CHAIN)
    pub deliverable_count: u8,           // Number of deliverables provided
    pub submission_type: u8,             // 1=Document, 2=Code, 3=Design, 4=Video, 5=Other
    pub estimated_completion_hours: u16, // Time spent on bounty
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

// 🎯 HASH & OFF-CHAIN DATA MANAGEMENT
pub const CONTENT_HASHES: Map<&str, ContentHash> = Map::new("content_hashes"); // hash -> metadata
pub const HASH_TO_ENTITY: Map<&str, String> = Map::new("hash_to_entity"); // hash -> entity_id
pub const ENTITY_TO_HASH: Map<&str, String> = Map::new("entity_to_hash"); // entity_id -> current_hash

// 📚 LOOKUP TABLES FOR EFFICIENT QUERIES
pub const CATEGORIES: Map<u8, String> = Map::new("categories"); // id -> category_name
pub const SKILLS: Map<u8, String> = Map::new("skills"); // id -> skill_name
pub const CATEGORY_COUNTER: Item<u8> = Item::new("category_counter");
pub const SKILL_COUNTER: Item<u8> = Item::new("skill_counter");

// 🔍 SEARCH INDEXES (FOR FAST FILTERING)
pub const JOBS_BY_CATEGORY: Map<u8, Vec<u64>> = Map::new("jobs_by_category");
pub const JOBS_BY_BUDGET_RANGE: Map<u8, Vec<u64>> = Map::new("jobs_by_budget_range");
pub const JOBS_BY_SKILL: Map<u8, Vec<u64>> = Map::new("jobs_by_skill");
pub const ACTIVE_JOBS: Map<u64, bool> = Map::new("active_jobs"); // For quick active job lookup

// 🎯 BOUNTY SEARCH INDEXES (FOR EFFICIENT BOUNTY FILTERING)
pub const BOUNTIES_BY_CATEGORY: Map<u8, Vec<u64>> = Map::new("bounties_by_category");
pub const BOUNTIES_BY_REWARD_RANGE: Map<u8, Vec<u64>> = Map::new("bounties_by_reward_range");
pub const BOUNTIES_BY_SKILL: Map<u8, Vec<u64>> = Map::new("bounties_by_skill");
pub const BOUNTIES_BY_DIFFICULTY: Map<u8, Vec<u64>> = Map::new("bounties_by_difficulty");
pub const ACTIVE_BOUNTIES: Map<u64, bool> = Map::new("active_bounties"); // For quick active bounty lookup
pub const FEATURED_BOUNTIES: Map<u64, bool> = Map::new("featured_bounties"); // For featured bounty display

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
