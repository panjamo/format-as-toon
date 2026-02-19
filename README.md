# format-as-toon

A Rust library and CLI tool to convert JSON to [TOON](https://github.com/toon-format/spec) (Token-Oriented Object Notation) — a compact, human-readable format that reduces token usage by 30–60% compared to JSON.

## Installation

### CLI

```bash
cargo install format-as-toon
```

### Library

```toml
[dependencies]
format-as-toon = "0.1"
serde_json = "1"
```

## CLI Usage

```bash
# From stdin
echo '{"name":"Alice","age":30}' | format-as-toon

# From file
format-as-toon data.json

# With options
format-as-toon -d pipe --key-folding safe -s 4 data.json
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--delimiter` | `-d` | Delimiter for array values: `comma`, `tab`, `pipe` | `comma` |
| `--spaces` | `-s` | Spaces per indentation level | `2` |
| `--key-folding` | `-k` | Key folding mode: `off`, `safe` | `off` |
| `--flatten-depth` | `-f` | Max depth for key folding | unlimited |

## Library Usage

```rust
use format_as_toon::{ToonOptions, KeyFolding, encode_toon};
use serde_json::json;

// Default options
let value = json!({"name": "Alice", "age": 30});
let output = encode_toon(&value, &ToonOptions::default());
assert_eq!(output, "name: Alice\nage: 30");

// With key folding
let value = json!({"data": {"metadata": {"name": "test"}}});
let opts = ToonOptions {
    key_folding: KeyFolding::Safe,
    ..ToonOptions::default()
};
let output = encode_toon(&value, &opts);
assert_eq!(output, "data.metadata.name: test");
```

## Examples

### Simple object

```
$ echo '{"name":"Alice","age":30}' | format-as-toon
name: Alice
age: 30
```

### Array of objects (tabular)

```
$ echo '{"users":[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]}' | format-as-toon
users[2]{id,name}:
  1,Alice
  2,Bob
```

### Key folding

```
$ echo '{"data":{"metadata":{"name":"test"}}}' | format-as-toon --key-folding safe
data.metadata.name: test
```

### Pipe delimiter

```
$ echo '{"items":["x","y","z"]}' | format-as-toon -d pipe
items[3|]: x|y|z
```

### Root array

```
$ echo '[1,2,3]' | format-as-toon
[3]: 1,2,3
```

## TOON Format Summary

TOON encodes the JSON data model with minimal syntax:

- **Objects** use indented key-value pairs (`key: value`)
- **Primitive arrays** are inline with length: `tags[3]: a,b,c`
- **Uniform object arrays** use tabular form: `users[2]{id,name}:` followed by rows
- **Strings** are unquoted unless they contain special characters or resemble reserved words/numbers
- **Numbers** use canonical decimal form (no trailing zeros)

Full spec: https://github.com/toon-format/spec

## License

MIT
