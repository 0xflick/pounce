use vergen_gitcl::{BuildBuilder, CargoBuilder, Emitter, GitclBuilder};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    Emitter::default()
        .add_instructions(&BuildBuilder::all_build().unwrap())
        .unwrap()
        .add_instructions(&CargoBuilder::all_cargo().unwrap())
        .unwrap()
        .add_instructions(&GitclBuilder::all_git().unwrap())
        .unwrap()
        .emit()
        .unwrap();
}
