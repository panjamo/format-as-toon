use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use format_as_toon::{Delimiter, KeyFolding, ToonOptions, encode_toon};
use serde_json::Value;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DelimiterArg {
    Comma,
    Tab,
    Pipe,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KeyFoldingArg {
    Off,
    Safe,
}

/// Convert JSON to TOON (Token-Oriented Object Notation).
///
/// Reads JSON from a file or stdin and outputs TOON to stdout.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Input JSON file (reads from stdin if omitted)
    input: Option<PathBuf>,

    /// Delimiter for array values and tabular rows
    #[arg(short, long, value_enum, default_value_t = DelimiterArg::Comma)]
    delimiter: DelimiterArg,

    /// Number of spaces per indentation level
    #[arg(short, long, default_value_t = 2)]
    spaces: usize,

    /// Key folding mode (collapse single-key chains into dotted paths)
    #[arg(short, long, value_enum, default_value_t = KeyFoldingArg::Off)]
    key_folding: KeyFoldingArg,

    /// Maximum depth for key folding (default: unlimited)
    #[arg(short, long)]
    flatten_depth: Option<usize>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let json_input = match &args.input {
        Some(path) => std::fs::read_to_string(path)?,
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    let value: Value = serde_json::from_str(&json_input)?;

    let opts = ToonOptions {
        delimiter: match args.delimiter {
            DelimiterArg::Comma => Delimiter::Comma,
            DelimiterArg::Tab => Delimiter::Tab,
            DelimiterArg::Pipe => Delimiter::Pipe,
        },
        indent: args.spaces,
        key_folding: match args.key_folding {
            KeyFoldingArg::Off => KeyFolding::Off,
            KeyFoldingArg::Safe => KeyFolding::Safe,
        },
        flatten_depth: args.flatten_depth.unwrap_or(usize::MAX),
    };

    let output = encode_toon(&value, &opts);
    print!("{output}");

    Ok(())
}
