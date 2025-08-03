const GIT_VERSION: &str = env!("BUILD_GIT_DESCRIBE");
const BUILD_DATE: &str = env!("BUILD_DATE");
const OPT_LEVEL: &str = env!("BUILD_OPT_LEVEL");

pub fn engine_name() -> String {
    let release_type = if OPT_LEVEL == "3" { "release" } else { "dev" };
    let date = BUILD_DATE.replace("-", "");
    let version = GIT_VERSION.to_string();

    format!("pounce {}-{}-{}", release_type, date, version)
}
