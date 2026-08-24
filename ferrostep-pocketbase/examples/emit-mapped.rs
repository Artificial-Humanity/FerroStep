//! Emit the generated files for a **mapped** deployment — one that referees
//! an existing collection rather than the generic ferrostep ones:
//!
//! ```sh
//! cargo run -p ferrostep-pocketbase --example emit-mapped -- \
//!     <config.json> <hooks-out.pb.js> [migration-out.js]
//! ```
//!
//! The config is deployment configuration, kept beside the workflow
//! definition it serves:
//!
//! ```json
//! {
//!   "map":     { …CollectionMap… },
//!   "release": { …ReleaseHook…, optional }
//! }
//! ```
//!
//! The hooks file lands in the server's `pb_hooks/` (⚠ a watching server
//! restarts itself when it does). The optional third argument writes the
//! guarded migration that adds the version column and creates the event
//! collection — placed in `pb_migrations/`, it applies at that same restart.

use serde::Deserialize;

#[derive(Deserialize)]
struct EmitConfig {
    map: ferrostep_pocketbase::CollectionMap,
    #[serde(default)]
    release: Option<ferrostep_pocketbase::ReleaseHook>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: emit-mapped <config.json> <hooks-out.pb.js> [migration-out.js]";
    let config_path = args.next().expect(usage);
    let hooks_out = args.next().expect(usage);
    let migration_out = args.next();

    let source = std::fs::read_to_string(&config_path).expect("readable config");
    let config: EmitConfig = serde_json::from_str(&source).expect("config parses");

    let hooks = ferrostep_pocketbase::hooks_file_mapped(&config.map, config.release.as_ref());
    std::fs::write(&hooks_out, hooks).expect("writable hooks path");
    println!("wrote {hooks_out}");

    if let Some(path) = migration_out {
        let migration = ferrostep_pocketbase::migration_file_mapped(&config.map);
        std::fs::write(&path, migration).expect("writable migration path");
        println!("wrote {path}");
    }
}
