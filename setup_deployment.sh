#!/bin/bash

# XWorks Contract - Quick Deployment Setup
# Run this script to set up the deployment environment

set -e

echo "🚀 XWorks Contract Deployment Setup"
echo "===================================="

# Step 1: Check if Docker is running
echo "📦 Checking Docker status..."
if ! docker info >/dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker Desktop."
    echo "   You can start it from Applications or run: open -a Docker"
    exit 1
else
    echo "✅ Docker is running"
fi

# Step 2: Check if Xion CLI is installed
echo "🔧 Checking Xion CLI..."
if ! command -v xiond &> /dev/null; then
    echo "❌ Xion CLI (xiond) is not installed"
    echo ""
    echo "📥 To install Xion CLI:"
    echo "1. Visit: https://docs.burnt.com/xion/learn/installation"
    echo "2. Download the binary for macOS"
    echo "3. Or install via package manager if available"
    echo ""
    echo "After installation, run this script again."
    exit 1
else
    echo "✅ Xion CLI is installed"
    xiond version
fi

# Step 3: Check if admin key exists
echo "🔑 Checking admin key..."
if ! xiond keys show admin >/dev/null 2>&1; then
    echo "❌ Admin key not found"
    echo ""
    echo "🔐 To create admin key:"
    echo "   xiond keys add admin"
    echo ""
    echo "🔐 To import existing key:"
    echo "   xiond keys add admin --recover"
    echo ""
    echo "After creating the key, run this script again."
    exit 1
else
    echo "✅ Admin key found"
    ADMIN_ADDR=$(xiond keys show admin -a)
    echo "   Address: $ADMIN_ADDR"
fi

# Step 4: Run final checks
echo "🧪 Running final contract checks..."
if ! cargo check >/dev/null 2>&1; then
    echo "❌ Contract compilation failed"
    exit 1
else
    echo "✅ Contract compiles successfully"
fi

if ! cargo test >/dev/null 2>&1; then
    echo "❌ Tests are failing"
    exit 1
else
    echo "✅ All tests pass"
fi

echo ""
echo "🎉 Deployment environment is ready!"
echo ""
echo "🚀 To deploy to testnet:"
echo "   ./scripts/deploy.sh testnet admin"
echo ""
echo "🚀 To deploy to mainnet:"
echo "   ./scripts/deploy.sh mainnet admin"
echo ""
echo "⚠️  Make sure your admin account has sufficient XION tokens:"
echo "   - Testnet: Get tokens from faucet"
echo "   - Mainnet: Transfer XION tokens to $ADMIN_ADDR"
