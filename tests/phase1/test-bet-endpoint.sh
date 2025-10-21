#!/bin/bash

# Test script for the betting endpoint
# This script will start the sequencer and test the /v1/bet endpoint

set -e

echo "🎲 Testing ZK Casino Betting Endpoint"
echo "======================================"

# Start the sequencer in the background
echo "Starting sequencer on port 3000..."
cd /Users/reece/code/projects/zkcasino
cargo run --package sequencer &
SEQUENCER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 3

# Test function
test_endpoint() {
    local description="$1"
    local json_data="$2"
    local expected_status="$3"
    
    echo
    echo "Test: $description"
    echo "Request: $json_data"
    echo "---"
    
    response=$(curl -s -w "HTTPSTATUS:%{http_code}" \
        -X POST \
        -H "Content-Type: application/json" \
        -d "$json_data" \
        http://localhost:3000/v1/bet)
    
    # Extract status code
    status_code=$(echo "$response" | grep -o "HTTPSTATUS:[0-9]*" | cut -d: -f2)
    
    # Extract response body
    body=$(echo "$response" | sed 's/HTTPSTATUS:[0-9]*$//')
    
    echo "Status: $status_code"
    echo "Response: $body"
    
    if [ "$status_code" = "$expected_status" ]; then
        echo "✅ PASS"
    else
        echo "❌ FAIL (expected $expected_status, got $status_code)"
    fi
}

# Test health endpoint
echo
echo "Testing health endpoint..."
health_response=$(curl -s http://localhost:3000/health)
echo "Health response: $health_response"

# Test valid bet (heads)
test_endpoint "Valid bet - Heads" \
    '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 5000, "guess": true}' \
    "200"

# Test valid bet (tails)
test_endpoint "Valid bet - Tails" \
    '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 10000, "guess": false}' \
    "200"

# Test invalid amount (too small)
test_endpoint "Invalid amount (too small)" \
    '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 500, "guess": true}' \
    "400"

# Test malformed JSON
test_endpoint "Malformed JSON" \
    '{"player_address": "invalid", "amount": "not_a_number"}' \
    "400"

# Test missing fields
test_endpoint "Missing fields" \
    '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"}' \
    "400"

echo
echo "======================================"
echo "Testing complete! Stopping sequencer..."

# Stop the sequencer
kill $SEQUENCER_PID 2>/dev/null || true
wait $SEQUENCER_PID 2>/dev/null || true

echo "✅ Sequencer stopped"
echo
echo "🎯 To manually test the endpoint:"
echo "1. Start sequencer: cargo run --package sequencer"
echo "2. Test bet: curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM\", \"amount\": 5000, \"guess\": true}' http://localhost:3000/v1/bet"

# Exit with success if we got this far
exit 0
echo