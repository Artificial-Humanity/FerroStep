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
    let (migration, hooks) =
        ferrostep_pocketbase::install_files(std::path::Path::new(&dir)).expect("writable dir");
    println!("wrote {}", migration.display());
    println!("wrote {}", hooks.display());
}
