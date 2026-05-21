use semantic_asset_discovery_core::{
    analyze_artifacts, AnalyzeInput, WasmAnalyzeError, WasmAnalyzeResult,
    DEFAULT_WASM_BODY_BUDGET_BYTES, REPORT_SCHEMA_VERSION,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn version() -> String {
    REPORT_SCHEMA_VERSION.to_string()
}

#[wasm_bindgen(js_name = analyzeArtifacts)]
pub fn analyze_artifacts_json(input_json: &str) -> String {
    let result = std::panic::catch_unwind(|| analyze_artifacts_json_inner(input_json));
    match result {
        Ok(output) => output,
        Err(_) => serialize_result(WasmAnalyzeResult::Err {
            error: WasmAnalyzeError {
                code: "analyze.panic".to_string(),
                message: "Analysis failed unexpectedly".to_string(),
            },
        }),
    }
}

pub fn analyze_artifacts_json_inner(input_json: &str) -> String {
    match serde_json::from_str::<AnalyzeInput>(input_json) {
        Ok(input) => {
            let body_size: u64 = input
                .artifacts
                .iter()
                .map(|artifact| artifact.body.len() as u64)
                .sum();
            if body_size > DEFAULT_WASM_BODY_BUDGET_BYTES {
                return serialize_result(WasmAnalyzeResult::Err {
                    error: WasmAnalyzeError {
                        code: "analyze.payload_too_large".to_string(),
                        message: "Analyze input exceeds the WASM body budget".to_string(),
                    },
                });
            }

            match analyze_artifacts(input) {
                Ok(report) => serialize_result(WasmAnalyzeResult::Ok { report }),
                Err(error) => serialize_result(WasmAnalyzeResult::Err {
                    error: WasmAnalyzeError {
                        code: "analyze.invalid_input".to_string(),
                        message: error.to_string(),
                    },
                }),
            }
        }
        Err(error) => serialize_result(WasmAnalyzeResult::Err {
            error: WasmAnalyzeError {
                code: "analyze.invalid_input".to_string(),
                message: format!("Invalid analyze input: {error}"),
            },
        }),
    }
}

fn serialize_result(result: WasmAnalyzeResult) -> String {
    serde_json::to_string(&result).unwrap_or_else(|_| {
        r#"{"ok":false,"error":{"code":"analyze.serialization_failed","message":"Analysis result could not be serialized"}}"#
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_asset_discovery_core::{AnalyzeOptions, FetchedArtifact};

    fn input_with_body(body: Vec<u8>) -> AnalyzeInput {
        AnalyzeInput {
            entry_url: "https://example.test/catalog.jsonld".to_string(),
            analyzed_at: Some("2026-05-19T00:00:00Z".to_string()),
            artifacts: vec![FetchedArtifact {
                url: "https://example.test/catalog.jsonld".to_string(),
                final_url: None,
                status: 200,
                media_type: Some("application/ld+json".to_string()),
                request_accept: None,
                redirect_chain: Vec::new(),
                headers: Vec::new(),
                body,
                fetched_at: "2026-05-19T00:00:00Z".to_string(),
                depth: 0,
                discovered_from: None,
                discovered_by: None,
            }],
            options: AnalyzeOptions::default(),
        }
    }

    #[test]
    fn returns_ok_envelope() {
        let input = input_with_body(
            br#"{"@context":{},"@id":"https://example.test/catalog","@type":"dcat:Catalog"}"#
                .to_vec(),
        );
        let output =
            analyze_artifacts_json_inner(&serde_json::to_string(&input).expect("input serializes"));
        let value: serde_json::Value = serde_json::from_str(&output).expect("output json");
        assert_eq!(value["ok"], true);
        assert_eq!(
            value["report"]["schema_version"],
            "semantic-asset-discovery.report.v1"
        );
    }

    #[test]
    fn exposes_report_schema_version() {
        assert_eq!(version(), "semantic-asset-discovery.report.v1");
    }

    #[test]
    fn returns_invalid_input_envelope() {
        let output = analyze_artifacts_json_inner("{");
        let value: serde_json::Value = serde_json::from_str(&output).expect("output json");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "analyze.invalid_input");
    }

    #[test]
    fn enforces_payload_budget() {
        let input = input_with_body(vec![b'a'; (DEFAULT_WASM_BODY_BUDGET_BYTES + 1) as usize]);
        let output =
            analyze_artifacts_json_inner(&serde_json::to_string(&input).expect("input serializes"));
        let value: serde_json::Value = serde_json::from_str(&output).expect("output json");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "analyze.payload_too_large");
    }
}
