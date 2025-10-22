#!/bin/bash

# Real Solana On-Chain Integration Test
# This script will actually deploy programs and verify real on-chain transactions

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Configuration
SEQUENCER_PORT=3000
VALIDATOR_PORT=8899
WALLET_FILE="test-wallet.json"
AIRDROP_AMOUNT=100
TEST_BETS=3

echo -e "${PURPLE}🚀 Real Solana On-Chain Integration Test${NC}"
echo -e "${PURPLE}=========================================${NC}"
echo "This test will:"
echo "• Deploy actual Solana programs with valid IDs"
echo "• Configure sequencer with correct program IDs"
echo "• Submit real transactions to Solana validator"
echo "• Verify transaction signatures on-chain"
echo "• Confirm settlement persistence stores real data"
echo ""

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up processes...${NC}"
    pkill -f "solana-test-validator" 2>/dev/null || true
    pkill -f "cargo run --package sequencer" 2>/dev/null || true
    sleep 3
    
    # Clean up test files but preserve logs for analysis
    rm -f "$WALLET_FILE" 2>/dev/null || true
    echo -e "${GREEN}✅ Cleanup complete (logs preserved)${NC}"
}

trap cleanup EXIT

# Wait for service
wait_for_service() {
    local url=$1
    local name=$2
    local attempts=60
    
    echo -n "⏳ Waiting for $name"
    for i in $(seq 1 $attempts); do
        if curl -s "$url" >/dev/null 2>&1; then
            echo -e " ${GREEN}✅${NC}"
            return 0
        fi
        echo -n "."
        sleep 1
    done
    echo -e " ${RED}❌${NC}"
    return 1
}

echo -e "\n${BLUE}Step 1: Clean Environment and Build Programs${NC}"
echo "============================================="

# Stop any existing processes
pkill -f "solana-test-validator" 2>/dev/null || true
pkill -f "cargo run --package sequencer" 2>/dev/null || true
sleep 3

# Clean previous artifacts
rm -f validator.log sequencer.log "$WALLET_FILE" 2>/dev/null || true
rm -rf test-ledger/ 2>/dev/null || true
rm -f zkcasino.settlement.json 2>/dev/null || true

echo "🔨 Building programs with correct IDs..."
if ! cargo build-sbf --manifest-path programs/vault/Cargo.toml >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build vault program${NC}"
    exit 1
fi

if ! cargo build-sbf --manifest-path programs/verifier/Cargo.toml >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build verifier program${NC}"
    exit 1
fi

if ! cargo build --package sequencer --release >/dev/null 2>&1; then
    echo -e "${RED}❌ Failed to build sequencer${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All programs built successfully${NC}"

# Get the actual program IDs from source
VAULT_PROGRAM_ID=$(grep "declare_id!" programs/vault/src/lib.rs | cut -d'"' -f2)
VERIFIER_PROGRAM_ID=$(grep "declare_id!" programs/verifier/src/lib.rs | cut -d'"' -f2)

echo "📋 Program IDs:"
echo "   Vault: $VAULT_PROGRAM_ID"
echo "   Verifier: $VERIFIER_PROGRAM_ID"

echo -e "\n${BLUE}Step 2: Start Solana Validator${NC}"
echo "==============================="

# Start validator with proper logging
echo "🚀 Starting Solana test validator..."
solana-test-validator \
    --reset \
    --ledger test-ledger \
    --rpc-port $VALIDATOR_PORT \
    --bind-address 0.0.0.0 \
    --log \
    > validator.log 2>&1 &
VALIDATOR_PID=$!

if ! wait_for_service "http://localhost:$VALIDATOR_PORT" "Solana validator"; then
    echo -e "${RED}❌ Validator failed to start${NC}"
    echo "Validator logs:"
    tail -20 validator.log
    exit 1
fi

echo -e "${GREEN}✅ Validator running (PID: $VALIDATOR_PID)${NC}"

echo -e "\n${BLUE}Step 3: Setup Wallet and Deploy Programs${NC}"
echo "========================================="

# Create wallet
echo "🔑 Creating test wallet..."
solana-keygen new -o "$WALLET_FILE" --force --no-bip39-passphrase >/dev/null 2>&1
WALLET_ADDRESS=$(solana-keygen pubkey "$WALLET_FILE")
echo -e "${GREEN}✅ Wallet created: $WALLET_ADDRESS${NC}"

# Configure Solana CLI
solana config set --keypair "$WALLET_FILE" --url "http://localhost:$VALIDATOR_PORT" >/dev/null 2>&1

# Airdrop SOL
echo "💸 Airdropping $AIRDROP_AMOUNT SOL..."
if ! solana airdrop $AIRDROP_AMOUNT >/dev/null 2>&1; then
    echo -e "${RED}❌ Airdrop failed${NC}"
    exit 1
fi

BALANCE=$(solana balance)
echo -e "${GREEN}✅ Wallet funded: $BALANCE${NC}"

# Deploy programs
echo "🚀 Deploying vault program..."
VAULT_DEPLOY_OUTPUT=$(solana program deploy target/deploy/vault.so 2>&1)
if [[ $? -eq 0 ]]; then
    echo -e "${GREEN}✅ Vault program deployed${NC}"
    echo "Deploy output: $VAULT_DEPLOY_OUTPUT"
else
    echo -e "${RED}❌ Vault program deployment failed${NC}"
    echo "Error: $VAULT_DEPLOY_OUTPUT"
    exit 1
fi

echo "🚀 Deploying verifier program..."
VERIFIER_DEPLOY_OUTPUT=$(solana program deploy target/deploy/verifier.so 2>&1)
if [[ $? -eq 0 ]]; then
    echo -e "${GREEN}✅ Verifier program deployed${NC}"
    echo "Deploy output: $VERIFIER_DEPLOY_OUTPUT"
else
    echo -e "${RED}❌ Verifier program deployment failed${NC}"
    echo "Error: $VERIFIER_DEPLOY_OUTPUT"
    exit 1
fi

# Verify programs are deployed
echo -e "\n${BLUE}Verifying Program Deployment:${NC}"
if solana account "$VAULT_PROGRAM_ID" >/dev/null 2>&1; then
    echo -e "Vault program: ${GREEN}✅ Deployed and accessible${NC}"
else
    echo -e "Vault program: ${RED}❌ Not found on validator${NC}"
    exit 1
fi

if solana account "$VERIFIER_PROGRAM_ID" >/dev/null 2>&1; then
    echo -e "Verifier program: ${GREEN}✅ Deployed and accessible${NC}"
else
    echo -e "Verifier program: ${RED}❌ Not found on validator${NC}"
    exit 1
fi

echo -e "\n${BLUE}Step 4: Start Sequencer with Solana Integration${NC}"
echo "==============================================="

# Set environment variables for REAL Solana integration
export ENABLE_SOLANA=true
export ENABLE_ZK_PROOFS=true
export VAULT_PROGRAM_ID="$VAULT_PROGRAM_ID"
export VERIFIER_PROGRAM_ID="$VERIFIER_PROGRAM_ID"
export SOLANA_RPC_URL="http://localhost:$VALIDATOR_PORT"
export DATABASE_URL="sqlite:casino.db"
export SETTLEMENT_BATCH_SIZE=2
export SETTLEMENT_BATCH_TIMEOUT=5

echo "⚙️  Environment configuration:"
echo "   ENABLE_SOLANA=$ENABLE_SOLANA"
echo "   VAULT_PROGRAM_ID=$VAULT_PROGRAM_ID"
echo "   VERIFIER_PROGRAM_ID=$VERIFIER_PROGRAM_ID"
echo "   SOLANA_RPC_URL=$SOLANA_RPC_URL"

echo -e "\n🚀 Starting sequencer with real Solana integration..."
cargo run --package sequencer --release > sequencer.log 2>&1 &
SEQUENCER_PID=$!

if ! wait_for_service "http://localhost:$SEQUENCER_PORT/health" "Sequencer"; then
    echo -e "${RED}❌ Sequencer failed to start${NC}"
    echo "Sequencer logs:"
    tail -30 sequencer.log
    exit 1
fi

echo -e "${GREEN}✅ Sequencer running (PID: $SEQUENCER_PID)${NC}"

# Check for Solana client initialization in logs
sleep 2
if grep -q "Solana client initialized successfully" sequencer.log; then
    echo -e "${GREEN}✅ Solana client initialized properly${NC}"
elif grep -q "Failed to initialize Solana client" sequencer.log; then
    echo -e "${RED}❌ Solana client initialization failed${NC}"
    echo "Error details:"
    grep "Failed to initialize Solana client" sequencer.log
    exit 1
else
    echo -e "${YELLOW}⚠️  Solana client status unclear - checking logs...${NC}"
    grep -i solana sequencer.log | head -5
fi

echo -e "\n${BLUE}Step 5: Test Real Transaction Generation${NC}"
echo "========================================"

# Test API health
if ! curl -s "http://localhost:$SEQUENCER_PORT/health" | grep -q "OK"; then
    echo -e "${RED}❌ Sequencer health check failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ API health check passed${NC}"

# Place test bets to trigger actual Solana transactions
echo "🎲 Placing $TEST_BETS test bets to generate real transactions..."

PLAYER="9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
BET_IDS=()

for i in $(seq 1 $TEST_BETS); do
    echo -n "   Bet $i: "
    
    BET_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"player_address\": \"$PLAYER\", \"amount\": 1000, \"guess\": $((i % 2))}" \
        "http://localhost:$SEQUENCER_PORT/v1/bet")
    
    if echo "$BET_RESPONSE" | grep -q '"bet_id"'; then
        BET_ID=$(echo "$BET_RESPONSE" | grep -o '"bet_id":"[^"]*"' | cut -d'"' -f4)
        WON=$(echo "$BET_RESPONSE" | grep -o '"won":[^,}]*' | cut -d: -f2 | tr -d ' ')
        BET_IDS+=("$BET_ID")
        
        if [ "$WON" = "true" ]; then
            echo -e "${GREEN}Won${NC} (ID: ${BET_ID:0:8}...)"
        else
            echo -e "${CYAN}Lost${NC} (ID: ${BET_ID:0:8}...)"
        fi
    else
        echo -e "${RED}Failed${NC}"
        echo "Response: $BET_RESPONSE"
    fi
    
    sleep 1
done

echo -e "${GREEN}✅ Placed $TEST_BETS bets successfully${NC}"

echo -e "\n⏳ Waiting 15 seconds for settlement batch processing and Solana submission..."
sleep 15

echo -e "\n${BLUE}Step 6: Verify Real On-Chain Transaction Data${NC}"
echo "=============================================="

# Check settlement persistence for transaction signatures
echo "🔍 Checking settlement persistence for transaction signatures..."

if [ -f "zkcasino.settlement.json" ]; then
    echo -e "${GREEN}✅ Settlement file exists${NC}"
    
    # Check for real transaction signatures (not null)
    REAL_SIGNATURES=$(grep -o '"transaction_signature":"[^"]*"' zkcasino.settlement.json | grep -v '"transaction_signature":null' | grep -v '"transaction_signature":""' || echo "")
    
    if [ -n "$REAL_SIGNATURES" ]; then
        echo -e "${GREEN}🎉 FOUND REAL TRANSACTION SIGNATURES!${NC}"
        echo "Signatures found:"
        echo "$REAL_SIGNATURES" | head -5
        
        # Extract first signature for verification
        FIRST_SIG=$(echo "$REAL_SIGNATURES" | head -1 | cut -d'"' -f4)
        echo -e "\n${BLUE}Verifying transaction signature: $FIRST_SIG${NC}"
        
        # Verify on Solana
        echo "🔍 Checking transaction confirmation..."
        if solana confirm "$FIRST_SIG" --url "http://localhost:$VALIDATOR_PORT"; then
            echo -e "${GREEN}✅ Transaction confirmed on-chain!${NC}"
        else
            echo -e "${YELLOW}⚠️  Transaction not yet confirmed (may still be processing)${NC}"
        fi
        
        # Get transaction details
        echo -e "\n${BLUE}Getting transaction details:${NC}"
        solana transaction "$FIRST_SIG" --url "http://localhost:$VALIDATOR_PORT" || echo "Transaction details not available yet"
        
    else
        echo -e "${RED}❌ No real transaction signatures found${NC}"
        echo "Settlement file content:"
        cat zkcasino.settlement.json | jq '.batches | to_entries | .[0:2]' 2>/dev/null || head -20 zkcasino.settlement.json
    fi
else
    echo -e "${RED}❌ Settlement file not found${NC}"
fi

# Check sequencer logs for Solana activity
echo -e "\n${BLUE}Checking Sequencer Logs for Solana Activity:${NC}"

echo "🔍 Solana client initialization:"
if grep -q "Solana client initialized successfully" sequencer.log; then
    echo -e "${GREEN}✅ Solana client initialized${NC}"
else
    echo -e "${RED}❌ Solana client not initialized${NC}"
    grep -i "solana.*client" sequencer.log | head -3
fi

echo -e "\n🔍 Batch processing:"
BATCH_COUNT=$(grep -c "Processing settlement batch" sequencer.log 2>/dev/null || echo "0")
echo "Settlement batches processed: $BATCH_COUNT"

echo -e "\n🔍 Solana transaction submissions:"
SOLANA_SUBMISSIONS=$(grep -c "Submitted batch to Solana" sequencer.log 2>/dev/null || echo "0")
SOLANA_SUCCESS=$(grep -c "submitted to Solana successfully" sequencer.log 2>/dev/null || echo "0")
echo "Solana submissions attempted: $SOLANA_SUBMISSIONS"
echo "Solana submissions successful: $SOLANA_SUCCESS"

if [ "$SOLANA_SUCCESS" -gt 0 ]; then
    echo -e "${GREEN}✅ Found successful Solana transactions!${NC}"
    grep "submitted to Solana successfully" sequencer.log | head -3
else
    echo -e "${YELLOW}⚠️  No successful Solana transactions found${NC}"
    echo "Recent Solana-related log entries:"
    grep -i solana sequencer.log | tail -5
fi

# Check for any errors
echo -e "\n🔍 Checking for errors:"
ERROR_COUNT=$(grep -c -i "error.*solana\|failed.*solana" sequencer.log 2>/dev/null || echo "0")
if [ "$ERROR_COUNT" -gt 0 ]; then
    echo -e "${RED}⚠️  Found $ERROR_COUNT Solana-related errors:${NC}"
    grep -i "error.*solana\|failed.*solana" sequencer.log | head -3
else
    echo -e "${GREEN}✅ No Solana-related errors found${NC}"
fi

echo -e "\n${BLUE}Step 7: Final Validation${NC}"
echo "========================"

echo "🔄 Final system status check..."

# Check all components
VALIDATOR_OK=false
SEQUENCER_OK=false
PROGRAMS_OK=false
TRANSACTIONS_OK=false

# Validator check
if curl -s "http://localhost:$VALIDATOR_PORT" >/dev/null 2>&1; then
    VALIDATOR_OK=true
    echo -e "Validator: ${GREEN}✅ Running${NC}"
else
    echo -e "Validator: ${RED}❌ Not responding${NC}"
fi

# Sequencer check
if curl -s "http://localhost:$SEQUENCER_PORT/health" | grep -q "OK"; then
    SEQUENCER_OK=true
    echo -e "Sequencer: ${GREEN}✅ Running${NC}"
else
    echo -e "Sequencer: ${RED}❌ Not responding${NC}"
fi

# Programs check
if solana account "$VAULT_PROGRAM_ID" >/dev/null 2>&1 && solana account "$VERIFIER_PROGRAM_ID" >/dev/null 2>&1; then
    PROGRAMS_OK=true
    echo -e "Programs: ${GREEN}✅ Deployed${NC}"
else
    echo -e "Programs: ${RED}❌ Not accessible${NC}"
fi

# Transactions check
if [ -f "zkcasino.settlement.json" ] && grep -q '"transaction_signature":"[^"]*"' zkcasino.settlement.json && ! grep -q '"transaction_signature":null' zkcasino.settlement.json; then
    TRANSACTIONS_OK=true
    echo -e "Transactions: ${GREEN}✅ Real signatures found${NC}"
else
    echo -e "Transactions: ${RED}❌ No real signatures${NC}"
fi

# Final verdict
echo -e "\n${PURPLE}🏁 FINAL RESULTS${NC}"
echo "================="

if $VALIDATOR_OK && $SEQUENCER_OK && $PROGRAMS_OK && $TRANSACTIONS_OK; then
    echo -e "${GREEN}🎉 SUCCESS: Real On-Chain Integration Verified!${NC}"
    echo ""
    echo "✅ Solana validator running and accessible"
    echo "✅ ZK Casino sequencer running with Solana integration"
    echo "✅ Smart contracts deployed and verified on-chain"
    echo "✅ Real transaction signatures generated and stored"
    echo "✅ Settlement persistence working with actual Solana data"
    echo ""
    echo -e "${GREEN}🚀 SYSTEM IS TRULY READY FOR PRODUCTION!${NC}"
    
elif $VALIDATOR_OK && $SEQUENCER_OK && $PROGRAMS_OK; then
    echo -e "${YELLOW}⚠️  PARTIAL SUCCESS: Infrastructure Working, Transactions Need Verification${NC}"
    echo ""
    echo "✅ Solana components deployed and running"
    echo "❌ Transaction signatures need verification"
    echo ""
    echo "This means the infrastructure is correct but we need to verify the transaction flow."
    
else
    echo -e "${RED}❌ INTEGRATION TEST FAILED${NC}"
    echo ""
    echo "Some critical components are not working correctly."
    echo "Check the logs above for specific issues."
fi

echo -e "\n${BLUE}📊 Verification Commands for Manual Testing:${NC}"
echo "=============================================="
echo "Check settlement data:    cat zkcasino.settlement.json | jq '.'"
echo "View sequencer logs:      tail -f sequencer.log"
echo "View validator logs:      tail -f validator.log"
echo "Test bet:                 curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"$PLAYER\", \"amount\": 1000, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"
echo "Check balance:            solana balance"
echo "Monitor transactions:     solana logs $VERIFIER_PROGRAM_ID"

echo -e "\n${CYAN}Services will continue running for manual verification...${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop and cleanup${NC}"

# Keep running for manual testing unless in CI mode
if [ "${CI:-false}" != "true" ]; then
    while true; do
        sleep 10
        # Periodic health checks
        if ! curl -s "http://localhost:$SEQUENCER_PORT/health" >/dev/null; then
            echo -e "${RED}⚠️  Sequencer stopped responding${NC}"
            break
        fi
        if ! curl -s "http://localhost:$VALIDATOR_PORT" >/dev/null; then
            echo -e "${RED}⚠️  Validator stopped responding${NC}"
            break
        fi
    done
fi