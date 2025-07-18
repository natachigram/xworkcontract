use crate::state::{
    AuditLog, Bounty, BountyStatus, BountySubmission, BountySubmissionStatus, Config,
    ContactPreference, Dispute, EscrowState, Job, JobStatus, Proposal, ProposalMilestone, Rating,
    SecurityMetrics, UserStats,
};
use cosmwasm_std::{Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub platform_fee_percent: Option<u64>,
    pub min_escrow_amount: Option<Uint128>,
    pub dispute_period_days: Option<u64>,
    pub max_job_duration_days: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MilestoneInput {
    pub title: String,
    pub description: String,
    pub amount: Uint128,
    pub deadline_days: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardTierInput {
    pub position: u64,
    pub percentage: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WinnerSelection {
    pub submission_id: u64,
    pub position: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ExecuteMsg {
    // 🎯 JOB MANAGEMENT (HYBRID ON-CHAIN/OFF-CHAIN)
    PostJob {
        // 🌐 OFF-CHAIN CONTENT (will be hashed and stored externally)
        title: String,
        description: String,
        company: Option<String>,
        location: Option<String>,
        category: String,             // Will be converted to category_id
        skills_required: Vec<String>, // Will be converted to skill_tags
        documents: Option<Vec<String>>,
        milestones: Option<Vec<MilestoneInput>>,

        // 🔥 ON-CHAIN ESSENTIAL DATA
        budget: Uint128,
        duration_days: u64,
        experience_level: u8, // 1=Entry, 2=Mid, 3=Senior
        is_remote: bool,
        urgency_level: u8, // 1=Low, 2=Medium, 3=High, 4=Urgent

        // 🌐 WEB2 BACKEND REFERENCE
        off_chain_storage_key: String, // Key for retrieving from web2 backend
    },
    EditJob {
        job_id: u64,
        title: Option<String>,
        description: Option<String>,
        budget: Option<Uint128>,
        category: Option<String>,
        skills_required: Option<Vec<String>>,
        duration_days: Option<u64>,
        documents: Option<Vec<String>>,
        milestones: Option<Vec<MilestoneInput>>,
        off_chain_storage_key: String, // Key for web2 backend updates
    },
    DeleteJob {
        job_id: u64,
    },
    CancelJob {
        job_id: u64,
    },

    // 🎯 PROPOSAL MANAGEMENT (HYBRID ON-CHAIN/OFF-CHAIN)
    SubmitProposal {
        job_id: u64,

        // 🌐 OFF-CHAIN CONTENT (will be hashed and stored externally)
        cover_letter: String,
        milestones: Option<Vec<ProposalMilestone>>,
        portfolio_samples: Option<Vec<String>>, // Links to work samples

        // 🔥 ON-CHAIN ESSENTIAL DATA
        delivery_time_days: u64,
        contact_preference: ContactPreference,
        agreed_to_terms: bool,
        agreed_to_escrow: bool,
        estimated_hours: Option<u16>,

        // 🌐 WEB2 BACKEND REFERENCE
        off_chain_storage_key: String,
    },
    EditProposal {
        proposal_id: u64,
        cover_letter: Option<String>,
        delivery_time_days: Option<u64>,
        milestones: Option<Vec<ProposalMilestone>>,
    },
    WithdrawProposal {
        proposal_id: u64,
    },
    AcceptProposal {
        job_id: u64,
        proposal_id: u64,
    },

    // Escrow Management
    CreateEscrow {
        job_id: u64,
    },
    CreateEscrowNative {
        job_id: u64,
        amount: Uint128,
    },
    CreateEscrowCw20 {
        job_id: u64,
        token_address: String,
        amount: Uint128,
    },
    FundEscrow {
        escrow_id: String,
    },
    ReleaseEscrow {
        escrow_id: String,
    },
    RefundEscrow {
        escrow_id: String,
    },

    // Work Management
    CompleteJob {
        job_id: u64,
    },
    CompleteMilestone {
        job_id: u64,
        milestone_id: u64,
    },
    ApproveMilestone {
        job_id: u64,
        milestone_id: u64,
    },

    // Rating System
    SubmitRating {
        job_id: u64,
        rating: u8,
        comment: String,
    },

    // Dispute Management
    RaiseDispute {
        job_id: u64,
        reason: String,
        evidence: Vec<String>,
    },
    ResolveDispute {
        dispute_id: String,
        resolution: String,
        release_to_freelancer: bool,
    },

    // Admin Functions
    UpdateConfig {
        admin: Option<String>,
        platform_fee_percent: Option<u64>,
        min_escrow_amount: Option<Uint128>,
        dispute_period_days: Option<u64>,
        max_job_duration_days: Option<u64>,
    },
    PauseContract {},
    UnpauseContract {},

    // User Profile Management (HYBRID)
    UpdateUserProfile {
        // 🌐 OFF-CHAIN CONTENT
        display_name: Option<String>,
        bio: Option<String>,
        skills: Option<Vec<String>>,
        location: Option<String>,
        website: Option<String>,
        portfolio_links: Option<Vec<String>>,

        // 🔥 ON-CHAIN METADATA
        hourly_rate: Option<Uint128>,
        availability: Option<String>,

        // 🌐 WEB2 BACKEND REFERENCE
        off_chain_storage_key: String,
    },

    // Bounty Management
    CreateBounty {
        title: String,
        description: String,
        requirements: Vec<String>,
        total_reward: Uint128,
        category: String,
        skills_required: Vec<String>,
        submission_deadline_days: u64, // Days from now
        review_period_days: u64,
        max_winners: u64,
        reward_distribution: Vec<RewardTierInput>,
        documents: Option<Vec<String>>,
    },
    EditBounty {
        bounty_id: u64,
        title: Option<String>,
        description: Option<String>,
        requirements: Option<Vec<String>>,
        submission_deadline_days: Option<u64>,
        review_period_days: Option<u64>,
        documents: Option<Vec<String>>,
    },
    CancelBounty {
        bounty_id: u64,
    },
    SubmitToBounty {
        bounty_id: u64,
        title: String,
        description: String,
        deliverables: Vec<String>,
    },
    EditBountySubmission {
        submission_id: u64,
        title: Option<String>,
        description: Option<String>,
        deliverables: Option<Vec<String>>,
    },
    WithdrawBountySubmission {
        submission_id: u64,
    },
    ReviewBountySubmission {
        submission_id: u64,
        status: BountySubmissionStatus,
        review_notes: Option<String>,
        score: Option<u8>,
    },
    SelectBountyWinners {
        bounty_id: u64,
        winner_submissions: Vec<WinnerSelection>,
    },
    CreateBountyEscrow {
        bounty_id: u64,
    },
    ReleaseBountyRewards {
        bounty_id: u64,
    },

    // Security Functions
    BlockAddress {
        address: String,
        reason: String,
    },
    UnblockAddress {
        address: String,
    },
    ResetRateLimit {
        address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum QueryMsg {
    // Job Queries
    GetJob {
        job_id: u64,
    },
    GetJobs {
        start_after: Option<u64>,
        limit: Option<u32>,
        category: Option<String>,
        status: Option<JobStatus>,
        poster: Option<String>,
    },
    GetAllJobs {
        // For frontend landing page - gets all active jobs with basic filtering
        limit: Option<u32>,
        category: Option<String>,
    },
    GetUserJobs {
        user: String,
        status: Option<JobStatus>,
    },

    // Proposal Queries
    GetProposal {
        proposal_id: u64,
    },
    GetJobProposals {
        job_id: u64,
    },
    GetUserProposals {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    // Escrow Queries
    GetEscrow {
        escrow_id: String,
    },
    GetJobEscrow {
        job_id: u64,
    },

    // Rating Queries
    GetUserRatings {
        user: String,
    },
    GetJobRating {
        job_id: u64,
        rater: String,
    },

    // Stats Queries
    GetUserStats {
        user: String,
    },
    GetPlatformStats {},

    // Dispute Queries
    GetDispute {
        dispute_id: String,
    },
    GetJobDisputes {
        job_id: u64,
    },
    GetUserDisputes {
        user: String,
    },

    // Bounty Queries
    GetBounty {
        bounty_id: u64,
    },
    GetBounties {
        start_after: Option<u64>,
        limit: Option<u32>,
        category: Option<String>,
        status: Option<BountyStatus>,
        poster: Option<String>,
    },
    GetAllBounties {
        // For frontend landing page - gets all active bounties with basic filtering
        limit: Option<u32>,
        category: Option<String>,
    },
    GetUserBounties {
        user: String,
        status: Option<BountyStatus>,
    },
    GetBountySubmission {
        submission_id: u64,
    },
    GetBountySubmissions {
        bounty_id: u64,
        status: Option<BountySubmissionStatus>,
    },
    GetUserBountySubmissions {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    // Config Query
    GetConfig {},

    // Security Queries
    GetSecurityMetrics {},
    GetAuditLogs {
        start_after: Option<String>,
        limit: Option<u32>,
        action_filter: Option<String>,
    },
    IsAddressBlocked {
        address: String,
    },
    GetRateLimitStatus {
        address: String,
    },
}

// Response types
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct JobResponse {
    pub job: Job,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct JobsResponse {
    pub jobs: Vec<Job>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ProposalResponse {
    pub proposal: Proposal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ProposalsResponse {
    pub proposals: Vec<Proposal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EscrowResponse {
    pub escrow: EscrowState,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RatingsResponse {
    pub ratings: Vec<Rating>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserStatsResponse {
    pub stats: UserStats,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PlatformStatsResponse {
    pub total_jobs: u64,
    pub open_jobs: u64,
    pub in_progress_jobs: u64,
    pub completed_jobs: u64,
    pub total_bounties: u64,
    pub open_bounties: u64,
    pub completed_bounties: u64,
    pub total_users: u64,
    pub total_value_locked: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DisputeResponse {
    pub dispute: Dispute,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DisputesResponse {
    pub disputes: Vec<Dispute>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SecurityMetricsResponse {
    pub metrics: SecurityMetrics,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuditLogsResponse {
    pub logs: Vec<AuditLog>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AddressBlockedResponse {
    pub is_blocked: bool,
    pub reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RateLimitStatusResponse {
    pub current_count: u64,
    pub limit: u64,
    pub window_start: Timestamp,
    pub is_limited: bool,
}

// Bounty Response Types
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BountyResponse {
    pub bounty: Bounty,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BountiesResponse {
    pub bounties: Vec<Bounty>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BountySubmissionResponse {
    pub submission: BountySubmission,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BountySubmissionsResponse {
    pub submissions: Vec<BountySubmission>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SearchResponse {
    pub jobs: Vec<Job>,
    pub bounties: Vec<Bounty>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TrendingResponse {
    pub trending_jobs: Vec<Job>,
    pub trending_bounties: Vec<Bounty>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CategoriesResponse {
    pub job_categories: Vec<(String, u64)>,
    pub bounty_categories: Vec<(String, u64)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EscrowsResponse {
    pub escrows: Vec<EscrowState>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserProfileResponse {
    pub profile: crate::state::UserProfile,
}
