# XWork Smart Contract API Documentation

## Table of Contents
- [Execute Messages](#execute-messages)
- [Query Messages](#query-messages)
- [Response Types](#response-types)
- [Error Codes](#error-codes)
- [Integration Examples](#integration-examples)

## Execute Messages

### Job Management

#### PostJob
Post a new freelance job to the platform.

```json
{
  "post_job": {
    "title": "string",                    // Job title (5-100 chars)
    "description": "string",              // Job description (10-2000 chars)
    "budget": "string",                   // Budget in uxion (e.g., "5000000000")
    "category": "string",                 // Job category (1-50 chars)
    "skills_required": ["string"],        // Required skills (1-20 items)
    "duration_days": number,              // Project duration (1-365 days)
    "documents": ["string"],              // Optional: Document URLs/IPFS hashes
    "milestones": [                       // Optional: Project milestones
      {
        "title": "string",                // Milestone title (1-100 chars)
        "description": "string",          // Milestone description (max 500 chars)
        "amount": "string",               // Milestone payment amount
        "deadline_days": number           // Days from job start
      }
    ]
  }
}
```

**Rate Limit**: 5 jobs per day per user  
**Security**: Reentrancy protected, input sanitized

#### EditJob
Edit an existing job (only by job poster, only if status is "Open").

```json
{
  "edit_job": {
    "job_id": number,
    "title": "string",                    // Optional: New title
    "description": "string",              // Optional: New description
    "budget": "string",                   // Optional: New budget
    "category": "string",                 // Optional: New category
    "skills_required": ["string"],        // Optional: New skills
    "duration_days": number,              // Optional: New duration
    "documents": ["string"],              // Optional: New documents
    "milestones": [...]                   // Optional: New milestones
  }
}
```

#### CancelJob
Cancel a job (only by job poster).

```json
{
  "cancel_job": {
    "job_id": number
  }
}
```

#### DeleteJob
Delete a job (only by job poster, only if no proposals).

```json
{
  "delete_job": {
    "job_id": number
  }
}
```

### Proposal Management

#### SubmitProposal
Submit a proposal for a job.

```json
{
  "submit_proposal": {
    "job_id": number,
    "bid_amount": "string",               // Bid amount in uxion
    "cover_letter": "string",             // Cover letter (10-2000 chars)
    "delivery_time_days": number,         // Proposed delivery time (1-365 days)
    "milestones": [                       // Optional: Proposal milestones
      {
        "title": "string",
        "description": "string",
        "amount": "string",
        "deadline_days": number
      }
    ]
  }
}
```

**Rate Limit**: 20 proposals per day per user  
**Restrictions**: Cannot propose on own jobs

#### EditProposal
Edit an existing proposal (only by proposer, only if not accepted).

```json
{
  "edit_proposal": {
    "proposal_id": number,
    "bid_amount": "string",               // Optional: New bid amount
    "cover_letter": "string",             // Optional: New cover letter
    "delivery_time_days": number,         // Optional: New delivery time
    "milestones": [...]                   // Optional: New milestones
  }
}
```

#### WithdrawProposal
Withdraw a proposal (only by proposer, only if not accepted).

```json
{
  "withdraw_proposal": {
    "proposal_id": number
  }
}
```

#### AcceptProposal
Accept a proposal for a job (only by job poster).

```json
{
  "accept_proposal": {
    "job_id": number,
    "proposal_id": number
  }
}
```

### Escrow Management

#### CreateEscrow
Create escrow for an accepted job.

```json
{
  "create_escrow": {
    "job_id": number
  }
}
```

**Payment Required**: Must send exact job budget amount in uxion  
**Security**: Reentrancy protected, amount validation

#### ReleaseEscrow
Release escrow funds to freelancer.

```json
{
  "release_escrow": {
    "escrow_id": "string"
  }
}
```

**Authorization**: Job poster or auto-release after dispute period

#### RefundEscrow
Refund escrow to client (admin only, for dispute resolution).

```json
{
  "refund_escrow": {
    "escrow_id": "string"
  }
}
```

**Authorization**: Admin only

### Work Management

#### SubmitWork
Submit completed work for a job.

```json
{
  "submit_work": {
    "job_id": number,
    "deliverables": ["string"],           // URLs/IPFS hashes of deliverables
    "notes": "string"                     // Optional: Additional notes
  }
}
```

#### RequestRevision
Request revision of submitted work.

```json
{
  "request_revision": {
    "job_id": number,
    "feedback": "string",                 // Revision feedback (10-1000 chars)
    "deadline_extension_days": number     // Optional: Additional time
  }
}
```

#### ApproveWork
Approve submitted work.

```json
{
  "approve_work": {
    "job_id": number
  }
}
```

### Dispute Management

#### RaiseDispute
Raise a dispute for a job.

```json
{
  "raise_dispute": {
    "job_id": number,
    "reason": "string",                   // Dispute reason (1-1000 chars)
    "evidence": ["string"]                // Evidence URLs/IPFS hashes
  }
}
```

**Authorization**: Job poster or assigned freelancer

#### ResolveDispute
Resolve a dispute (admin only).

```json
{
  "resolve_dispute": {
    "dispute_id": "string",
    "resolution": "string",               // Resolution details (1-2000 chars)
    "release_to_freelancer": boolean      // true = release funds, false = refund
  }
}
```

### Rating System

#### SubmitRating
Submit a rating for completed work.

```json
{
  "submit_rating": {
    "job_id": number,
    "rating": number,                     // 1-5 stars
    "comment": "string"                   // Rating comment (max 500 chars)
  }
}
```

### Admin Functions

#### UpdateConfig
Update contract configuration (admin only).

```json
{
  "update_config": {
    "admin": "string",                    // Optional: New admin address
    "platform_fee_percent": number,      // Optional: New fee (max 10%)
    "min_escrow_amount": "string",        // Optional: New minimum escrow
    "dispute_period_days": number,        // Optional: New dispute period
    "max_job_duration_days": number       // Optional: New max duration
  }
}
```

#### PauseContract
Pause contract operations (admin only).

```json
{
  "pause_contract": {}
}
```

#### UnpauseContract
Resume contract operations (admin only).

```json
{
  "unpause_contract": {}
}
```

## Query Messages

### Job Queries

#### Jobs
Get all jobs with pagination.

```json
{
  "jobs": {
    "start_after": number,                // Optional: Start after job_id
    "limit": number,                      // Optional: Max results (default 30)
    "status_filter": "string",            // Optional: "Open", "InProgress", "Completed", "Cancelled", "Disputed"
    "category_filter": "string",          // Optional: Filter by category
    "poster_filter": "string"             // Optional: Filter by poster address
  }
}
```

**Response**: `JobsResponse`

#### Job
Get specific job details.

```json
{
  "job": {
    "job_id": number
  }
}
```

**Response**: `JobResponse`

#### JobProposals
Get all proposals for a specific job.

```json
{
  "job_proposals": {
    "job_id": number,
    "start_after": number,                // Optional: Start after proposal_id
    "limit": number                       // Optional: Max results (default 30)
  }
}
```

**Response**: `ProposalsResponse`

### Proposal Queries

#### Proposals
Get all proposals with pagination.

```json
{
  "proposals": {
    "start_after": number,                // Optional: Start after proposal_id
    "limit": number,                      // Optional: Max results (default 30)
    "freelancer_filter": "string",        // Optional: Filter by freelancer
    "job_filter": number                  // Optional: Filter by job_id
  }
}
```

**Response**: `ProposalsResponse`

#### Proposal
Get specific proposal details.

```json
{
  "proposal": {
    "proposal_id": number
  }
}
```

**Response**: `ProposalResponse`

#### UserProposals
Get all proposals by a specific user.

```json
{
  "user_proposals": {
    "user": "string",                     // User address
    "start_after": number,                // Optional: Start after proposal_id
    "limit": number                       // Optional: Max results (default 30)
  }
}
```

**Response**: `ProposalsResponse`

### Escrow Queries

#### Escrow
Get escrow details.

```json
{
  "escrow": {
    "escrow_id": "string"
  }
}
```

**Response**: `EscrowResponse`

#### JobEscrow
Get escrow for a specific job.

```json
{
  "job_escrow": {
    "job_id": number
  }
}
```

**Response**: `EscrowResponse`

### User Queries

#### UserStats
Get user statistics.

```json
{
  "user_stats": {
    "user": "string"                      // User address
  }
}
```

**Response**: `UserStatsResponse`

#### UserRatings
Get ratings for a user.

```json
{
  "user_ratings": {
    "user": "string",                     // User address
    "start_after": "string",              // Optional: Start after rating_id
    "limit": number                       // Optional: Max results (default 30)
  }
}
```

**Response**: `RatingsResponse`

### Platform Queries

#### Config
Get contract configuration.

```json
{
  "config": {}
}
```

**Response**: `ConfigResponse`

#### PlatformStats
Get platform-wide statistics.

```json
{
  "platform_stats": {}
}
```

**Response**: `PlatformStatsResponse`

#### Disputes
Get disputes with pagination.

```json
{
  "disputes": {
    "start_after": "string",              // Optional: Start after dispute_id
    "limit": number,                      // Optional: Max results (default 30)
    "status_filter": "string",            // Optional: Filter by status
    "job_filter": number                  // Optional: Filter by job_id
  }
}
```

**Response**: `DisputesResponse`

## Response Types

### JobResponse
```json
{
  "id": number,
  "poster": "string",
  "title": "string",
  "description": "string",
  "budget": "string",
  "category": "string",
  "skills_required": ["string"],
  "duration_days": number,
  "documents": ["string"],
  "status": "string",
  "assigned_freelancer": "string",
  "created_at": "number",
  "updated_at": "number",
  "deadline": "number",
  "milestones": [...],
  "escrow_id": "string",
  "total_proposals": number
}
```

### ProposalResponse
```json
{
  "id": number,
  "freelancer": "string",
  "job_id": number,
  "bid_amount": "string",
  "cover_letter": "string",
  "delivery_time_days": number,
  "submitted_at": "number",
  "milestones": [...]
}
```

### EscrowResponse
```json
{
  "id": "string",
  "job_id": number,
  "client": "string",
  "freelancer": "string",
  "amount": "string",
  "platform_fee": "string",
  "funded_at": "number",
  "released": boolean,
  "dispute_status": "string",
  "dispute_raised_at": "number",
  "dispute_deadline": "number"
}
```

### UserStatsResponse
```json
{
  "total_jobs_posted": number,
  "total_jobs_completed": number,
  "total_earned": "string",
  "total_spent": "string",
  "average_rating": number,
  "total_ratings": number,
  "completion_rate": number
}
```

### ConfigResponse
```json
{
  "admin": "string",
  "platform_fee_percent": number,
  "min_escrow_amount": "string",
  "dispute_period_days": number,
  "max_job_duration_days": number,
  "paused": boolean
}
```

## Error Codes

| Error | Description |
|-------|-------------|
| `Unauthorized` | Insufficient permissions |
| `ContractPaused` | Contract is paused |
| `JobNotFound` | Job ID doesn't exist |
| `ProposalNotFound` | Proposal ID doesn't exist |
| `EscrowNotFound` | Escrow ID doesn't exist |
| `EscrowAlreadyExists` | Escrow already created for job |
| `InvalidInput` | Input validation failed |
| `InsufficientFunds` | Payment amount too low |
| `RateLimitExceeded` | Daily action limit reached |
| `ReentrancyGuard` | Reentrancy attack detected |
| `DisputePeriodActive` | Cannot release during dispute |
| `PlatformFeeTooHigh` | Fee exceeds 10% maximum |

## Integration Examples

### Frontend Integration (JavaScript)

```javascript
import { CosmWasmClient, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";

// Query contract
const client = await CosmWasmClient.connect("https://rpc.xion-testnet-1.burnt.com:443");
const jobs = await client.queryContractSmart(contractAddress, {
  jobs: { limit: 10 }
});

// Execute transaction
const signingClient = await SigningCosmWasmClient.connectWithSigner(
  "https://rpc.xion-testnet-1.burnt.com:443",
  signer
);

const msg = {
  post_job: {
    title: "Smart Contract Development",
    description: "Need a CosmWasm contract",
    budget: "5000000000",
    category: "Development",
    skills_required: ["Rust", "CosmWasm"],
    duration_days: 30
  }
};

const result = await signingClient.execute(
  senderAddress,
  contractAddress,
  msg,
  "auto",
  undefined,
  [{ denom: "uxion", amount: "5000000000" }]
);
```

### React Hook Example

```typescript
import { useQuery } from '@tanstack/react-query';
import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate';

const useJobs = (contractAddress: string) => {
  return useQuery({
    queryKey: ['jobs', contractAddress],
    queryFn: async () => {
      const client = await CosmWasmClient.connect(rpcUrl);
      return client.queryContractSmart(contractAddress, {
        jobs: { limit: 50 }
      });
    }
  });
};

const useJobDetails = (contractAddress: string, jobId: number) => {
  return useQuery({
    queryKey: ['job', contractAddress, jobId],
    queryFn: async () => {
      const client = await CosmWasmClient.connect(rpcUrl);
      return client.queryContractSmart(contractAddress, {
        job: { job_id: jobId }
      });
    },
    enabled: !!jobId
  });
};
```

### Python Integration

```python
from cosmpy.aerial.client import LedgerClient
from cosmpy.aerial.wallet import LocalWallet
from cosmpy.crypto.keypairs import PrivateKey

# Setup client
private_key = PrivateKey.from_string("your_private_key")
wallet = LocalWallet(private_key)
client = LedgerClient("https://rpc.xion-testnet-1.burnt.com:443")

# Query jobs
jobs_query = {"jobs": {"limit": 10}}
result = client.query_contract_smart(contract_address, jobs_query)

# Post job
post_job_msg = {
    "post_job": {
        "title": "Python Development",
        "description": "Need a Python script for data analysis",
        "budget": "1000000000",
        "category": "Development",
        "skills_required": ["Python", "Data Analysis"],
        "duration_days": 14
    }
}

tx = client.execute_contract(
    wallet.address(),
    contract_address,
    post_job_msg,
    coins=[{"denom": "uxion", "amount": "1000000000"}]
)
```

For more detailed examples and integration patterns, see the [examples](../examples/) directory.
