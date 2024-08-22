use vergen_git2::{BuildBuilder, CargoBuilder, Emitter, Git2Builder};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    Emitter::default()
        .add_instructions(&BuildBuilder::all_build().unwrap())
        .unwrap()
        .add_instructions(&CargoBuilder::all_cargo().unwrap())
        .unwrap()
        .add_instructions(&Git2Builder::all_git().unwrap())
        .unwrap()
        .emit()
        .unwrap();
}
