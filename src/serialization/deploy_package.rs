use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DeployPackage {
    #[serde(rename = "localDirectory")]
    pub local_directory: String,
    #[serde(rename = "targetDirectory")]
    pub target_directory: String,
    #[serde(rename = "preDeployActions")]
    pub pre_deploy_actions: Option<Vec<String>>,
    #[serde(rename = "postDeployActions")]
    pub post_deploy_actions: Option<Vec<String>>,
}
