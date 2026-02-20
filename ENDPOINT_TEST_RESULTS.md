# Wallet Balance Endpoint - Test Results

## Endpoint Information
- **URL**: `GET /api/wallet/balance`
- **Query Parameters**: 
  - `address` (required) - Stellar wallet address
  - `refresh` (optional) - Force cache refresh

## Test Scenarios

### Test 1: Invalid Address Format ❌
**Request:**
```bash
GET /api/wallet/balance?address=INVALID123
```

**Expected Response: 400 Bad Request**
```json
{
  "error": {
    "code": "INVALID_ADDRESS",
    "message": "Invalid Stellar wallet address format",
    "details": "Stellar addresses must be 56 characters starting with 'G'"
  }
}
```

---

### Test 2: Non-Existent Wallet 🚫
**Request:**
```bash
GET /api/wallet/balance?address=GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
```

**Expected Response: 404 Not Found**
```json
{
  "error": {
    "code": "WALLET_NOT_FOUND",
    "message": "Wallet address not found on Stellar network",
    "details": "This wallet has not been activated. Fund it with at least 1 XLM to activate.",
    "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
  }
}
```

---

### Test 3: Valid Wallet Address ✅
**Request:**
```bash
GET /api/wallet/balance?address=GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR
```

**Expected Response: 200 OK**
```json
{
  "wallet_address": "GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR",
  "chain": "stellar",
  "balances": {
    "xlm": {
      "total": "10000.0000000",
      "available": "9999.0000000",
      "reserved": "1.0000000"
    },
    "cngn": {
      "balance": "0.00",
      "trustline_exists": false,
      "issuer": null
    }
  },
  "trustlines": [],
  "minimum_xlm_required": "1.0000000",
  "last_updated": "2026-02-20T14:43:44Z",
  "cached": false
}
```

---

### Test 4: Wallet with cNGN Trustline ✅
**Request:**
```bash
GET /api/wallet/balance?address=<ADDRESS_WITH_CNGN>
```

**Expected Response: 200 OK**
```json
{
  "wallet_address": "GXXX...XXX",
  "chain": "stellar",
  "balances": {
    "xlm": {
      "total": "100.5000000",
      "available": "98.5000000",
      "reserved": "2.0000000"
    },
    "cngn": {
      "balance": "5000.00",
      "trustline_exists": true,
      "issuer": "GCKFBEIYV2U22IO2BJ4KVJOIP7XPWQGQFKKWXR6DOSJBV7STMAQSMTGG"
    }
  },
  "trustlines": [
    {
      "asset_code": "cNGN",
      "asset_issuer": "GCKFBEIYV2U22IO2BJ4KVJOIP7XPWQGQFKKWXR6DOSJBV7STMAQSMTGG",
      "balance": "5000.00",
      "limit": "unlimited"
    }
  ],
  "minimum_xlm_required": "2.0000000",
  "last_updated": "2026-02-20T14:43:44Z",
  "cached": false
}
```

**Note:** Reserve calculation:
- Base reserve: 1.0 XLM
- Trustline reserve: 0.5 XLM × 2 trustlines = 1.0 XLM
- Total reserved: 2.0 XLM
- Available: 100.5 - 2.0 = 98.5 XLM

---

### Test 5: Force Refresh 🔄
**Request:**
```bash
GET /api/wallet/balance?address=GAIH...&refresh=true
```

**Expected Response: 200 OK**
- Same structure as Test 3
- `cached: false` (bypasses cache)
- Fresh data from Stellar network

---

### Test 6: Cache Hit ⚡
**Request (called twice within 30 seconds):**
```bash
GET /api/wallet/balance?address=GAIH...
GET /api/wallet/balance?address=GAIH...  # < 30s later
```

**Expected Response: 200 OK**
- Second request returns `cached: true`
- Response time < 5ms (from Redis)
- Same data as first request

---

## Performance Expectations

| Scenario | Expected Response Time |
|----------|----------------------|
| Cache Hit | < 5ms |
| Cache Miss (Stellar query) | < 200ms |
| Invalid Address | < 1ms (validation only) |

---

## How to Run Tests

### Option 1: Using the test script
```bash
cd /home/mac/work/rust_projects/Aframp-backend
./test_wallet_balance.sh
```

### Option 2: Manual curl commands
```bash
# Start the server
cargo run --release

# Test invalid address
curl "http://localhost:8000/api/wallet/balance?address=INVALID"

# Test valid address
curl "http://localhost:8000/api/wallet/balance?address=GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR"

# Test force refresh
curl "http://localhost:8000/api/wallet/balance?address=GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR&refresh=true"
```

### Option 3: Using httpie (if installed)
```bash
http GET "localhost:8000/api/wallet/balance" address==GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR"
```

---

## Prerequisites

1. **Server must be running:**
   ```bash
   cargo run --release
   ```

2. **Environment variables configured:**
   - `DATABASE_URL` - PostgreSQL connection
   - `REDIS_URL` - Redis connection
   - `STELLAR_HORIZON_URL` - Stellar Horizon API
   - `CNGN_ISSUER_ADDRESS` - cNGN issuer address

3. **Services running:**
   - PostgreSQL (for database)
   - Redis (for caching)
   - Internet connection (for Stellar API)

---

## Verification Checklist

- [ ] Invalid address returns 400
- [ ] Non-existent wallet returns 404
- [ ] Valid wallet returns 200 with correct data
- [ ] XLM balance shows total, available, reserved
- [ ] cNGN trustline status is correct
- [ ] Reserve calculation is accurate
- [ ] Cache works (second request is faster)
- [ ] Force refresh bypasses cache
- [ ] Response includes timestamp
- [ ] Response indicates if cached

---

## Notes

- Test addresses are from Stellar testnet
- cNGN issuer: `GCKFBEIYV2U22IO2BJ4KVJOIP7XPWQGQFKKWXR6DOSJBV7STMAQSMTGG`
- Cache TTL: 30 seconds
- All monetary values use string format (no floats)
- XLM precision: 7 decimal places
