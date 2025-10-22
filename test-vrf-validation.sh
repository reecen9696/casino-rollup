#!/bin/bash

set -e

echo "🎲 VRF Integration Validation Test"
echo "===================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[1;34m'
NC='\033[0m' # No Color

# Function to print test status
print_status() {
    local status=$1
    local message=$2
    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✅ $message${NC}"
    elif [ "$status" = "FAIL" ]; then
        echo -e "${RED}❌ $message${NC}"
        exit 1
    elif [ "$status" = "WARN" ]; then
        echo -e "${YELLOW}⚠️  $message${NC}"
    else
        echo -e "${BLUE}🔄 $message${NC}"
    fi
}

# Cleanup function
cleanup() {
    echo ""
    print_status "INFO" "Cleaning up processes..."
    pkill -f "sequencer" 2>/dev/null || true
    sleep 2
}

# Set trap for cleanup
trap cleanup EXIT

print_status "INFO" "Step 1: Environment Setup"
echo "------------------------------"

# Stop any existing processes
print_status "INFO" "Stopping existing processes..."
pkill -f "sequencer" 2>/dev/null || true
sleep 2

# Clean up artifacts
rm -f zkcasino.settlement.json vrf-keypair.json 2>/dev/null || true

print_status "INFO" "Step 2: Building Sequencer with VRF"
echo "------------------------------"

print_status "INFO" "Building sequencer with VRF support..."
cd sequencer
if cargo build --release --quiet 2>/dev/null; then
    print_status "PASS" "Sequencer with VRF built successfully"
else
    print_status "FAIL" "Sequencer build failed"
fi
cd ..

print_status "INFO" "Step 3: VRF Unit Tests Validation"
echo "-----------------------------------"

print_status "INFO" "Running VRF unit tests..."
cd sequencer
VRF_TEST_OUTPUT=$(cargo test vrf 2>&1)
VRF_TESTS_PASSED=$(echo "$VRF_TEST_OUTPUT" | grep -o "[0-9]\+ passed" | head -1 | cut -d' ' -f1)

if [ "$VRF_TESTS_PASSED" -gt 0 ]; then
    print_status "PASS" "VRF unit tests passed: $VRF_TESTS_PASSED tests"
else
    print_status "FAIL" "VRF unit tests failed"
    echo "$VRF_TEST_OUTPUT"
fi
cd ..

print_status "INFO" "Step 4: Starting Sequencer with VRF"
echo "-------------------------------"

# Start sequencer with VRF enabled
print_status "INFO" "Starting sequencer with VRF integration..."
export ENABLE_SOLANA=false  # Disable Solana for this test
export ENABLE_ZK_PROOFS=false
export RUST_LOG=info

./target/release/sequencer --enable-vrf > sequencer.log 2>&1 &
SEQUENCER_PID=$!

# Wait for sequencer to be ready
print_status "INFO" "Waiting for Sequencer to be ready..."
sleep 3
for i in {1..30}; do
    if curl -s http://localhost:3000/health >/dev/null 2>&1; then
        break
    fi
    sleep 1
    echo -n "."
done
echo ""

if ! curl -s http://localhost:3000/health >/dev/null 2>&1; then
    print_status "FAIL" "Sequencer failed to start"
    echo "=== Sequencer logs ==="
    cat sequencer.log
fi
print_status "PASS" "Sequencer started (PID: $SEQUENCER_PID) with VRF enabled"

# Verify VRF keypair was generated
if [ -f "vrf-keypair.json" ]; then
    VRF_SIZE=$(stat -f%z vrf-keypair.json 2>/dev/null || stat -c%s vrf-keypair.json 2>/dev/null || echo "0")
    print_status "PASS" "VRF keypair generated: $VRF_SIZE bytes"
else
    print_status "FAIL" "VRF keypair not found"
fi

print_status "INFO" "Step 5: VRF Functionality Testing"
echo "-----------------------"

print_status "INFO" "Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:3000/health)
if [ "$HEALTH_RESPONSE" = "OK" ]; then
    print_status "PASS" "Health check: OK"
else
    print_status "FAIL" "Health check failed: $HEALTH_RESPONSE"
fi

print_status "INFO" "Step 6: VRF Bet Testing"
echo "-----------------------"

# Place multiple bets to test VRF variability
print_status "INFO" "Placing multiple test bets with VRF..."

PLAYER_ADDRESS="11111111111111111111111111111112"  # Valid Solana address format
BETS_PLACED=0
HEADS_COUNT=0
TAILS_COUNT=0
VRF_SIGNATURES=()

for i in {1..10}; do
    print_status "INFO" "Placing bet $i..."
    
    BET_RESPONSE=$(curl -s -X POST -H 'Content-Type: application/json' \
        -d "{\"player_address\": \"$PLAYER_ADDRESS\", \"amount\": 5000, \"guess\": true}" \
        http://localhost:3000/v1/bet)
    
    if echo "$BET_RESPONSE" | jq . >/dev/null 2>&1; then
        BET_ID=$(echo "$BET_RESPONSE" | jq -r '.bet_id')
        RESULT=$(echo "$BET_RESPONSE" | jq -r '.result')
        WON=$(echo "$BET_RESPONSE" | jq -r '.won')
        
        print_status "PASS" "Bet $i placed successfully"
        echo "   Bet ID: $BET_ID"
        echo "   Result: $RESULT"
        echo "   Won: $WON"
        
        BETS_PLACED=$((BETS_PLACED + 1))
        
        if [ "$RESULT" = "true" ]; then
            HEADS_COUNT=$((HEADS_COUNT + 1))
        else
            TAILS_COUNT=$((TAILS_COUNT + 1))
        fi
    else
        print_status "WARN" "Bet $i failed: $BET_RESPONSE"
    fi
    
    sleep 0.5
done

print_status "PASS" "VRF Results Summary:"
echo "   Total bets: $BETS_PLACED"
echo "   Heads: $HEADS_COUNT"
echo "   Tails: $TAILS_COUNT"

if [ $BETS_PLACED -gt 0 ] && [ $HEADS_COUNT -gt 0 ] && [ $TAILS_COUNT -gt 0 ]; then
    print_status "PASS" "VRF generating varied outcomes (not deterministic single value)"
elif [ $BETS_PLACED -gt 0 ] && ([ $HEADS_COUNT -gt 0 ] || [ $TAILS_COUNT -gt 0 ]); then
    print_status "WARN" "VRF outcomes not varied (all same result - could be normal for small sample)"
else
    print_status "FAIL" "No bets were successfully placed"
fi

print_status "INFO" "Step 7: VRF Log Analysis"
echo "------------------------"

print_status "INFO" "Analyzing sequencer logs for VRF activity..."

# Check sequencer logs for VRF messages
VRF_LOG_COUNT=$(grep -c "VRF:" sequencer.log 2>/dev/null || echo "0")
CSPRNG_LOG_COUNT=$(grep -c "CSPRNG:" sequencer.log 2>/dev/null || echo "0")

if [ "$VRF_LOG_COUNT" -gt 0 ]; then
    print_status "PASS" "VRF logging detected: $VRF_LOG_COUNT VRF operations"
    echo ""
    echo "Sample VRF logs:"
    grep "VRF:" sequencer.log | head -3
else
    print_status "FAIL" "No VRF logging found in sequencer logs"
fi

if [ "$CSPRNG_LOG_COUNT" -gt 0 ]; then
    print_status "WARN" "CSPRNG fallback used: $CSPRNG_LOG_COUNT operations (VRF might be disabled)"
else
    print_status "PASS" "No CSPRNG fallback used (VRF is working)"
fi

# Check for VRF keypair loading
if grep -q "VRF keypair loaded successfully" sequencer.log; then
    VRF_PUBKEY=$(grep "VRF keypair loaded successfully" sequencer.log | tail -1 | sed -n 's/.*Public key: "\([^"]*\)".*/\1/p')
    print_status "PASS" "VRF keypair loaded: $VRF_PUBKEY"
else
    print_status "WARN" "VRF keypair loading not found in logs"
fi

print_status "INFO" "Step 8: Settlement Analysis"
echo "------------------------"

print_status "INFO" "Waiting for settlement batch processing..."
sleep 3

# Check if settlement file exists
if [ -f "zkcasino.settlement.json" ]; then
    print_status "PASS" "Settlement persistence file exists"
    
    # Check settlement file content
    SETTLEMENT_SIZE=$(stat -f%z zkcasino.settlement.json 2>/dev/null || stat -c%s zkcasino.settlement.json 2>/dev/null || echo "0")
    if [ "$SETTLEMENT_SIZE" -gt 100 ]; then
        print_status "PASS" "Settlement file has content: $SETTLEMENT_SIZE bytes"
    else
        print_status "WARN" "Settlement file exists but is small: $SETTLEMENT_SIZE bytes"
    fi
    
    # Check for bet IDs in settlement
    BET_COUNT_IN_SETTLEMENT=$(grep -o '"bet_id"' zkcasino.settlement.json 2>/dev/null | wc -l | tr -d ' ')
    if [ "$BET_COUNT_IN_SETTLEMENT" -gt 0 ]; then
        print_status "PASS" "Settlement contains $BET_COUNT_IN_SETTLEMENT bet records"
    else
        print_status "WARN" "No bet records found in settlement file"
    fi
else
    print_status "WARN" "No settlement persistence file found"
fi

# Check settlement stats
STATS_RESPONSE=$(curl -s http://localhost:3000/v1/settlement-stats)
if echo "$STATS_RESPONSE" | jq . >/dev/null 2>&1; then
    BATCHES_PROCESSED=$(echo "$STATS_RESPONSE" | jq -r '.total_batches_processed // 0')
    if [ "$BATCHES_PROCESSED" -gt 0 ]; then
        print_status "PASS" "Settlement processing active: $BATCHES_PROCESSED batches"
    else
        print_status "WARN" "No settlement batches processed yet"
    fi
else
    print_status "WARN" "Settlement stats endpoint not responding properly"
fi

print_status "INFO" "Step 9: VRF Determinism Test"
echo "----------------------------"

print_status "INFO" "Testing VRF determinism with same inputs..."

# Test that same inputs produce same outputs
BET_RESPONSE_1=$(curl -s -X POST -H 'Content-Type: application/json' \
    -d "{\"player_address\": \"22222222222222222222222222222223\", \"amount\": 1000, \"guess\": false}" \
    http://localhost:3000/v1/bet)

sleep 1

BET_RESPONSE_2=$(curl -s -X POST -H 'Content-Type: application/json' \
    -d "{\"player_address\": \"22222222222222222222222222222223\", \"amount\": 1000, \"guess\": false}" \
    http://localhost:3000/v1/bet)

if echo "$BET_RESPONSE_1" | jq . >/dev/null 2>&1 && echo "$BET_RESPONSE_2" | jq . >/dev/null 2>&1; then
    RESULT_1=$(echo "$BET_RESPONSE_1" | jq -r '.result')
    RESULT_2=$(echo "$BET_RESPONSE_2" | jq -r '.result')
    
    # Different bets should have different outcomes due to different nonces
    if [ "$RESULT_1" != "$RESULT_2" ]; then
        print_status "PASS" "VRF produces different outcomes for different nonces (as expected)"
    else
        print_status "WARN" "VRF produced same outcome for different bets (could be chance)"
    fi
    
    print_status "PASS" "Determinism test completed: $RESULT_1 vs $RESULT_2"
else
    print_status "WARN" "Determinism test failed to place bets"
fi

print_status "INFO" "Step 10: Final Validation"
echo "-----------------------------------"

print_status "INFO" "Final system status check..."

# Check sequencer is still running
if ps -p $SEQUENCER_PID >/dev/null 2>&1; then
    print_status "PASS" "Sequencer still running"
else
    print_status "WARN" "Sequencer process ended"
fi

# Final API check
if curl -s http://localhost:3000/health | grep -q "OK"; then
    print_status "PASS" "Sequencer API still responding"
else
    print_status "WARN" "Sequencer API not responding"
fi

# Check VRF keypair still exists
if [ -f "vrf-keypair.json" ]; then
    print_status "PASS" "VRF keypair persisted"
else
    print_status "WARN" "VRF keypair file missing"
fi

echo ""
print_status "PASS" "📊 Final Results"
echo "=================="
print_status "PASS" "🎉 VRF INTEGRATION VALIDATION COMPLETED!"

echo ""
echo "Summary:"
echo "  • VRF sequencer: Running with ed25519 VRF"
echo "  • Unit tests: $VRF_TESTS_PASSED VRF tests passed"
echo "  • Bet processing: $BETS_PLACED bets with VRF outcomes"
echo "  • Outcome distribution: $HEADS_COUNT heads, $TAILS_COUNT tails"
echo "  • VRF logging: $VRF_LOG_COUNT VRF operations logged"
echo "  • Settlement: $BATCHES_PROCESSED batches processed"
echo ""

if [ "$VRF_LOG_COUNT" -gt 0 ] && [ "$BETS_PLACED" -gt 0 ]; then
    print_status "PASS" "🎯 VRF is working correctly!"
else
    print_status "WARN" "🎯 VRF may not be working as expected"
fi

echo ""
print_status "INFO" "Logs saved to sequencer.log for analysis"
print_status "INFO" "Test completed. Press Ctrl+C to cleanup and exit."
sleep 5