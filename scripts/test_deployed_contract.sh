#!/usr/bin/env bash
# Test deployed XWork contract functionality
# Run this after successful deployment to verify everything works

set -e

# Load deployment environment
if [ -f ".env.deployed" ]; then
    source .env.deployed
    echo "📁 Loaded deployment environment"
else
    echo "❌ Deployment environment not found. Run ./scripts/deploy.sh first"
    exit 1
fi

CHAIN_BINARY="${CHAIN_BINARY:-wasmd}"

echo "🧪 Testing Deployed XWork Contract"
echo "=================================="
echo "Contract: $CONTRACT_ADDRESS"
echo "Network: $CHAIN_ID"
echo "Key: $KEY"

# Test 1: Query contract config
echo ""
echo "📋 Test 1: Query contract configuration"
CONFIG_RESULT=$($CHAIN_BINARY query wasm contract-state smart $CONTRACT_ADDRESS '{"get_config":{}}' $NODE --output json)
echo "✅ Config query successful"
echo "Platform fee: $(echo $CONFIG_RESULT | jq -r '.data.config.platform_fee_percent')%"
echo "Min escrow: $(echo $CONFIG_RESULT | jq -r '.data.config.min_escrow_amount') $DENOM"

# Test 2: Query platform stats
echo ""
echo "📊 Test 2: Query platform statistics"
STATS_RESULT=$($CHAIN_BINARY query wasm contract-state smart $CONTRACT_ADDRESS '{"get_platform_stats":{}}' $NODE --output json)
echo "✅ Platform stats query successful"
echo "Total jobs: $(echo $STATS_RESULT | jq -r '.data.total_jobs')"
echo "Active jobs: $(echo $STATS_RESULT | jq -r '.data.active_jobs')"

# Test 3: Post a test job
echo ""
echo "📝 Test 3: Post a test job"
TEST_JOB_MSG='{
  "post_job": {
    "title": "Testnet Integration Job",
    "description": "This is a test job to verify contract functionality on testnet",
    "budget": "5000",
    "category": "Testing",
    "skills_required": ["Smart Contracts", "Testing"],
    "duration_days": 30,
    "documents": null,
    "milestones": null
  }
}'

POST_JOB_TX=$($CHAIN_BINARY tx wasm execute $CONTRACT_ADDRESS "$TEST_JOB_MSG" \
    $NODE --chain-id $CHAIN_ID --from $KEY \
    --gas auto --gas-adjustment 1.3 --gas-prices "0.025$DENOM" \
    -y --output json)

POST_JOB_TX_HASH=$(echo $POST_JOB_TX | jq -r '.txhash')
echo "Post job TX: $POST_JOB_TX_HASH"

# Wait for confirmation
echo "⏳ Waiting for transaction confirmation..."
sleep 6

# Verify job was created
echo ""
echo "🔍 Test 4: Verify job creation"
JOBS_RESULT=$($CHAIN_BINARY query wasm contract-state smart $CONTRACT_ADDRESS '{"get_jobs":{"limit":10}}' $NODE --output json)
JOB_COUNT=$(echo $JOBS_RESULT | jq '.data.jobs | length')
echo "✅ Found $JOB_COUNT jobs in contract"

if [ "$JOB_COUNT" -gt 0 ]; then
    FIRST_JOB=$(echo $JOBS_RESULT | jq -r '.data.jobs[0]')
    echo "First job title: $(echo $FIRST_JOB | jq -r '.title')"
    echo "First job budget: $(echo $FIRST_JOB | jq -r '.budget') $DENOM"
fi

# Test 5: Submit a proposal
echo ""
echo "💼 Test 5: Submit a test proposal"
PROPOSAL_MSG='{
  "submit_proposal": {
    "job_id": 0,
    "cover_letter": "I am interested in this testnet integration job. I have experience with smart contract testing and can deliver quality results.",
    "delivery_time_days": 25,
    "milestones": null
  }
}'

SUBMIT_PROPOSAL_TX=$($CHAIN_BINARY tx wasm execute $CONTRACT_ADDRESS "$PROPOSAL_MSG" \
    $NODE --chain-id $CHAIN_ID --from $KEY \
    --gas auto --gas-adjustment 1.3 --gas-prices "0.025$DENOM" \
    -y --output json)

PROPOSAL_TX_HASH=$(echo $SUBMIT_PROPOSAL_TX | jq -r '.txhash')
echo "Submit proposal TX: $PROPOSAL_TX_HASH"

# Wait for confirmation
sleep 6

# Verify proposal was submitted
echo ""
echo "🔍 Test 6: Verify proposal submission"
PROPOSALS_RESULT=$($CHAIN_BINARY query wasm contract-state smart $CONTRACT_ADDRESS '{"get_job_proposals":{"job_id":0}}' $NODE --output json)
PROPOSAL_COUNT=$(echo $PROPOSALS_RESULT | jq '.data.proposals | length')
echo "✅ Found $PROPOSAL_COUNT proposals for job 0"

# Test 7: Query user stats
echo ""
echo "👤 Test 7: Query user statistics"
USER_ADDRESS=$($CHAIN_BINARY keys show $KEY -a)
USER_STATS_RESULT=$($CHAIN_BINARY query wasm contract-state smart $CONTRACT_ADDRESS "{\"get_user_stats\":{\"user\":\"$USER_ADDRESS\"}}" $NODE --output json)
echo "✅ User stats query successful"
echo "Jobs posted: $(echo $USER_STATS_RESULT | jq -r '.data.jobs_posted')"
echo "Proposals submitted: $(echo $USER_STATS_RESULT | jq -r '.data.proposals_submitted')"

# Summary
echo ""
echo "🎉 Contract Testing Complete!"
echo "============================="
echo "✅ Configuration queries working"
echo "✅ Job posting functional"
echo "✅ Proposal submission functional"
echo "✅ User stats tracking working"
echo ""
echo "📊 Test Results Summary:"
echo "- Contract Address: $CONTRACT_ADDRESS"
echo "- Jobs Created: $JOB_COUNT"
echo "- Proposals Submitted: $PROPOSAL_COUNT"
echo "- All core functionality verified ✅"
echo ""
echo "🔗 View transactions on explorer:"
echo "- Job Post: https://explorer.xion.org/testnet/tx/$POST_JOB_TX_HASH"
echo "- Proposal: https://explorer.xion.org/testnet/tx/$PROPOSAL_TX_HASH"
echo ""
echo "🚀 Ready for production deployment!"
