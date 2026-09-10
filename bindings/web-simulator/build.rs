use std::fs;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let device = std::env::var("CARGO_PKG_NAME").unwrap();

    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let release_id = std::env::var("OSSM_RELEASE_ID").unwrap_or_else(|_| "dev".into());

    let build_time = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let info = format!("{device}\0{commit}\0{release_id}\0{build_time}\0");
    fs::write(format!("{out_dir}/ossm_build_info.bin"), info.as_bytes()).unwrap();
}
