# Rates API Test Script (PowerShell)
# Tests all endpoints of the rates API

$BaseUrl = "http://localhost:8000"

function Print-Header {
    param([string]$Message)
    Write-Host "========================================" -ForegroundColor Blue
    Write-Host $Message -ForegroundColor Blue
    Write-Host "========================================" -ForegroundColor Blue
}

function Print-Test {
    param([string]$Message)
    Write-Host "TEST: $Message" -ForegroundColor Yellow
}

function Print-Success {
    param([string]$Message)
    Write-Host "✓ $Message" -ForegroundColor Green
}

function Print-Error {
    param([string]$Message)
    Write-Host "✗ $Message" -ForegroundColor Red
}

function Test-Endpoint {
    param(
        [string]$Name,
        [string]$Url,
        [int]$ExpectedStatus
    )
    
    Print-Test $Name
    Write-Host "URL: $Url"
    
    try {
        $response = Invoke-WebRequest -Uri $Url -Method Get -UseBasicParsing -ErrorAction Stop
        $statusCode = $response.StatusCode
        $body = $response.Content
        
        if ($statusCode -eq $ExpectedStatus) {
            Print-Success "Status: $statusCode"
            $body | ConvertFrom-Json | ConvertTo-Json -Depth 10
        } else {
            Print-Error "Expected $ExpectedStatus, got $statusCode"
            Write-Host $body
        }
    } catch {
        $statusCode = $_.Exception.Response.StatusCode.value__
        if ($statusCode -eq $ExpectedStatus) {
            Print-Success "Status: $statusCode (expected error)"
            $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
            $body = $reader.ReadToEnd()
            $body | ConvertFrom-Json | ConvertTo-Json -Depth 10
        } else {
            Print-Error "Expected $ExpectedStatus, got $statusCode"
            Write-Host $_.Exception.Message
        }
    }
    
    Write-Host ""
}

# Main tests
Print-Header "Rates API Test Suite"
Write-Host ""

# Test 1: Single pair - NGN to cNGN
Test-Endpoint `
    -Name "Single Pair: NGN to cNGN" `
    -Url "$BaseUrl/api/rates?from=NGN&to=cNGN" `
    -ExpectedStatus 200

# Test 2: Single pair - cNGN to NGN
Test-Endpoint `
    -Name "Single Pair: cNGN to NGN" `
    -Url "$BaseUrl/api/rates?from=cNGN&to=NGN" `
    -ExpectedStatus 200

# Test 3: Multiple pairs
Test-Endpoint `
    -Name "Multiple Pairs" `
    -Url "$BaseUrl/api/rates?pairs=NGN/cNGN,cNGN/NGN" `
    -ExpectedStatus 200

# Test 4: All pairs
Test-Endpoint `
    -Name "All Pairs" `
    -Url "$BaseUrl/api/rates" `
    -ExpectedStatus 200

# Test 5: Invalid currency
Test-Endpoint `
    -Name "Invalid Currency (should fail)" `
    -Url "$BaseUrl/api/rates?from=XYZ&to=cNGN" `
    -ExpectedStatus 400

# Test 6: Invalid pair
Test-Endpoint `
    -Name "Invalid Pair (should fail)" `
    -Url "$BaseUrl/api/rates?from=NGN&to=BTC" `
    -ExpectedStatus 400

# Test 7: Missing parameter
Test-Endpoint `
    -Name "Missing Parameter (should fail)" `
    -Url "$BaseUrl/api/rates?from=NGN" `
    -ExpectedStatus 400

# Test 8: Check headers
Print-Test "Response Headers"
Write-Host "URL: $BaseUrl/api/rates?from=NGN&to=cNGN"
try {
    $response = Invoke-WebRequest -Uri "$BaseUrl/api/rates?from=NGN&to=cNGN" -Method Get -UseBasicParsing
    Write-Host "Cache-Control: $($response.Headers['Cache-Control'])"
    Write-Host "ETag: $($response.Headers['ETag'])"
    Write-Host "Access-Control-Allow-Origin: $($response.Headers['Access-Control-Allow-Origin'])"
} catch {
    Write-Host "Error fetching headers: $($_.Exception.Message)"
}
Write-Host ""

# Test 9: OPTIONS preflight
Print-Test "OPTIONS Preflight"
Write-Host "URL: $BaseUrl/api/rates"
try {
    $response = Invoke-WebRequest -Uri "$BaseUrl/api/rates" -Method Options -UseBasicParsing
    Write-Host "Status: $($response.StatusCode)"
    Write-Host "Access-Control-Allow-Origin: $($response.Headers['Access-Control-Allow-Origin'])"
    Write-Host "Access-Control-Allow-Methods: $($response.Headers['Access-Control-Allow-Methods'])"
} catch {
    Write-Host "Error with OPTIONS: $($_.Exception.Message)"
}
Write-Host ""

Print-Header "Test Suite Complete"
