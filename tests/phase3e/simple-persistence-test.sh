#!/bin/bash

# Simple working persistence test based on manual-test.sh format

set -e

echo "=== Simple Persistence Test ==="

cleanup() {
    echo "Cleaning up..."
    pkill -f sequencer || true
    rm -f simple_test.db* 2>/dev/null || true
}

trap cleanup EXIT

echo "Starting sequencer..."
./target/debug/sequencer --database-url sqlite:simple_test.db --port 3040 > sequencer.log 2>&1 &
SEQUENCER_PID=$!

sleep 4

if ! kill -0 $SEQUENCER_PID 2>/dev/null; then
    echo "❌ Sequencer failed to start"
    cat sequencer.log
    exit 1
fi

echo "✅ Sequencer started (PID: $SEQUENCER_PID)"

echo "Testing valid bets..."

# Submit 60 bets to trigger batch processing (batch size is 50)
for i in {1..60}; do
    RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
      -d "{\"player_address\": \"player_$(printf "%04d" $i)\", \"amount\": $((1000 + i * 10)), \"guess\": $((i % 2 == 1))}" \
      http://localhost:3040/v1/bet)
    
    if [[ $i -le 5 ]] || [[ $i -ge 55 ]]; then
        echo "Bet $i response: $RESPONSE"
    elif [[ $i == 30 ]]; then
        echo "... (processing bets 6-54) ..."
    fi
    
    # Small delay to avoid overwhelming
    sleep 0.02
done

echo "Waiting for settlement processing..."
sleep 15

echo "Checking for settlement files..."
echo "Files in current directory:"
ls -la *.settlement.json 2>/dev/null || echo "No settlement.json files"
ls -la simple_test.* 2>/dev/null || echo "No simple_test files"

echo "Checking sequencer logs for settlement activity..."
echo "=== Last 30 lines of sequencer.log ==="
tail -30 sequencer.log

echo "Stopping sequencer..."
kill $SEQUENCER_PID 2>/dev/null || true
wait $SEQUENCER_PID 2>/dev/null || true

echo "Final check for persistence files..."
find . -name "*settlement*" -type f 2>/dev/null || echo "No settlement files found"

echo "Test completed."