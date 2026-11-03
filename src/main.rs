mod cli;
mod config;
mod handlers;
mod ssh;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Cli
    let cli = Cli::parse();

    // Add a tunnel to config.toml
    match &cli.command {
        // Future work: add UX to this command, like when typing rtm add
        // the cli will ask for name, local_port, remote_host, etc...
        Commands::Add {
            name,

            local_port,
            remote_host,
            remote_port,

            ssh_host,
            ssh_port,
            ssh_user,
            ssh_key_path,

            retry_on_failure,
        } => {
            handlers::add_tunnel(
                name.clone(),
                *local_port,
                remote_host.clone(),
                *remote_port,
                ssh_host.clone(),
                *ssh_port,
                ssh_user.clone(),
                ssh_key_path.clone(),
                *retry_on_failure,
            )?;
        }
        // Start a tunnel with tunnel's name
        // Future work: after grouping the tunnel, and allow start with group name
        Commands::Up { name } => {
            handlers::start_tunnel(&name).await?;
        }
        // List out all tunnel which in config.toml
        Commands::Ls {} => {
            handlers::list_tunnels()?;
        } // Commands::Rm {} => {}
          // Commands::Stats {} => {}
          // Commands::Addgp {} => {}
          // Commands::Rmgp {} => {}
          // Commands::Assgp {} => {}
    }
    Ok(())
}
