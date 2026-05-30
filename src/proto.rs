use crate::config::AzureCliPluginConfig;
use crate::platforms;
use extism_pdk::*;
use proto_pdk::*;

static NAME: &str = "Azure CLI";

#[plugin_fn]
pub fn register_tool(Json(_): Json<RegisterToolInput>) -> FnResult<Json<RegisterToolOutput>> {
    Ok(Json(RegisterToolOutput {
        name: NAME.into(),
        type_of: PluginType::CommandLine,
        plugin_version: Some(env!("CARGO_PKG_VERSION").parse().unwrap()),
        ..RegisterToolOutput::default()
    }))
}

/// Minimum Azure CLI version we will offer.
///
/// Releases older than this predate the current archive layout (and Python
/// 3.10 requirement). Keeping the floor here drops the bulk of historic /
/// submodule tags (`azure-cli-vm-2.0.65`, `rc2.0.53`, etc.).
fn min_supported_version() -> Version {
    Version::new(2, 40, 0)
}

#[plugin_fn]
pub fn load_versions(Json(_): Json<LoadVersionsInput>) -> FnResult<Json<LoadVersionsOutput>> {
    let tags = load_git_tags("https://github.com/Azure/azure-cli")?;
    let floor = min_supported_version();

    let versions = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix("azure-cli-"))
        .filter_map(|s| Version::parse(s).ok())
        .filter(|v| v.pre.is_empty() && *v >= floor)
        .map(|v| v.to_string())
        .collect::<Vec<_>>();

    Ok(Json(LoadVersionsOutput::from(versions)?))
}

#[plugin_fn]
pub fn native_install(
    Json(input): Json<NativeInstallInput>,
) -> FnResult<Json<NativeInstallOutput>> {
    let env = get_host_environment()?;

    // macOS and Windows are served via `download_prebuilt`. Signal proto to
    // skip the native path and fall through to the prebuilt archive logic.
    // (`installed: false` without `skip_install: true` would be treated as a
    // failed native install.)
    if env.os != HostOS::Linux {
        return Ok(Json(NativeInstallOutput {
            installed: false,
            skip_install: true,
            ..NativeInstallOutput::default()
        }));
    }

    platforms::install_via_pip(input)
}

#[plugin_fn]
pub fn download_prebuilt(
    Json(input): Json<DownloadPrebuiltInput>,
) -> FnResult<Json<DownloadPrebuiltOutput>> {
    let env = get_host_environment()?;

    check_supported_os_and_arch(
        NAME,
        &env,
        permutations![
            HostOS::MacOS => [HostArch::Arm64, HostArch::X64],
            HostOS::Windows => [HostArch::X64],
        ],
    )?;

    let version = input.context.version.to_string();
    let config = get_tool_config::<AzureCliPluginConfig>()?;

    let (download_url, download_name) =
        platforms::macos_or_windows_download_url(&env, &version, &config.dist_url)?;

    Ok(Json(DownloadPrebuiltOutput {
        download_url,
        download_name: Some(download_name),
        // Verified by inspecting 2.86.0 macOS arm64 tarball (`tar -tzf`) and
        // Windows x64 ZIP (`unzip -l`): both extract at the archive root with
        // no leading version-named folder.
        archive_prefix: None,
        ..DownloadPrebuiltOutput::default()
    }))
}

/// Tool identifier used in `.tool-versions` files. Matches the proto plugin
/// ID and the asdf convention (`asdf-azure-cli` uses the same name).
const TOOL_VERSIONS_NAME: &str = "azure-cli";

#[plugin_fn]
pub fn detect_version_files(
    Json(_): Json<DetectVersionInput>,
) -> FnResult<Json<DetectVersionOutput>> {
    Ok(Json(DetectVersionOutput {
        // asdf-style .tool-versions file. proto walks upward from the cwd and
        // reads any it finds; `parse_version_file` extracts the azure-cli line.
        files: vec![".tool-versions".into()],
        ..DetectVersionOutput::default()
    }))
}

#[plugin_fn]
pub fn parse_version_file(
    Json(input): Json<ParseVersionFileInput>,
) -> FnResult<Json<ParseVersionFileOutput>> {
    // `.tool-versions` lines look like `azure-cli 2.86.0` (one tool per line,
    // whitespace-separated). Comments start with `#`. The version may also be
    // an alias like `latest`. We hand whatever appears verbatim to proto via
    // `UnresolvedVersionSpec::parse`, which understands semver and aliases.
    let version = input.content.lines().find_map(|line| {
        let trimmed = line.split('#').next()?.trim();
        let mut tokens = trimmed.split_whitespace();
        let name = tokens.next()?;
        if name != TOOL_VERSIONS_NAME {
            return None;
        }
        let raw = tokens.next()?;
        UnresolvedVersionSpec::parse(raw).ok()
    });

    Ok(Json(ParseVersionFileOutput { version }))
}

#[plugin_fn]
pub fn locate_executables(
    Json(_): Json<LocateExecutablesInput>,
) -> FnResult<Json<LocateExecutablesOutput>> {
    let env = get_host_environment()?;

    // Where `az` lives inside the install dir depends on which install
    // strategy ran. macOS/Windows use the prebuilt archive (entry at
    // `bin/az[.cmd]`); Linux uses the pip-venv path (`venv/bin/az`).
    let exe_path = match env.os {
        HostOS::Windows => "bin/az.cmd",
        HostOS::Linux => "venv/bin/az",
        _ => "bin/az",
    };

    // Use iter::once().collect() so the resulting collection's type is
    // inferred from the field type — works whether `exes` is std HashMap or
    // FxHashMap (proto_pdk_api uses the latter from rustc_hash).
    let exes =
        std::iter::once(("az".to_string(), ExecutableConfig::new_primary(exe_path))).collect();

    Ok(Json(LocateExecutablesOutput {
        exes,
        ..LocateExecutablesOutput::default()
    }))
}
