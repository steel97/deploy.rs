use super::packaging::PackageCreator;
use crate::core::constants::{CHUNK_UPLOAD_BUFFER, CHUNK_UPLOAD_RETRIES, SUDO_PREPEND};
use crate::serialization::deploy_package::DeployPackage;
use crate::states::ui_state::{TargetState, UIScreen, UITargetState};
use crate::{
    serialization::{config::Config, deploy_target::DeployTarget},
    states::ui_state::UIStore,
};
use anyhow::anyhow;
use futures::future::join_all;
use futures::lock::Mutex;
use futures::StreamExt;
use russh::client::AuthResult;
use russh::keys::PrivateKeyWithHashAlg;
use russh::{client::Handle, *};
use russh_sftp::client::SftpSession;
use std::cmp::min;
use std::fs::File;
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncWriteExt;
//use tokio::time::{sleep, Duration};

const CMD_FILES_LIMIT: u16 = 512;

pub struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
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
        let mut target_index = 0;
        for element in &config_res.targets {
            copyied_deploy_targets.push(element.clone());

            let name = element.name.to_owned().unwrap_or(String::from("unnamed"));

            {
                let target = TargetState {
                    state: UITargetState::TARGET_START,
                    name: name,
                    upload_package: "none".to_string(),
                    upload_pos: 0,
                    upload_len: 0,
                };
                let mut ui_state_res = ui_state.lock().await;
                ui_state_res.set_deployment_target(target_index.clone(), target);
            }

            target_index = target_index + 1;
        }
    }

    // 2. deploy each target
    let mut target_index = 0;
    let mut deploy_tasks = Vec::new();
    for deploy_target in &copyied_deploy_targets {
        deploy_tasks.push(tokio::spawn(deploy(
            config.clone(),
            ui_state.clone(),
            deploy_target.clone(),
            target_index.clone(),
        )));
        target_index = target_index + 1;
    }

    join_all(deploy_tasks).await;

    {
        let mut ui_state_res = ui_state.lock().await;
        ui_state_res.set_screen(UIScreen::FINISHED);
    }

    //sleep(Duration::from_millis(500)).await;

    {
        let mut ui_state_res = ui_state.lock().await;
        ui_state_res.set_screen(UIScreen::FINISHED_END);
    }

    Ok(())
}

pub async fn create_session(
    auth_type: String,
    auth_str: String,
    host: String,
    port: u16,
) -> anyhow::Result<Handle<Client>, anyhow::Error> {
    let mut session: Handle<Client>;

    // ssh config
    let ssh_config = russh::client::Config::default();
    let ssh_config = Arc::new(ssh_config);
    let sh = Client {};

    session = russh::client::connect(ssh_config, (host.to_owned(), port), sh).await?;

    let mut auth = false;
    let creds: Vec<&str> = auth_str.split(":").collect();
    match auth_type.as_str() {
        "certificate" => {
            let mut cert_pass: Option<&str> = None;
            if creds.len() > 2 {
                cert_pass = Some(creds[2]);
            }
            let key = Arc::new(russh::keys::load_secret_key(creds[1], cert_pass).unwrap()); // should panic if key wrong
            let pre_res = session
                .authenticate_publickey(
                    creds[0],
                    PrivateKeyWithHashAlg::new(
                        key.clone(),
                        session.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await?;
            auth = pre_res == AuthResult::Success;
        }
        "password" => match session.authenticate_password(creds[0], creds[1]).await {
            Err(_) => return Err(anyhow!("authentication failed")),
            Ok(res) => {
                auth = res == AuthResult::Success;
            }
        },
        _ => {}
    }

    if !auth {
        return Err(anyhow!("authentication failed #2"));
    }

    Ok(session)
}

pub async fn deploy(
    config: Arc<Mutex<Config>>,
    ui_state: Arc<Mutex<UIStore>>,
    target: DeployTarget,
    target_index: u32,
) -> anyhow::Result<(), anyhow::Error> {
    // parse credentials
    let (auth_type, auth_str) = target.authentication.iter().next().unwrap(); // should panic if config has errors
    let mut target_package_names: HashMap<String, String> = HashMap::new();
    let mut checksums: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut deploy_states_uploaded: HashMap<String, bool> = HashMap::new();
    let mut deploy_states_post_action_successed: HashMap<String, bool> = HashMap::new();

    'pre_deploy_connection: loop {
        // create ssh session
        let session: Handle<Client> = match create_session(
            auth_type.to_string(),
            auth_str.to_string(),
            target.host.to_string(),
            target.port,
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                continue 'pre_deploy_connection;
            }
        };
        // 3. deploy packages
        {
            let mut ui_state_res = ui_state.lock().await;
            let target_state = ui_state_res
                .deployment_targets
                .get_mut(&target_index)
                .unwrap();
            target_state.state = UITargetState::TARGET_CHECKSUM;
            target_state.upload_package = "".to_string();
        }

        for package in &target.packages {
            // check if we already deployed package
            if target_package_names.contains_key(package) {
                continue;
            }

            let mut is_shit_happend = false;
            let mut tmp_file_name = String::new();
            {
                let mut channel = match session.channel_open_session().await {
                    Ok(r) => r,
                    Err(_) => continue 'pre_deploy_connection,
                };

                match channel.exec(true, "mktemp").await {
                    Err(_) => continue 'pre_deploy_connection,
                    _ => {}
                }; // ignore sudo here (important)
                   //let mut is_msg_read = false;
                while let Some(res) = channel.wait().await {
                    match res {
                        russh::ChannelMsg::ExtendedData { ref data, ext } => {
                            if ext == 1 {
                                is_shit_happend = true;
                            }
                            match std::str::from_utf8(data) {
                                Ok(v) => {
                                    tmp_file_name += v;
                                }
                                Err(_) => {
                                    continue 'pre_deploy_connection;
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
                                    continue 'pre_deploy_connection;
                                }
                            };
                        }
                        russh::ChannelMsg::Eof => {
                            //is_msg_read = true;
                            //break;
                        }
                        _ => {}
                    }
                }
            }
            if is_shit_happend {
                continue 'pre_deploy_connection;
            }

            // #USE_REMOTE_CHECKSUM
            {
                let mut ui_state_res = ui_state.lock().await;
                let target_state = ui_state_res
                    .deployment_targets
                    .get_mut(&target_index)
                    .unwrap();
                target_state.upload_package = package.to_string()
            }

            if !checksums.contains_key(package) {
                checksums.insert(package.to_string(), HashMap::new());
            }

            // iterate through external files & try to compute all checksums
            let package_element: DeployPackage;
            {
                let config_res = config.lock().await;
                let package_element_base = &config_res.packages[package];
                package_element = package_element_base.clone();
            }
            let mut files: Vec<String> = Vec::new();
            PackageCreator::collect_files_ext(
                package_element.local_directory.to_string(),
                &mut files,
            );
            // #USE_REMOTE_CHECKSUM_ACCUMULATED_HASHER

            let mut cmdpars: Vec<String> = Vec::new();
            let mut cmdpars_buf = String::new();
            let mut cmdctr = 0;
            for file in &files {
                cmdpars_buf +=
                    &format!(" \"{}{}\"", package_element.target_directory, file).to_string();
                cmdctr += 1;
                if cmdctr > CMD_FILES_LIMIT {
                    cmdpars.push(cmdpars_buf);
                    cmdpars_buf = String::new();
                    cmdctr = 0;
                }
            }

            if !cmdpars_buf.is_empty() {
                cmdpars.push(cmdpars_buf);
            }

            let mut cmd_res: String = String::new();

            for (_i, cmdpars_entry) in cmdpars.iter().enumerate() {
                let mut is_shit_happend = false;
                {
                    let mut channel = match session.channel_open_session().await {
                        Ok(r) => r,
                        Err(_) => continue 'pre_deploy_connection,
                    };
                    let fmt = format!("{}sha1sum{}", SUDO_PREPEND, cmdpars_entry);
                    match channel.exec(true, fmt).await {
                        Err(_) => continue 'pre_deploy_connection,
                        _ => {}
                    };

                    while let Some(res) = channel.wait().await {
                        match res {
                            russh::ChannelMsg::ExtendedData { ref data, ext } => {
                                if ext == 1 {
                                    is_shit_happend = true;
                                }

                                match std::str::from_utf8(data) {
                                    Ok(v) => {
                                        cmd_res += v;
                                    }
                                    Err(_) => {
                                        continue 'pre_deploy_connection;
                                    }
                                };
                            }
                            russh::ChannelMsg::Data { ref data } => {
                                match std::str::from_utf8(data) {
                                    Ok(v) => {
                                        cmd_res += v;
                                    }
                                    Err(_) => {
                                        continue 'pre_deploy_connection;
                                    }
                                };
                            }
                            russh::ChannelMsg::Eof => {
                                //is_msg_read = true;
                                //break;
                            }
                            _ => continue,
                        }
                    }
                }

                if is_shit_happend {
                    // file sometimes missing (initial upload as an example, so this is expected)
                    //continue 'pre_deploy_connection;
                }
            }

            let cmd_y_res: Vec<&str> = cmd_res.split("\n").collect();
            //panic!("cmd_res: {:?}", cmd_res);
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

        break 'pre_deploy_connection;
    }

    // 2. prepare & upload packages
    let mut ongoing_deploy_packages_state: Vec<String> = Vec::new();
    'ongoing_deploy_connection: loop {
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
        let session: Handle<Client> = match create_session(
            auth_type.to_string(),
            auth_str.to_string(),
            target.host.to_string(),
            target.port,
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                continue 'ongoing_deploy_connection;
            }
        };

        let channel = match session.channel_open_session().await {
            Ok(r) => r,
            Err(_) => continue 'ongoing_deploy_connection,
        };

        match channel.request_subsystem(true, "sftp").await {
            Err(..) => continue 'ongoing_deploy_connection,
            _ => {}
        }

        let sftp = match SftpSession::new(channel.into_stream()).await {
            Err(..) => continue 'ongoing_deploy_connection,
            Ok(res) => res,
        };

        for package in &target.packages {
            if ongoing_deploy_packages_state.contains(package) {
                continue;
            }

            let package_element: DeployPackage;
            {
                let config_res = config.lock().await;
                let package_element_base = &config_res.packages[package];
                package_element = package_element_base.clone();
            }

            let creator = PackageCreator::new(checksums_deep_copy.get(package).unwrap());

            let local_temp_dir = tempfile::tempdir()?;
            let local_temp_file_path = local_temp_dir.path().join("archive.tar.gz");
            let local_temp_file_path_copy = local_temp_dir.path().join("archive.tar.gz");
            let res: bool;

            {
                let local_temp_file = File::create(local_temp_file_path)?;
                res = creator.prepare_package_for_target(
                    &local_temp_file,
                    package_element.local_directory.to_string(),
                );
            }

            //, out byte[] hashes, out int writtenEntries);
            if res {
                // read local file
                let file = tokio::fs::File::open(&local_temp_file_path_copy).await?;
                let total_size = file.metadata().await.unwrap().len();
                let mut reader_stream =
                    tokio_util::io::ReaderStream::with_capacity(file, CHUNK_UPLOAD_BUFFER);

                {
                    let mut ui_state_res = ui_state.lock().await;
                    let target_state = ui_state_res
                        .deployment_targets
                        .get_mut(&target_index)
                        .unwrap();
                    target_state.state = UITargetState::TARGET_UPLOADING;
                    target_state.upload_package = package.to_string();
                    target_state.upload_pos = 0;
                    target_state.upload_len = total_size;
                }

                // open remote file ()
                let mut remote_file = match sftp
                    .create(target_package_names.get(package).unwrap())
                    .await
                {
                    Err(..) => continue 'ongoing_deploy_connection,
                    Ok(res) => res,
                };

                let mut uploaded = 0;
                while let Some(chunk) = reader_stream.next().await {
                    if let Ok(chunk) = &chunk {
                        let mut chunk_upload_retries = 0;
                        'upload_loop: loop {
                            if chunk_upload_retries > CHUNK_UPLOAD_RETRIES {
                                continue 'ongoing_deploy_connection;
                            }

                            let chunk_upload_res = remote_file.write_all(chunk).await;
                            match chunk_upload_res {
                                Ok(_) => {
                                    break 'upload_loop;
                                }
                                Err(_) => {
                                    chunk_upload_retries += 1;
                                    continue 'upload_loop;
                                }
                            }
                        }

                        let new = min(uploaded + (chunk.len() as u64), total_size);
                        uploaded = new;
                        {
                            let mut ui_state_res = ui_state.lock().await;
                            let target_state = ui_state_res
                                .deployment_targets
                                .get_mut(&target_index)
                                .unwrap();
                            target_state.state = UITargetState::TARGET_UPLOADING;
                            target_state.upload_pos = uploaded;
                            target_state.upload_len = total_size;
                        }
                    }
                }

                deploy_states_uploaded.insert(package.to_string(), true);
            } else {
                {
                    let mut ui_state_res = ui_state.lock().await;
                    let target_state = ui_state_res
                        .deployment_targets
                        .get_mut(&target_index)
                        .unwrap();
                    target_state.state = UITargetState::TARGET_NO_CHANGES;
                    target_state.upload_package = package.to_string();
                }
            }

            ongoing_deploy_packages_state.push(package.to_string());
        }

        break 'ongoing_deploy_connection;
    }

    'post_deploy_connection: loop {
        // create ssh session
        let session: Handle<Client> = match create_session(
            auth_type.to_string(),
            auth_str.to_string(),
            target.host.to_string(),
            target.port,
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                continue 'post_deploy_connection;
            }
        };

        for package in &target.packages {
            {
                let mut ui_state_res = ui_state.lock().await;
                let target_state = ui_state_res
                    .deployment_targets
                    .get_mut(&target_index)
                    .unwrap();
                target_state.state = UITargetState::TARGET_FINISHING;
                target_state.upload_package = package.to_string();
            }

            let package_element: DeployPackage;
            {
                let config_res = config.lock().await;
                let package_element_base = &config_res.packages[package];
                package_element = package_element_base.clone();
            }

            if deploy_states_post_action_successed.contains_key(package) {
                continue;
            }

            if deploy_states_uploaded.contains_key(package) {
                // 3. execute pre deploy actions
                for action in package_element.pre_deploy_actions.iter().flatten() {
                    let mut channel = match session.channel_open_session().await {
                        Ok(r) => r,
                        Err(_) => continue 'post_deploy_connection,
                    };
                    let fmt = format!("{}", action);
                    match channel.exec(true, fmt).await {
                        Err(_) => continue 'post_deploy_connection,
                        _ => {}
                    };

                    while let Some(res) = channel.wait().await {
                        match res {
                            russh::ChannelMsg::Data { data: _ } => {
                                // std out
                            }
                            russh::ChannelMsg::ExtendedData { data: _, ext } => {
                                if ext == 1 {
                                    // std err
                                }
                            }
                            russh::ChannelMsg::ExitStatus { exit_status: _ } => {}
                            russh::ChannelMsg::Eof => {
                                //break;
                            }
                            _ => continue,
                        }
                    }
                }
                // 4. deploy package
                {
                    let mut channel = match session.channel_open_session().await {
                        Ok(r) => r,
                        Err(_) => continue 'post_deploy_connection,
                    };
                    /*let _ = channel
                    .request_pty(false, "deploy", 100, 50, 0, 0, &[])
                    .await;*/

                    //"{}sh -c \"cd '{}';tar -xzf '{}'\"",
                    let fmt = format!(
                        "{}tar -xzf '{}' --directory '{}'",
                        SUDO_PREPEND,
                        target_package_names.get(package).unwrap(),
                        package_element.target_directory
                    );
                    match channel.exec(true, fmt).await {
                        Err(_) => continue 'post_deploy_connection,
                        _ => {}
                    };

                    while let Some(_res) = channel.wait().await {}
                }

                // 5. execute post deploy actions
                for action in package_element.post_deploy_actions.iter().flatten() {
                    let mut channel = match session.channel_open_session().await {
                        Ok(r) => r,
                        Err(_) => continue 'post_deploy_connection,
                    };
                    let fmt = format!("{}", action);
                    match channel.exec(true, fmt).await {
                        Err(_) => continue 'post_deploy_connection,
                        _ => {}
                    };

                    while let Some(res) = channel.wait().await {
                        match res {
                            russh::ChannelMsg::Eof => {
                                //break;
                            }
                            _ => continue,
                        }
                    }
                }

                // 6. cleanup remote
                {
                    let mut channel = match session.channel_open_session().await {
                        Ok(r) => r,
                        Err(_) => continue 'post_deploy_connection,
                    };
                    let fmt = format!(
                        "{}rm -f \"{}\"",
                        SUDO_PREPEND,
                        target_package_names.get(package).unwrap()
                    );
                    match channel.exec(true, fmt).await {
                        Err(_) => continue 'post_deploy_connection,
                        _ => {}
                    };

                    while let Some(res) = channel.wait().await {
                        match res {
                            russh::ChannelMsg::Eof => {
                                //break;
                            }
                            _ => continue,
                        }
                    }
                }
            }

            deploy_states_post_action_successed.insert(package.to_string(), true);
        }

        for package in &target.packages {
            deploy_states_post_action_successed.insert(package.to_string(), false);
            // not much sense, but leave for now (ported from C#)
        }

        break 'post_deploy_connection;
    }

    {
        let mut ui_state_res = ui_state.lock().await;
        let target_state = ui_state_res
            .deployment_targets
            .get_mut(&target_index)
            .unwrap();
        target_state.state = UITargetState::TARGET_FINISHED;
    }

    Ok(())
}
