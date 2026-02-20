#!/bin/bash
# Wallet Balance Endpoint Test Script

BASE_URL="http://localhost:8000"
TESTNET_ADDRESS="GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR"
INVALID_ADDRESS="INVALID123"
NONEXISTENT_ADDRESS="GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"

echo "========================================="
echo "Wallet Balance Endpoint Tests"
echo "========================================="
echo ""

# Test 1: Invalid Address Format (400)
echo "Test 1: Invalid Address Format"
echo "Expected: 400 Bad Request"
echo "Request: GET /api/wallet/balance?address=$INVALID_ADDRESS"
echo ""
curl -s -w "HTTP Status: %{http_code}\n" \
  "$BASE_URL/api/wallet/balance?address=$INVALID_ADDRESS" | jq . 2>/dev/null || echo "Error or server not running"
echo ""
echo "-----------------------------------------"
echo ""

# Test 2: Non-existent Wallet (404)
echo "Test 2: Non-existent Wallet Address"
echo "Expected: 404 Not Found"
echo "Request: GET /api/wallet/balance?address=$NONEXISTENT_ADDRESS"
echo ""
curl -s -w "HTTP Status: %{http_code}\n" \
  "$BASE_URL/api/wallet/balance?address=$NONEXISTENT_ADDRESS" | jq . 2>/dev/null || echo "Error or server not running"
echo ""
echo "-----------------------------------------"
echo ""

# Test 3: Valid Address (200)
echo "Test 3: Valid Stellar Testnet Address"
echo "Expected: 200 OK with balance data"
echo "Request: GET /api/wallet/balance?address=$TESTNET_ADDRESS"
echo ""
curl -s -w "HTTP Status: %{http_code}\n" \
  "$BASE_URL/api/wallet/balance?address=$TESTNET_ADDRESS" | jq . 2>/dev/null || echo "Error or server not running"
echo ""
echo "-----------------------------------------"
echo ""

# Test 4: Force Refresh (200)
echo "Test 4: Force Refresh (bypass cache)"
echo "Expected: 200 OK with cached=false"
echo "Request: GET /api/wallet/balance?address=$TESTNET_ADDRESS&refresh=true"
echo ""
curl -s -w "HTTP Status: %{http_code}\n" \
  "$BASE_URL/api/wallet/balance?address=$TESTNET_ADDRESS&refresh=true" | jq . 2>/dev/null || echo "Error or server not running"
echo ""
echo "-----------------------------------------"
echo ""

# Test 5: Cache Hit (200)
echo "Test 5: Cache Hit (should be fast)"
echo "Expected: 200 OK with cached=true (if called twice quickly)"
echo "Request: GET /api/wallet/balance?address=$TESTNET_ADDRESS"
echo ""
START=$(date +%s%N)
curl -s -w "HTTP Status: %{http_code}\n" \
  "$BASE_URL/api/wallet/balance?address=$TESTNET_ADDRESS" | jq . 2>/dev/null || echo "Error or server not running"
END=$(date +%s%N)
DURATION=$((($END - $START) / 1000000))
echo "Response time: ${DURATION}ms"
echo ""
echo "========================================="
echo "Tests Complete"
echo "========================================="
