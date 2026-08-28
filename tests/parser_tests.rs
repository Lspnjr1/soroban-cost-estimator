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
