use crate::config::Tunnel;
use russh::client::{AuthResult, DisconnectReason};
use russh::keys::{self, known_hosts};
use russh::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};

const RETRY_DELAY: Duration = Duration::from_secs(5);

struct Client {
    host: String,
    port: u16,
    disconnect_tx: watch::Sender<Option<String>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key.fingerprint(Default::default());
        match known_hosts::check_known_hosts(&self.host, self.port, server_public_key) {
            Ok(true) => Ok(true),
            Ok(false) => {
                eprintln!(
                    "❌ Host key for {}:{} is not in ~/.ssh/known_hosts ({fingerprint})",
                    self.host, self.port
                );
                Ok(false)
            }
            Err(keys::Error::KeyChanged { line }) => {
                eprintln!(
                    "❌ Host key for {}:{} has changed (known_hosts line {line}, {fingerprint})",
                    self.host, self.port
                );
                Ok(false)
            }
            Err(e) => {
                eprintln!(
                    "❌ Host key check failed for {}:{}: {e}",
                    self.host, self.port
                );
                Ok(false)
            }
        }
    }

    async fn disconnected(
        &mut self,
        reason: DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        let _ = self.disconnect_tx.send(Some(format!("{reason:?}")));
        match reason {
            DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            DisconnectReason::Error(e) => Err(e),
        }
    }
}

pub async fn start_tunnel(tunnel: Tunnel) -> Result<(), Box<dyn std::error::Error>> {
    let retry = tunnel.retry_on_failure.unwrap_or(false);

    loop {
        match run_session(&tunnel).await {
            Ok(()) => return Ok(()),
            Err(e) if retry => {
                eprintln!("❌ Tunnel failed: {e}");
                eprintln!("↻ Retrying in {}s...", RETRY_DELAY.as_secs());
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn run_session(tunnel: &Tunnel) -> Result<(), Box<dyn std::error::Error>> {
    let (disconnect_tx, mut disconnect_rx) = watch::channel(None);
    let config = Arc::new(client::Config::default());
    let handler = Client {
        host: tunnel.ssh_host.clone(),
        port: tunnel.ssh_port,
        disconnect_tx,
    };
    let ssh_addr = format!("{}:{}", tunnel.ssh_host, tunnel.ssh_port);
    println!("🔌 Connecting to SSH: {ssh_addr}");

    let mut session =
        client::connect(config, (&tunnel.ssh_host[..], tunnel.ssh_port), handler).await?;
    println!("🔌 Authorizing");
    authenticate(&mut session, tunnel).await?;

    let session = Arc::new(Mutex::new(session));
    let local_addr = format!("127.0.0.1:{}", tunnel.local_port);
    let listener = TcpListener::bind(&local_addr).await?;

    println!("✅ Successfully connected to tunnel: {}", tunnel.name);
    println!("🚀 Listening on {local_addr}");

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                println!("Connection from {peer}");

                let shared_session = session.clone();
                let target_host = tunnel.remote_host.clone();
                let target_port = tunnel.remote_port;
                let local_port = tunnel.local_port;
                tokio::spawn(async move {
                    forward_connection(
                        shared_session,
                        stream,
                        target_host,
                        target_port,
                        local_port,
                    )
                    .await;
                });
            }
            result = disconnect_rx.changed() => {
                result?;
                if let Some(reason) = disconnect_rx.borrow().clone() {
                    return Err(format!("SSH session closed: {reason}").into());
                }
            }
        }
    }
}

fn identity_files(tunnel: &Tunnel) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if let Some(path) = tunnel
        .ssh_key_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(format!("SSH key not found: {}", path.display()).into());
        }
        return Ok(vec![path]);
    }

    let home = directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or("Cannot locate home directory for default SSH keys")?;
    let dir = home.join(".ssh");
    let names = [
        "id_rsa",
        "id_ecdsa",
        "id_ecdsa_sk",
        "id_ed25519",
        "id_ed25519_sk",
    ];
    let found: Vec<_> = names
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .collect();
    if found.is_empty() {
        return Err(format!(
            "No SSH key path set and no default key in {}",
            dir.display()
        )
        .into());
    }
    Ok(found)
}

async fn authenticate(
    session: &mut client::Handle<Client>,
    tunnel: &Tunnel,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = identity_files(tunnel)?;
    let mut failures = Vec::new();

    for path in paths {
        println!("🔑 Using key {}", path.display());
        let pem = match std::fs::read_to_string(&path) {
            Ok(pem) => pem,
            Err(e) => {
                failures.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let key_pair = match keys::decode_secret_key(&pem, None) {
            Ok(key) => Arc::new(key),
            Err(e) => {
                failures.push(format!("{}: {e}", path.display()));
                continue;
            }
        };

        let hash_alg = if matches!(key_pair.algorithm(), keys::Algorithm::Rsa { .. }) {
            match session.best_supported_rsa_hash().await? {
                Some(alg) => alg,
                None => Some(keys::HashAlg::Sha256),
            }
        } else {
            None
        };
        let key = keys::PrivateKeyWithHashAlg::new(key_pair, hash_alg);

        match session
            .authenticate_publickey(&tunnel.ssh_user, key)
            .await?
        {
            AuthResult::Success => return Ok(()),
            AuthResult::Failure { .. } => {
                failures.push(format!("{}: rejected by server", path.display()));
            }
        }
    }

    Err(format!("SSH authentication failed: {}", failures.join("; ")).into())
}

async fn forward_connection(
    session: Arc<Mutex<client::Handle<Client>>>,
    mut stream: TcpStream,
    target_host: String,
    target_port: u16,
    local_port: u16,
) {
    let channel = {
        let session_guard = session.lock().await;
        let opened = session_guard
            .channel_open_direct_tcpip(
                target_host,
                target_port.into(),
                "127.0.0.1",
                local_port.into(),
            )
            .await;
        drop(session_guard);
        match opened {
            Ok(channel) => channel,
            Err(e) => {
                println!("Failed to open channel: {e:?}");
                return;
            }
        }
    };

    let mut channel_stream = channel.into_stream();
    match tokio::io::copy_bidirectional(&mut stream, &mut channel_stream).await {
        Ok(_) => println!("Tunnel closed."),
        Err(e) => println!("Tunnel broken: {e:?}"),
    }
}
