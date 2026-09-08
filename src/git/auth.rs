//! Mobile connection validation and explicit SSH host-key trust. Desktop Git
//! keeps its own credential helpers, SSH agent and known_hosts behavior.
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use git2::{Cred, CredentialType, Error};
use zeroize::Zeroizing;

#[derive(Clone)]
pub enum Authentication {
    Https {
        username: String,
        password: Zeroizing<String>,
    },
    Ssh {
        username: String,
        private_key: Zeroizing<String>,
        host: String,
        fingerprints: Vec<[u8; 32]>,
    },
}

pub enum Connection {
    Https,
    Ssh {
        username: String,
        host: String,
        port: u16,
    },
}

pub fn connection(input: &str) -> Result<Connection, Error> {
    let invalid = || {
        Error::from_str(
            "Use an HTTPS URL without credentials or an SSH URL with a username and repository path",
        )
    };
    if input.chars().any(char::is_whitespace) || input.contains('\0') {
        return Err(invalid());
    }
    let expanded;
    let source = if input.contains("://") {
        input
    } else {
        let (host, path) = input.split_once(':').ok_or_else(invalid)?;
        if !host.contains('@') || path.is_empty() || path.starts_with('/') {
            return Err(invalid());
        }
        expanded = format!("ssh://{host}/{path}");
        &expanded
    };
    let url = url::Url::parse(source).map_err(|_| invalid())?;
    if url.host_str().is_none()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path().trim_matches('/').is_empty()
    {
        return Err(invalid());
    }
    match url.scheme() {
        "https" if url.username().is_empty() => Ok(Connection::Https),
        "ssh" if !url.username().is_empty() && !url.username().contains('%') => {
            Ok(Connection::Ssh {
                username: url.username().into(),
                host: url.host_str().unwrap().into(),
                port: url.port().unwrap_or(22),
            })
        }
        _ => Err(invalid()),
    }
}

// Verified against GitHub's documentation, 2026-09-08. Never trust a fetched
// host key automatically. Rotation requires an explicit configuration/update.
// https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints
const GITHUB_FINGERPRINTS: [&str; 3] = [
    "SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s",
    "SHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM",
    "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU",
];

// Verified against both GitLab's documentation and its live instance
// configuration, 2026-09-08. These pins do not apply to self-managed GitLab.
// https://docs.gitlab.com/user/gitlab_com/#ssh-host-keys-fingerprints
// https://gitlab.com/help/instance_configuration
const GITLAB_FINGERPRINTS: [&str; 3] = [
    "SHA256:HbW3g8zUjNSksFbqTiUWPWg2Bq1x8xdGUrliXFzSnUw",
    "SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8",
    "SHA256:ROQFvPThGrW4RuWLoL9tq9I9zJ42fK4XywyRtbOz/EQ",
];

fn fingerprint(value: &str) -> Result<[u8; 32], Error> {
    value.strip_prefix("SHA256:").and_then(|s| STANDARD_NO_PAD.decode(s).ok())
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Error::from_str("SSH server fingerprint must be SHA256 followed by a base64 SHA-256 digest (SHA256:...)"))
}

impl Authentication {
    pub fn ssh(url: &str, private_key: String, server_fingerprint: &str) -> Result<Self, Error> {
        let Connection::Ssh {
            username,
            host,
            port,
        } = connection(url)?
        else {
            return Err(Error::from_str("SSH keys require an SSH repository URL"));
        };
        let private_key = Zeroizing::new(private_key);
        let key = ssh_key::PrivateKey::from_openssh(&*private_key)
            .map_err(|_| Error::from_str("Cannot read the repository's SSH private key"))?;
        if key.algorithm() != ssh_key::Algorithm::Ed25519 || key.is_encrypted() {
            return Err(Error::from_str(
                "An unencrypted Ed25519 key is required in memory",
            ));
        }
        let fingerprints = if !server_fingerprint.trim().is_empty() {
            vec![fingerprint(server_fingerprint.trim())?]
        } else if (host == "github.com" && port == 22) || (host == "ssh.github.com" && port == 443)
        {
            GITHUB_FINGERPRINTS
                .iter()
                .map(|s| fingerprint(s))
                .collect::<Result<_, _>>()?
        } else if (host == "gitlab.com" && port == 22)
            || (host == "altssh.gitlab.com" && port == 443)
        {
            GITLAB_FINGERPRINTS
                .iter()
                .map(|s| fingerprint(s))
                .collect::<Result<_, _>>()?
        } else {
            return Err(Error::from_str(
                "Enter the SSH server's SHA256 fingerprint, verified with its administrator",
            ));
        };
        Ok(Self::Ssh {
            username,
            private_key,
            host,
            fingerprints,
        })
    }

    pub fn credential(&self, allowed: CredentialType) -> Result<Cred, Error> {
        match self {
            Self::Https { username, password }
                if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) =>
            {
                Cred::userpass_plaintext(username, password)
            }
            Self::Ssh {
                username,
                private_key,
                ..
            } if allowed.contains(CredentialType::SSH_KEY) => {
                Cred::ssh_key_from_memory(username, None, private_key, None)
            }
            Self::Https { username, .. } | Self::Ssh { username, .. }
                if allowed.contains(CredentialType::USERNAME) =>
            {
                Cred::username(username)
            }
            _ => Err(Error::from_str(
                "The server did not accept the configured authentication method",
            )),
        }
    }

    pub fn check_host(&self, host: &str, digest: &[u8; 32]) -> Result<(), Error> {
        match self {
            Self::Ssh {
                host: expected,
                fingerprints,
                ..
            } if expected == host && fingerprints.contains(digest) => Ok(()),
            _ => Err(Error::from_str(&format!(
                "SSH server key mismatch for {host} (received SHA256:{}); verify the server before changing its fingerprint",
                STANDARD_NO_PAD.encode(digest)
            ))),
        }
    }
}

pub fn generate_ssh_key() -> Result<serde_json::Value, Error> {
    let key =
        ssh_key::PrivateKey::random(&mut ssh_key::rand_core::OsRng, ssh_key::Algorithm::Ed25519)
            .map_err(|_| {
                Error::from_str(
                    "Cannot generate an SSH key from the operating system's random source",
                )
            })?;
    Ok(serde_json::json!({
        "private_key": key.to_openssh(ssh_key::LineEnding::LF).map_err(|_| Error::from_str("Cannot encode SSH key"))?.as_str(),
        "public_key": key.public_key().to_openssh().map_err(|_| Error::from_str("Cannot encode SSH public key"))?,
        "fingerprint": key.fingerprint(ssh_key::HashAlg::Sha256).to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mobile_urls_reject_credentials_and_unsafe_transports() {
        for url in [
            "https://github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "ssh://git@ssh.github.com:443/owner/repo.git",
        ] {
            assert!(connection(url).is_ok(), "{url}");
        }
        for url in [
            "http://github.com/repo",
            "file:///tmp/repo",
            "https://token@github.com/repo",
            "https://user:token@github.com/repo",
            "ssh://git:password@host/repo",
            "ssh://host/repo",
            "git@host:",
            "git@host:repo\n",
            "https://github.com/repo?token=secret",
        ] {
            assert!(connection(url).is_err(), "{url}");
        }
    }
    #[test]
    fn generated_keys_round_trip_and_host_trust_is_explicit() {
        let key = generate_ssh_key().unwrap();
        let private = key["private_key"].as_str().unwrap();
        let parsed = ssh_key::PrivateKey::from_openssh(private).unwrap();
        assert_eq!(parsed.public_key().to_openssh().unwrap(), key["public_key"]);
        let auth =
            Authentication::ssh("git@github.com:owner/repo.git", private.into(), "").unwrap();
        let good = fingerprint(GITHUB_FINGERPRINTS[2]).unwrap();
        assert!(auth.check_host("github.com", &good).is_ok());
        assert!(auth.check_host("github.com", &[0; 32]).is_err());
        assert!(auth.check_host("another.example", &good).is_err());
        assert!(Authentication::ssh("git@another.example:repo.git", private.into(), "").is_err());
        assert!(
            Authentication::ssh("ssh://git@github.com:2222/repo.git", private.into(), "").is_err()
        );
        let custom = Authentication::ssh(
            "git@another.example:repo.git",
            private.into(),
            GITHUB_FINGERPRINTS[2],
        )
        .unwrap();
        assert!(custom.check_host("another.example", &good).is_ok());
    }

    #[test]
    fn gitlab_pins_are_scoped_to_official_hosts_and_ports() {
        let key = generate_ssh_key().unwrap();
        let private = key["private_key"].as_str().unwrap();
        for (url, host) in [
            ("git@gitlab.com:group/repo.git", "gitlab.com"),
            (
                "ssh://git@altssh.gitlab.com:443/group/repo.git",
                "altssh.gitlab.com",
            ),
        ] {
            let auth = Authentication::ssh(url, private.into(), "").unwrap();
            for pin in GITLAB_FINGERPRINTS {
                assert!(auth.check_host(host, &fingerprint(pin).unwrap()).is_ok());
            }
            assert!(
                auth.check_host(host, &fingerprint(GITHUB_FINGERPRINTS[2]).unwrap())
                    .is_err()
            );
            assert!(
                auth.check_host(
                    "gitlab.example.com",
                    &fingerprint(GITLAB_FINGERPRINTS[1]).unwrap()
                )
                .is_err()
            );
        }
        for url in [
            "git@gitlab.example.com:group/repo.git",
            "ssh://git@gitlab.com:2222/group/repo.git",
            "git@altssh.gitlab.com:group/repo.git",
        ] {
            assert!(Authentication::ssh(url, private.into(), "").is_err());
        }
    }
}
