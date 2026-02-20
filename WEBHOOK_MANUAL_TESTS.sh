#!/bin/bash
# Manual Webhook Testing Guide

echo "=== Webhook System Manual Testing Guide ==="
echo ""
echo "Prerequisites:"
echo "1. Server running: cargo run"
echo "2. Database running with migrations applied"
echo "3. Environment variables set:"
echo "   - FLUTTERWAVE_WEBHOOK_SECRET=your_secret"
echo "   - PAYSTACK_WEBHOOK_SECRET=your_secret"
echo ""

# Test 1: Flutterwave Valid Webhook
echo "Test 1: Flutterwave Valid Webhook"
echo "-----------------------------------"
cat << 'EOF'
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: your_flutterwave_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 12345,
    "event": "charge.completed",
    "data": {
      "tx_ref": "test_tx_123",
      "status": "successful",
      "amount": 5000,
      "currency": "NGN",
      "flw_ref": "FLW_REF_123"
    }
  }'

Expected: 200 OK with {"status":"ok"}
EOF
echo ""

# Test 2: Flutterwave Invalid Signature
echo "Test 2: Flutterwave Invalid Signature"
echo "--------------------------------------"
cat << 'EOF'
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: wrong_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 12346,
    "event": "charge.completed",
    "data": {"tx_ref": "test_tx_124"}
  }'

Expected: 401 Unauthorized with "Invalid signature"
EOF
echo ""

# Test 3: Paystack Valid Webhook
echo "Test 3: Paystack Valid Webhook"
echo "-------------------------------"
cat << 'EOF'
# Generate HMAC SHA512 signature first:
# echo -n '{"event":"charge.success","data":{"reference":"test_tx_125"}}' | \
#   openssl dgst -sha512 -hmac "your_paystack_secret" | awk '{print $2}'

curl -X POST http://localhost:8000/webhooks/paystack \
  -H "x-paystack-signature: <computed_hmac_sha512>" \
  -H "Content-Type: application/json" \
  -d '{
    "event": "charge.success",
    "data": {
      "reference": "test_tx_125",
      "status": "success",
      "amount": 500000,
      "currency": "NGN"
    }
  }'

Expected: 200 OK with {"status":"ok"}
EOF
echo ""

# Test 4: Duplicate Webhook
echo "Test 4: Duplicate Webhook (Idempotency)"
echo "----------------------------------------"
cat << 'EOF'
# Send the same webhook twice with same event ID
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: your_flutterwave_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 99999,
    "event": "charge.completed",
    "data": {"tx_ref": "test_tx_duplicate"}
  }'

# Send again immediately
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: your_flutterwave_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 99999,
    "event": "charge.completed",
    "data": {"tx_ref": "test_tx_duplicate"}
  }'

Expected: Both return 200 OK, but second one logs "Already processed"
EOF
echo ""

# Test 5: Missing Signature
echo "Test 5: Missing Signature"
echo "-------------------------"
cat << 'EOF'
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "Content-Type: application/json" \
  -d '{
    "id": 12347,
    "event": "charge.completed",
    "data": {"tx_ref": "test_tx_126"}
  }'

Expected: 401 Unauthorized with "Missing signature"
EOF
echo ""

# Test 6: Invalid JSON
echo "Test 6: Invalid JSON Payload"
echo "-----------------------------"
cat << 'EOF'
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: your_flutterwave_secret" \
  -H "Content-Type: application/json" \
  -d 'invalid json'

Expected: 400 Bad Request with "Invalid JSON"
EOF
echo ""

# Database Verification
echo "Database Verification Queries"
echo "------------------------------"
cat << 'EOF'
-- Check recent webhooks
SELECT id, provider, event_type, status, retry_count, created_at
FROM webhook_events
ORDER BY created_at DESC
LIMIT 10;

-- Check for duplicates (should see only one per event_id)
SELECT provider, event_id, COUNT(*) as count
FROM webhook_events
GROUP BY provider, event_id
HAVING COUNT(*) > 1;

-- Check failed webhooks
SELECT id, provider, event_type, error_message, retry_count
FROM webhook_events
WHERE status = 'failed';

-- Check pending retries
SELECT id, provider, event_type, retry_count, error_message
FROM webhook_events
WHERE status = 'pending' AND retry_count > 0;
EOF
echo ""

# Log Verification
echo "Log Verification"
echo "----------------"
cat << 'EOF'
# Check for successful processing
grep "Webhook processed successfully" logs/app.log

# Check for invalid signatures
grep "Invalid webhook signature" logs/app.log

# Check for duplicate detection
grep "Webhook already processed" logs/app.log

# Check retry worker activity
grep "Retried pending webhooks" logs/app.log
EOF
echo ""

echo "=== Testing Complete ==="
echo ""
echo "For automated provider testing:"
echo "1. Flutterwave: Dashboard → Settings → Webhooks → Test"
echo "2. Paystack: Dashboard → Settings → Webhooks → Test"
