use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct AzureCliPluginConfig {
    /// Base URL where the GitHub release archives are hosted.
    /// The plugin appends `/azure-cli-<version>/<asset_name>` to this value.
    /// Override for air-gapped corporate mirrors.
    pub dist_url: String,
}

impl Default for AzureCliPluginConfig {
    fn default() -> Self {
        Self {
            dist_url: "https://github.com/Azure/azure-cli/releases/download".into(),
        }
    }
}
