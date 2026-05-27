//! WASI smoke runner for the `@vuec-rs/wasm` compiler ABI.
//!
//! The binary reads optional JSON cases from stdin, executes the Rust-side
//! JSON helper functions, and prints a deterministic JSON smoke report.

#![forbid(unsafe_code)]

use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let output = match run() {
        Ok(value) => value,
        Err(error) => json!({
            "status": "fail",
            "errors": [{
                "code": "VUEC_WASI_SMOKE",
                "message": error.to_string(),
            }],
        }),
    };
    println!(
        "{}",
        serde_json::to_string(&output).unwrap_or_else(|_| "{}".into())
    );
}

fn run() -> Result<Value, Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request = if input.trim().is_empty() {
        default_request()
    } else {
        serde_json::from_str(&input)?
    };
    let cases = request
        .get("cases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![request]);
    let cases = cases
        .into_iter()
        .map(run_case)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "status": "pass",
        "cases": cases,
    }))
}

fn default_request() -> Value {
    json!({
        "cases": [
            {
                "name": "vue2-template",
                "command": "compileVue2",
                "source": "<div>{{ msg }}</div>"
            },
            {
                "name": "vue3-dom",
                "command": "compileVue3Dom",
                "source": "<div>{{ msg }}</div>",
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true
                }
            },
            {
                "name": "sfc-template",
                "command": "compileSfcTemplate",
                "source": "<template><div>{{ msg }}</div></template>",
                "options": {
                    "filename": "Wasi.vue"
                }
            }
        ]
    })
}

fn run_case(case: Value) -> Result<Value, Box<dyn std::error::Error>> {
    let name = case
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("anonymous");
    let command = case
        .get("command")
        .and_then(Value::as_str)
        .ok_or("missing command")?;
    let source = case.get("source").and_then(Value::as_str).unwrap_or("");
    let options = case.get("options").cloned().unwrap_or(Value::Null);
    let result = match command {
        "compileVue2" => vuec_wasm::compile_vue2_json(source, options),
        "compileVue3Dom" => vuec_wasm::compile_vue3_dom_json(source, options),
        "compileSfcTemplate" => vuec_wasm::compile_sfc_template_json(source, options),
        other => {
            return Err(format!("unsupported command `{other}`").into());
        }
    };
    Ok(json!({
        "name": name,
        "command": command,
        "result": result,
    }))
}
