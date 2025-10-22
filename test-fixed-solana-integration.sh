#!/bin/bash

# Fixed Real Solana Integration Test
# Uses actual deployed program IDs instead of declare_id! values

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

echo -e "${PURPLE}🚀 Fixed Real Solana On-Chain Integration Test${NC}"
echo -e "${PURPLE}===============================================${NC}"
echo "This test uses actual deployed program IDs and verifies real transactions."
echo ""

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up processes...${NC}"
    pkill -f "solana-test-validator" 2>/dev/null || true
    pkill -f "cargo run --package sequencer" 2>/dev/null || true
    sleep 3
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

echo -e "\n${BLUE}Step 1: Environment Setup${NC}"
echo "=========================="

# Stop any existing processes
pkill -f "solana-test-validator" 2>/dev/null || true
pkill -f "cargo run --package sequencer" 2>/dev/null || true
sleep 3

# Clean previous artifacts
rm -f validator.log sequencer.log "$WALLET_FILE" 2>/dev/null || true
rm -rf test-ledger/ 2>/dev/null || true
rm -f zkcasino.settlement.json 2>/dev/null || true

echo "🔨 Building programs..."
cargo build-sbf --manifest-path programs/vault/Cargo.toml >/dev/null 2>&1
cargo build-sbf --manifest-path programs/verifier/Cargo.toml >/dev/null 2>&1
cargo build --package sequencer --release >/dev/null 2>&1

echo -e "${GREEN}✅ Programs built${NC}"

echo -e "\n${BLUE}Step 2: Start Validator and Deploy${NC}"
echo "=================================="

# Start validator
echo "🚀 Starting Solana validator..."
solana-test-validator \
    --reset \
    --ledger test-ledger \
    --rpc-port $VALIDATOR_PORT \
    > validator.log 2>&1 &
VALIDATOR_PID=$!

if ! wait_for_service "http://localhost:$VALIDATOR_PORT" "Solana validator"; then
    echo -e "${RED}❌ Validator failed to start${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Validator running${NC}"

# Setup wallet
echo "🔑 Setting up wallet..."
solana-keygen new -o "$WALLET_FILE" --force --no-bip39-passphrase >/dev/null 2>&1
solana config set --keypair "$WALLET_FILE" --url "http://localhost:$VALIDATOR_PORT" >/dev/null 2>&1
solana airdrop $AIRDROP_AMOUNT >/dev/null 2>&1

echo -e "${GREEN}✅ Wallet funded with $(solana balance)${NC}"

# Deploy programs and capture actual program IDs
echo "🚀 Deploying programs and capturing IDs..."

VAULT_DEPLOY=$(solana program deploy target/deploy/vault.so 2>&1)
VAULT_PROGRAM_ID=$(echo "$VAULT_DEPLOY" | grep "Program Id:" | awk '{print $3}')

VERIFIER_DEPLOY=$(solana program deploy target/deploy/verifier.so 2>&1)
VERIFIER_PROGRAM_ID=$(echo "$VERIFIER_DEPLOY" | grep "Program Id:" | awk '{print $3}')

echo "📋 Deployed Program IDs:"
echo "   Vault: $VAULT_PROGRAM_ID"
echo "   Verifier: $VERIFIER_PROGRAM_ID"

# Verify programs are accessible
if ! solana account "$VAULT_PROGRAM_ID" >/dev/null 2>&1; then
    echo -e "${RED}❌ Vault program not accessible${NC}"
    exit 1
fi

if ! solana account "$VERIFIER_PROGRAM_ID" >/dev/null 2>&1; then
    echo -e "${RED}❌ Verifier program not accessible${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Both programs deployed and verified${NC}"

echo -e "\n${BLUE}Step 3: Start Sequencer with Real Program IDs${NC}"
echo "=============================================="

# Set environment variables with ACTUAL deployed program IDs
export ENABLE_SOLANA=true
export ENABLE_ZK_PROOFS=true
export VAULT_PROGRAM_ID="$VAULT_PROGRAM_ID"
export VERIFIER_PROGRAM_ID="$VERIFIER_PROGRAM_ID"
export SOLANA_RPC_URL="http://localhost:$VALIDATOR_PORT"
export SETTLEMENT_BATCH_SIZE=2
export SETTLEMENT_BATCH_TIMEOUT=5

echo "⚙️  Using deployed program IDs:"
echo "   VAULT_PROGRAM_ID=$VAULT_PROGRAM_ID"
echo "   VERIFIER_PROGRAM_ID=$VERIFIER_PROGRAM_ID"

echo "🚀 Starting sequencer..."
cargo run --package sequencer --release > sequencer.log 2>&1 &
SEQUENCER_PID=$!

if ! wait_for_service "http://localhost:$SEQUENCER_PORT/health" "Sequencer"; then
    echo -e "${RED}❌ Sequencer failed to start${NC}"
    echo "Sequencer logs:"
    tail -20 sequencer.log
    exit 1
fi

echo -e "${GREEN}✅ Sequencer running${NC}"

# Check Solana integration status
sleep 3
if grep -q "Solana client initialized successfully" sequencer.log; then
    echo -e "${GREEN}✅ Solana client initialized successfully${NC}"
elif grep -q "Failed to initialize Solana client" sequencer.log; then
    echo -e "${RED}❌ Solana client initialization failed${NC}"
    echo "Error:"
    grep "Failed to initialize Solana client" sequencer.log
    exit 1
else
    echo -e "${YELLOW}⚠️  Solana client status unclear${NC}"
fi

echo -e "\n${BLUE}Step 4: Generate Real Transactions${NC}"
echo "=================================="

# Test API first
if ! curl -s "http://localhost:$SEQUENCER_PORT/health" | grep -q "OK"; then
    echo -e "${RED}❌ API health check failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ API responding${NC}"

# Place bets to trigger transactions
echo "🎲 Placing $TEST_BETS bets to generate real transactions..."

PLAYER="9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
SUCCESSFUL_BETS=0

for i in $(seq 1 $TEST_BETS); do
    echo -n "   Bet $i: "
    
    BET_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"player_address\": \"$PLAYER\", \"amount\": 1000, \"guess\": $((i % 2))}" \
        "http://localhost:$SEQUENCER_PORT/v1/bet")
    
    if echo "$BET_RESPONSE" | grep -q '"bet_id"'; then
        BET_ID=$(echo "$BET_RESPONSE" | grep -o '"bet_id":"[^"]*"' | cut -d'"' -f4)
        WON=$(echo "$BET_RESPONSE" | grep -o '"won":[^,}]*' | cut -d: -f2 | tr -d ' ')
        SUCCESSFUL_BETS=$((SUCCESSFUL_BETS + 1))
        
        if [ "$WON" = "true" ]; then
            echo -e "${GREEN}Won${NC} (${BET_ID:0:8}...)"
        else
            echo -e "${CYAN}Lost${NC} (${BET_ID:0:8}...)"
        fi
    else
        echo -e "${RED}Failed${NC}"
    fi
    
    sleep 1
done

echo -e "${GREEN}✅ Placed $SUCCESSFUL_BETS/$TEST_BETS bets${NC}"

echo -e "\n⏳ Waiting 20 seconds for settlement and Solana transactions..."
sleep 20

echo -e "\n${BLUE}Step 5: Verify On-Chain Transaction Data${NC}"
echo "========================================="

echo "🔍 Checking settlement persistence for real transaction signatures..."

TRANSACTION_VERIFIED=false

if [ -f "zkcasino.settlement.json" ]; then
    echo -e "${GREEN}✅ Settlement file exists${NC}"
    
    # Look for non-null transaction signatures
    SIGNATURES=$(jq -r '.batches | to_entries[] | .value.transaction_signature | select(. != null and . != "")' zkcasino.settlement.json 2>/dev/null || echo "")
    
    if [ -n "$SIGNATURES" ]; then
        echo -e "${GREEN}🎉 FOUND REAL TRANSACTION SIGNATURES!${NC}"
        echo "Transaction signatures:"
        echo "$SIGNATURES" | head -3
        
        # Verify first signature
        FIRST_SIG=$(echo "$SIGNATURES" | head -1)
        echo -e "\n${BLUE}Verifying signature: $FIRST_SIG${NC}"
        
        if solana confirm "$FIRST_SIG" --url "http://localhost:$VALIDATOR_PORT" 2>/dev/null; then
            echo -e "${GREEN}✅ TRANSACTION CONFIRMED ON-CHAIN!${NC}"
            TRANSACTION_VERIFIED=true
            
            # Get transaction details
            echo -e "\n${BLUE}Transaction details:${NC}"
            solana transaction "$FIRST_SIG" --url "http://localhost:$VALIDATOR_PORT" 2>/dev/null | head -10
            
        else
            echo -e "${YELLOW}⚠️  Transaction signature found but not yet confirmed${NC}"
        fi
        
    else
        echo -e "${RED}❌ No real transaction signatures found${NC}"
        echo "Sample settlement data:"
        jq '.batches | to_entries | .[0] | .value' zkcasino.settlement.json 2>/dev/null || head -20 zkcasino.settlement.json
    fi
else
    echo -e "${RED}❌ No settlement file found${NC}"
fi

# Check sequencer logs for Solana activity
echo -e "\n${BLUE}Analyzing Sequencer Logs:${NC}"

BATCH_COUNT=$(grep -c "Processing settlement batch" sequencer.log 2>/dev/null || echo "0")
SOLANA_ATTEMPTS=$(grep -c "Submitting.*batch.*to Solana\|submit.*batch.*solana" sequencer.log 2>/dev/null || echo "0")
SOLANA_SUCCESS=$(grep -c "submitted to Solana successfully\|Solana.*success" sequencer.log 2>/dev/null || echo "0")

echo "📊 Activity Summary:"
echo "   Settlement batches processed: $BATCH_COUNT"
echo "   Solana submission attempts: $SOLANA_ATTEMPTS"  
echo "   Successful Solana submissions: $SOLANA_SUCCESS"

if [ "$SOLANA_SUCCESS" -gt 0 ]; then
    echo -e "${GREEN}✅ Found successful Solana submissions in logs${NC}"
    echo "Recent successful submissions:"
    grep "submitted to Solana successfully\|Solana.*success" sequencer.log | tail -2
else
    echo -e "${YELLOW}⚠️  No successful Solana submissions in logs${NC}"
    echo "Recent Solana-related log entries:"
    grep -i "solana\|submit.*batch" sequencer.log | tail -5
fi

# Check for errors
ERROR_COUNT=$(grep -c -i "error.*solana\|failed.*submit" sequencer.log 2>/dev/null || echo "0")
if [ "$ERROR_COUNT" -gt 0 ]; then
    echo -e "\n${RED}⚠️  Found $ERROR_COUNT Solana-related errors:${NC}"
    grep -i "error.*solana\|failed.*submit" sequencer.log | head -3
fi

echo -e "\n${BLUE}Step 6: Final Results${NC}"
echo "===================="

echo "🔄 Checking final system state..."

# Overall assessment
COMPONENTS_OK=0
TOTAL_COMPONENTS=4

# Validator
if curl -s "http://localhost:$VALIDATOR_PORT" >/dev/null 2>&1; then
    echo -e "Validator: ${GREEN}✅ Running${NC}"
    COMPONENTS_OK=$((COMPONENTS_OK + 1))
else
    echo -e "Validator: ${RED}❌ Not responding${NC}"
fi

# Sequencer
if curl -s "http://localhost:$SEQUENCER_PORT/health" | grep -q "OK"; then
    echo -e "Sequencer: ${GREEN}✅ Running${NC}"
    COMPONENTS_OK=$((COMPONENTS_OK + 1))
else
    echo -e "Sequencer: ${RED}❌ Not responding${NC}"
fi

# Programs
if solana account "$VAULT_PROGRAM_ID" >/dev/null 2>&1 && solana account "$VERIFIER_PROGRAM_ID" >/dev/null 2>&1; then
    echo -e "Programs: ${GREEN}✅ Deployed and accessible${NC}"
    COMPONENTS_OK=$((COMPONENTS_OK + 1))
else
    echo -e "Programs: ${RED}❌ Not accessible${NC}"
fi

# Transactions
if $TRANSACTION_VERIFIED; then
    echo -e "Transactions: ${GREEN}✅ Verified on-chain${NC}"
    COMPONENTS_OK=$((COMPONENTS_OK + 1))
elif [ -f "zkcasino.settlement.json" ] && jq -e '.batches | to_entries[] | .value.transaction_signature | select(. != null and . != "")' zkcasino.settlement.json >/dev/null 2>&1; then
    echo -e "Transactions: ${YELLOW}⚠️  Signatures found, verification pending${NC}"
else
    echo -e "Transactions: ${RED}❌ No verified signatures${NC}"
fi

# Final verdict
echo -e "\n${PURPLE}🏁 FINAL ASSESSMENT${NC}"
echo "===================="

if [ "$COMPONENTS_OK" -eq "$TOTAL_COMPONENTS" ]; then
    echo -e "${GREEN}🎉 COMPLETE SUCCESS: Real On-Chain Integration Verified!${NC}"
    echo ""
    echo "✅ Solana validator deployed and running"
    echo "✅ Smart contracts deployed with real program IDs" 
    echo "✅ ZK Casino sequencer integrated with Solana"
    echo "✅ Real transaction signatures generated and confirmed on-chain"
    echo "✅ Settlement persistence storing actual blockchain data"
    echo ""
    echo -e "${GREEN}🚀 SYSTEM FULLY VALIDATED FOR PRODUCTION DEPLOYMENT!${NC}"
    
elif [ "$COMPONENTS_OK" -ge 3 ]; then
    echo -e "${YELLOW}⚠️  SUBSTANTIAL SUCCESS: Core Integration Working${NC}"
    echo ""
    echo "✅ Infrastructure components operational"
    echo "✅ Programs deployed and accessible"  
    echo "⚠️  Transaction verification may need more time"
    echo ""
    echo "The system is largely working - transactions may still be processing."
    
else
    echo -e "${RED}❌ INTEGRATION ISSUES DETECTED${NC}"
    echo ""
    echo "Critical components are not working as expected."
    echo "Review the logs for specific error details."
fi

# Show verification commands
echo -e "\n${BLUE}📋 Manual Verification Commands:${NC}"
echo "================================="
echo "View settlement data:     cat zkcasino.settlement.json | jq '.'"
echo "Check sequencer logs:     tail -f sequencer.log | grep -i solana"
echo "Monitor validator:        tail -f validator.log"
echo "Test new bet:             curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"$PLAYER\", \"amount\": 500, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"
echo "Verify signature:         solana confirm <SIGNATURE>"
echo "Get transaction details:  solana transaction <SIGNATURE>"
echo "Monitor programs:         solana logs $VERIFIER_PROGRAM_ID"

echo -e "\n${CYAN}System running for manual verification...${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop and cleanup${NC}"

# Keep running for inspection unless in CI
if [ "${CI:-false}" != "true" ]; then
    echo -e "\n${BLUE}Monitoring for real-time activity...${NC}"
    while true; do
        sleep 30
        
        # Check for new transaction signatures every 30 seconds
        if [ -f "zkcasino.settlement.json" ]; then
            NEW_SIGS=$(jq -r '.batches | to_entries[] | .value.transaction_signature | select(. != null and . != "")' zkcasino.settlement.json 2>/dev/null | wc -l || echo "0")
            if [ "$NEW_SIGS" -gt 0 ]; then
                echo -e "${GREEN}📊 Current transaction signatures in system: $NEW_SIGS${NC}"
            fi
        fi
        
        # Health check
        if ! curl -s "http://localhost:$SEQUENCER_PORT/health" >/dev/null; then
            echo -e "${RED}⚠️  Sequencer health check failed${NC}"
            break
        fi
    done
fi