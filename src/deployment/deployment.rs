use crate::{
    serialization::{config::Config, deploy_target::DeployTarget},
    states::ui_state::UIStore,
};
use anyhow::anyhow;
use async_trait::async_trait;
use futures::lock::Mutex;
use russh::{client::Handle, *};
use russh_keys::*;
use std::{any, collections::HashMap, sync::Arc, time::Duration};
use tokio::{fs::File, io::BufReader};

use super::packaging::PackageCreator;

static SUDO_PREPEND: &str = "sudo ";
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
    let mut target_package_names: HashMap<String, String> = HashMap::new();
    let mut checksums: HashMap<String, HashMap<String, String>> = HashMap::new();

    'preDeployConnection: loop {
        // create ssh session
        let mut session: Handle<Client>;

        // ssh config
        let ssh_config = russh::client::Config::default();
        let ssh_config = Arc::new(ssh_config);
        let sh = Client {};
        match russh::client::connect(ssh_config, (target.host.to_owned(), target.port), sh).await {
            Err(..) => continue 'preDeployConnection,
            Ok(res) => {
                session = res;
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
                match session.authenticate_publickey(creds[0], key.clone()).await {
                    Err(..) => continue 'preDeployConnection,
                    Ok(res) => {
                        auth = res;
                    }
                }
            }
            "password" => match session.authenticate_password(creds[0], creds[1]).await {
                Err(..) => continue 'preDeployConnection,
                Ok(res) => {
                    auth = res;
                }
            },
            _ => {}
        }
        //println!("auth = {}", auth);
        if auth {
            // 3. deploy packages
            for package in &target.packages {
                // check if we already deployed package
                if target_package_names.contains_key(package) {
                    continue;
                }

                let mut channel = session.channel_open_session().await?;
                channel.exec(true, "mktemp").await?; // ignore sudo here (important)
                let mut tmp_file_name = String::new();
                while let Some(res) = channel.wait().await {
                    match res {
                        russh::ChannelMsg::ExtendedData { ref data, ext } => {
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    tmp_file_name += v;
                                }
                                Err(_) => {
                                    continue 'preDeployConnection;
                                }
                            };
                        }
                        russh::ChannelMsg::Data { ref data } => {
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    let st = v.to_string();
                                    let dt: Vec<&str> = st.split("\n").collect();
                                    tmp_file_name = Some(dt[0]).unwrap_or("").to_string();
                                }
                                Err(_) => {
                                    continue 'preDeployConnection;
                                }
                            };
                        }
                        _ => {}
                    }
                }

                // #USE_REMOTE_CHECKSUM
                // [1/4] Computing checksum {package}
                if !checksums.contains_key(package) {
                    checksums.insert(package.to_string(), HashMap::new());
                }

                // iterate through external files & try to compute all checksums
                let config_res = config.lock().await;
                let package_element = &config_res.packages[package];
                let mut files: Vec<String> = Vec::new();
                PackageCreator::collect_files_ext(
                    package_element.local_directory.to_string(),
                    &mut files,
                );
                /*for element in &files {
                    println!("file {}", element);
                }*/
                // #USE_REMOTE_CHECKSUM_ACCUMULATED_HASHER
                let mut cmdpars = String::new();
                for file in &files {
                    cmdpars +=
                        &format!(" \"{}{}\"", package_element.target_directory, file).to_string();
                }

                let mut channel = session.channel_open_session().await?;
                let fmt = format!("{}sha1sum{}", SUDO_PREPEND, cmdpars);
                channel.exec(true, fmt).await?;

                let mut cmd_res = String::new();
                while let Some(res) = channel.wait().await {
                    match res {
                        russh::ChannelMsg::ExtendedData { ref data, ext } => {
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    cmd_res += v;
                                }
                                Err(_) => {
                                    continue 'preDeployConnection;
                                }
                            };
                        }
                        russh::ChannelMsg::Data { ref data } => {
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    cmd_res += v;
                                }
                                Err(_) => {
                                    continue 'preDeployConnection;
                                }
                            };
                        }
                        _ => continue,
                    }
                }

                let cmd_y_res: Vec<&str> = cmd_res.split("\n").collect();
                for (i, cmyr) in cmd_y_res.iter().enumerate() {
                    if files.len() <= i {
                        continue;
                    }
                    let sum_vec: Vec<&str> = cmyr.split(" ").collect();
                    let sum = sum_vec[0];
                    checksums.get_mut(package).unwrap().remove(&files[i]); // check for duplicates
                    checksums
                        .get_mut(package)
                        .unwrap()
                        .insert(files[i].to_string(), sum.to_string());
                }

                target_package_names.insert(package.to_string(), tmp_file_name);
            }

            break 'preDeployConnection;
        } else {
            return Err(anyhow!("Authentication failed"));
        }
    }

    // 2. prepare & upload packages
    let mut ongoing_deploy_packages_state: Vec<String> = Vec::new();
    'ongoinDeployConnection: loop {
        let mut checksums_deep_copy: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (key, value) in &checksums {
            checksums_deep_copy.insert(key.to_string(), HashMap::new());
            for (k1, v1) in value {
                checksums_deep_copy
                    .get_mut(key)
                    .unwrap()
                    .insert(k1.to_string(), v1.to_string());
            }
        }

        // create ssh session
        let mut session: Handle<Client>;

        // ssh config
        let ssh_config = russh::client::Config::default();
        let ssh_config = Arc::new(ssh_config);
        let sh = Client {};
        match russh::client::connect(ssh_config, (target.host.to_owned(), target.port), sh).await {
            Err(..) => continue 'ongoinDeployConnection,
            Ok(res) => {
                session = res;
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
                match session.authenticate_publickey(creds[0], key.clone()).await {
                    Err(..) => continue 'ongoinDeployConnection,
                    Ok(res) => {
                        auth = res;
                    }
                }
            }
            "password" => match session.authenticate_password(creds[0], creds[1]).await {
                Err(..) => continue 'ongoinDeployConnection,
                Ok(res) => {
                    auth = res;
                }
            },
            _ => {}
        }
        //println!("auth = {}", auth);
        if auth {
            for package in &target.packages {
                if ongoing_deploy_packages_state.contains(package) {
                    continue;
                }

                let config_res = config.lock().await;
                let package_element = &config_res.packages[package];

                let creator = PackageCreator::new(checksums_deep_copy.get(package).unwrap());
                let res =
                    creator.prepare_package_for_target(package_element.local_directory.to_string());
                //, out byte[] hashes, out int writtenEntries);
                if res.1 {

                    /*
                    packageElement.Uploaded = true;
                            lock (CustomConsole.ConsoleLock)
                            {
                                CustomConsole.ResetLine();
                                Console.WriteLine($"[2/4] Uploading {package} [{Path.GetFileName(archivePath)}]");
                            }
                    */
                } else {
                    /*
                    lock (CustomConsole.ConsoleLock)
                        {
                            CustomConsole.ResetLine();
                            Console.WriteLine($"[2/4] No changes for {package}");
                        }
                     */
                }

                ongoing_deploy_packages_state.push(package.to_string());
            }

            break 'ongoinDeployConnection;
        }
    }

    Ok(())
}
