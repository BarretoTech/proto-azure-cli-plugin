//! Per-OS install logic for the Azure CLI plugin.
//!
//! macOS and Windows are served by `download_prebuilt` — Microsoft publishes
//! self-contained archives on GitHub that bundle their own Python runtime.
//!
//! Linux has no such artifact, so we fall back to `native_install`: create a
//! per-version virtualenv with the host's `python3`, then `pip install
//! azure-cli==<version>` into it.

use crate::config::AzureCliPluginConfig;
use extism_pdk::*;
use proto_pdk::*;

/// Builds the GitHub release asset filename for the prebuilt archive path.
/// Returns an error for unsupported OS/arch combinations.
pub fn macos_or_windows_asset_name(env: &HostEnvironment, version: &str) -> FnResult<String> {
    let name = match (&env.os, &env.arch) {
        (HostOS::MacOS, HostArch::Arm64) => {
            format!("azure-cli-{version}-macos-arm64.tar.gz")
        }
        (HostOS::MacOS, HostArch::X64) => {
            format!("azure-cli-{version}-macos-x86_64.tar.gz")
        }
        (HostOS::Windows, HostArch::X64) => {
            format!("azure-cli-{version}-x64.zip")
        }
        _ => {
            return Err(plugin_err!(
                "Azure CLI: no prebuilt archive for {os} {arch}. Linux is handled via `native_install`; everything else is unsupported.",
                os = env.os,
                arch = env.arch,
            ));
        }
    };

    Ok(name)
}

/// Assembles the full download URL: `{dist_url}/azure-cli-{version}/{asset_name}`.
pub fn macos_or_windows_download_url(
    env: &HostEnvironment,
    version: &str,
    dist_url: &str,
) -> FnResult<(String, String)> {
    let asset_name = macos_or_windows_asset_name(env, version)?;
    let url = format!(
        "{base}/azure-cli-{version}/{asset_name}",
        base = dist_url.trim_end_matches('/'),
    );
    Ok((url, asset_name))
}

/// Minimum Python (major, minor) required by recent Azure CLI releases.
const MIN_PYTHON: (u32, u32) = (3, 10);

/// Linux install path: create a venv with system Python, then pip-install
/// `azure-cli==<version>` into it. Returns `installed: true` on success.
///
/// Surfaces user-facing errors via `plugin_err!` (aborts the install with a
/// clear message) rather than `installed: false` (which would cause Proto to
/// fall through to `download_prebuilt`, which we know cannot succeed on Linux).
pub fn install_via_pip(input: NativeInstallInput) -> FnResult<Json<NativeInstallOutput>> {
    let env = get_host_environment()?;
    let _ = get_tool_config::<AzureCliPluginConfig>()?; // reserved for future pip mirror config

    // 1. python3 must be on PATH.
    if !command_exists(&env, "python3") {
        return Err(plugin_err!(
            "Azure CLI: `python3` was not found on PATH. Install Python {major}.{minor}+ (e.g. `sudo apt install python3.10 python3.10-venv` on Debian/Ubuntu, `sudo dnf install python3.10` on Fedora) and try again.",
            major = MIN_PYTHON.0,
            minor = MIN_PYTHON.1,
        ));
    }

    // 2. Verify version >= MIN_PYTHON.
    let py_version = exec_captured(
        "python3",
        [
            "-c",
            "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ],
    )?;
    if py_version.exit_code != 0 {
        return Err(plugin_err!(
            "Azure CLI: failed to query Python version: {}",
            py_version.stderr.trim()
        ));
    }
    let raw = py_version.stdout.trim();
    let (major, minor) = parse_python_version(raw).ok_or_else(|| {
        plugin_err!(
            "Azure CLI: could not parse Python version `{}` (expected MAJOR.MINOR)",
            raw
        )
    })?;
    if (major, minor) < MIN_PYTHON {
        return Err(plugin_err!(
            "Azure CLI requires Python {req_major}.{req_minor}+ but `python3` reports {got_major}.{got_minor}. Install a newer Python on PATH and try again.",
            req_major = MIN_PYTHON.0,
            req_minor = MIN_PYTHON.1,
            got_major = major,
            got_minor = minor,
        ));
    }

    // (Progress feedback is provided by the streamed venv / pip output below.)
    let _ = (major, minor);

    // 3. Resolve the venv directory inside the install dir.
    let venv_vpath = input.install_dir.join("venv");
    let venv_dir = venv_vpath
        .real_path()
        .ok_or_else(|| plugin_err!("Azure CLI: install directory has no host-side real path"))?;
    let venv_dir_str = venv_dir.to_string_lossy().to_string();

    // 4. Create the venv.
    let venv_result = exec_streamed("python3", ["-m", "venv", venv_dir_str.as_str()])?;
    if venv_result.exit_code != 0 {
        return Err(plugin_err!(
            "Azure CLI: `python3 -m venv` failed (exit {}). The most common cause is a missing venv module — on Debian/Ubuntu install the `python3-venv` package (e.g. `sudo apt install python3-venv` or the version-suffixed `python3.10-venv`). stderr: {}",
            venv_result.exit_code,
            venv_result.stderr.trim()
        ));
    }

    // 5. pip install azure-cli==<version> into the venv.
    let version = input.context.version.to_string();
    let spec = format!("azure-cli=={version}");
    let venv_python = venv_dir.join("bin").join("python");
    let venv_python_str = venv_python.to_string_lossy().to_string();

    let pip_result = exec_streamed(
        venv_python_str.as_str(),
        [
            "-m",
            "pip",
            "install",
            "--no-cache-dir",
            "--disable-pip-version-check",
            spec.as_str(),
        ],
    )?;
    if pip_result.exit_code != 0 {
        return Err(plugin_err!(
            "Azure CLI: `pip install {spec}` failed (exit {}). stderr: {}",
            pip_result.exit_code,
            pip_result.stderr.trim()
        ));
    }

    Ok(Json(NativeInstallOutput {
        installed: true,
        ..NativeInstallOutput::default()
    }))
}

/// Parses a `MAJOR.MINOR` string into `(u32, u32)`.
fn parse_python_version(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}
