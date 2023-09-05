use crate::{
    serialization::{config::Config, deploy_target::DeployTarget},
    states::ui_state::UIStore,
};
use anyhow::anyhow;
use async_trait::async_trait;
use futures::lock::Mutex;
use russh::{client::Handle, *};
use russh_keys::*;
use std::{any, collections::HashMap, sync::Arc};
use tokio::{fs::File, io::BufReader};

struct Client {}

#[async_trait]
impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        self,
        server_public_key: &key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        //println!("check_server_key: {:?}", server_public_key);
        Ok((self, true))
    }

    async fn data(
        self,
        channel: ChannelId,
        data: &[u8],
        session: client::Session,
    ) -> Result<(Self, client::Session), Self::Error> {
        /*println!(
            "data on channel {:?}: {:?}",
            channel,
            std::str::from_utf8(data)
        );*/
        Ok((self, session))
    }
}

pub async fn begin_deployment(
    config: Arc<Mutex<Config>>,
    ui_state: Arc<Mutex<UIStore>>,
) -> anyhow::Result<()> {
    // 1. loop through targets
    let mut copyied_deploy_targets: Vec<DeployTarget> = Vec::new();
    {
        let config_res = config.lock().await;
        for element in &config_res.targets {
            copyied_deploy_targets.push(element.clone());
        }
    }

    // 2. deploy each target
    for deploy_target in &copyied_deploy_targets {
        deploy(config.clone(), ui_state.clone(), &deploy_target).await?;
    }

    Ok(())
}

pub async fn deploy(
    config: Arc<Mutex<Config>>,
    ui_state: Arc<Mutex<UIStore>>,
    target: &DeployTarget,
) -> anyhow::Result<(), anyhow::Error> {
    {
        let mut ui_state_res = ui_state.lock().await;
        let name = target.name.to_owned().unwrap_or(String::from("unnamed"));
        ui_state_res.set_deployment_target(name);
    }

    // parse credentials
    let (auth_type, auth_str) = target.authentication.iter().next().unwrap(); // should panic if config has errors
    let target_package_names: HashMap<String, String> = HashMap::new();

    'preDeployConnection: loop {
        // create ssh session
        let mut session: Handle<Client>;

        loop {
            // ssh config
            let ssh_config = russh::client::Config::default();
            let ssh_config = Arc::new(ssh_config);
            let sh = Client {};
            match russh::client::connect(ssh_config, (target.host.to_owned(), target.port), sh)
                .await
            {
                Err(..) => continue,
                Ok(res) => {
                    session = res;
                    break;
                }
            }
        }

        let mut auth = false;
        let creds: Vec<&str> = auth_str.split(":").collect();
        match auth_type.as_str() {
            "certificate" => {
                let mut cert_pass: Option<&str> = None;
                if creds.len() > 2 {
                    cert_pass = Some(creds[2]);
                }
                //let file = File::open(creds[1]).await.unwrap();
                //let reader = BufReader::new(file);
                //let key =
                //    Arc::new(russh_keys::openssh::decode_openssh(reader.buffer(), cert_pass).unwrap());
                let key = Arc::new(russh_keys::load_secret_key(creds[1], cert_pass).unwrap()); // should panic if key wrong
                loop {
                    match session.authenticate_publickey(creds[0], key.clone()).await {
                        Err(..) => continue,
                        Ok(res) => {
                            auth = res;
                            break;
                        }
                    }
                }
            }
            "password" => loop {
                match session.authenticate_password(creds[0], creds[1]).await {
                    Err(..) => continue,
                    Ok(res) => {
                        auth = res;
                        break;
                    }
                }
            },
            _ => {}
        }
        //println!("auth = {}", auth);
        if auth {
            let mut channel = session.channel_open_session().await?;
            // 3. deploy packages
            for package in &target.packages {
                // check if we already deployed package
                if target_package_names.contains_key(package) {
                    continue;
                }

                channel.exec(true, "mktemp").await?; // ignore sudo here (important)
                let mut tmp_file_name: String;
                while let Some(res) = channel.wait().await {
                    match res {
                        russh::ChannelMsg::Data { ref data } => {
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    let st = v.to_string();
                                    let dt: Vec<&str> = st.split("\n").collect();
                                    tmp_file_name = Some(dt[0]).unwrap_or("").to_string();
                                    break;
                                }
                                Err(_) => {
                                    println!("shit happend");
                                    continue 'preDeployConnection;
                                }
                            };
                        }
                        _ => continue,
                    }
                }

                break 'preDeployConnection;
            }
        } else {
            return Err(anyhow!("Authentication failed"));
        }
    }

    Ok(())
}
