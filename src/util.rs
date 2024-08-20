const VERSION: &str = env!("VERGEN_GIT_DESCRIBE");
const BUILD_DATE: &str = env!("VERGEN_BUILD_DATE");
const OPT_LEVEL: &str = env!("VERGEN_CARGO_OPT_LEVEL");

pub fn engine_name() -> String {
    let release_type = if OPT_LEVEL == "3" { "release" } else { "dev" };
    let date = BUILD_DATE.replace("-", "");
    format!("pounce {}-{}-{}", release_type, date, VERSION)
}
