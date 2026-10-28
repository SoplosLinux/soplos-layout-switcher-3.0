// Privileged operations. The only thing here that needs administrator rights is
// installing the packages of the GNOME Shell extensions a layout needs. The app
// never hands a shell command to root: it writes a named operation as JSON to a
// private file and re-invokes this same binary under pkexec as
// `--run-ops <file>`. The root side checks the operation again, against a fixed
// list of packages, and runs it with a fixed command line, never through a
// shell. Every package a layout needs travels in one call, so the password is
// asked once.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use super::extensions::APT_PACKAGES;
use super::report::Report;

const MAX_OPS_FILE_BYTES: u64 = 100_000;

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Op {
    /// Installs packages of the list `extensions::APT_PACKAGES`.
    InstallPackages { packages: Vec<String> },
}

/// Whether a package is one of those the program installs.
pub fn allowed_package(name: &str) -> bool {
    APT_PACKAGES.iter().any(|(_, package)| *package == name)
}

/// Check of a request, on the user's side and again on root's.
pub fn validate(op: &Op) -> Result<(), String> {
    match op {
        Op::InstallPackages { packages } => {
            if packages.is_empty() || packages.len() > APT_PACKAGES.len() {
                return Err("Nothing to install".to_string());
            }
            match packages.iter().find(|name| !allowed_package(name)) {
                Some(name) => Err(format!("Not a package of this program: {name}")),
                None => Ok(()),
            }
        }
    }
}

fn execute(op: &Op) -> bool {
    match op {
        Op::InstallPackages { packages } => {
            for package in packages {
                println!("@@step installing {package}");
            }
            let _ = std::io::stdout().flush();
            Command::new("/usr/bin/apt-get")
                .args(["install", "-y"])
                .args(packages)
                .env("DEBIAN_FRONTEND", "noninteractive")
                .stdin(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
    }
}

/// Root side, called from main() before Tauri starts when pkexec re-invokes
/// this binary as `--run-ops <file>`. Returns the exit code: 0 only when every
/// operation worked.
pub fn run_ops_file(path: &str) -> i32 {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) => {
            eprintln!("soplos-layout-switcher: cannot read the operations file: {e}");
            return 1;
        }
    };
    // Only a regular file written by the user who authenticated is honoured.
    if !meta.file_type().is_file() || meta.len() > MAX_OPS_FILE_BYTES {
        eprintln!("soplos-layout-switcher: refusing the operations file");
        return 1;
    }
    if let Ok(uid) = std::env::var("PKEXEC_UID") {
        if meta.uid().to_string() != uid {
            eprintln!("soplos-layout-switcher: the operations file does not belong to the requesting user");
            return 1;
        }
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("soplos-layout-switcher: cannot read the operations file: {e}");
            return 1;
        }
    };
    let _ = std::fs::remove_file(path);

    let ops: Vec<Op> = match serde_json::from_str(&text) {
        Ok(ops) => ops,
        Err(e) => {
            eprintln!("soplos-layout-switcher: invalid operations: {e}");
            return 1;
        }
    };
    for (index, op) in ops.iter().enumerate() {
        if let Err(e) = validate(op) {
            eprintln!("soplos-layout-switcher: refused operation {index}: {e}");
            return 1;
        }
        // A failed step stops the batch: the later ones were meant to follow it.
        if !execute(op) {
            return 1;
        }
    }
    0
}

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(1);

// The request file lives in the user's runtime directory when there is one
// (private to the user), and is created with create_new and mode 0600 so
// nobody can pre-create or read it.
fn write_ops_file(json: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let id = NEXT_FILE_ID.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("soplos-layout-switcher-ops-{}-{id}-{nanos}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
    file.write_all(json.as_bytes())?;
    Ok(path)
}

/// Installs the packages as administrator, in one go (one password), and tells
/// `report` what happened. True when they were installed.
pub fn install_packages(packages: Vec<String>, report: &dyn Report) -> bool {
    let ops = vec![Op::InstallPackages { packages }];
    if let Some(error) = ops.iter().find_map(|op| validate(op).err()) {
        report.line(&error);
        return false;
    }
    let json = match serde_json::to_string(&ops) {
        Ok(json) => json,
        Err(e) => {
            report.line(&format!("Could not encode the request: {e}"));
            return false;
        }
    };
    let file = match write_ops_file(&json) {
        Ok(file) => file,
        Err(e) => {
            report.line(&format!("Could not write the request: {e}"));
            return false;
        }
    };
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "soplos-layout-switcher".to_string());
    let output = Command::new("pkexec").arg(exe).arg("--run-ops").arg(&file).stdin(Stdio::null()).output();
    let _ = std::fs::remove_file(&file);
    match output {
        Ok(out) => {
            for line in String::from_utf8_lossy(&out.stdout).lines().chain(String::from_utf8_lossy(&out.stderr).lines()) {
                if line.starts_with("@@step ") {
                    report.step(&line["@@step ".len()..]);
                }
            }
            out.status.success()
        }
        Err(e) => {
            report.line(&format!("Could not run pkexec: {e}"));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_packages_of_the_program_can_be_installed() {
        assert!(validate(&Op::InstallPackages { packages: vec!["gnome-shell-extension-dashtodock".into()] }).is_ok());
        assert!(validate(&Op::InstallPackages { packages: vec!["gnome-shell-extension-dashtodock".into(), "gnome-shell-extension-arc-menu".into()] }).is_ok());
        assert!(validate(&Op::InstallPackages { packages: vec![] }).is_err());
        assert!(validate(&Op::InstallPackages { packages: vec!["openssh-server".into()] }).is_err());
        assert!(validate(&Op::InstallPackages { packages: vec!["gnome-shell-extension-dashtodock; reboot".into()] }).is_err());
        assert!(validate(&Op::InstallPackages { packages: vec!["-o".into(), "APT::Foo=1".into()] }).is_err());
    }

    #[test]
    fn the_request_travels_as_json_and_comes_back() {
        let ops = vec![Op::InstallPackages { packages: vec!["gnome-shell-extension-appindicator".into()] }];
        let json = serde_json::to_string(&ops).unwrap();
        assert!(json.contains("\"op\":\"install-packages\""));
        let back: Vec<Op> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 1);
    }
}
