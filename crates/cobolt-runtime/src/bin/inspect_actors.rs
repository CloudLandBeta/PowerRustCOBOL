// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Developer aid: dump an INDEXED file's record format and hex-dump its first
//! record.
//!
//! Both paths come from the command line. They used to be hardcoded absolute
//! macOS paths into one developer's project folder, which made the tool useless
//! on Linux and Windows — and on any other machine.
//!
//! ```sh
//! cargo run -p cobolt-runtime --bin inspect_actors -- <file.cidx> <file.idx>
//! ```

use cobolt_runtime::GridSession;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(cidx_arg), Some(data_arg)) = (args.first(), args.get(1)) else {
        eprintln!(
            "usage: inspect_actors <definition.cidx> <data.idx>\n\
             \n\
             Prints the record format and storage mode from the definition, then\n\
             hex-dumps the first record of the data file."
        );
        return ExitCode::FAILURE;
    };
    let cidx_path = Path::new(cidx_arg);
    let data_path = Path::new(data_arg);

    let def = match cobolt_indexed::load_indexed(cidx_path) {
        Ok(def) => def,
        Err(e) => {
            eprintln!("cannot parse {}: {e}", cidx_path.display());
            return ExitCode::FAILURE;
        }
    };

    println!("Record format: {:?}", def.record_format);
    println!("Storage: {:?}", def.storage);

    let mut session = match GridSession::open(&def, data_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot open {}: {e}", data_path.display());
            return ExitCode::FAILURE;
        }
    };
    let rows = session.rows();
    println!("Total rows: {}", rows.len());
    if let Some(first) = rows.first() {
        println!("First record len: {}", first.len());
        println!("Raw bytes (hex):");
        for chunk in first.chunks(16) {
            for b in chunk {
                print!("{:02X} ", b);
            }
            print!(" | ");
            for b in chunk {
                let c = *b;
                if c.is_ascii_graphic() || c == b' ' {
                    print!("{}", c as char);
                } else {
                    print!(".");
                }
            }
            println!();
        }
    }
    ExitCode::SUCCESS
}
