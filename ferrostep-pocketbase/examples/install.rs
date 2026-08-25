//! Write the generated PocketBase files under a working directory:
//!
//! ```sh
//! cargo run -p ferrostep-pocketbase --example install -- /path/to/pocketbase-dir
//! ```
//!
//! The directory is the one the `pocketbase` binary runs from: the migration
//! lands in `pb_migrations/` (applied at the next start) and the routes in
//! `pb_hooks/` (a watching server restarts itself when the file lands).

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: install <pocketbase working directory>");
    // The default binding: a `ferrostep_actors` auth collection, roles read
    // from its `role` field. Nothing to configure for a first deployment —
    // and a deployment with its own auth collection points this at that one
    // instead of creating a second place identities live.
    let actors = ferrostep_pocketbase::ActorBinding::default();
    let (migration, hooks) = ferrostep_pocketbase::install_files(std::path::Path::new(&dir), &actors)
        .expect("writable dir");
    println!("wrote {}", migration.display());
    println!("wrote {}", hooks.display());
}
