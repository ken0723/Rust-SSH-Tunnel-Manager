use crate::config::{Config, Tunnel};
use crate::ssh;
use comfy_table::Table;
use inquire::{required, Confirm, CustomType, Text};
use std::error::Error;

pub fn add_tunnel(
    name: Option<String>,
    local_port: Option<u16>,
    remote_host: Option<String>,
    remote_port: Option<u16>,
    ssh_host: Option<String>,
    ssh_port: Option<u16>,
    ssh_user: Option<String>,
    ssh_key_path: Option<String>,
    retry_on_failure: Option<bool>,
) -> Result<(), Box<dyn Error>> {
    let final_name = match name {
        Some(val) => val.clone(),
        None => Text::new("Tunnel name")
            .with_validator(required!("Name cannot be empty!"))
            .prompt()?,
    };
    let overwrite = if Config::load()?.tunnels.contains_key(&final_name) {
        let confirmed = Confirm::new(&format!("Tunnel '{final_name}' already exists. Overwrite?"))
            .with_default(false)
            .with_parser(&|answer| match answer.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" => Ok(true),
                "n" | "no" => Ok(false),
                _ => Err(()),
            })
            .with_error_message("Enter y/n or yes/no")
            .prompt()?;
        if !confirmed {
            println!("Tunnel '{final_name}' was not changed");
            return Ok(());
        }
        true
    } else {
        false
    };

    let final_local_port = match local_port {
        Some(val) => val.clone(),
        None => CustomType::<u16>::new("Local port").prompt()?,
    };

    let final_remote_host = match remote_host {
        Some(val) => val.clone(),
        None => Text::new("Remote host")
            .with_validator(required!("Remote host cannot be empty!"))
            .prompt()?,
    };

    let final_remote_port = match remote_port {
        Some(val) => val.clone(),
        None => CustomType::<u16>::new("Remote port").prompt()?,
    };

    let final_ssh_host = match ssh_host {
        Some(val) => val.clone(),
        None => Text::new("SSH host")
            .with_validator(required!("SSH host cannot be empty!"))
            .prompt()?,
    };

    let final_ssh_port = match ssh_port {
        Some(val) => val.clone(),
        None => CustomType::<u16>::new("SSH port")
            .with_default(22)
            .prompt()?,
    };

    let final_ssh_user = match ssh_user {
        Some(val) => val.clone(),
        None => Text::new("SSH user")
            .with_validator(required!("SSH user cannot be empty!"))
            .with_default("ec2-user")
            .prompt()?,
    };

    let final_ssh_key_path = match ssh_key_path {
        Some(val) => val.clone(),
        None => Text::new("SSH Key Path(Allow Empty)").prompt()?,
    };

    let final_retry_on_failure = match retry_on_failure {
        Some(val) => val.clone(),
        None => Confirm::new("Retry on failure")
            .with_default(false)
            .prompt()?,
    };

    let final_ssh_key_path = final_ssh_key_path
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();

    let final_ssh_key_path = if final_ssh_key_path.is_empty() {
        None
    } else {
        Some(final_ssh_key_path)
    };

    let data = Tunnel {
        name: final_name.clone(),

        local_port: final_local_port,
        remote_host: final_remote_host,
        remote_port: final_remote_port,

        ssh_host: final_ssh_host,
        ssh_port: final_ssh_port,
        ssh_user: final_ssh_user,
        ssh_key_path: final_ssh_key_path,

        retry_on_failure: Some(final_retry_on_failure),
    };

    // println!("tunnel data: {:?}", data);

    Config::save(data, overwrite)?;
    if overwrite {
        println!("✅ Successfully updated tunnel: {final_name}");
    } else {
        println!("✅ Successfully added tunnel: {final_name}");
    }
    Ok(())
}

pub async fn start_tunnel(name: &str) -> Result<(), Box<dyn Error>> {
    match Config::get(name) {
        Ok(tunnel) => {
            if let Err(e) = ssh::start_tunnel(tunnel).await {
                eprintln!("❌ Error connecting tunnel: {e}");
            }
        }
        Err(e) => eprintln!("❌ Error get tunnel: {e}"),
    }
    Ok(())
}

pub fn list_tunnels() -> Result<(), Box<dyn Error>> {
    match Config::load() {
        Ok(read_data) => {
            let mut list: Vec<_> = read_data.tunnels.values().collect();
            list.sort_by_key(|t| &t.name);

            let mut config_table = Table::new();
            config_table.set_header(vec![
                "Name",
                "Local",
                "Remote",
                "SSH",
                "SSH Key",
                "Retry on failure",
            ]);

            for (name, t) in read_data.tunnels {
                let retry_display = match &t.retry_on_failure {
                    Some(true) => "Yes",
                    Some(false) => "No",
                    None => "No",
                };

                let key_path_display = match &t.ssh_key_path {
                    Some(path) => path,
                    None => "Default",
                };

                config_table.add_row(vec![
                    name,
                    format!("localhost:{}", &t.local_port),
                    format!("{}:{}", &t.remote_host, &t.remote_port.to_string()),
                    format!("{}@{}:{}", &t.ssh_user, &t.ssh_host, &t.ssh_port),
                    key_path_display.to_string(),
                    retry_display.to_string(),
                ]);
            }
            println!("{config_table}");
        }
        Err(e) => {
            eprintln!("Error on listing config: {}", e);
        }
    }
    Ok(())
}
