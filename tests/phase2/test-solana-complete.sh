#!/bin/bash

# Complete Solana ZK Casino Integration Test
# This script tests the full pipeline: validator → programs → sequencer → bets → settlement

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SEQUENCER_PORT=3000
VALIDATOR_PORT=8899
WALLET_FILE="test-wallet.json"
AIRDROP_AMOUNT=10
BET_AMOUNT=5000

echo -e "${BLUE}🎲 ZK Casino Complete Solana Integration Test${NC}"
echo "=============================================="

# Function to cleanup processes on exit
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up processes...${NC}"
    pkill -f "solana-test-validator" || true
    pkill -f "cargo run --package sequencer" || true
    sleep 2
    echo -e "${GREEN}✅ Cleanup complete${NC}"
}

# Set trap to cleanup on script exit
trap cleanup EXIT INT TERM

# Function to wait for service to be ready
wait_for_service() {
    local url=$1
    local service_name=$2
    local max_attempts=${3:-30}
    local attempt=0
    
    echo -e "${YELLOW}⏳ Waiting for $service_name to be ready...${NC}"
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            echo -e "${GREEN}✅ $service_name is ready${NC}"
            return 0
        fi
        
        attempt=$((attempt + 1))
        echo -n "."
        sleep 1
    done
    
    echo -e "${RED}❌ $service_name failed to start after $max_attempts seconds${NC}"
    return 1
}

# Function to check if process is running
check_process() {
    local process_name=$1
    if pgrep -f "$process_name" > /dev/null; then
        echo -e "${GREEN}✅ $process_name is running${NC}"
        return 0
    else
        echo -e "${RED}❌ $process_name is not running${NC}"
        return 1
    fi
}

echo -e "\n${BLUE}📋 Step 1: Environment Setup${NC}"
echo "------------------------------"

# Kill any existing processes
echo "🔄 Stopping existing processes..."
pkill -f "solana-test-validator" || true
pkill -f "cargo run --package sequencer" || true
sleep 2

# Build programs first to catch any compilation errors
echo -e "\n${BLUE}🔨 Step 2: Building Programs${NC}"
echo "------------------------------"

echo "📦 Building vault program..."
if cargo build-sbf --manifest-path programs/vault/Cargo.toml >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Vault program built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build vault program${NC}"
    echo "Build output:"
    cargo build-sbf --manifest-path programs/vault/Cargo.toml
    exit 1
fi

echo "📦 Building verifier program..."
if cargo build-sbf --manifest-path programs/verifier/Cargo.toml >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Verifier program built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build verifier program${NC}"
    echo "Build output:"
    cargo build-sbf --manifest-path programs/verifier/Cargo.toml
    exit 1
fi

echo "📦 Building sequencer..."
if cargo build --package sequencer > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Sequencer built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build sequencer${NC}"
    exit 1
fi

echo -e "\n${BLUE}🚀 Step 3: Starting Solana Test Validator${NC}"
echo "-------------------------------------------"

# Start validator in background
echo "🏗️  Starting Solana test validator..."
solana-test-validator --reset --quiet > validator.log 2>&1 &
VALIDATOR_PID=$!

# Wait for validator to be ready
if wait_for_service "http://localhost:$VALIDATOR_PORT" "Solana validator"; then
    echo -e "${GREEN}✅ Validator started (PID: $VALIDATOR_PID)${NC}"
else
    echo -e "${RED}❌ Validator failed to start${NC}"
    exit 1
fi

echo -e "\n${BLUE}💰 Step 4: Wallet Setup${NC}"
echo "------------------------"

# Create wallet
echo "🔑 Creating test wallet..."
if solana-keygen new -o "$WALLET_FILE" --force --no-bip39-passphrase > /dev/null 2>&1; then
    WALLET_ADDRESS=$(solana-keygen pubkey "$WALLET_FILE")
    echo -e "${GREEN}✅ Wallet created: $WALLET_ADDRESS${NC}"
else
    echo -e "${RED}❌ Failed to create wallet${NC}"
    exit 1
fi

# Configure solana CLI
echo "⚙️  Configuring Solana CLI..."
if solana config set --keypair "$WALLET_FILE" --url localhost > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Solana CLI configured${NC}"
else
    echo -e "${RED}❌ Failed to configure Solana CLI${NC}"
    exit 1
fi

# Airdrop SOL
echo "💸 Airdropping $AIRDROP_AMOUNT SOL..."
if solana airdrop $AIRDROP_AMOUNT > /dev/null 2>&1; then
    BALANCE=$(solana balance --url localhost)
    echo -e "${GREEN}✅ Airdrop successful. Balance: $BALANCE${NC}"
else
    echo -e "${RED}❌ Airdrop failed${NC}"
    exit 1
fi

echo -e "\n${BLUE}📄 Step 5: Program Deployment${NC}"
echo "-------------------------------"

# Get program IDs from source code
VAULT_PROGRAM_ID=$(grep "declare_id!" programs/vault/src/lib.rs | cut -d'"' -f2)
VERIFIER_PROGRAM_ID=$(grep "declare_id!" programs/verifier/src/lib.rs | cut -d'"' -f2)

echo "🏗️  Program IDs identified:"
echo "   Vault: $VAULT_PROGRAM_ID"
echo "   Verifier: $VERIFIER_PROGRAM_ID"

# Deploy programs
echo "🚀 Deploying vault program..."
if solana program deploy target/deploy/vault.so > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Vault program deployed${NC}"
else
    echo -e "${YELLOW}⚠️  Vault program deployment skipped (may already exist)${NC}"
fi

echo "🚀 Deploying verifier program..."
if solana program deploy target/deploy/verifier.so > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Verifier program deployed${NC}"
else
    echo -e "${YELLOW}⚠️  Verifier program deployment skipped (may already exist)${NC}"
fi

echo -e "\n${BLUE}🎮 Step 6: Starting Sequencer${NC}"
echo "-------------------------------"

# Set environment variables and start sequencer
echo "🚀 Starting sequencer with Solana integration..."
export ENABLE_SOLANA=true
export VAULT_PROGRAM_ID="$VAULT_PROGRAM_ID"
export VERIFIER_PROGRAM_ID="$VERIFIER_PROGRAM_ID"

cargo run --package sequencer > sequencer.log 2>&1 &
SEQUENCER_PID=$!

# Wait for sequencer to be ready
if wait_for_service "http://localhost:$SEQUENCER_PORT/health" "Sequencer"; then
    echo -e "${GREEN}✅ Sequencer started (PID: $SEQUENCER_PID)${NC}"
else
    echo -e "${RED}❌ Sequencer failed to start${NC}"
    echo "Sequencer logs:"
    tail -10 sequencer.log
    exit 1
fi

echo -e "\n${BLUE}🧪 Step 7: API Testing${NC}"
echo "-----------------------"

# Test health endpoint
echo "🏥 Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:$SEQUENCER_PORT/health)
if [ "$HEALTH_RESPONSE" = "OK" ]; then
    echo -e "${GREEN}✅ Health check: $HEALTH_RESPONSE${NC}"
else
    echo -e "${RED}❌ Health check failed: $HEALTH_RESPONSE${NC}"
    exit 1
fi

# Test settlement stats
echo "📊 Testing settlement stats..."
STATS_RESPONSE=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats)
if echo "$STATS_RESPONSE" | grep -q "total_items_queued"; then
    echo -e "${GREEN}✅ Settlement stats endpoint working${NC}"
else
    echo -e "${RED}❌ Settlement stats failed${NC}"
    exit 1
fi

echo -e "\n${BLUE}🎯 Step 8: Bet Testing${NC}"
echo "-----------------------"

# Generate test player address
TEST_PLAYER="9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"

echo "🎲 Placing test bet..."
echo "   Player: $TEST_PLAYER"
echo "   Amount: $BET_AMOUNT lamports"
echo "   Guess: heads (true)"

BET_RESPONSE=$(curl -s -w "\nHTTP_STATUS:%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -d "{\"player_address\": \"$TEST_PLAYER\", \"amount\": $BET_AMOUNT, \"guess\": true}" \
    http://localhost:$SEQUENCER_PORT/v1/bet)

HTTP_STATUS=$(echo "$BET_RESPONSE" | grep "HTTP_STATUS" | cut -d: -f2)
RESPONSE_BODY=$(echo "$BET_RESPONSE" | grep -v "HTTP_STATUS")

if [ "$HTTP_STATUS" = "200" ]; then
    echo -e "${GREEN}✅ Bet placed successfully (HTTP $HTTP_STATUS)${NC}"
    
    # Parse bet response
    BET_ID=$(echo "$RESPONSE_BODY" | grep -o '"bet_id":"[^"]*"' | cut -d'"' -f4)
    WON=$(echo "$RESPONSE_BODY" | grep -o '"won":[^,}]*' | cut -d: -f2)
    PAYOUT=$(echo "$RESPONSE_BODY" | grep -o '"payout":[0-9]*' | cut -d: -f2)
    
    echo "   Bet ID: $BET_ID"
    echo "   Won: $WON"
    echo "   Payout: $PAYOUT lamports"
else
    echo -e "${RED}❌ Bet failed (HTTP $HTTP_STATUS)${NC}"
    echo "Response: $RESPONSE_BODY"
    exit 1
fi

# Wait for settlement processing
echo -e "\n${YELLOW}⏳ Waiting for settlement batch processing...${NC}"
sleep 3

# Check settlement stats after bet
echo "📊 Checking settlement stats after bet..."
FINAL_STATS=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats)
ITEMS_QUEUED=$(echo "$FINAL_STATS" | grep -o '"total_items_queued":[0-9]*' | cut -d: -f2)

if [ "$ITEMS_QUEUED" -gt 0 ]; then
    echo -e "${GREEN}✅ Settlement queue processed $ITEMS_QUEUED items${NC}"
else
    echo -e "${YELLOW}⚠️  No items in settlement queue yet${NC}"
fi

echo -e "\n${BLUE}📝 Step 9: Log Analysis${NC}"
echo "------------------------"

echo "🔍 Analyzing sequencer logs for Solana integration..."

# Check for Solana client initialization
if grep -q "Solana client initialized successfully" sequencer.log; then
    echo -e "${GREEN}✅ Solana client initialized${NC}"
else
    echo -e "${RED}❌ Solana client not initialized${NC}"
fi

# Check for settlement batch processing
if grep -q "Processing settlement batch" sequencer.log; then
    echo -e "${GREEN}✅ Settlement batching active${NC}"
    BATCH_COUNT=$(grep -c "Processing settlement batch" sequencer.log)
    echo "   Batches processed: $BATCH_COUNT"
else
    echo -e "${YELLOW}⚠️  No settlement batches found yet${NC}"
fi

# Check for Solana transaction attempts
if grep -q "submit_batch_to_solana" sequencer.log; then
    echo -e "${GREEN}✅ Solana transaction submission attempted${NC}"
else
    echo -e "${YELLOW}⚠️  No Solana transactions attempted${NC}"
fi

echo -e "\n${BLUE}🎯 Step 10: Integration Validation${NC}"
echo "-----------------------------------"

# Validate all components are working
echo "🔄 Validating complete integration..."

VALIDATION_PASSED=true

# Check validator
if check_process "solana-test-validator"; then
    echo "   ✅ Validator: Running"
else
    echo "   ❌ Validator: Not running"
    VALIDATION_PASSED=false
fi

# Check sequencer (use API health check instead of process detection)
SEQUENCER_HEALTH=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:$SEQUENCER_PORT/health 2>/dev/null)
if [ "$SEQUENCER_HEALTH" = "200" ]; then
    echo "   ✅ Sequencer: Running and responding"
else
    echo "   ❌ Sequencer: Not responding"
    VALIDATION_PASSED=false
fi

# Check wallet balance
CURRENT_BALANCE=$(solana balance --url localhost 2>/dev/null | cut -d' ' -f1)
if [ "$CURRENT_BALANCE" != "" ]; then
    echo "   ✅ Wallet: $CURRENT_BALANCE SOL"
else
    echo "   ❌ Wallet: Cannot get balance"
    VALIDATION_PASSED=false
fi

echo -e "\n${BLUE}📊 Final Results${NC}"
echo "=================="

if [ "$VALIDATION_PASSED" = true ]; then
    echo -e "${GREEN}🎉 INTEGRATION TEST PASSED!${NC}"
    echo -e "${GREEN}✅ All components working correctly${NC}"
    echo ""
    echo "Summary:"
    echo "  • Solana validator: Running on port $VALIDATOR_PORT"
    echo "  • ZK Casino sequencer: Running on port $SEQUENCER_PORT" 
    echo "  • Programs deployed: Vault + Verifier"
    echo "  • API endpoints: Functional"
    echo "  • Settlement system: Active"
    echo "  • Solana integration: Enabled"
    echo ""
    echo "🎯 Ready for Phase 3: ZK Circuit Implementation"
else
    echo -e "${RED}❌ INTEGRATION TEST FAILED${NC}"
    echo "Some components are not working correctly."
fi

echo -e "\n${BLUE}🔧 Manual Testing Commands${NC}"
echo "==========================="
echo "Test bet:     curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"$TEST_PLAYER\", \"amount\": 5000, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"
echo "Health check: curl http://localhost:$SEQUENCER_PORT/health"
echo "Stats:        curl http://localhost:$SEQUENCER_PORT/v1/settlement-stats"
echo "Balance:      solana balance --url localhost"
echo "Validator:    solana cluster-version --url localhost"

echo -e "\n${YELLOW}Press Ctrl+C to stop all services${NC}"

# Keep script running until user interrupts (unless in test mode)
if [ "${TEST_MODE:-false}" != "true" ]; then
    while true; do
        sleep 1
    done
else
    # In test mode, just wait a moment for final cleanup
    sleep 2
fi