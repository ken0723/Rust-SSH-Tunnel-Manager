use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Add {
        name: Option<String>,

        local_port: Option<u16>,
        remote_host: Option<String>,
        remote_port: Option<u16>,

        ssh_host: Option<String>,
        ssh_port: Option<u16>,
        ssh_user: Option<String>,
        ssh_key_path: Option<String>,

        retry_on_failure: Option<bool>,
    },
    // Start a tunnel
    // TODO:
    // Future work: allow start a group of tunnel
    Up {
        name: String,
    },
    // List all added tunnel
    // TODO:
    // Future work, to listing out all started tunnel,
    // and that allow typing 'rtun ls -a' to list out all tunnel,
    // include non-started
    Ls {},
    // TODO:
    // Remove a tunnel
    // Rm {},

    // TODO:
    // Like Docker stats, keep checking the started tunnel
    // Stats {},

    // TODO:
    // Add group
    // Addgp {},

    // TODO:
    // Remove group (with all child tunnel)
    // Rmgp {},

    // TODO:
    // Assign tunnel to a group
    // Assgp {},
}
