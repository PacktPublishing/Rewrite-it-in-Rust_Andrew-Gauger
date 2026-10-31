# Mock manufacturer API

A stand-in for the manufacturer's custom-order API that the **Send to Manufacturer** button on the Sales Orders form calls. It runs on this PC only, at `http://localhost:8765/`. That is the address in Company Info (`ManufacturerApiUrl`).

It is **deliberately not idempotent**. Every order it receives becomes a new manufacturer order, even an exact repeat of the previous one, because Cairn's request carries nothing that would identify a repeat.

## Run it

```powershell
.\Start-MockManufacturer.ps1
```

Leave the window open while you use Access. Press Ctrl+C to stop it. The orders it receives are saved in `ledger.json` next to the script.

| Option | Effect |
|---|---|
| `-Mode normal` | Create the order and answer `201 {"mfrOrderId":"MFR-200001","status":"accepted"}` (default) |
| `-Mode drop` | Create the order, then drop the connection without answering |
| `-Mode slow -DelaySec 20` | Wait, then create and answer. Access freezes for the whole wait, because the call is synchronous. |
| `-Mode error` | Answer 500 and create nothing |
| `-Times 1` | With `-Mode`, go back to normal after this many orders |
| `-NoKeepAlive` | Close the connection after every answer (see below) |
| `-Ledger <path>` | Where to keep the orders |

You can change the mode while it runs:

```powershell
Invoke-RestMethod -Method Post 'http://localhost:8765/control?mode=drop&times=1'
Invoke-RestMethod http://localhost:8765/orders      # every order the manufacturer holds
```

`POST /control?mode=stop` shuts it down.

## How a dropped connection becomes a double order

In drop mode the manufacturer has the order, but Access never hears back. What happens next depends on the connection:

- **Keep-alive on (default, like most real servers).** Access reuses the connection from its previous order. When that connection drops, the Windows HTTP stack underneath `MSXML2.XMLHTTP` quietly sends the POST again by itself. The manufacturer gets two orders. Access logs one call and shows one manufacturer order number. Nobody sees an error.
- **`-NoKeepAlive`.** Each order uses a fresh connection. The drop reaches Access as an error ("The download of the specified resource has failed"), and Access asks *Try again?* Pressing Retry sends the same request again, and the manufacturer gets two orders. Access logs both calls but shows only the second number.

The screenshot harness captures both (`mfr-silent-*` and `mfr-retry-*`).
