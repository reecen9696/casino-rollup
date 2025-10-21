#!/bin/bash

# ZK Casino Integration Status Check
# Quick validation of current system state

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🎯 ZK Casino Integration Status${NC}"
echo "================================"

SEQUENCER_PORT=3000
VALIDATOR_PORT=8899
ALL_GOOD=true

echo -e "\n${BLUE}🔍 System Status Check${NC}"
echo "-----------------------"

# Check Validator
echo -n "Solana Validator: "
if curl -s http://localhost:$VALIDATOR_PORT > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Running${NC}"
else
    echo -e "${RED}❌ Not running${NC}"
    ALL_GOOD=false
fi

# Check Sequencer
echo -n "ZK Casino Sequencer: "
HEALTH=$(curl -s http://localhost:$SEQUENCER_PORT/health 2>/dev/null)
if [ "$HEALTH" = "OK" ]; then
    echo -e "${GREEN}✅ Running and healthy${NC}"
else
    echo -e "${RED}❌ Not responding${NC}"
    ALL_GOOD=false
fi

# Check Solana Integration
echo -n "Solana Integration: "
if [ "$ENABLE_SOLANA" = "true" ]; then
    echo -e "${GREEN}✅ Enabled${NC}"
else
    echo -e "${YELLOW}⚠️  Disabled${NC}"
fi

echo -e "\n${BLUE}🧪 Quick API Test${NC}"
echo "------------------"

# Test Settlement Stats
echo -n "Settlement System: "
STATS=$(curl -s http://localhost:$SEQUENCER_PORT/v1/settlement-stats 2>/dev/null)
if echo "$STATS" | grep -q "total_items_queued"; then
    QUEUED=$(echo "$STATS" | grep -o '"total_items_queued":[0-9]*' | cut -d: -f2)
    PROCESSED=$(echo "$STATS" | grep -o '"total_batches_processed":[0-9]*' | cut -d: -f2)
    echo -e "${GREEN}✅ Active (Queued: $QUEUED, Processed: $PROCESSED)${NC}"
else
    echo -e "${RED}❌ Not responding${NC}"
    ALL_GOOD=false
fi

# Test Quick Bet
echo -n "Bet Processing: "
BET_RESPONSE=$(curl -s -w "STATUS:%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"player_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", "amount": 1000, "guess": true}' \
    http://localhost:$SEQUENCER_PORT/v1/bet 2>/dev/null)

STATUS=$(echo "$BET_RESPONSE" | grep -o "STATUS:[0-9]*" | cut -d: -f2)
if [ "$STATUS" = "200" ]; then
    echo -e "${GREEN}✅ Working${NC}"
else
    echo -e "${RED}❌ Failed (Status: $STATUS)${NC}"
    ALL_GOOD=false
fi

echo -e "\n${BLUE}📊 Current Configuration${NC}"
echo "-------------------------"

# Show environment
echo "Environment Variables:"
echo "  ENABLE_SOLANA: ${ENABLE_SOLANA:-'not set'}"
echo "  VAULT_PROGRAM_ID: ${VAULT_PROGRAM_ID:-'not set'}"
echo "  VERIFIER_PROGRAM_ID: ${VERIFIER_PROGRAM_ID:-'not set'}"

# Show wallet if available
if command -v solana >/dev/null 2>&1; then
    BALANCE=$(solana balance --url localhost 2>/dev/null || echo "N/A")
    echo "  Wallet Balance: $BALANCE"
fi

echo -e "\n${BLUE}🎯 Overall Status${NC}"
echo "==================="

if [ "$ALL_GOOD" = true ]; then
    echo -e "${GREEN}🎉 SYSTEM FULLY OPERATIONAL!${NC}"
    echo -e "${GREEN}✅ All components working correctly${NC}"
    echo ""
    echo "✨ Ready for:"
    echo "  • API testing and load testing"
    echo "  • Settlement batch processing"
    echo "  • Solana transaction submission (when validator connected)"
    echo "  • Phase 3: ZK Circuit Implementation"
else
    echo -e "${YELLOW}⚠️  SOME ISSUES DETECTED${NC}"
    echo "Check the individual component status above."
fi

echo -e "\n${BLUE}🔧 Quick Commands${NC}"
echo "=================="
echo "Health:     curl http://localhost:$SEQUENCER_PORT/health"
echo "Stats:      curl http://localhost:$SEQUENCER_PORT/v1/settlement-stats"
echo "Test bet:   curl -X POST -H 'Content-Type: application/json' -d '{\"player_address\": \"test\", \"amount\": 1000, \"guess\": true}' http://localhost:$SEQUENCER_PORT/v1/bet"
echo "Start all:  ./test-solana-complete.sh"