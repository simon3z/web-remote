//! Privilege drop for the `evdev` sink (R7, R8).
//!
//! The `evdev` sink opens `/dev/uinput` as root, registers the device, then
//! this drops the process to the invoking user so the web server runs
//! unprivileged. The uinput fd is inherited across the drop (R5). The
//! `wayland` sink needs none of this.

use nix::unistd::{setresgid, setresuid, Gid, Uid};

/// Who to drop to.
pub enum DropUser {
    /// The invoking user under `sudo` (`SUDO_UID`/`SUDO_GID`).
    Sudo,
    /// A specific name or uid (from `--user`).
    Name(String),
    /// Fallback: `nobody`.
    Nobody,
}

impl DropUser {
    pub fn resolve(name: Option<&String>) -> DropUser {
        match name {
            Some(n) => DropUser::Name(n.clone()),
            None => {
                if std::env::var("SUDO_UID").is_ok() {
                    DropUser::Sudo
                } else {
                    DropUser::Nobody
                }
            }
        }
    }

    /// Resolve to (uid, gid).
    pub fn ids(&self) -> anyhow::Result<(u32, u32)> {
        let uid = self.uid()?;
        let gid = match self {
            DropUser::Sudo => std::env::var("SUDO_GID")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(uid),
            _ => uid,
        };
        Ok((uid, gid))
    }

    fn uid(&self) -> anyhow::Result<u32> {
        match self {
            DropUser::Sudo => std::env::var("SUDO_UID")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| anyhow::anyhow!("SUDO_UID not set")),
            DropUser::Name(n) => {
                if let Ok(uid) = n.parse::<u32>() {
                    Ok(uid)
                } else {
                    let out = std::process::Command::new("id").args(["-u", n]).output()?;
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    s.parse::<u32>()
                        .map_err(|_| anyhow::anyhow!("can't resolve uid for {n:?}"))
                }
            }
            DropUser::Nobody => {
                let out = std::process::Command::new("id")
                    .args(["-u", "nobody"])
                    .output()?;
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                s.parse::<u32>()
                    .map_err(|_| anyhow::anyhow!("can't resolve uid for nobody"))
            }
        }
    }
}

/// Drop the effective + real + saved uid and gid to `uid`/`gid` (R7, R8).
///
/// After this, the process (and the inherited uinput fd) runs as the invoking
/// user. The kernel keeps the fd valid because it was opened while root.
pub fn drop_privileges(uid: u32, gid: u32) -> anyhow::Result<()> {
    setresgid(Gid::from_raw(0), Gid::from_raw(gid), Gid::from_raw(gid))?;
    setresuid(Uid::from_raw(0), Uid::from_raw(uid), Uid::from_raw(uid))?;
    Ok(())
}
