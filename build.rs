use vergen_git2::{BuildBuilder, CargoBuilder, Emitter, Git2Builder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    Emitter::default()
        .add_instructions(&BuildBuilder::all_build()?)?
        .add_instructions(&CargoBuilder::all_cargo()?)?
        .add_instructions(&Git2Builder::all_git()?)?
        .emit()?;
    Ok(())
}
