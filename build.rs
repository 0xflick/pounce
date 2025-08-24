use std::env;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/index");

    // Set build date (using simple format)
    set_build_date();

    // Set cargo information
    set_cargo_info();

    // Set git information
    set_git_info();
}

fn set_build_date() {
    let build_date = get_current_date();
    println!("cargo::rustc-env=BUILD_DATE={build_date}");
}

fn set_cargo_info() {
    // Get optimization level from environment
    let opt_level = env::var("OPT_LEVEL").unwrap_or_else(|_| "0".to_string());
    println!("cargo::rustc-env=BUILD_OPT_LEVEL={opt_level}");
}

fn set_git_info() {
    // Check if git is available first
    if !is_git_available() {
        println!("cargo::rustc-env=BUILD_GIT_DESCRIBE=unknown");
        println!("cargo::rustc-env=BUILD_GIT_SHA=unknown");
        println!("cargo::rustc-env=BUILD_GIT_BRANCH=unknown");
        return;
    }

    // Get git describe
    if let Some(git_describe) = run_git_command(&["describe", "--always", "--dirty", "--tags"]) {
        println!("cargo::rustc-env=BUILD_GIT_DESCRIBE={git_describe}");
    } else {
        println!("cargo::rustc-env=BUILD_GIT_DESCRIBE=unknown");
    }

    // Get commit hash
    if let Some(commit_hash) = run_git_command(&["rev-parse", "HEAD"]) {
        println!("cargo::rustc-env=BUILD_GIT_SHA={commit_hash}");
    } else {
        println!("cargo::rustc-env=BUILD_GIT_SHA=unknown");
    }

    // Get branch name
    if let Some(branch) = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]) {
        println!("cargo::rustc-env=BUILD_GIT_BRANCH={branch}");
    } else {
        println!("cargo::rustc-env=BUILD_GIT_BRANCH=unknown");
    }
}

fn get_current_date() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let days_since_epoch = duration.as_secs() / 86400; // 86400 seconds in a day
            let (year, month, day) = days_to_date(days_since_epoch);
            format!("{year:04}-{month:02}-{day:02}")
        }
        Err(_) => "unknown".to_string(),
    }
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Unix epoch starts at 1970-01-01
    let mut year = 1970;
    let mut remaining_days = days;

    // Handle years
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }

    // Handle months
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &days_in_month in &days_in_months {
        if remaining_days >= days_in_month as u64 {
            remaining_days -= days_in_month as u64;
            month += 1;
        } else {
            break;
        }
    }

    let day = remaining_days + 1; // Days are 1-indexed

    (year, month, day)
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || (year.is_multiple_of(400))
}

fn run_git_command(args: &[&str]) -> Option<String> {
    run_command("git", args)
}

fn is_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).ok()?;
        Some(stdout.trim().to_string())
    } else {
        None
    }
}
