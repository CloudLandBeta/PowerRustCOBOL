use cobolt_indexed::IndexedDefinition;
use cobolt_runtime::GridSession;
use std::path::Path;

fn main() {
    let cidx_path = Path::new("/Users/emersonlopes/Documents/PowerDemo2/indexed/actors.cidx");
    let data_path = Path::new("/Users/emersonlopes/Documents/PowerDemo2/data/actors.idx");

    let def = cobolt_indexed::load_indexed(cidx_path).expect("Failed to parse xml");

    println!("Record format: {:?}", def.record_format);
    println!("Storage: {:?}", def.storage);

    let mut session = GridSession::open(&def, &data_path).expect("Failed to open grid session");
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
}
