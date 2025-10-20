#!/bin/bash

# Manual test script for the betting endpoint
echo "🎲 ZK Casino Betting Endpoint Demo"
echo "=================================="

# Kill any existing processes on ports 3001-3003
for port in 3001 3002 3003; do
    lsof -ti :$port | xargs kill -9 2>/dev/null || true
done

echo "Starting sequencer on port 3002..."
cd /Users/reece/code/projects/zkcasino
cargo run --package sequencer -- --port 3002 &
SEQUENCER_PID=$!

# Wait for startup
sleep 4

echo -e "\n1. Testing health endpoint:"
curl -s http://localhost:3002/health
echo -e "\n"

echo -e "\n2. Testing valid bet (heads):"
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 5000, "guess": true}' \
  http://localhost:3002/v1/bet | jq .
echo -e "\n"

echo -e "\n3. Testing valid bet (tails):"
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 10000, "guess": false}' \
  http://localhost:3002/v1/bet | jq .
echo -e "\n"

echo -e "\n4. Testing invalid bet (amount too small):"
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 500, "guess": true}' \
  http://localhost:3002/v1/bet
echo -e "\n"

echo -e "\nStopping sequencer..."
kill $SEQUENCER_PID 2>/dev/null
wait $SEQUENCER_PID 2>/dev/null

echo -e "\n✅ Demo complete!"
echo -e "\nTo run manually:"
echo "1. cargo run --package sequencer -- --port 3002"
echo "2. curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM\", \"amount\": 5000, \"guess\": true}' http://localhost:3002/v1/bet"