<#
.SYNOPSIS
    A stand-in for the manufacturer's custom-order API that Cairn's
    "Send to Manufacturer" button calls. Runs on this PC only.

.DESCRIPTION
    Deliberately NOT idempotent: every POST /orders creates a new manufacturer
    order, even if it is byte-for-byte the same request as the last one. There
    is no correlation ID to check, because Cairn never sends one.

    Modes (set with -Mode, or while running with POST /control?mode=...):
      normal  create the order and answer 201 {"mfrOrderId":"MFR-200001",...}
      drop    create the order, then drop the connection without answering.
              Either the HTTP stack resends it silently (keep-alive on) or
              Access asks the user to retry (-NoKeepAlive); both ways the
              manufacturer ends up with the order twice. See NOTES below.
      slow    wait -DelaySec seconds, then create and answer (Access freezes
              for the whole wait: the call is synchronous)
      error   answer 500 and create nothing
    Add &times=N to a control call to fall back to normal after N orders.

    Endpoints
      POST /orders             place an order (JSON body)
      GET  /orders             every order the manufacturer holds (the ledger)
      POST /control?mode=drop  change mode; mode=stop shuts the service down
      GET  /health             {"ok":true,"mode":"..."}

.EXAMPLE
    .\Start-MockManufacturer.ps1
.EXAMPLE
    .\Start-MockManufacturer.ps1 -Mode drop -Ledger .\ledger.json
#>
param(
    [int]$Port = 8765,
    [ValidateSet('normal', 'drop', 'slow', 'error')] [string]$Mode = 'normal',
    [int]$DelaySec = 20,
    [string]$Ledger = (Join-Path $PSScriptRoot 'ledger.json'),
    [int]$FirstOrderNumber = 200001,
    [int]$Times = 0,              # with -Mode: go back to normal after this many orders (0 = stay)
    [switch]$NoKeepAlive          # close the connection after every answer (see NOTES)
)
<# NOTES
    Keep-alive matters. With it on (the default, like most real servers), Access
    reuses the connection for the next order. If that reused connection drops,
    the Windows HTTP stack under MSXML2.XMLHTTP quietly sends the POST again
    itself: the manufacturer gets two orders, Access logs one attempt, and
    nobody sees a Retry prompt. With -NoKeepAlive each order uses a fresh
    connection, a drop reaches Access as an error, and the user is asked to retry.
#>
$ErrorActionPreference = 'Stop'

$orders = New-Object System.Collections.Generic.List[object]
if (Test-Path $Ledger) {
    # PowerShell 5.1 hands back a JSON array as one object; the pipeline unrolls it
    (Get-Content $Ledger -Raw | ConvertFrom-Json) | ForEach-Object { if ($_) { $orders.Add($_) } }
}
$next = $FirstOrderNumber
foreach ($o in $orders) { $n = [int]($o.mfrOrderId -replace '\D', ''); if ($n -ge $next) { $next = $n + 1 } }
$script:mode = $Mode
$script:modeLeft = $Times
$script:keepAlive = -not $NoKeepAlive

function Save-Ledger { ConvertTo-Json -InputObject $orders.ToArray() -Depth 6 | Set-Content $Ledger -Encoding UTF8 }

function Send-Json($ctx, [int]$status, $obj) {
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $obj -Depth 6 -Compress))
    $ctx.Response.StatusCode = $status
    $ctx.Response.KeepAlive = $script:keepAlive
    $ctx.Response.ContentType = 'application/json'
    $ctx.Response.ContentLength64 = $bytes.Length
    $ctx.Response.OutputStream.Write($bytes, 0, $bytes.Length)
    $ctx.Response.Close()
}

function New-MfrOrder($body, $parsed) {
    $o = [ordered]@{
        mfrOrderId = "MFR-$($script:next)"
        receivedAt = (Get-Date).ToString('s')
        dealerPo   = $parsed.po
        customer   = $parsed.shipTo.name
        lines      = @($parsed.lines)
        rawRequest = $body
    }
    $script:next++
    $orders.Add([pscustomobject]$o)
    Save-Ledger
    if ($script:modeLeft -gt 0) { $script:modeLeft--; if ($script:modeLeft -eq 0) { $script:mode = 'normal' } }
    $o
}

$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://localhost:$Port/")
$listener.Start()
Write-Host "Mock manufacturer API on http://localhost:$Port/  (mode: $script:mode, keep-alive: $script:keepAlive, ledger: $Ledger)" -ForegroundColor Cyan
Write-Host "Ctrl+C to stop." -ForegroundColor DarkGray

try {
    while ($listener.IsListening) {
        $task = $listener.GetContextAsync()
        while (-not $task.AsyncWaitHandle.WaitOne(500)) { }
        $ctx = $task.Result
        $req = $ctx.Request
        $path = $req.Url.AbsolutePath.TrimEnd('/')
        $body = ''
        if ($req.HasEntityBody) { $body = (New-Object IO.StreamReader($req.InputStream, $req.ContentEncoding)).ReadToEnd() }
        $stamp = (Get-Date).ToString('HH:mm:ss')
        try {
            if ($req.HttpMethod -eq 'POST' -and $path -eq '/orders') {
                try { $parsed = $body | ConvertFrom-Json } catch {
                    Write-Host "$stamp 400 malformed JSON" -ForegroundColor Yellow
                    Send-Json $ctx 400 @{ error = 'malformed JSON'; detail = $_.Exception.Message }
                    continue
                }
                switch ($script:mode) {
                    'error' {
                        Write-Host "$stamp 500 (mode error) $($parsed.po)" -ForegroundColor Yellow
                        Send-Json $ctx 500 @{ error = 'internal error' }
                    }
                    'drop' {
                        $o = New-MfrOrder $body $parsed
                        Write-Host "$stamp created $($o.mfrOrderId) for $($parsed.po), then DROPPED the connection" -ForegroundColor Yellow
                        $ctx.Response.Abort()
                    }
                    'slow' {
                        Write-Host "$stamp slow: waiting $DelaySec s before answering $($parsed.po)" -ForegroundColor Yellow
                        Start-Sleep -Seconds $DelaySec
                        $o = New-MfrOrder $body $parsed
                        Send-Json $ctx 201 @{ mfrOrderId = $o.mfrOrderId; status = 'accepted' }
                    }
                    default {
                        $o = New-MfrOrder $body $parsed
                        $dupes = @($orders | Where-Object { $_.dealerPo -eq $parsed.po }).Count
                        Write-Host "$stamp 201 $($o.mfrOrderId) for $($parsed.po)$(if ($dupes -gt 1) { "  ($dupes orders for this PO)" })" -ForegroundColor Green
                        Send-Json $ctx 201 @{ mfrOrderId = $o.mfrOrderId; status = 'accepted' }
                    }
                }
            } elseif ($req.HttpMethod -eq 'GET' -and $path -eq '/orders') {
                Send-Json $ctx 200 ([object[]]@($orders | Select-Object mfrOrderId, receivedAt, dealerPo, customer, lines))
            } elseif ($req.HttpMethod -eq 'POST' -and $path -eq '/control') {
                $m = $req.QueryString['mode']
                if ($m -eq 'stop') { Send-Json $ctx 200 @{ ok = $true; stopping = $true }; break }
                if ($m -notin 'normal', 'drop', 'slow', 'error') { Send-Json $ctx 400 @{ error = "unknown mode '$m'" }; continue }
                $script:mode = $m
                $script:modeLeft = [int]('0' + $req.QueryString['times'])
                Write-Host "$stamp mode -> $m$(if ($script:modeLeft) { " for $($script:modeLeft) order(s)" })" -ForegroundColor Cyan
                Send-Json $ctx 200 @{ ok = $true; mode = $script:mode; times = $script:modeLeft }
            } elseif ($path -eq '/health') {
                Send-Json $ctx 200 @{ ok = $true; mode = $script:mode; orders = $orders.Count }
            } else {
                Send-Json $ctx 404 @{ error = 'not found' }
            }
        } catch {
            Write-Host "$stamp error: $($_.Exception.Message)" -ForegroundColor Red
            try { Send-Json $ctx 500 @{ error = $_.Exception.Message } } catch {}
        }
    }
} finally {
    $listener.Stop()
    $listener.Close()
    Write-Host 'Mock manufacturer API stopped.'
}
