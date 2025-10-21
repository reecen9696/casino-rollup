#!/bin/bash

# Quick Solana Integration Test - Fast validation
# Use this for rapid testing during development

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🚀 Quick Solana Integration Test${NC}"
echo "================================"

# Configuration
SEQUENCER_PORT=3000
TEST_PLAYER="9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"

# Check if validator is running
echo -e "\n${BLUE}1. Checking Solana Validator${NC}"
if curl -s http://localhost:8899 > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Validator is running${NC}"
else
    echo -e "${YELLOW}⚠️  Starting validator...${NC}"
    solana-test-validator --reset --quiet > /dev/null 2>&1 &
    echo "Waiting for validator..."
    sleep 10
fi

# Check if sequencer is running  
echo -e "\n${BLUE}2. Checking Sequencer${NC}"
if curl -s http://localhost:$SEQUENCER_PORT/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Sequencer is running${NC}"
else
    echo -e "${YELLOW}⚠️  Starting sequencer with Solana integration...${NC}"
    
    # Export Solana environment variables
    export ENABLE_SOLANA=true
    export VAULT_PROGRAM_ID="11111111111111111111111111111111"
    export VERIFIER_PROGRAM_ID="Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
    
    cd /Users/reece/code/projects/zkcasino
    cargo run --package sequencer > /dev/null 2>&1 &
    
    echo "Waiting for sequencer..."
    sleep 8
    
    if curl -s http://localhost:$SEQUENCER_PORT/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Sequencer started${NC}"
    else
        echo -e "${RED}❌ Sequencer failed to start${NC}"
        exit 1
    fi
fi

# Test API endpoints
echo -e "\n${BLUE}3. Testing API Endpoints${NC}"

# Health check
HEALTH=$(curl -s http://localhost:$SEQUENCER_PORT/health)
if [ "$HEALTH" = "OK" ]; then
    echo -e "${GREEN}✅ Health: $HEALTH${NC}"
else
    echo -e "${RED}❌ Health check failed${NC}"
    exit 1
fi

# Settlement stats
STATS=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats)
if echo "$STATS" | grep -q "total_items_queued"; then
    echo -e "${GREEN}✅ Settlement stats working${NC}"
else
    echo -e "${RED}❌ Settlement stats failed${NC}"
fi

# Test bet
echo -e "\n${BLUE}4. Testing Bet Submission${NC}"

BET_RESPONSE=$(curl -s -w "\nSTATUS:%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -d "{\"player_address\": \"$TEST_PLAYER\", \"amount\": 5000, \"guess\": true}" \
    http://localhost:$SEQUENCER_PORT/v1/bet)

STATUS=$(echo "$BET_RESPONSE" | grep STATUS | cut -d: -f2)
BODY=$(echo "$BET_RESPONSE" | grep -v STATUS)

if [ "$STATUS" = "200" ]; then
    echo -e "${GREEN}✅ Bet successful${NC}"
    
    BET_ID=$(echo "$BODY" | grep -o '"bet_id":"[^"]*"' | cut -d'"' -f4)
    WON=$(echo "$BODY" | grep -o '"won":[^,}]*' | cut -d: -f2)
    PAYOUT=$(echo "$BODY" | grep -o '"payout":[0-9]*' | cut -d: -f2)
    
    echo "   Bet ID: $BET_ID"
    echo "   Won: $WON"
    echo "   Payout: $PAYOUT"
else
    echo -e "${RED}❌ Bet failed (Status: $STATUS)${NC}"
    echo "Response: $BODY"
fi

# Check settlement processing
echo -e "\n${BLUE}5. Settlement Validation${NC}"
sleep 2  # Wait for settlement processing

FINAL_STATS=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats)
QUEUED=$(echo "$FINAL_STATS" | grep -o '"total_items_queued":[0-9]*' | cut -d: -f2)
PROCESSED=$(echo "$FINAL_STATS" | grep -o '"total_batches_processed":[0-9]*' | cut -d: -f2)

echo "Settlement Stats:"
echo "   Items queued: $QUEUED"
echo "   Batches processed: $PROCESSED"

if [ "$QUEUED" -gt 0 ]; then
    echo -e "${GREEN}✅ Settlement system active${NC}"
else
    echo -e "${YELLOW}⚠️  Settlement system warming up${NC}"
fi

echo -e "\n${GREEN}🎉 Quick Test Complete!${NC}"
echo -e "${BLUE}💡 For full integration test, run: ./test-solana-complete.sh${NC}"

echo -e "\n${BLUE}Manual Commands:${NC}"
echo "curl http://localhost:$SEQUENCER_PORT/health"
echo "curl http://localhost:$SEQUENCER_PORT/v1/settlement-stats"
echo "curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"$TEST_PLAYER\", \"amount\": 5000, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"