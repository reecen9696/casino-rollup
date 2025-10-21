#!/bin/bash

# Test script for Phase 2 localnet deployment and Solana integration

set -e

echo "🚀 Testing Phase 2 Solana Integration"
echo "=================================="

# Build the sequencer
echo "📦 Building sequencer..."
cd sequencer
cargo build --release
cd ..

# Start the sequencer with Solana integration enabled
echo "🌐 Starting sequencer with Solana integration..."
export ENABLE_SOLANA=true
export SOLANA_TESTNET=false  # Use local validator

# Start sequencer in background
cd sequencer
timeout 30s cargo run --release &
SEQUENCER_PID=$!
cd ..

# Wait for sequencer to start
echo "⏳ Waiting for sequencer to start..."
sleep 5

# Test basic health endpoint
echo "🔍 Testing health endpoint..."
curl -s http://localhost:3000/health | jq .

# Test with a few bets to trigger settlement batching
echo "💰 Submitting test bets..."
for i in {1..3}; do
    echo "Submitting bet $i..."
    curl -s -X POST http://localhost:3000/v1/bet \
        -H "Content-Type: application/json" \
        -d '{
            "player_address": "11111111111111111111111111111111",
            "amount": 1000000,
            "guess": true
        }' | jq .
    sleep 1
done

# Check settlement stats
echo "📊 Checking settlement statistics..."
curl -s http://localhost:3000/v1/settlement-stats | jq .

# Cleanup
echo "🧹 Cleaning up..."
kill $SEQUENCER_PID 2>/dev/null || true

echo "✅ Phase 2 Solana integration test completed!"
echo ""
echo "Note: For full testing with actual Solana transactions,"
echo "you need to run a local Solana validator with:"
echo "  solana-test-validator"
echo ""
echo "Then set ENABLE_SOLANA=true and restart the sequencer."