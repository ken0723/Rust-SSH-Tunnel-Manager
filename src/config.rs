use clap::Parser;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub tunnels: HashMap<String, Tunnel>,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Clone, Deserialize, Serialize, Debug, Parser)]
pub struct Tunnel {
    // base
    pub name: String,

    // tunnel data
    #[arg(long)]
    pub local_port: u16,
    pub remote_host: String,
    #[arg(long)]
    pub remote_port: u16,

    // ssh data
    pub ssh_host: String,
    #[arg(long)]
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_key_path: Option<String>,

    pub retry_on_failure: Option<bool>,
}

impl Config {
    // Load a file from path
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "rtun") {
            let config_dir = proj_dirs.config_dir();
            let config_path = config_dir.join("config.toml");

            // println!("path: {:?}", config_path);
            match fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str::<Config>(&content) {
                    Ok(mut config) => {
                        config.path = config_path;
                        Ok(config)
                    }
                    Err(e) => Err(e)?,
                },
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        // Create a default Config with HashMap
                        let mut default_config: Config = Config::default();
                        // File path
                        default_config.path = config_path.clone();
                        // Create dir
                        fs::create_dir_all(config_dir)?;
                        // Convert to toml format
                        let toml_default_config = toml::to_string(&default_config)?;
                        // Write to disk
                        fs::write(&config_path, toml_default_config)?;
                        return Ok(default_config);
                    } else {
                        Err(e)?
                    }
                }
            }
        } else {
            Err("Cannot find project directory".into())
        }
    }

    // Get the tunnel config
    pub fn get(tunnel_name: &str) -> Result<Tunnel, Box<dyn std::error::Error>> {
        let read_config = Config::load()?;
        let tunnel: Tunnel = read_config
            .tunnels
            .get(tunnel_name)
            .cloned()
            .ok_or(format!("Tunnel name: '{}' not found", tunnel_name))?;

        Ok(tunnel)
    }

    // Save a new tunnel to config.toml
    pub fn save(data: Tunnel, overwrite: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut read_config = Config::load()?;
        if read_config.tunnels.contains_key(&data.name) && !overwrite {
            return Err(format!("Tunnel '{}' already exists", data.name).into());
        }
        read_config.tunnels.insert(data.name.clone(), data);

        fs::write(&read_config.path, toml::to_string(&read_config)?)?;

        Ok(())
    }
}
