// The GNOME Shell extensions a layout needs: finding out whether they are
// installed, and installing the missing ones, as the 2.x program did:
// 1. An extension that has a Debian package gets that package, installed as
//    administrator with the others in one go.
// 2. One that has none, but ships with this program (forge, openbar), is copied
//    from the folder of bundled extensions.
// 3. Otherwise it is downloaded from extensions.gnome.org, best effort: there
//    may be no build for this version of the shell.
// 4. What is still missing is left to the user, in the extensions manager.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::environment::{find_tool, shell_version};
use super::privileged::install_packages;
use super::report::Report;

/// The extension and the Debian package that holds it, for those that have one.
pub const APT_PACKAGES: &[(&str, &str)] = &[
    ("dash-to-panel@jderose9.github.com", "gnome-shell-extension-dash-to-panel"),
    ("appindicatorsupport@rgcjonas.gmail.com", "gnome-shell-extension-appindicator"),
    ("arcmenu@arcmenu.com", "gnome-shell-extension-arc-menu"),
    ("dash-to-dock@micxgx.gmail.com", "gnome-shell-extension-dashtodock"),
    ("ding@rastersoft.com", "gnome-shell-extension-desktop-icons-ng"),
];

// Where the extensions that ship with the program are installed by the package.
const BUNDLED_DIR: &str = "/usr/share/soplos-layout-switcher/extensions";

/// An extension id is a name and a domain: letters, digits and . _ @ - only. It
/// becomes a folder and an argument, so nothing else is accepted.
pub fn valid_uuid(uuid: &str) -> bool {
    !uuid.is_empty()
        && uuid.len() <= 100
        && !uuid.starts_with(['.', '-'])
        && uuid.contains('@')
        && uuid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-'))
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn user_extension_dir(uuid: &str) -> PathBuf {
    home().join(".local/share/gnome-shell/extensions").join(uuid)
}

/// A folder with the id of the extension is not proof enough: a failed
/// install can leave an empty or broken one. Every real extension has a
/// metadata.json.
pub fn installed(uuid: &str) -> bool {
    user_extension_dir(uuid).join("metadata.json").is_file() || Path::new("/usr/share/gnome-shell/extensions").join(uuid).join("metadata.json").is_file()
}

fn package_installed(package: &str) -> bool {
    Command::new("dpkg").args(["-s", package]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn apt_package(uuid: &str) -> Option<&'static str> {
    APT_PACKAGES.iter().find(|(id, _)| *id == uuid).map(|(_, package)| *package)
}

/// The folder of bundled extensions: the one the package installs, or the one
/// in the source tree when the program runs from it.
fn bundled_dir() -> Option<PathBuf> {
    let installed = PathBuf::from(BUNDLED_DIR);
    if installed.is_dir() {
        return Some(installed);
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/extensions");
    source.is_dir().then_some(source)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&source)?;
        if meta.is_dir() {
            copy_tree(&source, &target)?;
        } else if meta.is_file() {
            std::fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

fn install_bundled(uuid: &str) -> bool {
    let Some(base) = bundled_dir() else { return false };
    let source = base.join(uuid);
    if !source.join("metadata.json").is_file() {
        return false;
    }
    let target = user_extension_dir(uuid);
    let _ = std::fs::remove_dir_all(&target);
    copy_tree(&source, &target).is_ok()
}

fn gnome_org_info_url(uuid: &str, shell_version: &str) -> String {
    let mut url = format!("https://extensions.gnome.org/extension-info/?uuid={uuid}");
    if !shell_version.is_empty() {
        url.push_str(&format!("&shell_version={shell_version}"));
    }
    url
}

/// The `download_url` field of the answer of extensions.gnome.org.
fn download_path(info: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(info).ok()?;
    let path = value.get("download_url")?.as_str()?;
    (path.starts_with('/') && !path.contains("..") && !path.contains(char::is_whitespace)).then(|| path.to_string())
}

fn download_from_gnome_org(uuid: &str, report: &dyn Report) -> bool {
    let (Some(curl), Some(gnome_extensions)) = (find_tool("curl"), find_tool("gnome-extensions")) else {
        return false;
    };
    let Ok(info) = Command::new(&curl).args(["-fsSL", "--max-time", "15", &gnome_org_info_url(uuid, &shell_version())]).stdin(Stdio::null()).stderr(Stdio::null()).output() else {
        return false;
    };
    if !info.status.success() {
        return false;
    }
    let Some(path) = download_path(&String::from_utf8_lossy(&info.stdout)) else {
        report.step(&format!("noBuild {uuid}"));
        return false;
    };
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let zip = dir.join(format!("soplos-layout-switcher-{}-{uuid}.zip", std::process::id()));
    let downloaded = Command::new(&curl)
        .args(["-fsSL", "--max-time", "60", "-o"])
        .arg(&zip)
        .arg(format!("https://extensions.gnome.org{path}"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let ok = downloaded && Command::new(gnome_extensions).args(["install", "--force"]).arg(&zip).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    let _ = std::fs::remove_file(&zip);
    ok
}

/// Makes sure every extension is installed, installing the missing ones. The
/// ones that could not be installed are opened in the extensions manager for
/// the user. Returns those.
pub fn ensure(required: &[&str], report: &dyn Report) -> Vec<String> {
    let missing: Vec<&str> = required.iter().copied().filter(|uuid| valid_uuid(uuid) && !installed(uuid)).collect();
    if missing.is_empty() {
        return Vec::new();
    }

    // The packages first, all together, so the password is asked once.
    let packages: Vec<String> = missing.iter().filter_map(|uuid| apt_package(uuid)).filter(|package| !package_installed(package)).map(str::to_string).collect();
    if !packages.is_empty() {
        report.step(&format!("installingPackages {}", packages.join(" ")));
        if !install_packages(packages, report) {
            report.step("packagesFailed");
        }
    }

    let mut manual = Vec::new();
    for uuid in missing {
        if installed(uuid) {
            continue;
        }
        // A package that was already there counts, as it did before.
        if apt_package(uuid).map(package_installed).unwrap_or(false) {
            continue;
        }
        if install_bundled(uuid) {
            report.step(&format!("installedBundled {uuid}"));
            continue;
        }
        if download_from_gnome_org(uuid, report) {
            report.step(&format!("downloaded {uuid}"));
            continue;
        }
        manual.push(uuid.to_string());
    }

    if !manual.is_empty() {
        report.step(&format!("manual {}", manual.join(" ")));
        if let Some(app) = find_tool("gnome-extensions-app") {
            let _ = Command::new(app).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        }
    }
    manual
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_id_is_a_name_and_a_domain() {
        for uuid in ["forge@jmmaranan.com", "openbar@neuromorph", "dash-to-dock@micxgx.gmail.com", "quick-settings-tweaks@qwreey"] {
            assert!(valid_uuid(uuid), "{uuid}");
        }
        for uuid in ["", "noat", "../x@y", "a@b/c", "a@b c", "a@b;rm", "-x@y", &format!("{}@x", "a".repeat(101))] {
            assert!(!valid_uuid(uuid), "{uuid}");
        }
    }

    #[test]
    fn every_packaged_extension_has_a_valid_id_and_a_package_name() {
        for (uuid, package) in APT_PACKAGES {
            assert!(valid_uuid(uuid), "{uuid}");
            assert!(package.starts_with("gnome-shell-extension-"), "{package}");
            assert_eq!(apt_package(uuid), Some(*package));
        }
        assert_eq!(apt_package("forge@jmmaranan.com"), None);
    }

    #[test]
    fn the_download_of_extensions_gnome_org_is_read_from_its_answer() {
        assert_eq!(download_path(r#"{"uuid":"x@y","download_url":"/download-extension/x@y.shell-extension.zip?version_tag=1"}"#), Some("/download-extension/x@y.shell-extension.zip?version_tag=1".to_string()));
        assert_eq!(download_path(r#"{"uuid":"x@y"}"#), None);
        assert_eq!(download_path(r#"{"download_url":"https://evil.example/x.zip"}"#), None);
        assert_eq!(download_path(r#"{"download_url":"/../etc/x"}"#), None);
        assert_eq!(download_path("not json"), None);
    }

    #[test]
    fn the_query_names_the_version_of_the_shell_when_it_is_known() {
        assert_eq!(gnome_org_info_url("a@b", "46"), "https://extensions.gnome.org/extension-info/?uuid=a@b&shell_version=46");
        assert_eq!(gnome_org_info_url("a@b", ""), "https://extensions.gnome.org/extension-info/?uuid=a@b");
    }

    #[test]
    fn the_extensions_that_ship_with_the_program_are_in_its_source_tree() {
        for uuid in ["forge@jmmaranan.com", "openbar@neuromorph"] {
            assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/extensions").join(uuid).join("metadata.json").is_file(), "{uuid}");
        }
    }
}
