#!/bin/bash
# XWork Contract Deployment Script - Updated for Testnet Integration
# This script handles the complete deployment process with proper testnet support

set -e

# Load environment if available
if [ -f ".env.testnet" ]; then
    source .env.testnet
    echo "📁 Loaded testnet environment"
fi

# Configuration
CONTRACT_NAME="xworks-freelance-contract"
NETWORK=${1:-"testnet"}  # testnet or mainnet
ADMIN_KEY=${2:-"${KEY:-benchmark}"}
CHAIN_BINARY="${CHAIN_BINARY:-wasmd}"

# Network configurations
case $NETWORK in
  "testnet")
    CHAIN_ID="${CHAIN_ID:-xion-testnet-1}"
    RPC_URL="${NODE#--node }"  # Remove --node prefix if present
    RPC_URL="${RPC_URL:-https://testnet-rpc.xion.org:443}"
    DENOM="${DENOM:-uxion}"
    GAS_PRICES="0.025$DENOM"
    ;;
  "mainnet")
    CHAIN_ID="xion-mainnet-1"
    RPC_URL="https://rpc.xion.burnt.com:443"
    DENOM="uxion"
    GAS_PRICES="0.025uxion"
    ;;
  *)
    echo "Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
    ;;
esac

echo "🚀 Deploying $CONTRACT_NAME to $NETWORK"
echo "Chain ID: $CHAIN_ID"
echo "RPC URL: $RPC_URL"

# Check prerequisites
check_prerequisites() {
    echo "🔍 Checking prerequisites..."
    
    # Check if chain binary is installed
    if ! command -v $CHAIN_BINARY &> /dev/null; then
        echo "❌ $CHAIN_BINARY is not installed. Run ./scripts/setup_testnet.sh first"
        echo "Visit: https://docs.burnt.com/xion/learn/installation"
        exit 1
    fi
    
    # Check if Docker is running (for optimization)
    if ! docker info >/dev/null 2>&1; then
        echo "❌ Docker is not running. Please start Docker for contract optimization."
        exit 1
    fi
    
    # Check if key exists
    if ! xiond keys show $ADMIN_KEY >/dev/null 2>&1; then
        echo "❌ Key '$ADMIN_KEY' not found. Please create or import a key first."
        echo "Run: xiond keys add $ADMIN_KEY"
        exit 1
    fi
    
    echo "✅ Prerequisites check passed"
}

# Optimize contract
optimize_contract() {
    echo "🔧 Optimizing contract..."
    
    # Remove old artifacts
    rm -f contract.wasm hash.txt
    
    # Run optimizer
    docker run --rm -v "$(pwd)":/code \
        --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/rust-optimizer:$RUST_OPTIMIZER_VERSION
    
    if [ ! -f "contract.wasm" ]; then
        echo "❌ Contract optimization failed"
        exit 1
    fi
    
    echo "✅ Contract optimized successfully"
    echo "📦 Contract size: $(du -h contract.wasm | cut -f1)"
}

# Store contract on-chain
store_contract() {
    echo "📤 Storing contract on $NETWORK..."
    
    # Get admin address
    ADMIN_ADDR=$(xiond keys show $ADMIN_KEY -a)
    echo "Admin address: $ADMIN_ADDR"
    
    # Check balance
    BALANCE=$(xiond query bank balances $ADMIN_ADDR --node $RPC_URL --chain-id $CHAIN_ID -o json | jq -r '.balances[] | select(.denom=="'$DENOM'") | .amount')
    if [ -z "$BALANCE" ] || [ "$BALANCE" -lt "100000" ]; then
        echo "❌ Insufficient balance. Need at least 0.1 XION for deployment."
        echo "Current balance: $BALANCE $DENOM"
        exit 1
    fi
    
    # Store contract
    STORE_TX=$(xiond tx wasm store contract.wasm \
        --from $ADMIN_KEY \
        --chain-id $CHAIN_ID \
        --node $RPC_URL \
        --gas-prices $GAS_PRICES \
        --gas auto \
        --gas-adjustment 1.3 \
        --broadcast-mode block \
        --yes \
        -o json)
    
    # Extract code ID
    CODE_ID=$(echo $STORE_TX | jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
    
    if [ -z "$CODE_ID" ] || [ "$CODE_ID" = "null" ]; then
        echo "❌ Failed to extract code ID from transaction"
        echo "Transaction result: $STORE_TX"
        exit 1
    fi
    
    echo "✅ Contract stored successfully"
    echo "📋 Code ID: $CODE_ID"
    
    # Save code ID for later use
    echo $CODE_ID > code_id.txt
}

# Instantiate contract
instantiate_contract() {
    echo "🎯 Instantiating contract..."
    
    CODE_ID=$(cat code_id.txt)
    ADMIN_ADDR=$(xiond keys show $ADMIN_KEY -a)
    
    # Create instantiate message
    INIT_MSG='{
        "admin": "'$ADMIN_ADDR'",
        "platform_fee_percent": 5,
        "min_escrow_amount": "1000",
        "dispute_period_days": 7,
        "max_job_duration_days": 365
    }'
    
    echo "Init message: $INIT_MSG"
    
    # Instantiate contract
    INSTANTIATE_TX=$(xiond tx wasm instantiate $CODE_ID "$INIT_MSG" \
        --from $ADMIN_KEY \
        --label "$CONTRACT_NAME-$(date +%s)" \
        --admin $ADMIN_ADDR \
        --chain-id $CHAIN_ID \
        --node $RPC_URL \
        --gas-prices $GAS_PRICES \
        --gas auto \
        --gas-adjustment 1.3 \
        --broadcast-mode block \
        --yes \
        -o json)
    
    # Extract contract address
    CONTRACT_ADDR=$(echo $INSTANTIATE_TX | jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value')
    
    if [ -z "$CONTRACT_ADDR" ] || [ "$CONTRACT_ADDR" = "null" ]; then
        echo "❌ Failed to extract contract address from transaction"
        echo "Transaction result: $INSTANTIATE_TX"
        exit 1
    fi
    
    echo "✅ Contract instantiated successfully"
    echo "📋 Contract Address: $CONTRACT_ADDR"
    
    # Save contract address
    echo $CONTRACT_ADDR > contract_address.txt
}

# Verify deployment
verify_deployment() {
    echo "🔍 Verifying deployment..."
    
    CONTRACT_ADDR=$(cat contract_address.txt)
    
    # Query contract config
    CONFIG_QUERY='{"get_config":{}}'
    CONFIG_RESULT=$(xiond query wasm contract-state smart $CONTRACT_ADDR "$CONFIG_QUERY" \
        --node $RPC_URL \
        --chain-id $CHAIN_ID \
        -o json)
    
    echo "✅ Contract is responsive"
    echo "📋 Contract configuration:"
    echo $CONFIG_RESULT | jq '.data'
}

# Generate deployment summary
generate_summary() {
    echo "📄 Generating deployment summary..."
    
    CODE_ID=$(cat code_id.txt)
    CONTRACT_ADDR=$(cat contract_address.txt)
    ADMIN_ADDR=$(xiond keys show $ADMIN_KEY -a)
    TIMESTAMP=$(date -u +"%Y-%m-%d %H:%M:%S UTC")
    
    cat > deployment_summary.json << EOF
{
    "contract_name": "$CONTRACT_NAME",
    "network": "$NETWORK",
    "chain_id": "$CHAIN_ID",
    "code_id": "$CODE_ID",
    "contract_address": "$CONTRACT_ADDR",
    "admin_address": "$ADMIN_ADDR",
    "deployed_at": "$TIMESTAMP",
    "deployment_config": {
        "platform_fee_percent": 5,
        "min_escrow_amount": "1000",
        "dispute_period_days": 7,
        "max_job_duration_days": 365
    }
}
EOF
    
    echo "✅ Deployment summary saved to deployment_summary.json"
}

# Main deployment flow
main() {
    echo "🎬 Starting XWorks contract deployment..."
    
    check_prerequisites
    optimize_contract
    store_contract
    instantiate_contract
    verify_deployment
    generate_summary
    
    echo ""
    echo "🎉 Deployment completed successfully!"
    echo "📋 Summary:"
    echo "   Network: $NETWORK"
    echo "   Code ID: $(cat code_id.txt)"
    echo "   Contract Address: $(cat contract_address.txt)"
    echo ""
    echo "🔗 You can now interact with your contract using the address above."
    echo "📚 Check deployment_summary.json for full details."
}

# Run main function
main
