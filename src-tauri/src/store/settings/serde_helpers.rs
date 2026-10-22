use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

/// Falls back to the default for a value the field doesn't know (a typo, a retired variant),
/// so one bad field doesn't quarantine the whole file and lose every other setting.
pub(super) fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let raw = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(raw.clone()).unwrap_or_else(|_| {
        log::warn!("Ignoring unknown setting value {raw}; using the default");
        T::default()
    }))
}
