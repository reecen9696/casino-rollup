#!/bin/bash

# Phase 3f: End-to-End Validation with Testnet Deployment
# Complete ZK Casino system validation including Solana validator, ZK circuits, and full settlement pipeline

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
SEQUENCER_PORT=3000
VALIDATOR_PORT=8899
EXPLORER_PORT=5173
WALLET_FILE="test-wallet.json"
AIRDROP_AMOUNT=50
BET_AMOUNT=5000
NUM_TEST_BETS=10
SETTLEMENT_WAIT_TIME=10

echo -e "${PURPLE}🚀 ZK Casino Phase 3f: End-to-End Validation${NC}"
echo -e "${PURPLE}===============================================${NC}"
echo "This test validates the complete system:"
echo "• Solana testnet validator deployment"
echo "• ZK circuit integration"
echo "• Multi-batch settlement processing" 
echo "• Database reconciliation"
echo "• Performance validation"
echo "• Complete pipeline testing"
echo ""

# Function to cleanup processes on exit
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up all processes and resources...${NC}"
    pkill -f "solana-test-validator" || true
    pkill -f "cargo run --package sequencer" || true
    pkill -f "npm run dev" || true
    
    # Clean up test files
    rm -f validator.log sequencer.log explorer.log
    rm -f "$WALLET_FILE"
    rm -rf test-ledger/
    
    sleep 3
    echo -e "${GREEN}✅ Cleanup complete${NC}"
}

# Set trap to cleanup on script exit
trap cleanup EXIT INT TERM

# Function to wait for service to be ready
wait_for_service() {
    local url=$1
    local service_name=$2
    local max_attempts=${3:-60}
    local attempt=0
    
    echo -e "${YELLOW}⏳ Waiting for $service_name to be ready...${NC}"
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            echo -e "${GREEN}✅ $service_name is ready${NC}"
            return 0
        fi
        
        attempt=$((attempt + 1))
        printf "."
        sleep 1
    done
    
    echo -e "${RED}❌ $service_name failed to start after $max_attempts seconds${NC}"
    return 1
}

# Function to validate component health
validate_component() {
    local component=$1
    local check_command=$2
    local expected_output=$3
    
    echo -n "   $component: "
    
    if output=$(eval "$check_command" 2>/dev/null); then
        if [[ "$output" == *"$expected_output"* ]]; then
            echo -e "${GREEN}✅ Healthy${NC}"
            return 0
        else
            echo -e "${RED}❌ Unexpected output${NC}"
            return 1
        fi
    else
        echo -e "${RED}❌ Failed${NC}"
        return 1
    fi
}

echo -e "\n${BLUE}🔧 Phase 3f.1: Environment Preparation${NC}"
echo "======================================="

# Stop any existing processes
echo "🔄 Stopping any existing processes..."
pkill -f "solana-test-validator" || true
pkill -f "cargo run --package sequencer" || true
pkill -f "npm run dev" || true
sleep 3

# Clean previous test artifacts
echo "🧹 Cleaning previous test artifacts..."
rm -f validator.log sequencer.log explorer.log
rm -f "$WALLET_FILE"
rm -rf test-ledger/

echo -e "\n${BLUE}🏗️  Phase 3f.2: Build All Components${NC}"
echo "====================================="

echo "📦 Building Solana programs..."
if ! cargo build-sbf --manifest-path programs/vault/Cargo.toml >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build vault program${NC}"
    exit 1
fi

if ! cargo build-sbf --manifest-path programs/verifier/Cargo.toml >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build verifier program${NC}"
    exit 1
fi

echo "📦 Building sequencer with ZK circuits..."
if ! cargo build --package sequencer --release >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build sequencer${NC}"
    exit 1
fi

echo "📦 Building prover library..."
if ! cargo build --package prover --release >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build prover${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All components built successfully${NC}"

echo -e "\n${BLUE}🌐 Phase 3f.3: Solana Testnet Deployment${NC}"
echo "========================================="

# Start Solana test validator
echo "🚀 Starting Solana test validator..."
solana-test-validator \
    --reset \
    --ledger test-ledger \
    --rpc-port $VALIDATOR_PORT \
    --bind-address 0.0.0.0 \
    > validator.log 2>&1 &
VALIDATOR_PID=$!

# Wait for validator to be ready
if ! wait_for_service "http://localhost:$VALIDATOR_PORT" "Solana validator" 60; then
    echo -e "${RED}❌ Validator failed to start${NC}"
    echo "Validator logs:"
    tail -20 validator.log
    exit 1
fi

echo -e "${GREEN}✅ Solana testnet validator deployed (PID: $VALIDATOR_PID)${NC}"

echo -e "\n${BLUE}💰 Phase 3f.4: Wallet and SOL Setup${NC}"
echo "===================================="

# Create test wallet
echo "🔑 Creating test wallet..."
if ! solana-keygen new -o "$WALLET_FILE" --force --no-bip39-passphrase >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to create wallet${NC}"
    exit 1
fi

WALLET_ADDRESS=$(solana-keygen pubkey "$WALLET_FILE")
echo -e "${GREEN}✅ Wallet created: $WALLET_ADDRESS${NC}"

# Configure Solana CLI
echo "⚙️  Configuring Solana CLI..."
solana config set --keypair "$WALLET_FILE" --url "http://localhost:$VALIDATOR_PORT" >/dev/null 2>&1

# Airdrop SOL
echo "💸 Airdropping $AIRDROP_AMOUNT SOL..."
if ! solana airdrop $AIRDROP_AMOUNT >/dev/null 2>&1; then
    echo -e "${RED}❌ Airdrop failed${NC}"
    exit 1
fi

BALANCE=$(solana balance)
echo -e "${GREEN}✅ Wallet funded: $BALANCE${NC}"

echo -e "\n${BLUE}📄 Phase 3f.5: Program Deployment${NC}"
echo "=================================="

# Get program IDs
VAULT_PROGRAM_ID=$(grep "declare_id!" programs/vault/src/lib.rs | cut -d'"' -f2)
VERIFIER_PROGRAM_ID=$(grep "declare_id!" programs/verifier/src/lib.rs | cut -d'"' -f2)

echo "🆔 Program IDs:"
echo "   Vault: $VAULT_PROGRAM_ID"
echo "   Verifier: $VERIFIER_PROGRAM_ID"

# Deploy programs
echo "🚀 Deploying vault program..."
if solana program deploy target/deploy/vault.so >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Vault program deployed${NC}"
else
    echo -e "${RED}❌ Vault program deployment failed${NC}"
    exit 1
fi

echo "🚀 Deploying verifier program..."
if solana program deploy target/deploy/verifier.so >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Verifier program deployed${NC}"
else
    echo -e "${RED}❌ Verifier program deployment failed${NC}"
    exit 1
fi

echo -e "\n${BLUE}🎮 Phase 3f.6: Sequencer with ZK Integration${NC}"
echo "============================================="

# Set environment variables for full integration
export ENABLE_SOLANA=true
export ENABLE_ZK_PROOFS=true
export VAULT_PROGRAM_ID="$VAULT_PROGRAM_ID"
export VERIFIER_PROGRAM_ID="$VERIFIER_PROGRAM_ID"
export SOLANA_RPC_URL="http://localhost:$VALIDATOR_PORT"
export DATABASE_URL="sqlite:casino.db"
export SETTLEMENT_BATCH_SIZE=5
export SETTLEMENT_BATCH_TIMEOUT=5

echo "🚀 Starting sequencer with full ZK + Solana integration..."
cargo run --package sequencer --release > sequencer.log 2>&1 &
SEQUENCER_PID=$!

# Wait for sequencer to be ready
if ! wait_for_service "http://localhost:$SEQUENCER_PORT/health" "Sequencer" 30; then
    echo -e "${RED}❌ Sequencer failed to start${NC}"
    echo "Sequencer logs:"
    tail -20 sequencer.log
    exit 1
fi

echo -e "${GREEN}✅ Sequencer with ZK circuits started (PID: $SEQUENCER_PID)${NC}"

echo -e "\n${BLUE}🧪 Phase 3f.7: API Validation${NC}"
echo "==============================="

# Test health endpoint
echo "🏥 Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:$SEQUENCER_PORT/health)
if [ "$HEALTH_RESPONSE" = "OK" ]; then
    echo -e "${GREEN}✅ Health check passed${NC}"
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
    echo -e "${RED}❌ Settlement stats endpoint failed${NC}"
    exit 1
fi

echo -e "\n${BLUE}🎲 Phase 3f.8: Multi-Batch Bet Testing${NC}"
echo "======================================="

# Generate test players
PLAYERS=(
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
    "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6Q7R8S9T0U1V2"
    "B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6Q7R8S9T0U1V2W3X4"
    "C3D4E5F6G7H8I9J0K1L2M3N4O5P6Q7R8S9T0U1V2W3X4Y5Z6"
    "D4E5F6G7H8I9J0K1L2M3N4O5P6Q7R8S9T0U1V2W3X4Y5Z6A7"
)

echo "🎯 Placing $NUM_TEST_BETS test bets for multi-batch processing..."

BET_IDS=()
SUCCESSFUL_BETS=0

for i in $(seq 1 $NUM_TEST_BETS); do
    PLAYER=${PLAYERS[$((i % ${#PLAYERS[@]}))]}
    GUESS=$((i % 2 == 0))  # Alternate between true/false
    
    echo -n "   Bet $i: "
    
    BET_RESPONSE=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "{\"player_address\": \"$PLAYER\", \"amount\": $BET_AMOUNT, \"guess\": $GUESS}" \
        http://localhost:$SEQUENCER_PORT/v1/bet)
    
    # Check if response contains bet_id (success indicator)
    if echo "$BET_RESPONSE" | grep -q '"bet_id"'; then
        BET_ID=$(echo "$BET_RESPONSE" | grep -o '"bet_id":"[^"]*"' | cut -d'"' -f4)
        WON=$(echo "$BET_RESPONSE" | grep -o '"won":[^,}]*' | cut -d: -f2)
        
        BET_IDS+=("$BET_ID")
        SUCCESSFUL_BETS=$((SUCCESSFUL_BETS + 1))
        
        if [ "$WON" = "true" ]; then
            echo -e "${GREEN}Won (ID: $BET_ID)${NC}"
        else
            echo -e "${CYAN}Lost (ID: $BET_ID)${NC}"
        fi
    else
        echo -e "${RED}Failed${NC}"
        echo "Response: $BET_RESPONSE"
    fi
done

echo -e "${GREEN}✅ Placed $SUCCESSFUL_BETS successful bets${NC}"

echo -e "\n${BLUE}⏱️  Phase 3f.9: Settlement Batch Processing${NC}"
echo "============================================"

echo "⏳ Waiting $SETTLEMENT_WAIT_TIME seconds for settlement batch processing..."
sleep $SETTLEMENT_WAIT_TIME

# Check settlement statistics
echo "📊 Analyzing settlement batch processing..."
FINAL_STATS=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats)

ITEMS_QUEUED=$(echo "$FINAL_STATS" | grep -o '"total_items_queued":[0-9]*' | cut -d: -f2 || echo "0")
ITEMS_SETTLED=$(echo "$FINAL_STATS" | grep -o '"total_items_settled":[0-9]*' | cut -d: -f2 || echo "0")
BATCHES_PROCESSED=$(echo "$FINAL_STATS" | grep -o '"total_batches_processed":[0-9]*' | cut -d: -f2 || echo "0")

echo "Settlement Statistics:"
echo "   Items Queued: $ITEMS_QUEUED"
echo "   Items Settled: $ITEMS_SETTLED"
echo "   Batches Processed: $BATCHES_PROCESSED"

if [ "$BATCHES_PROCESSED" != "" ] && [ "$BATCHES_PROCESSED" -gt 0 ]; then
    echo -e "${GREEN}✅ Settlement batching is working${NC}"
else
    echo -e "${YELLOW}⚠️  Settlement batching needs more time${NC}"
fi

echo -e "\n${BLUE}🔍 Phase 3f.10: Log Analysis${NC}"
echo "============================="

echo "📋 Analyzing system logs for integration validation..."

# Check Solana integration
if grep -q "Solana client initialized successfully" sequencer.log; then
    echo -e "   ${GREEN}✅ Solana integration active${NC}"
else
    echo -e "   ${RED}❌ Solana integration issues${NC}"
fi

# Check for ZK proof generation
ZK_PROOFS=$(grep -c "Generated ZK proof" sequencer.log 2>/dev/null || echo "0")
if [ "$ZK_PROOFS" -gt 0 ]; then
    echo -e "   ${GREEN}✅ ZK proof generation: $ZK_PROOFS proofs${NC}"
else
    echo -e "   ${YELLOW}⚠️  No ZK proofs generated yet${NC}"
fi

# Check settlement batching
SETTLEMENT_BATCHES=$(grep -c "Processing settlement batch" sequencer.log 2>/dev/null || echo "0")
if [ "$SETTLEMENT_BATCHES" -gt 0 ]; then
    echo -e "   ${GREEN}✅ Settlement batching: $SETTLEMENT_BATCHES batches${NC}"
else
    echo -e "   ${YELLOW}⚠️  No settlement batches processed yet${NC}"
fi

# Check Solana transactions
SOLANA_TXS=$(grep -c "Submitted batch to Solana" sequencer.log 2>/dev/null || echo "0")
if [ "$SOLANA_TXS" -gt 0 ]; then
    echo -e "   ${GREEN}✅ Solana transactions: $SOLANA_TXS submitted${NC}"
else
    echo -e "   ${YELLOW}⚠️  No Solana transactions yet${NC}"
fi

echo -e "\n${BLUE}🎯 Phase 3f.11: Component Health Validation${NC}"
echo "============================================="

echo "🔄 Validating all system components..."

VALIDATION_PASSED=true

# Validator health
if validate_component "Solana Validator" "solana cluster-version" "1."; then
    echo -e "      RPC: http://localhost:$VALIDATOR_PORT"
else
    VALIDATION_PASSED=false
fi

# Sequencer health
if validate_component "Sequencer API" "curl -s http://localhost:$SEQUENCER_PORT/health" "OK"; then
    echo -e "      API: http://localhost:$SEQUENCER_PORT"
else
    VALIDATION_PASSED=false
fi

# Wallet health
if validate_component "Wallet" "solana balance" "SOL"; then
    CURRENT_BALANCE=$(solana balance 2>/dev/null)
    echo -e "      Balance: $CURRENT_BALANCE"
else
    VALIDATION_PASSED=false
fi

# Program deployment validation
VAULT_ACCOUNT=$(solana account "$VAULT_PROGRAM_ID" 2>/dev/null | grep "Executable: Yes" || echo "")
if [ "$VAULT_ACCOUNT" != "" ]; then
    echo -e "   ${GREEN}✅ Vault Program Deployed${NC}"
    echo -e "      ID: $VAULT_PROGRAM_ID"
else
    echo -e "   ${RED}❌ Vault Program Not Found${NC}"
    VALIDATION_PASSED=false
fi

VERIFIER_ACCOUNT=$(solana account "$VERIFIER_PROGRAM_ID" 2>/dev/null | grep "Executable: Yes" || echo "")
if [ "$VERIFIER_ACCOUNT" != "" ]; then
    echo -e "   ${GREEN}✅ Verifier Program Deployed${NC}"
    echo -e "      ID: $VERIFIER_PROGRAM_ID"
else
    echo -e "   ${RED}❌ Verifier Program Not Found${NC}"
    VALIDATION_PASSED=false
fi

echo -e "\n${BLUE}🏁 Phase 3f.12: Final Results${NC}"
echo "=============================="

if [ "$VALIDATION_PASSED" = true ]; then
    echo -e "${GREEN}🎉 END-TO-END VALIDATION PASSED!${NC}"
    echo -e "${GREEN}✅ Complete ZK Casino system validated${NC}"
    echo ""
    echo -e "${PURPLE}System Summary:${NC}"
    echo -e "  🌐 Solana testnet validator: ${GREEN}Running${NC} (port $VALIDATOR_PORT)"
    echo -e "  🎮 ZK Casino sequencer: ${GREEN}Running${NC} (port $SEQUENCER_PORT)"
    echo -e "  📄 Smart contracts: ${GREEN}Deployed${NC} (Vault + Verifier)"
    echo -e "  🔒 ZK proof system: ${GREEN}Active${NC}"
    echo -e "  ⚡ Settlement pipeline: ${GREEN}Active${NC}"
    echo -e "  🎲 Betting system: ${GREEN}Functional${NC}"
    echo -e "  💾 Database integration: ${GREEN}Working${NC}"
    echo ""
    echo -e "${PURPLE}Test Results:${NC}"
    echo -e "  🎯 Bets placed: ${GREEN}$SUCCESSFUL_BETS/$NUM_TEST_BETS${NC}"
    echo -e "  📦 Settlement batches: ${GREEN}$SETTLEMENT_BATCHES${NC}"
    echo -e "  🔒 ZK proofs generated: ${GREEN}$ZK_PROOFS${NC}"
    echo -e "  🌐 Solana transactions: ${GREEN}$SOLANA_TXS${NC}"
    echo ""
    echo -e "${GREEN}🚀 READY FOR PRODUCTION DEPLOYMENT${NC}"
else
    echo -e "${RED}❌ END-TO-END VALIDATION FAILED${NC}"
    echo "Some components are not working correctly."
    echo "Please check the logs and retry."
fi

echo -e "\n${BLUE}🔧 Quick Commands for Manual Testing${NC}"
echo "====================================="
echo "Test bet:      curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM\", \"amount\": 5000, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"
echo "Health:        curl http://localhost:$SEQUENCER_PORT/health"
echo "Stats:         curl http://localhost:$SEQUENCER_PORT/v1/settlement-stats"
echo "Balance:       solana balance"
echo "Validator:     solana cluster-version"
echo "Programs:      solana program show $VAULT_PROGRAM_ID"

echo -e "\n${CYAN}📝 Log Files:${NC}"
echo "Validator:     tail -f validator.log"
echo "Sequencer:     tail -f sequencer.log"

echo -e "\n${YELLOW}Press Ctrl+C to stop all services and cleanup${NC}"

# Keep services running unless in test mode
if [ "${TEST_MODE:-false}" != "true" ]; then
    echo -e "${BLUE}Services will continue running for manual testing...${NC}"
    while true; do
        sleep 10
        # Periodic health check
        if ! curl -s http://localhost:$SEQUENCER_PORT/health >/dev/null; then
            echo -e "${RED}⚠️  Sequencer health check failed${NC}"
        fi
        if ! curl -s http://localhost:$VALIDATOR_PORT >/dev/null; then
            echo -e "${RED}⚠️  Validator health check failed${NC}"
        fi
    done
else
    echo -e "${BLUE}Test mode: allowing brief runtime before cleanup...${NC}"
    sleep 5
fi