// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Fetching a file the way engines are fetched, from a server on this machine:
//! progress counts every byte, a file that matches its SHA-256 is kept, and one
//! that does not is deleted.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use eon_converter_core::tools::fetch_file;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Serve `body` to every request, `requests` times, in chunks.
fn serve(body: Vec<u8>, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/engine.zip", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for _ in 0..requests {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            s.write_all(head.as_bytes()).unwrap();
            for chunk in body.chunks(16 * 1024) {
                s.write_all(chunk).unwrap();
            }
        }
    });
    url
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[tokio::test]
async fn progress_counts_every_byte_and_the_pin_decides() {
    let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let sha = hex(&Sha256::digest(&body));
    let url = serve(body.clone(), 2);
    let dir = std::env::temp_dir().join(format!("eon-converter-fetch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // As Converter::new does: rustls with the ring provider.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::new();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = Arc::clone(&seen);
    let kept = dir.join("kept.zip");
    fetch_file(&client, &url, &sha, &kept, &move |got, total| {
        s.lock().unwrap().push((got, total));
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read(&kept).unwrap(), body);
    let seen = seen.lock().unwrap().clone();
    assert!(seen.len() > 1, "progress was reported as it came: {seen:?}");
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "never goes back");
    assert_eq!(
        *seen.last().unwrap(),
        (body.len() as u64, Some(body.len() as u64))
    );

    let wrong = dir.join("wrong.zip");
    let err = fetch_file(&client, &url, &"0".repeat(64), &wrong, &|_, _| {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid");
    assert!(!wrong.exists(), "a file that does not match is deleted");
    let _ = std::fs::remove_dir_all(&dir);
}
