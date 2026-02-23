#!/bin/bash

# Rates API Test Script
# Tests all endpoints of the rates API

BASE_URL="http://localhost:8000"
COLORS=true

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

print_test() {
    echo -e "${YELLOW}TEST: $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

test_endpoint() {
    local name="$1"
    local url="$2"
    local expected_status="$3"
    
    print_test "$name"
    echo "URL: $url"
    
    response=$(curl -s -w "\n%{http_code}" "$url")
    status_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$status_code" = "$expected_status" ]; then
        print_success "Status: $status_code"
        echo "$body" | jq '.' 2>/dev/null || echo "$body"
    else
        print_error "Expected $expected_status, got $status_code"
        echo "$body"
    fi
    echo ""
}

# Main tests
print_header "Rates API Test Suite"
echo ""

# Test 1: Single pair - NGN to cNGN
test_endpoint \
    "Single Pair: NGN to cNGN" \
    "$BASE_URL/api/rates?from=NGN&to=cNGN" \
    "200"

# Test 2: Single pair - cNGN to NGN
test_endpoint \
    "Single Pair: cNGN to NGN" \
    "$BASE_URL/api/rates?from=cNGN&to=NGN" \
    "200"

# Test 3: Multiple pairs
test_endpoint \
    "Multiple Pairs" \
    "$BASE_URL/api/rates?pairs=NGN/cNGN,cNGN/NGN" \
    "200"

# Test 4: All pairs
test_endpoint \
    "All Pairs" \
    "$BASE_URL/api/rates" \
    "200"

# Test 5: Invalid currency
test_endpoint \
    "Invalid Currency (should fail)" \
    "$BASE_URL/api/rates?from=XYZ&to=cNGN" \
    "400"

# Test 6: Invalid pair
test_endpoint \
    "Invalid Pair (should fail)" \
    "$BASE_URL/api/rates?from=NGN&to=BTC" \
    "400"

# Test 7: Missing parameter
test_endpoint \
    "Missing Parameter (should fail)" \
    "$BASE_URL/api/rates?from=NGN" \
    "400"

# Test 8: Check headers
print_test "Response Headers"
echo "URL: $BASE_URL/api/rates?from=NGN&to=cNGN"
curl -s -I "$BASE_URL/api/rates?from=NGN&to=cNGN" | grep -E "(Cache-Control|ETag|Access-Control)"
echo ""

# Test 9: OPTIONS preflight
print_test "OPTIONS Preflight"
echo "URL: $BASE_URL/api/rates"
curl -s -X OPTIONS -I "$BASE_URL/api/rates" | grep -E "(Access-Control|Allow)"
echo ""

print_header "Test Suite Complete"
