use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let schema_dir = out_dir.join("codex-schemas");
    let _ = fs::create_dir_all(&schema_dir);

    let generated = Command::new("codex")
        .args([
            "app-server",
            "generate-json-schema",
            "--experimental",
            "--out",
            schema_dir.to_string_lossy().as_ref(),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !generated {
        let fallback = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CodexAppServerProtocolFallback",
  "description": "Minimal fallback used when codex app-server schema generation is unavailable at build time."
}"#;
        let _ = fs::write(
            schema_dir.join("codex_app_server_protocol.fallback.json"),
            fallback,
        );
    }
}
