//! actant-contracts CLI.
//!
//! Subcommands:
//!   diff             — print the current schema JSON to stdout
//!   check-compat     — fail if the current schema breaks the generated baseline
//!   codegen-ts       — write TS types into packages/actant-types/src/generated/

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use actant_contracts::schema::schemas;
use serde_json::Value;

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let result = match cmd.as_str() {
        "diff" | "schema" => cmd_schema(),
        "check-compat" => cmd_check_compat(args.collect()),
        "codegen-ts" => cmd_codegen_ts(args.collect()),
        "help" | "--help" | "-h" | "" => {
            usage();
            return;
        }
        other => Err(format!("unknown subcommand: {other}")),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}

fn usage() {
    eprintln!(
        "actant-contracts — single source of truth for ActantDB types\n\
         \n\
         USAGE:\n\
         \x20 actant-contracts <SUBCOMMAND>\n\
         \n\
         SUBCOMMANDS:\n\
         \x20 diff              print the full JSON-Schema set to stdout\n\
         \x20 check-compat [baseline] verify current schemas against a baseline JSON bundle\n\
         \x20 codegen-ts [out]  emit TS into packages/actant-types/src/generated\n\
        "
    );
}

fn cmd_schema() -> Result<(), String> {
    let s = schemas();
    println!(
        "{}",
        serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn cmd_check_compat(extra: Vec<String>) -> Result<(), String> {
    let current = schemas();
    let Value::Object(current_map) = current else {
        return Err("schemas() must emit a JSON object".into());
    };
    if current_map.is_empty() {
        return Err("no schemas defined".into());
    }

    let baseline_path = extra
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(default_schema_baseline);
    let baseline = read_schema_baseline(&baseline_path)?;
    let violations = compatibility_violations(&baseline, &Value::Object(current_map.clone()));
    if !violations.is_empty() {
        return Err(format!(
            "schema compatibility check failed against {}:\n{}",
            baseline_path.display(),
            violations.join("\n")
        ));
    }
    eprintln!(
        "check-compat: ok ({} types checked against {})",
        current_map.len(),
        baseline_path.display()
    );
    Ok(())
}

fn default_schema_baseline() -> PathBuf {
    repo_root()
        .join("packages")
        .join("actant-types")
        .join("src")
        .join("generated")
        .join("schemas.json")
}

fn read_schema_baseline(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read schema baseline {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse schema baseline {}: {e}", path.display()))
}

fn compatibility_violations(baseline: &Value, current: &Value) -> Vec<String> {
    let mut out = Vec::new();
    compare_schema_value("$", baseline, current, &mut out);
    out
}

fn compare_schema_value(path: &str, baseline: &Value, current: &Value, out: &mut Vec<String>) {
    let Some(base_obj) = baseline.as_object() else {
        return;
    };
    let Some(cur_obj) = current.as_object() else {
        out.push(format!("{path}: schema changed from object to non-object"));
        return;
    };

    if path == "$" {
        for (key, base_schema) in base_obj {
            match cur_obj.get(key) {
                Some(cur_schema) => {
                    compare_schema_value(&format!("{path}.{key}"), base_schema, cur_schema, out);
                }
                None => out.push(format!("{path}.{key}: type removed")),
            }
        }
        return;
    }

    compare_scalar_field(path, base_obj, cur_obj, "type", out);
    compare_scalar_field(path, base_obj, cur_obj, "$ref", out);
    compare_enum(path, baseline, current, "enum", out);
    compare_string_one_of(path, baseline, current, out);

    if let Some(base_props) = baseline.get("properties").and_then(Value::as_object) {
        let Some(cur_props) = current.get("properties").and_then(Value::as_object) else {
            out.push(format!("{path}.properties: object properties removed"));
            return;
        };
        for (name, base_prop) in base_props {
            let prop_path = format!("{path}.properties.{name}");
            match cur_props.get(name) {
                Some(cur_prop) => compare_schema_value(&prop_path, base_prop, cur_prop, out),
                None => out.push(format!("{prop_path}: property removed")),
            }
        }
        compare_required(path, baseline, current, out);
    }

    for key in ["definitions", "$defs"] {
        let Some(base_defs) = baseline.get(key).and_then(Value::as_object) else {
            continue;
        };
        let Some(cur_defs) = current.get(key).and_then(Value::as_object) else {
            out.push(format!("{path}.{key}: definitions removed"));
            continue;
        };
        for (name, base_def) in base_defs {
            let def_path = format!("{path}.{key}.{name}");
            match cur_defs.get(name) {
                Some(cur_def) => compare_schema_value(&def_path, base_def, cur_def, out),
                None => out.push(format!("{def_path}: definition removed")),
            }
        }
    }
}

fn compare_scalar_field(
    path: &str,
    baseline: &serde_json::Map<String, Value>,
    current: &serde_json::Map<String, Value>,
    field: &str,
    out: &mut Vec<String>,
) {
    let Some(base_value) = baseline.get(field) else {
        return;
    };
    match current.get(field) {
        Some(cur_value) if cur_value == base_value => {}
        Some(cur_value) => out.push(format!(
            "{path}.{field}: changed from {base_value} to {cur_value}"
        )),
        None => out.push(format!("{path}.{field}: removed")),
    }
}

fn compare_enum(path: &str, baseline: &Value, current: &Value, field: &str, out: &mut Vec<String>) {
    let Some(base_values) = baseline.get(field).and_then(Value::as_array) else {
        return;
    };
    let Some(cur_values) = current.get(field).and_then(Value::as_array) else {
        out.push(format!("{path}.{field}: enum removed"));
        return;
    };
    let cur_set: BTreeSet<String> = cur_values.iter().map(Value::to_string).collect();
    for value in base_values {
        if !cur_set.contains(&value.to_string()) {
            out.push(format!("{path}.{field}: enum value {value} removed"));
        }
    }
}

fn compare_string_one_of(path: &str, baseline: &Value, current: &Value, out: &mut Vec<String>) {
    let Some(base_values) = string_one_of_values(baseline) else {
        return;
    };
    let Some(cur_values) = string_one_of_values(current) else {
        out.push(format!("{path}.oneOf: string enum union removed"));
        return;
    };
    for value in base_values {
        if !cur_values.contains(&value) {
            out.push(format!("{path}.oneOf: variant {value:?} removed"));
        }
    }
}

fn string_one_of_values(schema: &Value) -> Option<BTreeSet<String>> {
    let variants = schema.get("oneOf")?.as_array()?;
    collect_string_enum(variants).map(|values| values.into_iter().collect::<BTreeSet<String>>())
}

fn compare_required(path: &str, baseline: &Value, current: &Value, out: &mut Vec<String>) {
    let base_required = string_array_set(baseline.get("required"));
    let cur_required = string_array_set(current.get("required"));
    for field in base_required.difference(&cur_required) {
        out.push(format!(
            "{path}.required: field {field:?} no longer required"
        ));
    }
    for field in cur_required.difference(&base_required) {
        out.push(format!("{path}.required: new required field {field:?}"));
    }
}

fn string_array_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

fn cmd_codegen_ts(extra: Vec<String>) -> Result<(), String> {
    let default_out = repo_root()
        .join("packages")
        .join("actant-types")
        .join("src")
        .join("generated");
    let out = match extra.first() {
        Some(s) => PathBuf::from(s),
        None => default_out,
    };
    std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;

    let s = schemas();
    let Value::Object(map) = s else {
        return Err("schemas() must emit a JSON object".into());
    };

    // Clear stale per-type files from earlier generators.
    if let Ok(entries) = std::fs::read_dir(&out) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("ts") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }

    // Collate top-level types + cross-referenced definitions into one file
    // to avoid duplicate-export collisions across split files.
    let mut emit = TsEmit::new();
    for (name, schema) in &map {
        emit.emit_named(name, schema)?;
        if let Some(defs) = schema.get("definitions").and_then(|v| v.as_object()) {
            for (dname, def) in defs {
                if map.contains_key(dname) {
                    continue;
                }
                emit.emit_named(dname, def)?;
            }
        }
    }
    let bundle = emit.finish();

    let bundle_path = out.join("actant.ts");
    std::fs::write(&bundle_path, bundle)
        .map_err(|e| format!("write {}: {e}", bundle_path.display()))?;

    // Emit a single bundled schemas.json for tooling.
    let schemas_path = out.join("schemas.json");
    std::fs::write(
        &schemas_path,
        serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write {}: {e}", schemas_path.display()))?;

    // Emit index.ts.
    let index = "// AUTO-GENERATED by `cargo run -p actant-contracts -- codegen-ts`.\n\
                 // Hand-edits forbidden. Source of truth: crates/actant-contracts/.\n\n\
                 export * from \"./actant.js\";\n";
    let index_path = out.join("index.ts");
    std::fs::write(&index_path, index)
        .map_err(|e| format!("write {}: {e}", index_path.display()))?;

    eprintln!("codegen-ts: wrote {} types to {}", map.len(), out.display());
    Ok(())
}

fn repo_root() -> PathBuf {
    // `cargo run -p actant-contracts -- codegen-ts` runs in the workspace root.
    // Fall back to CARGO_MANIFEST_DIR's ../../ for safety.
    if let Ok(env) = std::env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(env)
            .parent()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Translate a JSON Schema (as produced by schemars 0.8) into a TypeScript
/// type declaration file.
///
/// Handles the constructs `actant-contracts` actually emits:
/// - string enums (schemars 0.8 emits these as `oneOf` of single-value-enum objects)
/// - tagged unions (`oneOf` with a `tag` property)
/// - plain structs (`type: "object"` with `properties`)
/// - field references via `allOf: [{$ref: ...}]` (schemars 0.8 wraps refs)
/// - optional fields (via `required` whitelist and Option's `T | null`)
/// - Vec, primitives, serde_json::Value (-> unknown)
#[allow(dead_code)]
fn ts_from_schema(top_name: &str, schema: &Value) -> Result<String, String> {
    let defs = schema
        .get("definitions")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut emit = TsEmit::new();
    emit.emit_named(top_name, schema)?;
    for (name, def) in &defs {
        if name == top_name {
            continue;
        }
        emit.emit_named(name, def)?;
    }
    Ok(emit.finish())
}

struct TsEmit {
    decls: Vec<String>,
    seen: std::collections::HashSet<String>,
}

impl TsEmit {
    fn new() -> Self {
        Self {
            decls: Vec::new(),
            seen: std::collections::HashSet::new(),
        }
    }

    fn finish(mut self) -> String {
        let mut header = String::from(
            "// AUTO-GENERATED by `cargo run -p actant-contracts -- codegen-ts`.\n\
             // Hand-edits forbidden. Source of truth: crates/actant-contracts/.\n\n",
        );
        for d in self.decls.drain(..) {
            header.push_str(&d);
            header.push_str("\n\n");
        }
        header
    }

    fn emit_named(&mut self, name: &str, schema: &Value) -> Result<(), String> {
        if !self.seen.insert(name.to_string()) {
            return Ok(());
        }
        // Pattern 1: oneOf of single-string-enums = string union enum.
        if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
            if let Some(vals) = collect_string_enum(one_of) {
                let parts: Vec<String> = vals.iter().map(|s| format!("\"{s}\"")).collect();
                self.decls
                    .push(format!("export type {name} = {};", parts.join(" | ")));
                return Ok(());
            }
            // Pattern 2: tagged union (oneOf of object schemas, each with a tag prop)
            let union = self.tagged_union(one_of)?;
            self.decls.push(format!("export type {name} =\n{union};"));
            return Ok(());
        }
        // Direct string enum
        if let Some(enums) = schema.get("enum").and_then(|v| v.as_array()) {
            let parts: Vec<String> = enums
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("\"{s}\""))
                .collect();
            self.decls
                .push(format!("export type {name} = {};", parts.join(" | ")));
            return Ok(());
        }
        // Struct
        if schema.get("type").and_then(|v| v.as_str()) == Some("object")
            || schema.get("properties").is_some()
        {
            let body = self.object_body(schema)?;
            self.decls.push(format!("export interface {name} {body}"));
            return Ok(());
        }
        let alias = self.ts_type(schema)?;
        self.decls.push(format!("export type {name} = {alias};"));
        Ok(())
    }

    fn object_body(&self, schema: &Value) -> Result<String, String> {
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let required: std::collections::HashSet<String> = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        // Stable property order: required first (in spec order), then optional alphabetical.
        let mut keys: Vec<String> = props.keys().cloned().collect();
        keys.sort_by(|a, b| match (required.contains(a), required.contains(b)) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.cmp(b),
        });
        let mut body = String::from("{\n");
        for pname in keys {
            let pschema = &props[&pname];
            let opt = if required.contains(&pname) { "" } else { "?" };
            let ty = self.ts_type(pschema)?;
            body.push_str(&format!("  {pname}{opt}: {ty};\n"));
        }
        body.push('}');
        Ok(body)
    }

    fn tagged_union(&self, variants: &[Value]) -> Result<String, String> {
        let mut parts: Vec<String> = Vec::new();
        for v in variants {
            let body = self.object_body(v)?;
            parts.push(format!("  | {body}").replace('\n', "\n    "));
        }
        Ok(parts.join("\n"))
    }

    #[allow(clippy::only_used_in_recursion)]
    fn ts_type(&self, schema: &Value) -> Result<String, String> {
        // schemars 0.8 wraps a single $ref in allOf when annotating with description, etc.
        if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
            if all.len() == 1 {
                return self.ts_type(&all[0]);
            }
        }
        // $ref → bare definition name
        if let Some(r) = schema.get("$ref").and_then(|v| v.as_str()) {
            if let Some(name) = r.strip_prefix("#/definitions/") {
                return Ok(name.to_string());
            }
        }
        // oneOf / anyOf
        for key in ["anyOf", "oneOf"] {
            if let Some(arr) = schema.get(key).and_then(|v| v.as_array()) {
                // Special case: oneOf of single string enums (inline literal union).
                if let Some(vals) = collect_string_enum(arr) {
                    let parts: Vec<String> = vals.iter().map(|s| format!("\"{s}\"")).collect();
                    return Ok(parts.join(" | "));
                }
                let parts: Result<Vec<String>, String> =
                    arr.iter().map(|s| self.ts_type(s)).collect();
                let mut parts = parts?;
                parts.sort();
                parts.dedup();
                return Ok(parts.join(" | "));
            }
        }
        // Direct enum
        if let Some(enums) = schema.get("enum").and_then(|v| v.as_array()) {
            let parts: Vec<String> = enums
                .iter()
                .map(|v| match v {
                    Value::String(s) => format!("\"{s}\""),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Null => "null".to_string(),
                    _ => "unknown".to_string(),
                })
                .collect();
            return Ok(parts.join(" | "));
        }

        match schema.get("type") {
            Some(Value::String(t)) => Ok(match t.as_str() {
                "string" => "string".to_string(),
                "integer" | "number" => "number".to_string(),
                "boolean" => "boolean".to_string(),
                "null" => "null".to_string(),
                "array" => {
                    let items = schema
                        .get("items")
                        .map(|i| self.ts_type(i))
                        .transpose()?
                        .unwrap_or_else(|| "unknown".into());
                    format!("Array<{items}>")
                }
                "object" => "Record<string, unknown>".to_string(),
                _ => "unknown".to_string(),
            }),
            Some(Value::Array(arr)) => {
                let parts: Vec<String> = arr
                    .iter()
                    .map(|v| match v.as_str() {
                        Some("null") => "null".to_string(),
                        Some("string") => "string".to_string(),
                        Some("integer") | Some("number") => "number".to_string(),
                        Some("boolean") => "boolean".to_string(),
                        Some("array") => "Array<unknown>".to_string(),
                        Some("object") => "Record<string, unknown>".to_string(),
                        _ => "unknown".to_string(),
                    })
                    .collect();
                Ok(parts.join(" | "))
            }
            _ => Ok("unknown".to_string()),
        }
    }
}

/// If every variant of a `oneOf` is `{type: "string", enum: ["v"]}`,
/// return the list of values. Schemars 0.8 emits string-enums this way.
fn collect_string_enum(variants: &[Value]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for v in variants {
        let is_string = v.get("type").and_then(|t| t.as_str()) == Some("string");
        let enums = v.get("enum").and_then(|e| e.as_array())?;
        if !is_string || enums.len() != 1 {
            return None;
        }
        let s = enums[0].as_str()?;
        out.push(s.to_string());
    }
    Some(out)
}

#[cfg(test)]
mod compat_tests {
    use serde_json::json;

    use super::compatibility_violations;

    #[test]
    fn compatibility_allows_added_optional_fields_and_types() {
        let baseline = json!({
            "Thing": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" }
                }
            }
        });
        let current = json!({
            "Thing": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "label": { "type": "string" }
                }
            },
            "NewThing": { "type": "object" }
        });

        assert!(compatibility_violations(&baseline, &current).is_empty());
    }

    #[test]
    fn compatibility_rejects_removed_properties_and_new_required_fields() {
        let baseline = json!({
            "Thing": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" }
                }
            }
        });
        let current = json!({
            "Thing": {
                "type": "object",
                "required": ["id", "kind"],
                "properties": {
                    "id": { "type": "string" },
                    "kind": { "type": "string" }
                }
            }
        });

        let violations = compatibility_violations(&baseline, &current);
        assert!(violations.iter().any(|v| v.contains("name")));
        assert!(violations.iter().any(|v| v.contains("new required field")));
    }

    #[test]
    fn compatibility_rejects_removed_enum_variants() {
        let baseline = json!({
            "Verdict": {
                "oneOf": [
                    { "type": "string", "enum": ["allow"] },
                    { "type": "string", "enum": ["block"] }
                ]
            }
        });
        let current = json!({
            "Verdict": {
                "oneOf": [
                    { "type": "string", "enum": ["allow"] }
                ]
            }
        });

        let violations = compatibility_violations(&baseline, &current);
        assert!(violations.iter().any(|v| v.contains("block")));
    }
}
