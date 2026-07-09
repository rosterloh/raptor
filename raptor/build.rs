fn main() {
    // rust-embed's derive requires the folder to exist at compile time, even
    // in debug builds where the contents are read from disk at runtime. On a
    // fresh checkout `dx build` hasn't run yet, so create it empty.
    if std::env::var_os("CARGO_FEATURE_EMBED_UI").is_some() {
        let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../target/dx/raptor-ui/release/web/public");
        std::fs::create_dir_all(dir).unwrap();
    }
}
