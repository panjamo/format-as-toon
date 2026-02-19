//! TOON (Token-Oriented Object Notation) encoder.
//!
//! Converts [`serde_json::Value`] to TOON — a compact, human-readable format
//! that reduces token usage by 30–60% compared to JSON.
//!
//! # Example
//!
//! ```
//! use format_as_toon::{ToonOptions, encode_toon};
//! use serde_json::json;
//!
//! let value = json!({"name": "Alice", "age": 30});
//! let output = encode_toon(&value, &ToonOptions::default());
//! assert_eq!(output, "name: Alice\nage: 30");
//! ```

use serde_json::Value;

/// Delimiter used between array elements and tabular row values.
#[derive(Debug, Clone, Copy, Default)]
pub enum Delimiter {
    #[default]
    Comma,
    Tab,
    Pipe,
}

/// Key folding mode — collapses single-key object chains into dotted paths.
#[derive(Debug, Clone, Copy, Default)]
pub enum KeyFolding {
    #[default]
    Off,
    Safe,
}

/// Options controlling TOON encoding.
pub struct ToonOptions {
    /// Delimiter between array/row values.
    pub delimiter: Delimiter,
    /// Number of spaces per indentation level.
    pub indent: usize,
    /// Key folding mode.
    pub key_folding: KeyFolding,
    /// Maximum number of levels to fold (default: [`usize::MAX`]).
    pub flatten_depth: usize,
}

impl Default for ToonOptions {
    fn default() -> Self {
        Self {
            delimiter: Delimiter::Comma,
            indent: 2,
            key_folding: KeyFolding::Off,
            flatten_depth: usize::MAX,
        }
    }
}

/// Encode a JSON value as TOON.
pub fn encode_toon(value: &Value, opts: &ToonOptions) -> String {
    let mut out = String::new();
    encode_value(&mut out, value, 0, opts, true);
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

impl Delimiter {
    fn char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Tab => '\t',
            Self::Pipe => '|',
        }
    }

    fn header_symbol(self) -> &'static str {
        match self {
            Self::Comma => "",
            Self::Tab => "\t",
            Self::Pipe => "|",
        }
    }
}

fn encode_value(out: &mut String, value: &Value, depth: usize, opts: &ToonOptions, is_root: bool) {
    match value {
        Value::Object(map) => encode_object(out, map, depth, opts, is_root),
        Value::Array(arr) => encode_array_field(out, "", arr, depth, opts, ""),
        _ if is_root => out.push_str(&format_scalar(value, opts.delimiter)),
        _ => {}
    }
}

fn encode_object(
    out: &mut String,
    map: &serde_json::Map<String, Value>,
    depth: usize,
    opts: &ToonOptions,
    _is_root: bool,
) {
    let indent = " ".repeat(depth * opts.indent);
    let mut first = true;

    for (key, value) in map {
        if !first {
            out.push('\n');
        }

        if matches!(opts.key_folding, KeyFolding::Safe) && is_valid_identifier(key) {
            let mut chain = vec![key.as_str()];
            let mut current = value;
            while chain.len() - 1 < opts.flatten_depth {
                if let Value::Object(inner) = current {
                    if inner.len() == 1 {
                        let (k, v) = inner.iter().next().unwrap();
                        if is_valid_identifier(k) && !needs_quoting(k, opts.delimiter) {
                            chain.push(k.as_str());
                            current = v;
                            continue;
                        }
                    }
                }
                break;
            }

            if chain.len() > 1 {
                let folded_key = chain.join(".");
                encode_field(out, &folded_key, current, depth, opts, &indent);
                first = false;
                continue;
            }
        }

        encode_field(out, key, value, depth, opts, &indent);
        first = false;
    }
}

fn encode_field(
    out: &mut String,
    key: &str,
    value: &Value,
    depth: usize,
    opts: &ToonOptions,
    indent: &str,
) {
    let fkey = format_key(key, opts.delimiter);

    match value {
        Value::Object(inner) if !inner.is_empty() => {
            out.push_str(indent);
            out.push_str(&fkey);
            out.push(':');
            out.push('\n');
            encode_object(out, inner, depth + 1, opts, false);
        }
        Value::Object(_) => {
            out.push_str(indent);
            out.push_str(&fkey);
            out.push(':');
        }
        Value::Array(arr) => {
            encode_array_field(out, &fkey, arr, depth, opts, indent);
        }
        _ => {
            out.push_str(indent);
            out.push_str(&fkey);
            out.push_str(": ");
            out.push_str(&format_scalar(value, opts.delimiter));
        }
    }
}

fn encode_array_field(
    out: &mut String,
    key: &str,
    arr: &[Value],
    depth: usize,
    opts: &ToonOptions,
    indent: &str,
) {
    let n = arr.len();
    let dsym = opts.delimiter.header_symbol();
    let delim_ch = opts.delimiter.char();

    if n == 0 {
        out.push_str(indent);
        out.push_str(key);
        out.push_str(&format!("[0{dsym}]:"));
        return;
    }

    // All primitives -> inline
    if arr.iter().all(is_primitive) {
        let values: Vec<String> = arr
            .iter()
            .map(|v| format_scalar(v, opts.delimiter))
            .collect();
        out.push_str(indent);
        out.push_str(key);
        out.push_str(&format!("[{n}{dsym}]: "));
        out.push_str(&values.join(&delim_ch.to_string()));
        return;
    }

    // Tabular: all objects with identical keys, all primitive values
    if let Some(fields) = detect_tabular(arr) {
        let field_names: Vec<String> = fields
            .iter()
            .map(|f| format_key(f, opts.delimiter))
            .collect();
        let field_header = field_names.join(&delim_ch.to_string());
        out.push_str(indent);
        out.push_str(key);
        out.push_str(&format!("[{n}{dsym}]{{{field_header}}}:"));

        let child_indent = " ".repeat((depth + 1) * opts.indent);
        for item in arr {
            if let Value::Object(map) = item {
                let row: Vec<String> = fields
                    .iter()
                    .map(|f| format_scalar(map.get(f).unwrap_or(&Value::Null), opts.delimiter))
                    .collect();
                out.push('\n');
                out.push_str(&child_indent);
                out.push_str(&row.join(&delim_ch.to_string()));
            }
        }
        return;
    }

    // Expanded list form
    out.push_str(indent);
    out.push_str(key);
    out.push_str(&format!("[{n}{dsym}]:"));

    let child_indent = " ".repeat((depth + 1) * opts.indent);
    for item in arr {
        out.push('\n');
        match item {
            Value::Object(map) if !map.is_empty() => {
                let mut obj_out = String::new();
                encode_object(&mut obj_out, map, depth + 2, opts, false);
                // First field goes on same line as `-`
                if let Some(first_newline) = obj_out.find('\n') {
                    let first_line = &obj_out[..first_newline];
                    let rest = &obj_out[first_newline..];
                    out.push_str(&child_indent);
                    out.push_str("- ");
                    out.push_str(first_line.trim_start());
                    out.push_str(rest);
                } else {
                    out.push_str(&child_indent);
                    out.push_str("- ");
                    out.push_str(obj_out.trim_start());
                }
            }
            Value::Object(_) => {
                out.push_str(&child_indent);
                out.push('-');
            }
            Value::Array(inner) => {
                let inner_n = inner.len();
                out.push_str(&child_indent);
                if inner.iter().all(is_primitive) {
                    let values: Vec<String> = inner
                        .iter()
                        .map(|v| format_scalar(v, opts.delimiter))
                        .collect();
                    out.push_str(&format!("- [{inner_n}{dsym}]: "));
                    out.push_str(&values.join(&delim_ch.to_string()));
                } else {
                    out.push_str(&format!("- [{inner_n}{dsym}]:"));
                    let nested_indent = " ".repeat((depth + 2) * opts.indent);
                    for inner_item in inner {
                        out.push('\n');
                        out.push_str(&nested_indent);
                        out.push_str("- ");
                        out.push_str(&format_scalar(inner_item, opts.delimiter));
                    }
                }
            }
            _ => {
                out.push_str(&child_indent);
                out.push_str("- ");
                out.push_str(&format_scalar(item, opts.delimiter));
            }
        }
    }
}

fn detect_tabular(arr: &[Value]) -> Option<Vec<String>> {
    let first = arr.first()?.as_object()?;
    let keys: Vec<String> = first.keys().cloned().collect();
    if keys.is_empty() || !first.values().all(is_primitive) {
        return None;
    }
    for item in &arr[1..] {
        let obj = item.as_object()?;
        if obj.len() != keys.len() {
            return None;
        }
        for key in &keys {
            if !is_primitive(obj.get(key)?) {
                return None;
            }
        }
    }
    Some(keys)
}

fn is_primitive(v: &Value) -> bool {
    matches!(
        v,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn needs_quoting(s: &str, delimiter: Delimiter) -> bool {
    if s.is_empty() || matches!(s, "true" | "false" | "null") || s.starts_with('-') {
        return true;
    }
    if looks_like_number(s) {
        return true;
    }
    if s.len() > 1 && s.starts_with('0') && s.as_bytes()[1].is_ascii_digit() {
        return true;
    }
    if s.starts_with(' ') || s.ends_with(' ') {
        return true;
    }
    let delim = delimiter.char();
    s.chars().any(|c| {
        matches!(
            c,
            ':' | '"' | '\\' | '[' | ']' | '{' | '}' | '\n' | '\r' | '\t'
        ) || c == delim
    })
}

fn looks_like_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && b[i] == b'-' {
        i += 1;
    }
    if i >= b.len() || !b[i].is_ascii_digit() {
        return false;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        if i >= b.len() || !b[i].is_ascii_digit() {
            return false;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < b.len() && matches!(b[i], b'e' | b'E') {
        i += 1;
        if i < b.len() && matches!(b[i], b'+' | b'-') {
            i += 1;
        }
        if i >= b.len() || !b[i].is_ascii_digit() {
            return false;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    i == b.len()
}

fn format_key(key: &str, delimiter: Delimiter) -> String {
    if needs_quoting(key, delimiter) {
        format!("\"{}\"", escape_string(key))
    } else {
        key.to_string()
    }
}

fn format_scalar(value: &Value, delimiter: Delimiter) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => format_number(n),
        Value::String(s) => {
            if needs_quoting(s, delimiter) {
                format!("\"{}\"", escape_string(s))
            } else {
                s.clone()
            }
        }
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn format_number(n: &serde_json::Number) -> String {
    if let Some(f) = n.as_f64() {
        if f == 0.0 {
            return "0".to_string();
        }
        if f.is_nan() || f.is_infinite() {
            return "null".to_string();
        }
        if f.fract() == 0.0 && f.abs() < (i64::MAX as f64) {
            return format!("{}", f as i64);
        }
        let s = format!("{f}");
        if s.contains('e') || s.contains('E') {
            let formatted = format!("{f:.20}");
            return formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();
        }
        s
    } else {
        n.to_string()
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn default_opts() -> ToonOptions {
        ToonOptions::default()
    }

    #[test]
    fn test_simple_object() {
        let v: Value = serde_json::from_str(r#"{"name":"Alice","age":30}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "name: Alice\nage: 30");
    }

    #[test]
    fn test_nested_object() {
        let v: Value = serde_json::from_str(r#"{"user":{"name":"Alice","age":30}}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "user:\n  name: Alice\n  age: 30");
    }

    #[test]
    fn test_primitive_array() {
        let v: Value = serde_json::from_str(r#"{"tags":["a","b","c"]}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "tags[3]: a,b,c");
    }

    #[test]
    fn test_tabular_array() {
        let v: Value =
            serde_json::from_str(r#"{"users":[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]}"#)
                .unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "users[2]{id,name}:\n  1,Alice\n  2,Bob");
    }

    #[test]
    fn test_key_folding() {
        let v: Value = serde_json::from_str(r#"{"data":{"metadata":{"name":"test"}}}"#).unwrap();
        let opts = ToonOptions {
            key_folding: KeyFolding::Safe,
            ..default_opts()
        };
        let out = encode_toon(&v, &opts);
        assert_eq!(out, "data.metadata.name: test");
    }

    #[test]
    fn test_quoting() {
        let v: Value = serde_json::from_str(r#"{"x":"true","y":"","z":"a,b"}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "x: \"true\"\ny: \"\"\nz: \"a,b\"");
    }

    #[test]
    fn test_root_array() {
        let v: Value = serde_json::from_str("[1,2,3]").unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "[3]: 1,2,3");
    }

    #[test]
    fn test_pipe_delimiter() {
        let v: Value = serde_json::from_str(r#"{"items":["a","b"]}"#).unwrap();
        let opts = ToonOptions {
            delimiter: Delimiter::Pipe,
            ..default_opts()
        };
        let out = encode_toon(&v, &opts);
        assert_eq!(out, "items[2|]: a|b");
    }

    #[test]
    fn test_empty_object() {
        let v: Value = serde_json::from_str(r#"{"x":{}}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "x:");
    }

    #[test]
    fn test_empty_array() {
        let v: Value = serde_json::from_str(r#"{"x":[]}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "x[0]:");
    }

    #[test]
    fn test_number_formatting() {
        let v: Value = serde_json::from_str(r#"{"a":1.0,"b":-0,"c":3.14}"#).unwrap();
        let out = encode_toon(&v, &default_opts());
        assert_eq!(out, "a: 1\nb: 0\nc: 3.14");
    }

    #[test]
    fn test_flatten_depth() {
        let v: Value = serde_json::from_str(r#"{"a":{"b":{"c":{"d":"val"}}}}"#).unwrap();
        let opts = ToonOptions {
            key_folding: KeyFolding::Safe,
            flatten_depth: 1,
            ..default_opts()
        };
        let out = encode_toon(&v, &opts);
        assert_eq!(out, "a.b:\n  c.d: val");
    }
}
