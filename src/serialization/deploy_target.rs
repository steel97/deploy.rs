use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DeployTarget {
    pub name: Option<String>,
    pub host: String,
    pub port: u16,
    pub authentication: HashMap<String, String>,
    pub packages: Vec<String>,
}
