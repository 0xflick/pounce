const VERSION: &str = env!("VERGEN_GIT_DESCRIBE");
const DIRTY: &str = env!("VERGEN_GIT_DIRTY");
const BUILD_DATE: &str = env!("VERGEN_BUILD_DATE");
const OPT_LEVEL: &str = env!("VERGEN_CARGO_OPT_LEVEL");

pub fn engine_name() -> String {
    let release_type = if OPT_LEVEL == "3" { "release" } else { "dev" };
    let date = BUILD_DATE.replace("-", "");

    let version = if DIRTY == "true" {
        format!("{}-dirty", VERSION)
    } else {
        VERSION.to_string()
    };

    format!("pounce {}-{}-{}", release_type, date, version)
}
