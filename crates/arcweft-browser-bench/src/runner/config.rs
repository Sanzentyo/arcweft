use crate::model::BrowserMathBenchConfig;

pub(crate) fn parse_config(config_json: &str) -> Result<BrowserMathBenchConfig, String> {
    if config_json.trim().is_empty() {
        Ok(BrowserMathBenchConfig::default())
    } else {
        serde_json::from_str(config_json).map_err(|error| error.to_string())
    }
}
