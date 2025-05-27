const GIT_VERSION: &str = env!("VERGEN_GIT_DESCRIBE");
const CARGO_VERSION: &str = env!("CARGO_PKG_VERSION");
const DIRTY: &str = env!("VERGEN_GIT_DIRTY");
const BUILD_DATE: &str = env!("VERGEN_BUILD_DATE");
const OPT_LEVEL: &str = env!("VERGEN_CARGO_OPT_LEVEL");

pub fn engine_name() -> String {
    let release_type = if OPT_LEVEL == "3" { "release" } else { "dev" };
    let date = BUILD_DATE.replace("-", "");

    #[allow(clippy::const_is_empty)]
    let version = if GIT_VERSION.is_empty() || GIT_VERSION == "VERGEN_IDEMPOTENT_OUTPUT" {
        CARGO_VERSION.to_string()
    } else if DIRTY == "true" {
        format!("{}-dirty", GIT_VERSION)
    } else {
        GIT_VERSION.to_string()
    };

    format!("pounce {}-{}-{}", release_type, date, version)
}
