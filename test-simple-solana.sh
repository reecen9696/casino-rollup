#!/bin/bash

# Simple Direct Solana Verification Test
# No complex monitoring - just deploy, bet, and check results

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🎯 Direct Solana Integration Verification${NC}"
echo "========================================"

# Clean up first
pkill -f "solana-test-validator" 2>/dev/null || true
pkill -f "sequencer" 2>/dev/null || true
sleep 2

# Start fresh validator
echo "🚀 Starting clean Solana validator..."
solana-test-validator --reset --quiet &
VALIDATOR_PID=$!

# Wait for validator
echo "⏳ Waiting for validator..."
sleep 10

# Check validator
if ! curl -s http://localhost:8899 >/dev/null; then
    echo -e "${RED}❌ Validator not responding${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Validator running${NC}"

# Setup wallet
echo "🔑 Setting up wallet..."
solana-keygen new -o test-wallet.json --force --no-bip39-passphrase >/dev/null
solana config set --keypair test-wallet.json --url localhost >/dev/null
solana airdrop 50 >/dev/null

echo -e "${GREEN}✅ Wallet funded: $(solana balance)${NC}"

# Deploy programs
echo "🚀 Deploying programs..."
VAULT_OUTPUT=$(solana program deploy target/deploy/vault.so 2>&1)
VAULT_ID=$(echo "$VAULT_OUTPUT" | grep "Program Id:" | awk '{print $3}')

VERIFIER_OUTPUT=$(solana program deploy target/deploy/verifier.so 2>&1)  
VERIFIER_ID=$(echo "$VERIFIER_OUTPUT" | grep "Program Id:" | awk '{print $3}')

echo -e "${GREEN}✅ Programs deployed${NC}"
echo "   Vault: $VAULT_ID"
echo "   Verifier: $VERIFIER_ID"

# Start sequencer with real IDs
echo "🎮 Starting sequencer with deployed program IDs..."

export ENABLE_SOLANA=true
export VAULT_PROGRAM_ID="$VAULT_ID"
export VERIFIER_PROGRAM_ID="$VERIFIER_ID"
export SOLANA_RPC_URL="http://localhost:8899"

cargo run --package sequencer --release &
SEQUENCER_PID=$!

echo "⏳ Waiting for sequencer..."
sleep 15

# Check sequencer
if ! curl -s http://localhost:3000/health >/dev/null; then
    echo -e "${RED}❌ Sequencer not responding${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Sequencer running${NC}"

# Place a few bets
echo "🎲 Placing bets..."

for i in {1..3}; do
    echo -n "Bet $i: "
    RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 1000, "guess": true}' \
        http://localhost:3000/v1/bet)
    
    if [[ "$RESPONSE" == *"bet_id"* ]]; then
        echo -e "${GREEN}Success${NC}"
    else
        echo -e "${RED}Failed${NC}"
    fi
    sleep 2
done

# Wait for settlement
echo "⏳ Waiting 30 seconds for settlement processing..."
sleep 30

# Check results
echo -e "\n${BLUE}Results:${NC}"

echo "📊 Settlement stats:"
curl -s http://localhost:3000/v1/settlement-stats | jq '.' 2>/dev/null || curl -s http://localhost:3000/v1/settlement-stats

echo -e "\n📁 Settlement file:"
if [ -f "zkcasino.settlement.json" ]; then
    echo "✅ Settlement file exists"
    
    # Check for transaction signatures
    SIGS=$(grep -o '"transaction_signature":"[^"]*"' zkcasino.settlement.json | grep -v null | head -3)
    if [ -n "$SIGS" ]; then
        echo -e "${GREEN}🎉 Found transaction signatures:${NC}"
        echo "$SIGS"
        
        # Try to verify one
        FIRST_SIG=$(echo "$SIGS" | head -1 | cut -d'"' -f4)
        echo -e "\n🔍 Verifying: $FIRST_SIG"
        
        if solana confirm "$FIRST_SIG" 2>/dev/null; then
            echo -e "${GREEN}✅ VERIFIED ON-CHAIN!${NC}"
        else
            echo -e "${YELLOW}⚠️  Not confirmed yet${NC}"
        fi
    else
        echo -e "${RED}❌ No transaction signatures found${NC}"
    fi
else
    echo -e "${RED}❌ No settlement file${NC}"
fi

echo -e "\n${BLUE}Manual Commands:${NC}"
echo "Check settlement: cat zkcasino.settlement.json | jq '.'"
echo "Test bet: curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM\", \"amount\": 500, \"guess\": true}' http://localhost:3000/v1/bet"

echo -e "\n${YELLOW}Press Enter to cleanup and exit...${NC}"
read

# Cleanup
pkill -f "solana-test-validator" 2>/dev/null || true
pkill -f "sequencer" 2>/dev/null || true