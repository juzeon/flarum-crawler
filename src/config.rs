use serde::{Deserialize, Serialize};
use strip_prefix_suffix_sane::StripPrefixSuffixSane;
use tokio::fs::read_to_string;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub base_url: String,
    pub concurrency: usize,
    pub db: String,
}
impl Config {
    pub async fn load(path: &str) -> anyhow::Result<Self> {
        let c = read_to_string(path).await?;
        let mut config: Config = serde_yaml::from_str(c.as_str())?;
        config.base_url = config.base_url.strip_suffix_sane("/").to_string();
        Ok(config)
    }
}
