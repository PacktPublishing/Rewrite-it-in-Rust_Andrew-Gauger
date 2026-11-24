//! Real HTTP against the mock on an ephemeral port. The client is a
//! bare socket, so a dropped connection is plain to see: the read
//! ends with no bytes at all.

use mock_manufacturer::{serve, Mode, Settings};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

async fn start(mode: Mode, times: u32) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ledger = std::env::temp_dir()
        .join(format!("mock-manufacturer-{}.json", Uuid::new_v4()));
    let settings = Settings {
        mode,
        times,
        delay: Duration::from_millis(10),
        ledger,
        first_order_number: 200001,
    };
    tokio::spawn(serve(listener, settings));
    addr
}

/// `None` when the mock closed the connection without answering.
async fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
) -> Option<(u16, Value)> {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    if raw.is_empty() {
        return None;
    }
    let text = String::from_utf8(raw).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let status = head[9..12].parse().unwrap();
    Some((status, serde_json::from_str(body).unwrap()))
}

fn order(id: Option<Uuid>) -> String {
    let mut o = json!({
        "dealer": "CAIRN",
        "po": "SO-2026-1226",
        "customerPo": "",
        "shipTo": {"name": "Vista Bakery", "address": "6503 Enterprise Rd",
                   "city": "Lubbock", "state": "TX", "zip": "76161"},
        "lines": [{"sku": "CS-TH100", "qty": 385}],
    });
    if let Some(id) = id {
        o["correlationId"] = json!(id);
    }
    o.to_string()
}

async fn post(
    addr: SocketAddr,
    id: Option<Uuid>,
) -> Option<(u16, Value)> {
    send(addr, "POST", "/orders", &order(id)).await
}

async fn orders(addr: SocketAddr, path: &str) -> Vec<Value> {
    let (status, v) = send(addr, "GET", path, "").await.unwrap();
    assert_eq!(status, 200);
    v.as_array().unwrap().clone()
}

#[tokio::test]
async fn a_new_order_is_created() {
    let addr = start(Mode::Normal, 0).await;
    let got = post(addr, Some(Uuid::new_v4())).await;
    assert_eq!(
        got,
        Some((
            201,
            json!({"mfrOrderId": "MFR-200001", "status": "accepted"})
        ))
    );
}

#[tokio::test]
async fn a_repeat_correlation_id_gets_409_and_the_held_number() {
    let addr = start(Mode::Normal, 0).await;
    let id = Uuid::new_v4();
    post(addr, Some(Uuid::new_v4())).await.unwrap();
    post(addr, Some(id)).await.unwrap();
    let got = post(addr, Some(id)).await;
    assert_eq!(
        got,
        Some((
            409,
            json!({"error": "duplicate correlationId", "mfrOrderId": "MFR-200002"})
        ))
    );
    assert_eq!(orders(addr, "/orders").await.len(), 2);
}

#[tokio::test]
async fn a_repeat_gets_409_even_in_drop_mode() {
    let addr = start(Mode::Normal, 0).await;
    let id = Uuid::new_v4();
    post(addr, Some(id)).await.unwrap();
    send(addr, "POST", "/control?mode=drop", "").await.unwrap();
    let got = post(addr, Some(id)).await.unwrap();
    assert_eq!(got.0, 409);
    assert_eq!(got.1["mfrOrderId"], "MFR-200001");
    assert_eq!(orders(addr, "/orders").await.len(), 1);
}

#[tokio::test]
async fn an_order_with_no_id_is_accepted_every_time() {
    let addr = start(Mode::Normal, 0).await;
    assert_eq!(post(addr, None).await.unwrap().0, 201);
    assert_eq!(post(addr, None).await.unwrap().0, 201);
    let held = orders(addr, "/orders").await;
    assert_eq!(held.len(), 2);
    assert_eq!(held[0]["dealerPo"], held[1]["dealerPo"]);
}

#[tokio::test]
async fn drop_mode_records_the_order_and_never_answers() {
    let addr = start(Mode::Drop, 0).await;
    let id = Uuid::new_v4();
    assert_eq!(post(addr, Some(id)).await, None);
    let held = orders(addr, "/orders").await;
    assert_eq!(held.len(), 1);
    assert_eq!(held[0]["correlationId"], json!(id));
}

#[tokio::test]
async fn times_1_falls_back_to_normal() {
    let addr = start(Mode::Drop, 1).await;
    assert_eq!(post(addr, Some(Uuid::new_v4())).await, None);
    let got = post(addr, Some(Uuid::new_v4())).await.unwrap();
    assert_eq!(
        got,
        (
            201,
            json!({"mfrOrderId": "MFR-200002", "status": "accepted"})
        )
    );
}

#[tokio::test]
async fn lookup_by_correlation_id_returns_the_held_order() {
    let addr = start(Mode::Normal, 0).await;
    let id = Uuid::new_v4();
    post(addr, Some(Uuid::new_v4())).await.unwrap();
    post(addr, Some(id)).await.unwrap();
    let found =
        orders(addr, &format!("/orders?correlationId={id}")).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["mfrOrderId"], "MFR-200002");
    let none = format!("/orders?correlationId={}", Uuid::new_v4());
    assert!(orders(addr, &none).await.is_empty());
}
