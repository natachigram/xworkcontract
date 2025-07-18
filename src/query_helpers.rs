use crate::hash_utils::ContentHash;
use crate::helpers::{convert_category_to_id, convert_skill_to_id};
use crate::msg::*;
use crate::state::*;
use cosmwasm_std::{Deps, Order, StdResult, Uint128};

/// Convert category ID back to category name
fn convert_category_id_to_name(category_id: u8) -> String {
    match category_id {
        1 => "Web Development".to_string(),
        2 => "Mobile Development".to_string(),
        3 => "Design".to_string(),
        4 => "Writing".to_string(),
        5 => "Marketing".to_string(),
        6 => "Blockchain".to_string(),
        7 => "Data Science".to_string(),
        8 => "DevOps".to_string(),
        9 => "Testing".to_string(),
        10 => "Video Production".to_string(),
        _ => "Other".to_string(),
    }
}

/// Generic pagination helper for any collection
pub struct PaginationParams {
    pub start_after: Option<String>,
    pub limit: Option<u32>,
}

impl PaginationParams {
    pub fn new(start_after: Option<String>, limit: Option<u32>) -> Self {
        Self {
            start_after,
            limit: Some(limit.unwrap_or(50).min(100)),
        }
    }
}

/// 🎯 Enhanced JobResponse with hash reference for off-chain content
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct HashAwareJobResponse {
    pub id: u64,
    pub poster: String,
    pub budget: Uint128,
    pub duration_days: u64,
    pub status: JobStatus,
    pub assigned_freelancer: Option<String>,
    pub created_at: cosmwasm_std::Timestamp,
    pub updated_at: cosmwasm_std::Timestamp,
    pub deadline: cosmwasm_std::Timestamp,
    pub escrow_id: Option<String>,
    pub total_proposals: u64,

    // 🌐 HASH REFERENCE FOR OFF-CHAIN CONTENT
    pub content_hash: ContentHash,
    pub off_chain_data_key: String, // For web2 backend retrieval

    // 📊 ON-CHAIN SEARCHABLE METADATA
    pub category_id: u8,
    pub skill_tags: Vec<u8>,
    pub budget_range: u8,
    pub experience_level: u8,
    pub is_remote: bool,
    pub has_milestones: bool,
    pub urgency_level: u8,
}

/// 🎯 Enhanced ProposalResponse with hash reference
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct HashAwareProposalResponse {
    pub id: u64,
    pub freelancer: String,
    pub job_id: u64,
    pub delivery_time_days: u64,
    pub contact_preference: ContactPreference,
    pub agreed_to_terms: bool,
    pub agreed_to_escrow: bool,
    pub submitted_at: cosmwasm_std::Timestamp,

    // 🌐 HASH REFERENCE FOR OFF-CHAIN CONTENT
    pub content_hash: ContentHash,
    pub off_chain_data_key: String,

    // 📊 ON-CHAIN METADATA
    pub proposal_score: u8,
    pub has_milestones: bool,
    pub milestone_count: u8,
    pub estimated_hours: u16,
}

/// 🎯 Enhanced UserProfileResponse with hash reference
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct HashAwareUserProfileResponse {
    pub address: String,
    pub created_at: cosmwasm_std::Timestamp,
    pub updated_at: cosmwasm_std::Timestamp,

    // 🌐 HASH REFERENCE FOR OFF-CHAIN CONTENT
    pub content_hash: ContentHash,
    pub off_chain_data_key: String,

    // 📊 ON-CHAIN STATS AND METADATA
    pub total_jobs_completed: u64,
    pub average_rating: cosmwasm_std::Decimal,
    pub total_earned: Uint128,
    pub is_verified: bool,
    pub response_time_hours: u8,
}

/// Platform statistics calculation with hash-aware data
pub fn query_platform_stats(deps: Deps) -> StdResult<PlatformStatsResponse> {
    // Count jobs by status
    let mut total_jobs = 0u64;
    let mut open_jobs = 0u64;
    let mut in_progress_jobs = 0u64;
    let mut completed_jobs = 0u64;

    let jobs: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(job_pairs) = jobs {
        for (_, job) in job_pairs {
            total_jobs += 1;
            match job.status {
                JobStatus::Open => open_jobs += 1,
                JobStatus::InProgress => in_progress_jobs += 1,
                JobStatus::Completed => completed_jobs += 1,
                _ => {}
            }
        }
    }

    // Count bounties by status
    let mut total_bounties = 0u64;
    let mut open_bounties = 0u64;
    let mut completed_bounties = 0u64;

    let bounties: StdResult<Vec<_>> = BOUNTIES
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(bounty_pairs) = bounties {
        for (_, bounty) in bounty_pairs {
            total_bounties += 1;
            match bounty.status {
                BountyStatus::Open => open_bounties += 1,
                BountyStatus::Completed => completed_bounties += 1,
                _ => {}
            }
        }
    }

    // Count total users with profiles
    let total_users = USER_PROFILES
        .range(deps.storage, None, None, Order::Ascending)
        .count() as u64;

    // Calculate total value locked (from escrows)
    let mut total_value_locked = Uint128::zero();
    let escrows: StdResult<Vec<_>> = ESCROWS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(escrow_pairs) = escrows {
        for (_, escrow) in escrow_pairs {
            if !escrow.released {
                total_value_locked += escrow.amount;
            }
        }
    }

    Ok(PlatformStatsResponse {
        total_jobs,
        open_jobs,
        in_progress_jobs,
        completed_jobs,
        total_bounties,
        open_bounties,
        completed_bounties,
        total_users,
        total_value_locked,
    })
}

/// Enhanced job search with multiple filters
pub fn query_jobs_advanced(
    deps: Deps,
    params: PaginationParams,
    category: Option<String>,
    status: Option<JobStatus>,
    poster: Option<String>,
    min_budget: Option<Uint128>,
    max_budget: Option<Uint128>,
    skills_required: Option<Vec<String>>,
    job_type: Option<String>,
    remote_allowed: Option<bool>,
    experience_level: Option<String>,
) -> StdResult<JobsResponse> {
    let limit = params.limit.unwrap_or(50) as usize;
    let mut jobs = Vec::new();

    let poster_addr = if let Some(p) = poster {
        Some(deps.api.addr_validate(&p)?)
    } else {
        None
    };

    let items: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, Order::Descending)
        .collect();

    if let Ok(job_pairs) = items {
        for (id, job) in job_pairs {
            // Apply start_after filter
            if let Some(ref start_after_str) = params.start_after {
                if let Ok(start_after_id) = start_after_str.parse::<u64>() {
                    if id >= start_after_id {
                        continue;
                    }
                }
            }

            // Apply all filters
            let mut include = true;

            if let Some(ref filter_category) = category {
                // Map category string to ID for comparison
                let category_id = match filter_category.to_lowercase().as_str() {
                    "web development" => 1,
                    "mobile development" => 2,
                    "design" => 3,
                    "writing" => 4,
                    "marketing" => 5,
                    _ => 99, // Other
                };

                if job.category_id != category_id {
                    include = false;
                }
            }

            if let Some(ref filter_status) = status {
                if &job.status != filter_status {
                    include = false;
                }
            }

            if let Some(ref filter_poster) = poster_addr {
                if &job.poster != filter_poster {
                    include = false;
                }
            }

            if let Some(min_budget_val) = min_budget {
                if job.budget < min_budget_val {
                    include = false;
                }
            }

            if let Some(max_budget_val) = max_budget {
                if job.budget > max_budget_val {
                    include = false;
                }
            }

            if let Some(ref _filter_skills) = skills_required {
                // In hybrid architecture, skill matching would require off-chain content lookup
                // For now, skip skill filtering to avoid compilation errors
                // TODO: Implement skill matching via off-chain content hash resolution
            }

            if let Some(ref _filter_job_type) = job_type {
                // Job type filtering disabled in hybrid architecture
                // TODO: Implement job type via off-chain content or additional on-chain fields
            }

            if let Some(filter_remote) = remote_allowed {
                if job.is_remote != filter_remote {
                    include = false;
                }
            }

            if let Some(ref filter_exp_level) = experience_level {
                // Convert string experience level to u8 for comparison
                let exp_level_id = match filter_exp_level.to_lowercase().as_str() {
                    "entry" => 1,
                    "mid" => 2,
                    "senior" => 3,
                    _ => 2, // Default to mid
                };

                if job.experience_level != exp_level_id {
                    include = false;
                }
            }

            if include {
                jobs.push(job);
                if jobs.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(JobsResponse { jobs })
}

/// Enhanced bounty search with multiple filters
pub fn query_bounties_advanced(
    deps: Deps,
    params: PaginationParams,
    category: Option<String>,
    status: Option<BountyStatus>,
    creator: Option<String>,
    min_reward: Option<Uint128>,
    max_reward: Option<Uint128>,
    skills_required: Option<Vec<String>>,
) -> StdResult<BountiesResponse> {
    let limit = params.limit.unwrap_or(50) as usize;
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
            if let Some(ref start_after_str) = params.start_after {
                if let Ok(start_after_id) = start_after_str.parse::<u64>() {
                    if id >= start_after_id {
                        continue;
                    }
                }
            }

            // Apply all filters
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
                if &bounty.poster != filter_creator {
                    include = false;
                }
            }

            if let Some(min_reward_val) = min_reward {
                if bounty.total_reward < min_reward_val {
                    include = false;
                }
            }

            if let Some(max_reward_val) = max_reward {
                if bounty.total_reward > max_reward_val {
                    include = false;
                }
            }

            if let Some(ref filter_skills) = skills_required {
                let has_skill = filter_skills.iter().any(|skill| {
                    let skill_id = convert_skill_to_id(skill);
                    bounty.skill_tags.contains(&skill_id)
                });
                if !has_skill {
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

/// Search jobs and bounties by text query
pub fn search_content(
    deps: Deps,
    query: String,
    content_type: Option<String>, // "jobs", "bounties", or "all"
    limit: Option<u32>,
) -> StdResult<SearchResponse> {
    let limit = limit.unwrap_or(20).min(50) as usize;
    let query_lower = query.to_lowercase();
    let mut jobs = Vec::new();
    let mut bounties = Vec::new();

    let search_jobs = content_type.as_deref().unwrap_or("all") != "bounties";
    let search_bounties = content_type.as_deref().unwrap_or("all") != "jobs";

    // Search jobs
    if search_jobs {
        let job_items: StdResult<Vec<_>> = JOBS
            .range(deps.storage, None, None, Order::Descending)
            .collect();

        if let Ok(job_pairs) = job_items {
            for (_, job) in job_pairs {
                if job.status == JobStatus::Open {
                    // In hybrid architecture, detailed search requires off-chain content
                    // For now, we'll match based on available on-chain data
                    // TODO: Implement off-chain content search

                    // For now, include all open jobs in search results
                    // In production, this would query off-chain storage using content_hash
                    jobs.push(job);
                    if jobs.len() >= limit / 2 {
                        break;
                    }
                }
            }
        }
    }

    // Search bounties
    if search_bounties {
        let bounty_items: StdResult<Vec<_>> = BOUNTIES
            .range(deps.storage, None, None, Order::Descending)
            .collect();

        if let Ok(bounty_pairs) = bounty_items {
            for (_, bounty) in bounty_pairs {
                if bounty.status == BountyStatus::Open {
                    // Note: With ContentHash optimization, detailed content is off-chain
                    // For now, we'll do a simple match on bounty ID and basic fields
                    let matches = bounty.id.to_string().contains(&query_lower);
                    
                    if matches {
                        bounties.push(bounty);
                        if bounties.len() >= limit / 2 {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(SearchResponse { jobs, bounties })
}

/// Get trending/popular jobs and bounties
pub fn query_trending_content(deps: Deps) -> StdResult<TrendingResponse> {
    let mut popular_jobs = Vec::new();
    let mut popular_bounties = Vec::new();

    // Get jobs with most proposals (top 10)
    let job_items: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, Order::Descending)
        .collect();

    if let Ok(mut job_pairs) = job_items {
        // Sort by proposal count
        job_pairs.sort_by(|a, b| b.1.total_proposals.cmp(&a.1.total_proposals));

        for (_, job) in job_pairs.into_iter().take(10) {
            if job.status == JobStatus::Open && job.total_proposals > 0 {
                popular_jobs.push(job);
            }
        }
    }

    // Get bounties with most submissions (top 10)
    let bounty_items: StdResult<Vec<_>> = BOUNTIES
        .range(deps.storage, None, None, Order::Descending)
        .collect();

    if let Ok(mut bounty_pairs) = bounty_items {
        // Sort by submission count
        bounty_pairs.sort_by(|a, b| b.1.total_submissions.cmp(&a.1.total_submissions));

        for (_, bounty) in bounty_pairs.into_iter().take(10) {
            if bounty.status == BountyStatus::Open && bounty.total_submissions > 0 {
                popular_bounties.push(bounty);
            }
        }
    }

    Ok(TrendingResponse {
        trending_jobs: popular_jobs,
        trending_bounties: popular_bounties,
    })
}

/// Get categories with job/bounty counts
pub fn query_categories(deps: Deps) -> StdResult<CategoriesResponse> {
    let mut job_categories: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut bounty_categories: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    // Count job categories
    let job_items: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(job_pairs) = job_items {
        for (_, job) in job_pairs {
            if job.status == JobStatus::Open {
                // Map category_id back to category name
                let category_name = match job.category_id {
                    1 => "Web Development".to_string(),
                    2 => "Mobile Development".to_string(),
                    3 => "Design".to_string(),
                    4 => "Writing".to_string(),
                    5 => "Marketing".to_string(),
                    _ => "Other".to_string(),
                };

                *job_categories.entry(category_name).or_insert(0) += 1;
            }
        }
    }

    // Count bounty categories
    let bounty_items: StdResult<Vec<_>> = BOUNTIES
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

        if let Ok(bounty_pairs) = bounty_items {
            for (_, bounty) in bounty_pairs {
                if bounty.status == BountyStatus::Open {
                    let category_name = convert_category_id_to_name(bounty.category_id);
                    *bounty_categories.entry(category_name).or_insert(0) += 1;
                }
            }
        }    // Convert to sorted vectors
    let mut job_cats: Vec<_> = job_categories.into_iter().collect();
    job_cats.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    let mut bounty_cats: Vec<_> = bounty_categories.into_iter().collect();
    bounty_cats.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    Ok(CategoriesResponse {
        job_categories: job_cats,
        bounty_categories: bounty_cats,
    })
}

/// 🔄 Convert Job to HashAwareJobResponse for API consumption
pub fn job_to_hash_aware_response(job: &Job, off_chain_key: String) -> HashAwareJobResponse {
    HashAwareJobResponse {
        id: job.id,
        poster: job.poster.to_string(),
        budget: job.budget,
        duration_days: job.duration_days,
        status: job.status.clone(),
        assigned_freelancer: job
            .assigned_freelancer
            .as_ref()
            .map(|addr| addr.to_string()),
        created_at: job.created_at,
        updated_at: job.updated_at,
        deadline: job.deadline,
        escrow_id: job.escrow_id.clone(),
        total_proposals: job.total_proposals,
        content_hash: job.content_hash.clone(),
        off_chain_data_key: off_chain_key,
        category_id: job.category_id,
        skill_tags: job.skill_tags.clone(),
        budget_range: job.budget_range,
        experience_level: job.experience_level,
        is_remote: job.is_remote,
        has_milestones: job.has_milestones,
        urgency_level: job.urgency_level,
    }
}

/// 🔄 Convert Proposal to HashAwareProposalResponse  
pub fn proposal_to_hash_aware_response(
    proposal: &Proposal,
    off_chain_key: String,
) -> HashAwareProposalResponse {
    HashAwareProposalResponse {
        id: proposal.id,
        freelancer: proposal.freelancer.to_string(),
        job_id: proposal.job_id,
        delivery_time_days: proposal.delivery_time_days,
        contact_preference: proposal.contact_preference.clone(),
        agreed_to_terms: proposal.agreed_to_terms,
        agreed_to_escrow: proposal.agreed_to_escrow,
        submitted_at: proposal.submitted_at,
        content_hash: proposal.content_hash.clone(),
        off_chain_data_key: off_chain_key,
        proposal_score: proposal.proposal_score,
        has_milestones: proposal.has_milestones,
        milestone_count: proposal.milestone_count,
        estimated_hours: proposal.estimated_hours,
    }
}

/// 🔄 Convert UserProfile to HashAwareUserProfileResponse
pub fn user_profile_to_hash_aware_response(
    profile: &UserProfile,
    address: String,
    off_chain_key: String,
) -> HashAwareUserProfileResponse {
    HashAwareUserProfileResponse {
        address,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
        content_hash: profile.content_hash.clone(),
        off_chain_data_key: off_chain_key,
        total_jobs_completed: profile.total_jobs_completed,
        average_rating: profile.average_rating,
        total_earned: profile.total_earned,
        is_verified: profile.is_verified,
        response_time_hours: profile.response_time_hours,
    }
}

/// 🔍 Query hash-aware jobs with efficient filtering
pub fn query_hash_aware_jobs(
    deps: Deps,
    category_id: Option<u8>,
    budget_range: Option<u8>,
    skill_tags: Option<Vec<u8>>,
    is_remote: Option<bool>,
    experience_level: Option<u8>,
    limit: Option<u32>,
) -> StdResult<Vec<HashAwareJobResponse>> {
    let limit = limit.unwrap_or(50).min(100) as usize;
    let mut results = Vec::new();

    // Use search indexes for efficient filtering
    let candidate_jobs = if let Some(cat_id) = category_id {
        JOBS_BY_CATEGORY
            .may_load(deps.storage, cat_id)?
            .unwrap_or_default()
    } else if let Some(budget_r) = budget_range {
        JOBS_BY_BUDGET_RANGE
            .may_load(deps.storage, budget_r)?
            .unwrap_or_default()
    } else if let Some(skills) = skill_tags {
        if let Some(first_skill) = skills.first() {
            JOBS_BY_SKILL
                .may_load(deps.storage, *first_skill)?
                .unwrap_or_default()
        } else {
            vec![]
        }
    } else {
        // Fall back to iterating all active jobs
        ACTIVE_JOBS
            .range(deps.storage, None, None, Order::Descending)
            .map(|item| item.map(|(job_id, _)| job_id))
            .collect::<StdResult<Vec<_>>>()?
    };

    for job_id in candidate_jobs.iter().take(limit) {
        if let Ok(job) = JOBS.load(deps.storage, *job_id) {
            // Apply additional filters
            if let Some(remote) = is_remote {
                if job.is_remote != remote {
                    continue;
                }
            }
            if let Some(exp_level) = experience_level {
                if job.experience_level != exp_level {
                    continue;
                }
            }

            // Get off-chain key
            let entity_key = format!("job_{}", job_id);
            let off_chain_key = ENTITY_TO_HASH
                .load(deps.storage, &entity_key)
                .unwrap_or_default();

            results.push(job_to_hash_aware_response(&job, off_chain_key));
        }
    }

    Ok(results)
}

/// 🗂️ Get content hash by entity type and ID
pub fn get_content_hash_for_entity(
    deps: Deps,
    entity_type: &str,
    entity_id: &str,
) -> StdResult<Option<ContentHash>> {
    let entity_key = format!("{}_{}", entity_type, entity_id);
    if let Ok(hash_str) = ENTITY_TO_HASH.load(deps.storage, &entity_key) {
        CONTENT_HASHES.may_load(deps.storage, &hash_str)
    } else {
        Ok(None)
    }
}

/// 🔗 Resolve hash to off-chain data reference
pub fn resolve_hash_to_reference(deps: Deps, hash: &str) -> StdResult<Option<String>> {
    HASH_TO_ENTITY.may_load(deps.storage, hash)
}
