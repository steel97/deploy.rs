use std::{collections::HashMap, error::Error, fs::File, io::BufReader, path::Path};

use super::{deploy_package::DeployPackage, deploy_target::DeployTarget};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(rename = "usesudo")]
    pub use_sudo: Option<bool>,
    pub targets: Vec<DeployTarget>,
    pub packages: HashMap<String, DeployPackage>,
}

impl Config {
    pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let u = serde_json::from_reader(reader)?;
        Ok(u)
    }
}
