# XWork Smart Contract - Frontend Integration

**Contract Address**: `xion1nvx9jzdgddnk4vajjhruz3ta20l656459auntt`  
**Denomination**: `uusdc`

## Execute Functions (State-Changing)

### Job Management
- **`PostJob`** - Create a new job posting (requires payment for paid jobs)
- **`EditJob`** - Modify an existing job (only by job poster)
- **`DeleteJob`** - Remove a job posting (only by job poster)
- **`CancelJob`** - Cancel a job (alternative to delete)

### Proposal System
- **`SubmitProposal`** - Submit a proposal for a job
- **`AcceptProposal`** - Accept a freelancer's proposal (creates escrow)
- **`EditProposal`** - Modify your submitted proposal
- **`WithdrawProposal`** - Remove your proposal from a job

### Work Management
- **`CompleteJob`** - Mark job as completed (releases escrow)
- **`CompleteMilestone`** - Mark a milestone as done
- **`ApproveMilestone`** - Approve completed milestone (releases partial payment)

### Bounty System
- **`CreateBounty`** - Create a bounty with reward pool (requires payment)
- **`EditBounty`** - Modify bounty details (only by creator)
- **`CancelBounty`** - Cancel bounty and refund reward pool
- **`SubmitToBounty`** - Submit entry to a bounty (title, description, deliverables)
- **`EditBountySubmission`** - Edit your bounty submission
- **`WithdrawBountySubmission`** - Remove your bounty submission
- **`ReviewBountySubmission`** - Review and score submissions (by bounty creator)
- **`SelectBountyWinners`** - Choose winners by submission ID and position
- **`CreateBountyEscrow`** - Create escrow for bounty rewards
- **`ReleaseBountyRewards`** - Release rewards to winners

### User Management
- **`UpdateUserProfile`** - Set profile info (name, bio, skills, etc.)
- **`SubmitRating`** - Rate another user after job completion

### Escrow & Payments
- **`CreateEscrow`** - Create escrow for a job
- **`CreateEscrowNative`** - Create native token escrow
- **`CreateEscrowCw20`** - Create CW20 token escrow
- **`FundEscrow`** - **DEPRECATED** (use CreateEscrowNative or CreateEscrowCw20)
- **`ReleaseEscrow`** - Release payment to freelancer
- **`RefundEscrow`** - Refund payment to client

### Disputes
- **`RaiseDispute`** - Start a dispute for a job
- **`ResolveDispute`** - Admin resolves dispute and releases funds

### Admin Functions
- **`UpdateConfig`** - Update platform settings (admin only)
- **`PauseContract`** - Pause platform (admin only)
- **`UnpauseContract`** - Resume platform (admin only)

### Security Functions
- **`BlockAddress`** - Block a user (admin only)
- **`UnblockAddress`** - Unblock a user (admin only)
- **`ResetRateLimit`** - Reset rate limit for an address (admin only)

## Query Functions (Read-Only)

### Job Queries
- **`GetJob`** - Returns single job details by ID
- **`GetJobs`** - Returns filtered list of jobs (with pagination)
- **`GetAllJobs`** - Returns active jobs for homepage/landing page
- **`GetUserJobs`** - Returns jobs posted by specific user

### Proposal Queries
- **`GetProposal`** - Returns single proposal details
- **`GetJobProposals`** - Returns all proposals for a job
- **`GetUserProposals`** - Returns proposals submitted by user

### Bounty Queries
- **`GetBounty`** - Returns single bounty details
- **`GetBounties`** - Returns filtered list of bounties
- **`GetAllBounties`** - Returns active bounties for homepage/landing page
- **`GetUserBounties`** - Returns bounties created by user
- **`GetBountySubmission`** - Returns single bounty submission details
- **`GetBountySubmissions`** - Returns submissions for a bounty
- **`GetUserBountySubmissions`** - Returns user's bounty submissions

### User Queries
- **`GetUserStats`** - Returns user statistics (jobs, earnings, ratings)
- **`GetUserRatings`** - Returns ratings received by user
- **`GetJobRating`** - Returns rating for specific job and rater

### Escrow Queries
- **`GetEscrow`** - Returns escrow details by ID
- **`GetJobEscrow`** - Returns escrow for specific job

### Platform Queries
- **`GetConfig`** - Returns platform configuration
- **`GetPlatformStats`** - Returns platform statistics

### Dispute Queries
- **`GetDispute`** - Returns dispute details
- **`GetJobDisputes`** - Returns disputes for a job
- **`GetUserDisputes`** - Returns disputes involving user

### Security Queries
- **`GetSecurityMetrics`** - Returns security metrics (admin only)
- **`GetAuditLogs`** - Returns audit logs (admin only)
- **`IsAddressBlocked`** - Check if address is blocked
- **`GetRateLimitStatus`** - Get rate limit status for address

## Function Parameters

### Common Job Parameters
```
title: string
description: string  
budget: number (in uusdc)
category: string
skills_required: string[]
duration_days: number
documents?: string[] (optional)
milestones?: milestone[] (optional)
```

### Common Bounty Parameters
```
title: string
description: string
requirements: string[]
total_reward: number (in uusdc)
category: string
skills_required: string[]
submission_deadline_days: number
review_period_days: number
max_winners: number
reward_distribution: reward_tier[]
documents?: string[] (optional)
```

### Common Query Filters
```
start_after?: number (for pagination)
limit?: number (default 50)
category?: string
status?: string
poster/creator?: string (wallet address)
```

## Response Types

### Job Object
```
id, poster, title, description, budget, category, skills_required, 
status, deadline, created_at, updated_at, total_proposals, etc.
```

### Bounty Object
```
id, poster, title, description, requirements, total_reward, category,
status, submission_deadline, total_submissions, selected_winners, etc.
```

### User Stats
```
total_jobs_posted, total_jobs_completed, total_earned, total_spent,
average_rating, total_ratings, completion_rate
```

### Platform Stats
```
total_jobs, active_jobs, completed_jobs, total_users, 
total_volume, platform_fees_collected
```

## Status Values

**Job Status**: `"Open"`, `"InProgress"`, `"Completed"`, `"Cancelled"`, `"Disputed"`  
**Bounty Status**: `"Open"`, `"Completed"`, `"Cancelled"`  
**Submission Status**: `"Submitted"`, `"UnderReview"`, `"Accepted"`, `"Rejected"`

## Payment Notes

- All amounts are in `uusdc` (USDC micro-units: 1 USDC = 1,000,000 uusdc)
- Paid jobs require sending funds with the transaction
- Platform takes a configurable fee (default 5%)
- Escrow holds funds until job completion or dispute resolution
