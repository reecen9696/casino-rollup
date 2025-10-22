#!/bin/bash

# Enhanced Persistence Test with Detailed Logging
# This test will help us understand why persistence files aren't being created

set -e

echo "=== Enhanced Persistence Test with Detailed Logging ==="
echo

cleanup() {
    echo "Cleaning up..."
    pkill -f sequencer || true
    rm -f enhanced_test.db* 2>/dev/null || true
    rm -f enhanced_test.settlement.json 2>/dev/null || true
}

trap cleanup EXIT

echo "1. Building sequencer..."
cargo build --bin sequencer --quiet

echo "2. Starting sequencer with enhanced logging..."
# Start sequencer with more detailed logs
RUST_LOG=debug ./target/debug/sequencer --database-url sqlite:enhanced_test.db --port 3020 > sequencer_detailed.log 2>&1 &
SEQUENCER_PID=$!

# Give sequencer time to start
sleep 3

if ! kill -0 $SEQUENCER_PID 2>/dev/null; then
    echo "❌ FAILED: Sequencer failed to start"
    echo "Sequencer logs:"
    cat sequencer_detailed.log
    exit 1
fi

echo "✅ Sequencer started (PID: $SEQUENCER_PID)"

echo "3. Checking initial state..."
echo "Files before test:"
ls -la enhanced_test.* 2>/dev/null || echo "No enhanced_test files yet"

echo "4. Setting up player balances and submitting bets..."
# First deposit funds for each test player
for i in {1..5}; do
    echo "Depositing funds for player $i..."
    DEPOSIT_RESPONSE=$(curl -s -w "HTTP_CODE:%{http_code}" -X POST http://localhost:3020/v1/deposit \
        -H "Content-Type: application/json" \
        -d "{
            \"player_address\": \"test_player_$(printf \"%03d\" $i)\",
            \"amount\": 10000
        }")
    echo "Deposit $i: $DEPOSIT_RESPONSE"
done

echo "5. Submitting test bets with detailed tracking..."
# Submit bets and track responses
for i in {1..5}; do
    echo "Submitting bet $i..."
    RESPONSE=$(curl -s -w "HTTP_CODE:%{http_code}" -X POST http://localhost:3020/v1/bet \
        -H "Content-Type: application/json" \
        -d "{
            \"player_address\": \"test_player_$(printf \"%03d\" $i)\",
            \"amount\": $((1000 + i * 100)),
            \"guess\": $((i % 2 == 1))
        }")
    echo "Response $i: $RESPONSE"
    sleep 0.5
done

echo "6. Waiting for settlement processing..."
sleep 10

echo "7. Checking what files were created..."
echo "All files with 'enhanced_test' in name:"
find . -name "*enhanced_test*" -type f 2>/dev/null || echo "No enhanced_test files found"

echo "8. Checking settlement.json files:"
find . -name "*.settlement.json" -type f 2>/dev/null || echo "No settlement.json files found"

echo "9. Checking sequencer logs for settlement activity..."
echo "=== Sequencer Logs (last 50 lines) ==="
tail -50 sequencer_detailed.log

echo
echo "10. Testing bet endpoint directly..."
HEALTH_CHECK=$(curl -s http://localhost:3020/health || echo "Health check failed")
echo "Health check: $HEALTH_CHECK"

STATS=$(curl -s http://localhost:3020/stats || echo "Stats failed")
echo "Stats: $STATS"

echo "11. Killing sequencer and checking final state..."
kill $SEQUENCER_PID 2>/dev/null || true
wait $SEQUENCER_PID 2>/dev/null || true

echo "Final file check:"
ls -la enhanced_test.* 2>/dev/null || echo "No enhanced_test files found"
ls -la *.settlement.json 2>/dev/null || echo "No settlement.json files found"

echo
echo "=== Test Analysis ==="
if [ -f "enhanced_test.settlement.json" ]; then
    echo "✅ SUCCESS: Settlement persistence file was created"
    echo "File contents:"
    cat enhanced_test.settlement.json
else
    echo "❌ ISSUE: No settlement persistence file found"
    echo "This suggests either:"
    echo "  1. Bets aren't being added to settlement queue"
    echo "  2. Settlement batch processing isn't triggering"
    echo "  3. Persistence only happens under certain conditions"
fi

echo
echo "Cleaning up test files..."
rm -f sequencer_detailed.log 2>/dev/null || true