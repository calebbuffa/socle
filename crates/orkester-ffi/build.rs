use std::env;
use std::path::Path;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&crate_dir).parent().unwrap().parent().unwrap();
    let include_dir = workspace_root.join("include");
    std::fs::create_dir_all(&include_dir).unwrap();

    cbindgen::generate(&crate_dir)
        .expect("Unable to generate orkester-ffi bindings")
        .write_to_file(include_dir.join("orkester.h"));

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
