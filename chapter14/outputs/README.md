# Chapter 14: outputs

Every file here was made by running the code in `part 2/crate` at the
Chapter 14 build (tag `ch14-reports` when it lands). Machine: one Apple
silicon laptop, Homebrew WeasyPrint 70.0 (`brew install weasyprint`),
poppler's `pdfinfo` and `pdftotext`, dev Postgres 17 on port 5433 loaded
by `dev-seed.sh` and `dev-ipsum.sh` (placeholder rows, not Cairn's data).

| File | How it was made |
|---|---|
| `weasyprint-version.txt` | `weasyprint --version` |
| `reports-tests.txt` | `cargo test -p cairn-app --test reports -- --nocapture --test-threads=1`. No database: a small snapshot, an 8-line order (V1's clipped case), a 40-line order, three returns. Eleven tests, including the six month-end lists and the CSV. |
| `report-pipeline-debug.txt`, `report-pipeline-release.txt` | `cargo test [--release] -p cairn-app --test report_pipeline -- --ignored --nocapture --test-threads=1`. Through the real worker and Postgres: the 8-line invoice, a lookup behind two slow renders (a stand-in `weasyprint` that sleeps 3 s), three failure paths, and all five reports from the placeholder history, timed click to PDF. |
| `pdf/invoice-8-lines.pdf` | the invoice test: one page, all eight lines, the total |
| `pdf/invoice-40-lines.pdf` | the same template with 40 lines: three pages, header repeated, "Page 2 of 3" |
| `pdf/quote-1.pdf`, `receipt-00040.pdf`, `commission-2026-10.pdf`, `rma-2026-10.pdf` | the other four reports from the test snapshot |

## Numbers the prose may print (release build, one laptop)

- Invoice, 8 lines, click to PDF: 559 ms (cold start included).
- A customer lookup alone 18 ms, behind two 3-second renders 23 ms (debug: 17 and 39 ms).
- From the placeholder history (13,531 orders, 19,427 quotes, 2,192 RMAs): quote 401 ms, invoice 426 ms, receipt 406 ms, commission statements (15 pages) 783 ms, RMA summary (7 pages) 593 ms.
- Month-end lists, October 2026, same history: Orders 614 ms (4 pages), Sales by Product 416 ms, Quote Pipeline 404 ms, Cash Received CSV 48 ms (65 bytes: the placeholder history has no October payments), **Open Receivables 30,444 ms and 367 pages**. The last is the data, not the code: the placeholder history leaves thousands of orders unpaid, so the list is long. It is a real instance of a slow report that must not block the UI.
- Before the fetch read a whole table once past 20 names: commission 5,950 ms, RMA summary 6,284 ms. The cost was one query per customer (about 1,000 round trips), not Rust and not WeasyPrint.

## Not measured, and caveats

- The 107 orders over six lines (V1, `chapter6/TODO-research.md`) are not recounted. The placeholder history has 1,962 orders over six lines, a different dataset; do not print it as V1's number.
- The placeholder history has no September 2026 commission matching Carla Mendes's $25,701.07. That check needs Chapter 15's import.
- WeasyPrint's cold start was not separated from the first render.
- Fonts: the PDFs use Helvetica or Arial as the machine resolves them.
