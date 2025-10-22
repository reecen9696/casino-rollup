#!/bin/bash

# Debug script to isolate VRF bet hanging issue

echo "=== VRF Bet Hanging Debug Test ==="

# Start fresh sequencer
echo "1. Starting fresh sequencer..."
pkill -f sequencer 2>/dev/null || true
sleep 2

# Start sequencer in debug mode
echo "2. Starting sequencer with VRF enabled..."
RUST_LOG=debug ./target/release/sequencer --enable-vrf &
SEQUENCER_PID=$!
sleep 3

echo "3. Testing health check..."
curl -s http://localhost:3000/health
echo

echo "4. Test Case 1: Bet without deposit (should fail gracefully)"
echo "Attempting bet for new player..."
# Use background process and kill after timeout since timeout command not available on macOS
curl -v -X POST -H 'Content-Type: application/json' \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 1000, "guess": true}' \
  http://localhost:3000/v1/bet &
CURL_PID=$!
sleep 5
if kill -0 $CURL_PID 2>/dev/null; then
  echo "Request hanging - killing after 5 seconds"
  kill $CURL_PID 2>/dev/null
  echo "Result: TIMEOUT"
else
  wait $CURL_PID
  echo "Result: $?"
fi

echo -e "\n5. Test Case 2: Create deposit then bet"
echo "Creating deposit..."
curl -s -X POST -H 'Content-Type: application/json' \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 10000}' \
  http://localhost:3000/v1/deposit
echo

echo "Placing bet..."
curl -v -X POST -H 'Content-Type: application/json' \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 1000, "guess": true}' \
  http://localhost:3000/v1/bet &
CURL_PID=$!
sleep 5
if kill -0 $CURL_PID 2>/dev/null; then
  echo "Request hanging - killing after 5 seconds"
  kill $CURL_PID 2>/dev/null
  echo "Result: TIMEOUT"
else
  wait $CURL_PID
  echo "Result: $?"
fi

echo -e "\n6. Test Case 3: Second bet with same player"
echo "Placing second bet..."
curl -v -X POST -H 'Content-Type: application/json' \
  -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 500, "guess": false}' \
  http://localhost:3000/v1/bet &
CURL_PID=$!
sleep 5
if kill -0 $CURL_PID 2>/dev/null; then
  echo "Request hanging - killing after 5 seconds"
  kill $CURL_PID 2>/dev/null
  echo "Result: TIMEOUT"
else
  wait $CURL_PID
  echo "Result: $?"
fi

echo -e "\n7. Test Case 4: Different player with invalid address format"
echo "Attempting bet with non-Solana address..."
curl -v -X POST -H 'Content-Type: application/json' \
  -d '{"player_address": "invalid_address", "amount": 1000, "guess": true}' \
  http://localhost:3000/v1/bet &
CURL_PID=$!
sleep 5
if kill -0 $CURL_PID 2>/dev/null; then
  echo "Request hanging - killing after 5 seconds"
  kill $CURL_PID 2>/dev/null
  echo "Result: TIMEOUT"
else
  wait $CURL_PID
  echo "Result: $?"
fi

echo -e "\n8. Cleanup"
echo "Stopping sequencer..."
kill $SEQUENCER_PID 2>/dev/null
wait $SEQUENCER_PID 2>/dev/null

echo "Debug test complete."