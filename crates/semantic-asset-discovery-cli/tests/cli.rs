use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use std::thread;
use tempfile::tempdir;
use tiny_http::{Header, Response, Server};

#[test]
fn analyze_bundle_outputs_report() {
    let dir = tempdir().expect("tempdir");
    let bundle = dir.path().join("bundle.json");
    let body: Vec<u8> =
        br#"{"@context":{},"@id":"https://example.org/catalog","@type":"dcat:Catalog"}"#.to_vec();
    fs::write(
        &bundle,
        serde_json::to_string(&json!({
            "entry_url": "https://example.org/catalog.jsonld",
            "options": {},
            "artifacts": [{
                "url": "https://example.org/catalog.jsonld",
                "final_url": null,
                "status": 200,
                "media_type": "application/ld+json",
                "request_accept": null,
                "redirect_chain": [],
                "headers": [],
                "body": body,
                "fetched_at": "2026-05-19T00:00:00Z",
                "depth": 0,
                "discovered_from": null,
                "discovered_by": null
            }]
        }))
        .expect("write bundle"),
    )
    .expect("write bundle");

    Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args(["analyze-bundle", bundle.to_str().expect("path")])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schema_version\": \"semantic-asset-discovery.report.v1\"",
        ));
}

#[test]
fn validate_report_rejects_unknown_schema_version() {
    let dir = tempdir().expect("tempdir");
    let report = dir.path().join("report.json");
    fs::write(
        &report,
        r#"{"schema_version":"unknown","run_id":"run","entry_url":"https://example.org","analyzed_at":"2026-05-19T00:00:00Z","summary":{"artifact_count":0,"asset_count":0,"standard_count":0,"profile_count":0,"failed_artifact_count":0,"unsupported_artifact_count":0,"parse_error_count":0,"next_fetch_count":0,"truncated":false},"artifacts":[],"assets":[],"links":[],"standards":[],"profiles":[],"findings":[],"next_fetches":[]}"#,
    )
    .expect("write report");

    Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args(["validate-report", report.to_str().expect("path")])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("unsupported schema_version"));
}

#[test]
fn validate_report_rejects_supported_version_with_invalid_shape() {
    let dir = tempdir().expect("tempdir");
    let report = dir.path().join("report.json");
    fs::write(
        &report,
        r#"{"schema_version":"semantic-asset-discovery.report.v1","run_id":"run"}"#,
    )
    .expect("write report");

    Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args(["validate-report", report.to_str().expect("path")])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing field"));
}

#[test]
fn harvest_blocks_private_network_by_default() {
    Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args(["harvest", "http://127.0.0.1:9/catalog.jsonld"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("policy.private_network_blocked"));
}

#[test]
fn harvest_follows_declared_links_against_mock_server() {
    let server = Server::http("127.0.0.1:0").expect("mock server");
    let base_url = format!("http://{}", server.server_addr());
    let catalog_url = format!("{base_url}/catalog.jsonld");
    let openapi_url = format!("{base_url}/openapi.json");
    let server_thread = thread::spawn({
        let openapi_url = openapi_url.clone();
        move || {
            for _ in 0..2 {
                let request = server.recv().expect("request");
                let response_body = match request.url() {
                    "/catalog.jsonld" => json!({
                        "@context": {
                            "dcat": "http://www.w3.org/ns/dcat#",
                            "dcterms": "http://purl.org/dc/terms/"
                        },
                        "@id": "https://example.org/catalog",
                        "@type": "dcat:Catalog",
                        "dcterms:title": "Mock catalog",
                        "dcat:dataset": {
                            "@id": "https://example.org/datasets/mock",
                            "@type": "dcat:Dataset",
                            "dcterms:title": "Mock dataset",
                            "dcat:distribution": {
                                "@id": "https://example.org/datasets/mock/distribution",
                                "@type": "dcat:Distribution",
                                "dcat:accessService": {
                                    "@id": "https://example.org/services/mock",
                                    "@type": "dcat:DataService",
                                    "dcat:endpointDescription": openapi_url
                                }
                            }
                        }
                    })
                    .to_string(),
                    "/openapi.json" => json!({
                        "openapi": "3.1.0",
                        "info": {
                            "title": "Mock dataset API",
                            "version": "1.0.0"
                        },
                        "paths": {}
                    })
                    .to_string(),
                    other => panic!("unexpected path: {other}"),
                };
                let response = Response::from_string(response_body).with_header(
                    Header::from_bytes("content-type", "application/ld+json")
                        .expect("content-type header"),
                );
                request.respond(response).expect("respond");
            }
        }
    });

    Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args([
            "harvest",
            &catalog_url,
            "--allow-private-network",
            "--max-depth",
            "1",
            "--max-fetches",
            "3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"artifact_count\": 2"))
        .stdout(predicate::str::contains("\"fetched\""))
        .stdout(predicate::str::contains("\"rejected_fetches\""))
        .stdout(predicate::str::contains("Mock dataset"));

    server_thread.join().expect("server thread");
}

#[test]
fn harvest_outputs_discovery_run_envelope_after_facade_migration() {
    let server = Server::http("127.0.0.1:0").expect("mock server");
    let catalog_url = format!("http://{}/catalog.jsonld", server.server_addr());
    let server_thread = thread::spawn(move || {
        let request = server.recv().expect("request");
        let response = Response::from_string("{}").with_header(
            Header::from_bytes("content-type", "application/json").expect("content-type header"),
        );
        request.respond(response).expect("respond");
    });

    let output = Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .args([
            "harvest",
            &catalog_url,
            "--allow-private-network",
            "--max-depth",
            "0",
        ])
        .output()
        .expect("run harvest");
    assert!(
        output.status.success(),
        "harvest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("harvest emits json");
    assert_eq!(
        value["report"]["schema_version"],
        "semantic-asset-discovery.report.v1"
    );
    assert_eq!(value["fetched"]["entry_url"], catalog_url);
    assert!(value["fetched"]["fetched_count"].as_u64().is_some());
    assert!(value["fetched"]["rejected_count"].as_u64().is_some());
    assert!(value["fetched"]["redirect_count"].as_u64().is_some());
    assert!(value["fetched"]["total_decompressed_bytes"]
        .as_u64()
        .is_some());
    assert!(value["fetched"]["max_total_bytes"].as_u64().is_some());
    assert!(value["fetched"]["max_concurrent_fetches"]
        .as_u64()
        .is_some());
    assert!(value["fetched"]["total_elapsed_ms"].as_u64().is_some());
    assert!(value["rejected_fetches"].as_array().is_some());

    server_thread.join().expect("server thread");
}

#[test]
fn harvest_sends_bearer_token_from_env_without_echoing_secret() {
    let server = Server::http("127.0.0.1:0").expect("mock server");
    let catalog_url = format!("http://{}/catalog.jsonld", server.server_addr());
    let server_thread = thread::spawn(move || {
        let request = server.recv().expect("request");
        let authorization = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("authorization"))
            .map(|header| header.value.as_str().to_string());
        assert_eq!(
            authorization.as_deref(),
            Some("Bearer super-secret-demo-token")
        );
        let response = Response::from_string(
            json!({
                "@context": {"dcat": "http://www.w3.org/ns/dcat#"},
                "@id": "https://example.org/catalog",
                "@type": "dcat:Catalog"
            })
            .to_string(),
        )
        .with_header(
            Header::from_bytes("content-type", "application/ld+json").expect("content-type header"),
        );
        request.respond(response).expect("respond");
    });

    let output = Command::cargo_bin("semantic-asset-discovery")
        .expect("binary")
        .env("SAD_TEST_BEARER", "super-secret-demo-token")
        .args([
            "harvest",
            &catalog_url,
            "--allow-private-network",
            "--max-depth",
            "0",
            "--bearer-token-env",
            "SAD_TEST_BEARER",
        ])
        .output()
        .expect("run harvest");

    assert!(
        output.status.success(),
        "harvest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("super-secret-demo-token"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("super-secret-demo-token"));

    server_thread.join().expect("server thread");
}
