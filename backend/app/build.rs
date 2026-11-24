fn main() {
    // macOS: libpq (PostgreSQL client) lives under Homebrew, not in the
    // default library path. Point the linker at it so cargo build/test
    // don't need LIBRARY_PATH set in the environment.
    println!("cargo:rustc-link-search=/opt/homebrew/opt/libpq/lib");
}
