#!/usr/bin/env bash
# Xion Testnet Setup and Key Management
# This script helps set up a testnet environment for XWork contract testing

set -e

CHAIN_BINARY="xiond"
CHAIN_ID="xion-testnet-1"
NODE="https://testnet-rpc.xion.org:443"
FAUCET_URL="https://faucet.xion.org"

echo "🌐 Xion Testnet Setup Guide"
echo "=========================="

# Check if xiond is installed
if ! command -v $CHAIN_BINARY &> /dev/null; then
    echo "❌ $CHAIN_BINARY not found. Please install it first:"
    echo ""
    echo "Download and install xiond:"
    echo "  curl -L https://github.com/burnt-labs/xion/releases/download/v19.0.2/xiond_19.0.2_darwin_amd64.tar.gz -o /tmp/xiond.tar.gz"
    echo "  cd /tmp && tar -xzf xiond.tar.gz"
    echo "  sudo cp xiond /usr/local/bin/ && chmod +x /usr/local/bin/xiond"
    echo ""
    exit 1
fi

echo "✅ $CHAIN_BINARY found: $(which $CHAIN_BINARY)"

# Check network connectivity
echo ""
echo "🔗 Testing network connectivity..."
if $CHAIN_BINARY status --node $NODE &>/dev/null; then
    echo "✅ Connected to $CHAIN_ID"
    LATEST_BLOCK=$($CHAIN_BINARY status --node $NODE | jq -r '.SyncInfo.latest_block_height')
    echo "   Latest block: $LATEST_BLOCK"
else
    echo "❌ Cannot connect to $NODE"
    echo "   Please check your internet connection and try again"
    exit 1
fi

# Key management
echo ""
echo "🔑 Key Management"
echo "=================="

KEY_NAME="${1:-benchmark}"
echo "Using key name: $KEY_NAME"

# Check if key exists
if $CHAIN_BINARY keys show $KEY_NAME &>/dev/null; then
    echo "✅ Key '$KEY_NAME' already exists"
    ADDRESS=$($CHAIN_BINARY keys show $KEY_NAME -a)
    echo "   Address: $ADDRESS"
else
    echo "📋 Key '$KEY_NAME' not found. Creating new key..."
    echo "⚠️  IMPORTANT: Save the mnemonic phrase securely!"
    echo ""
    read -p "Press Enter to continue..."
    
    $CHAIN_BINARY keys add $KEY_NAME
    ADDRESS=$($CHAIN_BINARY keys show $KEY_NAME -a)
    echo ""
    echo "✅ Key created successfully!"
    echo "   Address: $ADDRESS"
fi

# Check balance
echo ""
echo "💰 Checking balance..."
BALANCE=$($CHAIN_BINARY query bank balances $ADDRESS --node $NODE --output json 2>/dev/null)
UXION_BALANCE=$(echo $BALANCE | jq -r '.balances[] | select(.denom=="uxion") | .amount // "0"')

echo "   Current balance: $UXION_BALANCE uxion"

# Check if funds are needed
MIN_BALANCE=10000000  # 10 XION (assuming 6 decimals)
if [ "$UXION_BALANCE" -lt "$MIN_BALANCE" ]; then
    echo ""
    echo "💸 Insufficient funds for contract testing"
    echo "   Required: ~10 XION (10,000,000 uxion)"
    echo "   Current:  $(echo "scale=6; $UXION_BALANCE/1000000" | bc) XION"
    echo ""
    echo "🚰 Get testnet tokens:"
    echo "   1. Visit: $FAUCET_URL"
    echo "   2. Enter your address: $ADDRESS"
    echo "   3. Request tokens"
    echo ""
    echo "   Alternative: Ask in Xion Discord #testnet channel"
    echo ""
    read -p "Press Enter after getting testnet tokens to continue..."
    
    # Re-check balance
    BALANCE=$($CHAIN_BINARY query bank balances $ADDRESS --node $NODE --output json)
    NEW_UXION_BALANCE=$(echo $BALANCE | jq -r '.balances[] | select(.denom=="uxion") | .amount // "0"')
    echo "   Updated balance: $NEW_UXION_BALANCE uxion"
    
    if [ "$NEW_UXION_BALANCE" -lt "$MIN_BALANCE" ]; then
        echo "⚠️  Still insufficient funds. You may encounter errors during testing."
    else
        echo "✅ Sufficient funds available!"
    fi
else
    echo "✅ Sufficient funds available!"
fi

# Export environment variables
echo ""
echo "🔧 Environment Setup"
echo "==================="
echo "Add these to your shell profile (~/.bashrc, ~/.zshrc):"
echo ""
echo "export CHAIN_ID=$CHAIN_ID"
echo "export NODE=--node $NODE"
echo "export KEY=$KEY_NAME"
echo "export DENOM=uxion"
echo ""

# Create environment file for scripts
cat > .env.testnet << EOF
# Xion Testnet Configuration
export CHAIN_ID=$CHAIN_ID
export NODE=--node $NODE
export KEY=$KEY_NAME
export DENOM=uxion
EOF

echo "📁 Environment file created: .env.testnet"
echo "   Source it with: source .env.testnet"

echo ""
echo "🚀 Ready for Contract Deployment!"
echo "================================="
echo "Next steps:"
echo "1. Source environment: source .env.testnet"
echo "2. Run gas benchmarking: ./scripts/benchmark_gas.sh"
echo "3. Deploy to testnet: ./scripts/deploy.sh"
echo ""
echo "Your testnet address: $ADDRESS"
echo "Contract will be deployed with this address as admin."
