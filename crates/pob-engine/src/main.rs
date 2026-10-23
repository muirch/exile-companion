//! exile-pob-engine --pob <dir>
//!     Serves JSON requests on stdin and stdout (see protocol.rs).
//! exile-pob-engine --pob <dir> --build <file.xml> <method> [json params]
//!     Loads a build, runs one method and prints the result. For trying things by hand.

use std::io::{stdin, stdout};
use std::path::PathBuf;
use std::process::ExitCode;

use exile_pob_engine::{Host, PobDir};
use serde_json::{json, Value};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut pob = None;
    let mut build = None;
    let mut rest = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--pob" => pob = args.next().map(PathBuf::from),
            "--build" => build = args.next().map(PathBuf::from),
            _ => rest.push(a),
        }
    }
    let pob =
        pob.ok_or("usage: exile-pob-engine --pob <dir> [--build <file.xml> <method> [json]]")?;
    let pob = PobDir::detect(&pob).map_err(|e| e.to_string())?;
    // Relative paths have to be resolved before the host moves into PoB's folder.
    let build = build
        .map(|b| std::fs::canonicalize(&b).map_err(|e| format!("{}: {e}", b.display())))
        .transpose()?;
    let host = Host::new(&pob).map_err(|e| e.to_string())?;

    let Some(build) = build else {
        let ready = json!({ "pob_version": pob.version });
        return host
            .serve(stdin().lock(), stdout().lock(), ready)
            .map_err(|e| e.to_string());
    };
    let xml = std::fs::read_to_string(&build).map_err(|e| format!("{}: {e}", build.display()))?;
    host.call("load_build", &json!({ "xml": xml }))
        .map_err(|e| e.to_string())?;
    let method = rest.first().map(String::as_str).unwrap_or("summary");
    let params: Value = match rest.get(1) {
        Some(p) => serde_json::from_str(p).map_err(|e| format!("params: {e}"))?,
        None => json!({}),
    };
    let out = host.call(method, &params).map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    Ok(())
}
