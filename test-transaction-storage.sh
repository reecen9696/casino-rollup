#!/bin/bash

# Test script to verify transaction signature storage fix
# Tests the core logic without requiring full Solana validator

echo "🧪 Testing Transaction Signature Storage Fix"
echo "============================================="

# Clean previous state
echo "🧹 Cleaning previous state..."
rm -rf settlement.json

# Create test bet data
echo "📝 Creating test bet..."
curl -X POST http://localhost:3000/bet \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 100,
    "prediction": 50,
    "nonce": "test_nonce_001"
  }' &

# Start sequencer in background
echo "🚀 Starting sequencer..."
ENABLE_SOLANA=true RUST_LOG=info ./target/release/sequencer &
SEQUENCER_PID=$!

# Wait for sequencer to initialize
sleep 3

# Send test bet
echo "📨 Sending test bet..."
curl -X POST http://localhost:3000/bet \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 100,
    "prediction": 50,
    "nonce": "test_nonce_002"
  }'

# Wait for processing
sleep 10

# Check settlement file for transaction signatures
echo "🔍 Checking settlement.json for transaction signatures..."

if [ -f "settlement.json" ]; then
    echo "✅ Settlement file exists"
    
    # Check if transaction signatures are no longer null
    null_signatures=$(grep -o '"transaction_signature": null' settlement.json | wc -l)
    non_null_signatures=$(grep -o '"transaction_signature": "[^"]*"' settlement.json | wc -l)
    
    echo "📊 Transaction signature analysis:"
    echo "   - Null signatures: $null_signatures"
    echo "   - Non-null signatures: $non_null_signatures"
    
    if [ $non_null_signatures -gt 0 ]; then
        echo "✅ SUCCESS: Found non-null transaction signatures!"
        echo "🎯 Transaction signature storage fix is working!"
        
        # Show sample signatures
        echo "📋 Sample transaction signatures:"
        grep '"transaction_signature"' settlement.json | head -3
    else
        echo "❌ FAIL: All transaction signatures are still null"
        echo "🐛 The fix may not be working correctly"
    fi
    
    echo "📄 Full settlement content:"
    cat settlement.json | jq '.' 2>/dev/null || cat settlement.json
else
    echo "❌ Settlement file not found"
fi

# Cleanup
echo "🧹 Cleaning up..."
kill $SEQUENCER_PID 2>/dev/null
wait $SEQUENCER_PID 2>/dev/null

echo "✅ Test completed!"