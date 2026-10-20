//! Same-user Unix-socket bridge for append-only integrations.
//!
//! The vault is unlocked interactively before this listener starts. The
//! protocol cannot unlock, read, list, overwrite, or remove vault entries.

use crate::vault::{self, Entry, VaultData};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const API_VERSION: &str = "v1";
const MAX_REQUEST_BYTES: usize = 32 * 1024;
const MAX_LABEL_BYTES: usize = 128;
const MAX_SECRET_BYTES: usize = 16 * 1024;
const IDLE_EXIT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Request {
    Status,
    Store {
        label: String,
        secret: Zeroizing<String>,
    },
}

#[derive(Debug, Serialize)]
struct Reply {
    api_version: &'static str,
    ok: bool,
    unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

impl Reply {
    fn ok(label: Option<String>) -> Self {
        Self {
            api_version: API_VERSION,
            ok: true,
            unlocked: true,
            label,
            error: None,
        }
    }

    fn error(error: &'static str) -> Self {
        Self {
            api_version: API_VERSION,
            ok: false,
            unlocked: true,
            label: None,
            error: Some(error),
        }
    }
}

struct SocketCleanup(PathBuf);

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn default_socket_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".local/state/cybervault/cybervault.sock")
}

pub fn run(vault_path: &Path, socket_path: &Path, password: String) -> Result<(), String> {
    if !socket_path.is_absolute() {
        return Err("socket path must be absolute".into());
    }
    let password = Zeroizing::new(password);
    let mut data = vault::load(vault_path, password.as_str())?;
    prepare_socket_path(socket_path)?;
    let listener = UnixListener::bind(socket_path)
        .map_err(|e| format!("could not bind {}: {e}", socket_path.display()))?;
    let _cleanup = SocketCleanup(socket_path.to_path_buf());
    if let Err(e) = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600)) {
        drop(_cleanup);
        return Err(format!("could not secure the socket: {e}"));
    }
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("could not configure the socket: {e}"))?;
    let _cleanup = SocketCleanup(socket_path.to_path_buf());

    println!(
        "cybervault: local bridge ready at {}",
        socket_path.display()
    );
    println!("cybervault: append-only; auto-exits after five minutes idle (Ctrl+C to stop)");
    let mut last_request = Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                last_request = Instant::now();
                let reply =
                    handle_connection(&mut stream, vault_path, password.as_str(), &mut data);
                let bytes = serde_json::to_vec(&reply).map_err(|e| e.to_string())?;
                stream
                    .write_all(&bytes)
                    .and_then(|()| stream.shutdown(std::net::Shutdown::Write))
                    .map_err(|e| format!("failed to return bridge response: {e}"))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if last_request.elapsed() >= IDLE_EXIT {
                    println!("cybervault: bridge locked after five minutes idle");
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("socket listener failed: {e}")),
        }
    }
}

fn prepare_socket_path(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "socket path has no parent directory".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("could not create socket directory: {e}"))?;
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("could not secure socket directory: {e}"))?;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            if UnixStream::connect(path).is_ok() {
                return Err("another CyberVault bridge is already running".into());
            }
            std::fs::remove_file(path)
                .map_err(|e| format!("could not remove stale socket: {e}"))?;
        }
        Ok(_) => return Err("refusing to replace a non-socket file at the socket path".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("could not inspect socket path: {e}")),
    }
    Ok(())
}

fn handle_connection(
    stream: &mut UnixStream,
    vault_path: &Path,
    password: &str,
    data: &mut VaultData,
) -> Reply {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut bytes = Zeroizing::new(Vec::new());
    if (&mut *stream)
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Reply::error("invalid_request");
    }
    if bytes.len() > MAX_REQUEST_BYTES {
        return Reply::error("request_too_large");
    }
    let request: Request = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(_) => return Reply::error("invalid_request"),
    };
    process_request(request, vault_path, password, data)
}

fn process_request(
    request: Request,
    vault_path: &Path,
    password: &str,
    data: &mut VaultData,
) -> Reply {
    match request {
        Request::Status => Reply::ok(None),
        Request::Store { label, mut secret } => {
            let label = label.trim();
            if label.is_empty()
                || label.len() > MAX_LABEL_BYTES
                || label.chars().any(char::is_control)
            {
                return Reply::error("invalid_label");
            }
            if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
                return Reply::error("invalid_secret");
            }
            if data.entries.contains_key(label) {
                return Reply::error("label_exists");
            }
            data.entries.insert(
                label.to_string(),
                Entry {
                    secret: std::mem::take(&mut *secret),
                    created: crate::today(),
                    note: Some("Stored by an authenticated HOLODRIVE integration".into()),
                },
            );
            if vault::save(vault_path, password, data).is_err() {
                data.entries.remove(label);
                return Reply::error("vault_write_failed");
            }
            Reply::ok(Some(label.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("cybervault-bridge-{}-{name}", std::process::id()))
            .join("vault.cvlt")
    }

    #[test]
    fn status_reports_only_unlocked_state() {
        let reply = process_request(
            Request::Status,
            Path::new("unused"),
            "unused",
            &mut VaultData::default(),
        );
        let json = serde_json::to_value(reply).unwrap();
        assert_eq!(json["api_version"], "v1");
        assert_eq!(json["unlocked"], true);
        assert!(json.get("secret").is_none());
    }

    #[test]
    fn store_encrypts_entry_and_refuses_overwrite() {
        let path = temp_vault("store");
        let password = "bridge-test-master-password";
        vault::save(&path, password, &VaultData::default()).unwrap();
        let mut data = vault::load(&path, password).unwrap();
        let request = || Request::Store {
            label: "Holodrive share: trip".into(),
            secret: Zeroizing::new("https://example.test/s/secret\npassphrase=one-time".into()),
        };
        let saved = process_request(request(), &path, password, &mut data);
        assert!(saved.ok);
        let reopened = vault::load(&path, password).unwrap();
        assert!(reopened.entries["Holodrive share: trip"]
            .secret
            .contains("passphrase=one-time"));
        let duplicate = process_request(request(), &path, password, &mut data);
        assert_eq!(duplicate.error, Some("label_exists"));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_store_payloads_are_rejected_without_saving() {
        let path = temp_vault("invalid");
        let mut data = VaultData::default();
        let empty = process_request(
            Request::Store {
                label: " ".into(),
                secret: Zeroizing::new("secret".into()),
            },
            &path,
            "unused",
            &mut data,
        );
        assert_eq!(empty.error, Some("invalid_label"));
        assert!(!path.exists());
    }
}
