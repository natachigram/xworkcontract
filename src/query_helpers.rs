use crate::contract_helpers::*;
use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;
use cosmwasm_std::{StdResult, Deps, Order, Uint128};
use cw_storage_plus::Bound;

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

/// Platform statistics calculation
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
    let total_users = count_items_with_filter(
        deps.storage,
        &USER_PROFILES,
        |_| true,
    )?;

    // Calculate total value locked (from escrows)
    let mut total_value_locked = Uint128::zero();
    let escrows: StdResult<Vec<_>> = ESCROWS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(escrow_pairs) = escrows {
        for (_, escrow) in escrow_pairs {
            if escrow.status == EscrowStatus::Pending {
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
                if &job.category != filter_category {
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

            if let Some(ref filter_skills) = skills_required {
                let has_skill = filter_skills.iter().any(|skill| {
                    job.skills_required.iter().any(|job_skill| {
                        job_skill.to_lowercase().contains(&skill.to_lowercase())
                    })
                });
                if !has_skill {
                    include = false;
                }
            }

            if let Some(ref filter_job_type) = job_type {
                if job.job_type.as_ref() != Some(filter_job_type) {
                    include = false;
                }
            }

            if let Some(filter_remote) = remote_allowed {
                if job.remote_allowed != Some(filter_remote) {
                    include = false;
                }
            }

            if let Some(ref filter_exp_level) = experience_level {
                if job.experience_level.as_ref() != Some(filter_exp_level) {
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
                if &bounty.creator != filter_creator {
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
                    bounty.skills_required.iter().any(|bounty_skill| {
                        bounty_skill.to_lowercase().contains(&skill.to_lowercase())
                    })
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
                    let matches = job.title.to_lowercase().contains(&query_lower) ||
                                  job.description.to_lowercase().contains(&query_lower) ||
                                  job.category.to_lowercase().contains(&query_lower) ||
                                  job.skills_required.iter().any(|skill| {
                                      skill.to_lowercase().contains(&query_lower)
                                  });

                    if matches {
                        jobs.push(job);
                        if jobs.len() >= limit / 2 {
                            break;
                        }
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
                    let matches = bounty.title.to_lowercase().contains(&query_lower) ||
                                  bounty.description.to_lowercase().contains(&query_lower) ||
                                  bounty.category.to_lowercase().contains(&query_lower) ||
                                  bounty.skills_required.iter().any(|skill| {
                                      skill.to_lowercase().contains(&query_lower)
                                  });

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
        job_pairs.sort_by(|a, b| b.1.proposal_count.cmp(&a.1.proposal_count));
        
        for (_, job) in job_pairs.into_iter().take(10) {
            if job.status == JobStatus::Open && job.proposal_count > 0 {
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
        bounty_pairs.sort_by(|a, b| b.1.submission_count.cmp(&a.1.submission_count));
        
        for (_, bounty) in bounty_pairs.into_iter().take(10) {
            if bounty.status == BountyStatus::Open && bounty.submission_count > 0 {
                popular_bounties.push(bounty);
            }
        }
    }

    Ok(TrendingResponse {
        popular_jobs,
        popular_bounties,
    })
}

/// Get categories with job/bounty counts
pub fn query_categories(deps: Deps) -> StdResult<CategoriesResponse> {
    let mut job_categories: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut bounty_categories: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    // Count job categories
    let job_items: StdResult<Vec<_>> = JOBS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    if let Ok(job_pairs) = job_items {
        for (_, job) in job_pairs {
            if job.status == JobStatus::Open {
                *job_categories.entry(job.category).or_insert(0) += 1;
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
                *bounty_categories.entry(bounty.category).or_insert(0) += 1;
            }
        }
    }

    // Convert to sorted vectors
    let mut job_cats: Vec<_> = job_categories.into_iter().collect();
    job_cats.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    let mut bounty_cats: Vec<_> = bounty_categories.into_iter().collect();
    bounty_cats.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    Ok(CategoriesResponse {
        job_categories: job_cats,
        bounty_categories: bounty_cats,
    })
}
