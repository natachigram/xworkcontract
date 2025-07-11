#!/usr# Configuration - modify these for your target network
CHAIN_BINARY="${CHAIN_BINARY:-xiond}"
CHAIN_ID="${CHAIN_ID:-xion-testnet-1}"
NODE="${NODE:---node tcp://testnet-rpc.xion.org:26657}"
KEY="${KEY:-benchmark}"
WASM_PATH="./artifacts/xworks_freelance_contract.wasm"
DENOM="${DENOM:-uxion}"

echo "🔗 Connecting to: $CHAIN_ID"
echo "🌐 Node: $NODE"
echo "🔑 Using key: $KEY"
echo "💰 Denomination: $DENOM"h
# Benchmark gas usage for key contract entrypoints on testnet or local chain
# Prerequisites: wasmd installed, and a key with sufficient funds

set -eo pipefail

# Configuration - modify these for your target network
CHAIN_BINARY="xiond"
CHAIN_ID="${CHAIN_ID:-xion-testnet-1}"  # Default to Xion testnet
NODE="${NODE:---node https://testnet-rpc.xion.org:443}"  # Default to Xion testnet RPC
KEY="${KEY:-benchmark}"  # Default key name
WASM_PATH="./artifacts/xworks_freelance_contract.wasm"
DENOM="${DENOM:-uxion}"  # Default denomination

echo "🔗 Connecting to: $CHAIN_ID"
echo "🌐 Node: $NODE"
echo "🔑 Using key: $KEY"
echo "💰 Denomination: $DENOM"

# Utility: send tx and print gas used
function send_and_print_gas() {
  local CONTRACT_ADDR=$1
  local MSG=$2
  local AMOUNT=$3
  echo -e "\n--- Executing: $MSG ---"
  
  if [ ! -z "$AMOUNT" ]; then
    TX_OUT=$($CHAIN_BINARY tx wasm execute $CONTRACT_ADDR "$MSG" $NODE --chain-id $CHAIN_ID --gas auto --gas-adjustment 1.3 --gas-prices "0.025$DENOM" --amount "$AMOUNT" --from $KEY -y --output json)
  else
    TX_OUT=$($CHAIN_BINARY tx wasm execute $CONTRACT_ADDR "$MSG" $NODE --chain-id $CHAIN_ID --gas auto --gas-adjustment 1.3 --gas-prices "0.025$DENOM" --from $KEY -y --output json)
  fi
  
  echo "Gas used: $(echo $TX_OUT | jq -r '.gas_used // "N/A"')"
  echo "Gas wanted: $(echo $TX_OUT | jq -r '.gas_wanted // "N/A"')"
  echo "TX Hash: $(echo $TX_OUT | jq -r '.txhash // "N/A"')"
}

# Check if key exists and has funds
echo "💳 Checking account balance..."
BALANCE=$($CHAIN_BINARY query bank balances $($CHAIN_BINARY keys show $KEY -a) $NODE --output json 2>/dev/null || echo '{"balances":[]}')
echo "Balance: $(echo $BALANCE | jq -r '.balances[] | select(.denom=="'$DENOM'") | .amount // "0"') $DENOM"

# 1. Store contract
echo -e "\n📦 Storing contract..."
STORE_OUT=$($CHAIN_BINARY tx wasm store $WASM_PATH $NODE --chain-id $CHAIN_ID --from $KEY --gas auto --gas-adjustment 1.5 --gas-prices "0.025$DENOM" -y --output json)
if [ $? -ne 0 ]; then
  echo "❌ Failed to store contract"
  exit 1
fi

CODE_ID=$(echo $STORE_OUT | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key=="code_id") | .value')
echo "✅ Code ID: $CODE_ID"

# 2. Instantiate contract
echo -e "\n🚀 Instantiating contract..."
ADMIN_ADDR=$($CHAIN_BINARY keys show $KEY -a)
INSTANTIATE_OUT=$($CHAIN_BINARY tx wasm instantiate $CODE_ID '{"admin": "'$ADMIN_ADDR'", "platform_fee_percent":5, "min_escrow_amount":"1000", "dispute_period_days":7, "max_job_duration_days":365}' --label "xwork-testnet" $NODE --chain-id $CHAIN_ID --from $KEY --gas auto --gas-adjustment 1.5 --gas-prices "0.025$DENOM" -y --output json)
if [ $? -ne 0 ]; then
  echo "❌ Failed to instantiate contract"
  exit 1
fi

CONTRACT_ADDR=$(echo $INSTANTIATE_OUT | jq -r '.logs[0].events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value')
echo "✅ Contract Address: $CONTRACT_ADDR"

# Benchmark entrypoints
echo -e "\n🔬 Starting gas benchmarking..."

# 3. PostJob (paid)
echo -e "\n📋 Benchmark: PostJob (paid)"
send_and_print_gas "$CONTRACT_ADDR" '{"post_job":{"title":"Bench Job","description":"Benchmarking gas usage","budget":"5000","category":"Dev","skills_required":["Rust"],"duration_days":30,"documents":null,"milestones":null}}'

# 4. PostJob (free)
echo -e "\n📋 Benchmark: PostJob (free)"
send_and_print_gas "$CONTRACT_ADDR" '{"post_job":{"title":"Bench Free","description":"Free job","budget":"0","category":"Dev","skills_required":["Rust"],"duration_days":30,"documents":null,"milestones":null}}'

# 5. SubmitProposal
echo -e "\n💼 Benchmark: SubmitProposal"
send_and_print_gas "$CONTRACT_ADDR" '{"submit_proposal":{"job_id":0,"cover_letter":"Benchmark proposal","delivery_time_days":25,"milestones":null}}'

# 6. AcceptProposal (accept the first proposal for the first job)
echo -e "\n✅ Benchmark: AcceptProposal"
send_and_print_gas "$CONTRACT_ADDR" '{"accept_proposal":{"job_id":0,"proposal_id":0}}'

# 7. CreateEscrow (for the paid job)
echo -e "\n💰 Benchmark: CreateEscrow"
send_and_print_gas "$CONTRACT_ADDR" '{"create_escrow":{"job_id":0}}' "5000$DENOM"

# 8. CompleteJob
echo -e "\n🎯 Benchmark: CompleteJob"
send_and_print_gas "$CONTRACT_ADDR" '{"complete_job":{"job_id":0}}'

# Summary
echo -e "\n📊 Gas benchmarking completed!"
echo "Contract deployed at: $CONTRACT_ADDR"
echo "View on explorer: https://explorer.xion.org/testnet/account/$CONTRACT_ADDR"
