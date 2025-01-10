use std::fmt::{Display, Formatter};

use derive_setters::Setters;
use serde::Serialize;

#[derive(Default, Serialize, Debug, Clone)]
pub struct Cwd(String);

impl Cwd {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for Cwd {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl<T: AsRef<str>> From<T> for Cwd {
    fn from(value: T) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(Default, Serialize, Debug, Setters, Clone)]
#[serde(rename_all = "camelCase")]
#[setters(strip_option)]
/// Represents the environment in which the application is running.
pub struct Environment {
    /// The operating system of the environment.
    pub os: String,
    /// The current working directory.
    pub cwd: Cwd,
    /// The shell being used.
    pub shell: String,
    /// The home directory, if available.
    pub home: Option<String>,
    /// A list of files in the current working directory.
    pub files: Vec<String>,
    /// The Forge API key.
    pub api_key: String,
    /// The large model ID.
    pub large_model_id: String,
    /// The small model ID.
    pub small_model_id: String,
}
