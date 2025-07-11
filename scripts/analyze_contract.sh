#!/usr/bin/env bash
# Contract size and optimization analysis

set -e

WASM_FILE="./artifacts/xworks_freelance_contract.wasm"
OPTIMIZED_FILE="./artifacts/xworks_freelance_contract_optimized.wasm"

echo "📊 Contract Analysis Report"
echo "=========================="

# Check if wasm file exists
if [ ! -f "$WASM_FILE" ]; then
    echo "❌ WASM file not found: $WASM_FILE"
    echo "Run 'cargo build --release --target wasm32-unknown-unknown' first"
    exit 1
fi

# Basic file info
echo "📁 File Information:"
echo "   Original WASM: $(ls -lh $WASM_FILE | awk '{print $5}')"

# Check if wasm-opt is available for optimization
if command -v wasm-opt >/dev/null 2>&1; then
    echo "🔧 Optimizing with wasm-opt..."
    wasm-opt -Oz --strip-debug --strip-producers "$WASM_FILE" -o "$OPTIMIZED_FILE"
    echo "   Optimized WASM: $(ls -lh $OPTIMIZED_FILE | awk '{print $5}')"
    
    # Calculate compression ratio
    ORIGINAL_SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE")
    OPTIMIZED_SIZE=$(stat -f%z "$OPTIMIZED_FILE" 2>/dev/null || stat -c%s "$OPTIMIZED_FILE")
    REDUCTION=$((100 - (OPTIMIZED_SIZE * 100 / ORIGINAL_SIZE)))
    echo "   Size reduction: ${REDUCTION}%"
else
    echo "⚠️  wasm-opt not found. Install binaryen for size optimization:"
    echo "   macOS: brew install binaryen"
    echo "   Ubuntu: apt install binaryen"
fi

# Memory and import analysis
echo ""
echo "🔍 Contract Analysis:"

# Check for exports
echo "   Exported functions:"
if command -v wasm-objdump >/dev/null 2>&1; then
    wasm-objdump -x "$WASM_FILE" | grep -A 20 "Export\[" | head -10
else
    echo "   (wasm-objdump not available for detailed analysis)"
fi

echo ""
echo "📈 Size Recommendations:"
echo "   - ✅ Good: <500KB"
echo "   - ⚠️  Large: 500KB-1MB" 
echo "   - ❌ Too Large: >1MB"

# Check actual size
SIZE_KB=$((ORIGINAL_SIZE / 1024))
if [ $SIZE_KB -lt 500 ]; then
    echo "   Status: ✅ Contract size is optimal ($SIZE_KB KB)"
elif [ $SIZE_KB -lt 1024 ]; then
    echo "   Status: ⚠️  Contract is large ($SIZE_KB KB) - consider optimization"
else
    echo "   Status: ❌ Contract is too large ($SIZE_KB KB) - optimization required"
fi

echo ""
echo "🚀 Next Steps:"
echo "   1. Run gas profiling: ./scripts/gas_profiler.py"
echo "   2. Review deployment checklist: DEPLOYMENT_CHECKLIST.md"
echo "   3. Test on testnet before mainnet deployment"
