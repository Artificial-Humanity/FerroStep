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
    /// Who the store recognises as an actor, and where it reads their role.
    ///
    /// ⚠ Defaulted, so a config written before this existed still emits —
    /// and emits the *binding* behaviour rather than the old trust-the-body
    /// behaviour, which is the whole point of it being a default rather than
    /// an opt-in.
    #[serde(default)]
    actors: ferrostep_pocketbase::ActorBinding,
    /// Path to the workflow definition this deployment referees, relative to
    /// this config file.
    ///
    /// ⚠ Optional, and what it buys is the CREATE guard: a new record must
    /// start in the definition's initial state. Without it the generated guard
    /// closes refereed columns to updates only — and since `create` is refused
    /// by the adapter in the mapped shape, direct creation is the one write
    /// left uncovered.
    ///
    /// ⚠ A PATH rather than the state itself, deliberately. The state is a
    /// workflow rule and belongs in the definition; writing it here would be a
    /// second copy of a value that already exists, which is the drift this
    /// project keeps deleting.
    #[serde(default)]
    workflow: Option<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: emit-mapped <config.json> <hooks-out.pb.js> [migration-out.js]";
    let config_path = args.next().expect(usage);
    let hooks_out = args.next().expect(usage);
    let migration_out = args.next();

    let source = std::fs::read_to_string(&config_path).expect("readable config");
    let config: EmitConfig = serde_json::from_str(&source).expect("config parses");

    // ⚠ Resolved against the CONFIG file's directory, not the process's
    // working directory — a generated artifact must not depend on where the
    // command was run from.
    let initial = config.workflow.as_ref().map(|rel| {
        let base = std::path::Path::new(&config_path).parent().unwrap_or(std::path::Path::new("."));
        let path = base.join(rel);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read workflow '{}': {e}", path.display()));
        let def: ferrostep_core::WorkflowDef = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("workflow '{}' does not parse: {e}", path.display()));
        def.initial
    });
    let hooks = ferrostep_pocketbase::hooks_file_mapped(
        &config.map,
        config.release.as_ref(),
        &config.actors,
        initial.as_deref(),
    );
    std::fs::write(&hooks_out, hooks).expect("writable hooks path");
    println!("wrote {hooks_out}");

    if let Some(path) = migration_out {
        let migration = ferrostep_pocketbase::migration_file_mapped(&config.map, &config.actors);
        std::fs::write(&path, migration).expect("writable migration path");
        println!("wrote {path}");
    }
}
