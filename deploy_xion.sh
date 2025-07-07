#!/bin/bash

# XWorks Contract Deployment - Following Xion Guidelines
# Based on: https://docs.burnt.com/xion/developers/section-overview/cosmwasm-resources/introductory-section/deployment-and-interaction

set -e

echo "🚀 XWorks Contract Deployment (Xion Network)"
echo "============================================"

# Step 1: Install Xion CLI if not present
install_xion_cli() {
    echo "📦 Installing Xion CLI..."
    
    # Check if xiond is already installed
    if command -v xiond &> /dev/null; then
        echo "✅ Xion CLI already installed"
        xiond version
        return 0
    fi
    
    echo "📥 Downloading Xion CLI for macOS..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Download the latest release (you may need to update this URL)
    echo "🔽 Downloading from GitHub releases..."
    curl -LO "https://github.com/burnt-labs/xion/releases/latest/download/xiond-darwin-amd64"
    
    # Make executable
    chmod +x xiond-darwin-amd64
    
    # Move to /usr/local/bin
    echo "📂 Installing to /usr/local/bin..."
    sudo mv xiond-darwin-amd64 /usr/local/bin/xiond
    
    # Clean up
    cd - > /dev/null
    rm -rf "$TEMP_DIR"
    
    # Verify installation
    if command -v xiond &> /dev/null; then
        echo "✅ Xion CLI installed successfully"
        xiond version
    else
        echo "❌ Installation failed"
        exit 1
    fi
}

# Step 2: Setup wallet
setup_wallet() {
    echo "🔑 Setting up wallet..."
    
    # Check if key exists
    if xiond keys show admin >/dev/null 2>&1; then
        echo "✅ Admin key already exists"
        ADMIN_ADDR=$(xiond keys show admin -a)
        echo "   Address: $ADMIN_ADDR"
        return 0
    fi
    
    echo "🆕 Creating new admin key..."
    echo "Choose an option:"
    echo "1. Create new key"
    echo "2. Import existing key from mnemonic"
    read -p "Enter choice (1 or 2): " choice
    
    case $choice in
        1)
            xiond keys add admin
            ;;
        2)
            xiond keys add admin --recover
            ;;
        *)
            echo "Invalid choice"
            exit 1
            ;;
    esac
    
    ADMIN_ADDR=$(xiond keys show admin -a)
    echo "✅ Admin key setup complete"
    echo "   Address: $ADMIN_ADDR"
}

# Step 3: Optimize contract (following Xion guidelines)
optimize_contract() {
    echo "🔧 Optimizing contract..."
    
    # Remove old artifacts
    rm -f contract.wasm hash.txt
    
    # Build optimized contract using rust-optimizer
    echo "🐳 Running rust-optimizer..."
    docker run --rm -v "$(pwd)":/code \
        --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/rust-optimizer:0.15.0
    
    if [ ! -f "contract.wasm" ]; then
        echo "❌ Contract optimization failed"
        exit 1
    fi
    
    echo "✅ Contract optimized successfully"
    echo "📦 Contract size: $(du -h contract.wasm | cut -f1)"
    
    # Generate checksum
    sha256sum contract.wasm | cut -d ' ' -f 1 > hash.txt
    echo "🔐 Checksum: $(cat hash.txt)"
}

# Step 4: Deploy to Xion testnet
deploy_to_testnet() {
    echo "🌐 Deploying to Xion testnet..."
    
    # Network configuration
    CHAIN_ID="xion-testnet-1"
    RPC_URL="https://testnet-rpc.xion.burnt.com:443"
    DENOM="uxion"
    GAS_PRICES="0.025uxion"
    
    ADMIN_ADDR=$(xiond keys show admin -a)
    echo "👤 Admin address: $ADMIN_ADDR"
    
    # Check balance
    echo "💰 Checking balance..."
    BALANCE=$(xiond query bank balances $ADMIN_ADDR --node $RPC_URL --chain-id $CHAIN_ID -o json | jq -r '.balances[] | select(.denom=="'$DENOM'") | .amount // "0"')
    echo "💰 Current balance: $BALANCE $DENOM"
    
    if [ "$BALANCE" -lt "100000" ]; then
        echo "⚠️  Low balance detected!"
        echo "📝 To get testnet tokens:"
        echo "   1. Visit Xion testnet faucet"
        echo "   2. Request tokens for address: $ADMIN_ADDR"
        echo "   3. Wait for confirmation and run this script again"
        read -p "Do you want to continue anyway? (y/N): " continue_choice
        if [[ ! "$continue_choice" =~ ^[Yy]$ ]]; then
            echo "❌ Deployment cancelled"
            exit 1
        fi
    fi
    
    # Store contract
    echo "📤 Storing contract on-chain..."
    STORE_TX=$(xiond tx wasm store contract.wasm \
        --from admin \
        --chain-id $CHAIN_ID \
        --node $RPC_URL \
        --gas-prices $GAS_PRICES \
        --gas auto \
        --gas-adjustment 1.3 \
        --broadcast-mode block \
        --yes \
        -o json)
    
    echo "📋 Store transaction result:"
    echo "$STORE_TX" | jq .
    
    # Extract code ID
    CODE_ID=$(echo $STORE_TX | jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
    
    if [ -z "$CODE_ID" ] || [ "$CODE_ID" = "null" ]; then
        echo "❌ Failed to extract code ID"
        echo "Transaction details:"
        echo "$STORE_TX"
        exit 1
    fi
    
    echo "✅ Contract stored successfully"
    echo "📋 Code ID: $CODE_ID"
    echo $CODE_ID > code_id.txt
    
    # Instantiate contract
    echo "🎯 Instantiating contract..."
    
    # Create instantiate message
    INIT_MSG='{
        "admin": "'$ADMIN_ADDR'",
        "platform_fee_percent": 5,
        "min_escrow_amount": "1000",
        "dispute_period_days": 7,
        "max_job_duration_days": 365
    }'
    
    echo "📝 Instantiate message: $INIT_MSG"
    
    INSTANTIATE_TX=$(xiond tx wasm instantiate $CODE_ID "$INIT_MSG" \
        --from admin \
        --label "xworks-freelance-contract-$(date +%s)" \
        --admin $ADMIN_ADDR \
        --chain-id $CHAIN_ID \
        --node $RPC_URL \
        --gas-prices $GAS_PRICES \
        --gas auto \
        --gas-adjustment 1.3 \
        --broadcast-mode block \
        --yes \
        -o json)
    
    echo "📋 Instantiate transaction result:"
    echo "$INSTANTIATE_TX" | jq .
    
    # Extract contract address
    CONTRACT_ADDR=$(echo $INSTANTIATE_TX | jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value')
    
    if [ -z "$CONTRACT_ADDR" ] || [ "$CONTRACT_ADDR" = "null" ]; then
        echo "❌ Failed to extract contract address"
        echo "Transaction details:"
        echo "$INSTANTIATE_TX"
        exit 1
    fi
    
    echo "✅ Contract instantiated successfully"
    echo "📋 Contract Address: $CONTRACT_ADDR"
    echo $CONTRACT_ADDR > contract_address.txt
    
    # Verify deployment
    echo "🔍 Verifying deployment..."
    CONFIG_QUERY='{"get_config":{}}'
    CONFIG_RESULT=$(xiond query wasm contract-state smart $CONTRACT_ADDR "$CONFIG_QUERY" \
        --node $RPC_URL \
        --chain-id $CHAIN_ID \
        -o json)
    
    echo "✅ Contract is responsive"
    echo "📋 Contract configuration:"
    echo $CONFIG_RESULT | jq '.data'
    
    # Generate deployment summary
    generate_deployment_summary $CODE_ID $CONTRACT_ADDR $ADMIN_ADDR "testnet"
}

# Generate deployment summary
generate_deployment_summary() {
    local code_id=$1
    local contract_addr=$2
    local admin_addr=$3
    local network=$4
    
    echo "📄 Generating deployment summary..."
    
    cat > deployment_summary.json << EOF
{
    "contract_name": "xworks-freelance-contract",
    "network": "$network",
    "chain_id": "xion-testnet-1",
    "code_id": "$code_id",
    "contract_address": "$contract_addr",
    "admin_address": "$admin_addr",
    "deployed_at": "$(date -u +"%Y-%m-%d %H:%M:%S UTC")",
    "deployment_config": {
        "platform_fee_percent": 5,
        "min_escrow_amount": "1000",
        "dispute_period_days": 7,
        "max_job_duration_days": 365
    },
    "rpc_url": "https://testnet-rpc.xion.burnt.com:443",
    "checksum": "$(cat hash.txt 2>/dev/null || echo 'N/A')"
}
EOF
    
    echo "✅ Deployment summary saved to deployment_summary.json"
}

# Main execution
main() {
    echo "🎬 Starting Xion deployment process..."
    
    # Check Docker
    if ! docker info >/dev/null 2>&1; then
        echo "❌ Docker is not running. Please start Docker."
        exit 1
    fi
    
    # Install dependencies
    install_xion_cli
    setup_wallet
    
    # Build and deploy
    optimize_contract
    deploy_to_testnet
    
    echo ""
    echo "🎉 Deployment completed successfully!"
    echo "📋 Summary:"
    echo "   Network: Xion Testnet"
    echo "   Code ID: $(cat code_id.txt)"
    echo "   Contract Address: $(cat contract_address.txt)"
    echo ""
    echo "🔗 You can now interact with your contract!"
    echo "📚 Check deployment_summary.json for full details."
    echo ""
    echo "📖 Next steps:"
    echo "   1. Test contract functionality"
    echo "   2. Update frontend with contract address"
    echo "   3. Deploy to mainnet when ready"
}

# Run main function
main
