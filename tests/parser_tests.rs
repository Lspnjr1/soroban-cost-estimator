use std::path::Path;

#[test]
fn test_load_minimal_wasm() {
    let path = Path::new("tests/fixtures/minimal.wasm");
    assert!(path.exists(), "test WASM fixture not found");

    let wasm_info =
        soroban_cost_estimator::wasm::parser::load_wasm(path).expect("failed to load test WASM");

    assert!(!wasm_info.bytes.is_empty(), "WASM should have bytes");
    assert_eq!(wasm_info.bytes.len(), 44, "unexpected WASM size");

    // Should find at least one exported function
    assert!(
        !wasm_info.functions.is_empty(),
        "WASM should have exported functions"
    );
    let names: Vec<String> = wasm_info.functions.iter().map(|f| f.name.clone()).collect();
    assert!(
        names.contains(&"add_one".to_string()),
        "should contain 'add_one' function, got: {:?}",
        names
    );
}

/// The real-contract fixture is a compiled Soroban contract (contractspecv0
/// custom section + typed params), structurally identical to what a real
/// submission would use — unlike `minimal.wasm`, which is bare WASM.
#[test]
fn test_load_real_soroban_contract_fixture() {
    let path = Path::new("tests/fixtures/contract.wasm");
    assert!(
        path.exists(),
        "real contract fixture not found; build with tests/fixtures/contract/build.sh"
    );

    let wasm_info = soroban_cost_estimator::wasm::parser::load_wasm(path)
        .expect("failed to load contract fixture");

    assert!(
        wasm_info.has_spec,
        "fixture should carry a contractspecv0 section"
    );

    let inc = wasm_info
        .functions
        .iter()
        .find(|f| f.name == "increment")
        .expect("fixture should export 'increment'");

    // One exported function, one typed argument: the spec must decode real
    // typed params, not bare WASM export signatures.
    assert_eq!(inc.param_count, 1);
    assert_eq!(
        inc.params.len(),
        1,
        "increment should declare one typed param"
    );
    assert_eq!(inc.params[0].name, "step");
    assert_eq!(inc.params[0].type_name, "i64");

    let formatted = soroban_cost_estimator::wasm::parser::format_function(inc);
    assert!(
        formatted.contains("step") && formatted.contains("i64"),
        "got: {formatted}"
    );
}

#[test]
fn test_invalid_wasm_rejected() {
    let invalid_bytes = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]; // magic only, no content
    let temp_dir = std::env::temp_dir();
    let invalid_path = temp_dir.join("invalid.wasm");
    std::fs::write(&invalid_path, &invalid_bytes).unwrap();

    let result = soroban_cost_estimator::wasm::parser::load_wasm(&invalid_path);
    assert!(result.is_err(), "invalid WASM should be rejected");

    let _ = std::fs::remove_file(&invalid_path);
}

#[test]
fn test_nonexistent_wasm() {
    let result = soroban_cost_estimator::wasm::parser::load_wasm(Path::new(
        "tests/fixtures/nonexistent.wasm",
    ));
    assert!(result.is_err(), "nonexistent file should error");
}

#[test]
fn test_capture_sections_minimal_wasm() {
    let path = Path::new("tests/fixtures/minimal.wasm");
    let wasm_info =
        soroban_cost_estimator::wasm::parser::load_wasm(path).expect("failed to load test WASM");

    assert_eq!(
        wasm_info.size,
        wasm_info.bytes.len(),
        "size must match bytes"
    );
    assert!(
        wasm_info.sections.count >= 3,
        "minimal module should have at least a type, function, and code section"
    );
}

#[test]
fn test_capture_sections_real_contract_has_contractspec() {
    let path = Path::new("tests/fixtures/contract.wasm");
    let wasm_info = soroban_cost_estimator::wasm::parser::load_wasm(path)
        .expect("failed to load contract fixture");

    assert!(wasm_info.sections.count > 0, "module should have sections");
    assert!(
        wasm_info
            .sections
            .custom_names
            .iter()
            .any(|name| name == "contractspecv0"),
        "custom section names must include contractspecv0, got: {:?}",
        wasm_info.sections.custom_names
    );
}

// ─────────────────────────────────────────────────────────────────────────
// contractmeta custom-section parsing helpers
// ─────────────────────────────────────────────────────────────────────────

/// Encodes an XDR string: 4-byte big-endian length + UTF-8 bytes, padded to a
/// 4-byte boundary (XDR strings are padded, `pad_len` in stellar-xdr).
fn xdr_string(s: &str) -> Vec<u8> {
    let mut out = (s.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    let padding = (4 - s.len() % 4) % 4;
    out.extend_from_slice(&[0u8; 4][..padding]);
    out
}

/// Encodes one `ScMetaEntry::ScMetaV0` union value: 4-byte discriminant 0,
/// then the `{ key, val }` XDR struct.
fn xdr_meta_entry(key: &str, val: &str) -> Vec<u8> {
    let mut out = 0u32.to_be_bytes().to_vec();
    out.extend_from_slice(&xdr_string(key));
    out.extend_from_slice(&xdr_string(val));
    out
}

/// Wraps `payload` in a WASM custom section (id 0) named `name`.
/// Short ASCII names only (a single length byte).
fn custom_section(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut content = Vec::new();
    content.push(name.len() as u8);
    content.extend_from_slice(name.as_bytes());
    content.extend_from_slice(payload);

    let mut section = vec![0u8]; // custom section id
    let mut size = content.len() as u32;
    loop {
        let mut byte = (size & 0x7f) as u8;
        size >>= 7;
        if size != 0 {
            byte |= 0x80;
        }
        section.push(byte);
        if size == 0 {
            break;
        }
    }
    section.extend_from_slice(&content);
    section
}

/// The bare fixture extended with a `contractmetav0` section carrying
/// name/version/description plus one custom key.
fn wasm_with_contract_meta() -> Vec<u8> {
    let mut bytes = std::fs::read("tests/fixtures/minimal.wasm").expect("read fixture");
    let mut payload = Vec::new();
    payload.extend_from_slice(&xdr_meta_entry("name", "MetaContract"));
    payload.extend_from_slice(&xdr_meta_entry("version", "9.9.9"));
    payload.extend_from_slice(&xdr_meta_entry("description", "A meta description"));
    payload.extend_from_slice(&xdr_meta_entry("custom_key", "custom_value"));
    bytes.extend_from_slice(&custom_section("contractmetav0", &payload));
    bytes
}

// ─────────────────────────────────────────────────────────────────────────
// contractmeta custom-section parsing
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn test_parse_contract_meta_extracts_name_version_description() {
    let bytes = wasm_with_contract_meta();
    let meta = soroban_cost_estimator::wasm::parser::parse_contract_meta(&bytes)
        .expect("meta should parse");

    assert_eq!(meta.name.as_deref(), Some("MetaContract"));
    assert_eq!(meta.version.as_deref(), Some("9.9.9"));
    assert_eq!(meta.description.as_deref(), Some("A meta description"));
    assert_eq!(meta.entries.len(), 4);
    assert!(
        meta.entries
            .contains(&("custom_key".to_string(), "custom_value".to_string()))
    );
}

#[test]
fn test_load_wasm_populates_contract_meta() {
    let bytes = wasm_with_contract_meta();
    let temp = std::env::temp_dir().join(format!("sce-meta-{}.wasm", std::process::id()));
    std::fs::write(&temp, &bytes).expect("write fixture");

    let wasm_info = soroban_cost_estimator::wasm::parser::load_wasm(&temp)
        .expect("wasm with appended custom section should load");
    assert_eq!(
        wasm_info.contract_meta.name.as_deref(),
        Some("MetaContract")
    );
    assert_eq!(wasm_info.contract_meta.version.as_deref(), Some("9.9.9"));

    let formatted =
        soroban_cost_estimator::wasm::parser::format_contract_meta(&wasm_info.contract_meta);
    assert!(formatted.contains("Contract meta: present"));
    assert!(formatted.contains("name: MetaContract"));
    assert!(formatted.contains("version: 9.9.9"));
    assert!(formatted.contains("description: A meta description"));
    assert!(formatted.contains("custom_key: custom_value"));

    let _ = std::fs::remove_file(&temp);
}

#[test]
fn test_parse_contract_meta_absent_without_section() {
    let bytes = std::fs::read("tests/fixtures/minimal.wasm").expect("read fixture");
    let meta = soroban_cost_estimator::wasm::parser::parse_contract_meta(&bytes)
        .expect("absent section is not an error");
    assert!(meta.is_empty());
    assert_eq!(
        soroban_cost_estimator::wasm::parser::format_contract_meta(&meta),
        "Contract meta: absent"
    );
}

/// The soroban-sdk-built fixture carries a real `contractmetav0` section with
/// build metadata (rustc / sdk versions); the parser must surface it.
#[test]
fn test_parse_contract_meta_real_fixture() {
    let path = Path::new("tests/fixtures/contract.wasm");
    let wasm_info = soroban_cost_estimator::wasm::parser::load_wasm(path)
        .expect("failed to load contract fixture");

    let meta = &wasm_info.contract_meta;
    assert!(
        !meta.is_empty(),
        "soroban-sdk-built fixture should carry a contractmeta section"
    );
    assert!(
        meta.entries.iter().any(|(k, _)| k == "rsver"),
        "fixture meta should include the rustc version entry"
    );
    assert!(
        meta.entries.iter().any(|(k, _)| k == "rssdkver"),
        "fixture meta should include the sdk version entry"
    );

    let formatted = soroban_cost_estimator::wasm::parser::format_contract_meta(meta);
    assert!(formatted.contains("Contract meta: present"));
    assert!(formatted.contains("rsver:"));
    assert!(formatted.contains("rssdkver:"));
}
