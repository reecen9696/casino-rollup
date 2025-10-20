#!/bin/bash

echo "🎯 ZK Casino High-Performance Load Test"
echo "======================================="
echo "Testing VF Node patterns: spawn_blocking, DashMap, instant response"
echo ""

# Test configuration
CONCURRENT_USERS=50
REQUESTS_PER_USER=20
TOTAL_REQUESTS=$((CONCURRENT_USERS * REQUESTS_PER_USER))

echo "Configuration:"
echo "- Concurrent users: $CONCURRENT_USERS"
echo "- Requests per user: $REQUESTS_PER_USER"
echo "- Total requests: $TOTAL_REQUESTS"
echo ""

# Function to send a single bet request
send_bet() {
    local user_id=$1
    local request_id=$2
    
    curl -s -X POST http://localhost:3030/v1/bet \
        -H "Content-Type: application/json" \
        -d "{\"player_address\":\"player_${user_id}\",\"amount\":10000,\"guess\":true}" \
        > /dev/null
    
    if [ $? -eq 0 ]; then
        echo "✓"
    else
        echo "✗"
    fi
}

# Record start time
start_time=$(date +%s.%N)

echo "🚀 Starting load test..."

# Create concurrent users
for user in $(seq 1 $CONCURRENT_USERS); do
    {
        for req in $(seq 1 $REQUESTS_PER_USER); do
            send_bet $user $req
        done
    } &
done

# Wait for all background jobs to complete
wait

# Record end time
end_time=$(date +%s.%N)

# Calculate performance metrics
duration=$(echo "$end_time - $start_time" | bc)
rps=$(echo "scale=2; $TOTAL_REQUESTS / $duration" | bc)
avg_latency=$(echo "scale=2; $duration * 1000 / $TOTAL_REQUESTS" | bc)

echo ""
echo "📊 Performance Results:"
echo "======================"
echo "Total time: ${duration}s"
echo "Requests per second: ${rps} RPS"
echo "Average latency: ${avg_latency}ms per request"
echo ""

# Compare with VF Node benchmark
echo "🎯 VF Node Comparison:"
echo "====================="
echo "VF Node target: 3000+ RPS, <1ms latency"
echo "ZK Casino result: ${rps} RPS, ${avg_latency}ms latency"

if (( $(echo "$rps > 1000" | bc -l) )); then
    echo "🎉 EXCELLENT: High-performance patterns working!"
else
    echo "⚠️  Performance needs optimization"
fi

echo ""
echo "✅ Load test completed"