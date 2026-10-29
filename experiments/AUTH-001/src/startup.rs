//! Disposable Linux startup: closed operator arguments and protected assets.
use crate::{
    bundle,
    database::Endpoint,
    password::blocklist::Blocklist,
    protected_file::{self, Asset, Owner},
};
use rustix::{
    fd::OwnedFd,
    fs::{fstat, openat2, FileType, Mode, OFlags, ResolveFlags, CWD},
};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use std::{
    fmt,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    os::fd::{AsFd, BorrowedFd},
    sync::Arc,
};
use zeroize::Zeroizing;

const OWNER: Owner = Owner {
    uid: 10001,
    gid: 10001,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StartupError {
    Arguments,
    ProcessIdentity,
    Root,
    File,
    Configuration,
    Certificate,
    Corpus,
    Clock,
}

pub(crate) struct Arguments {
    pub(crate) listen: SocketAddrV4,
    pub(crate) public: SocketAddrV4,
}
impl Arguments {
    pub(crate) fn parse(input: impl IntoIterator<Item = String>) -> Result<Self, StartupError> {
        let mut listen = None;
        let mut origin = None;
        let mut input = input.into_iter();
        while let Some(key) = input.next() {
            let value = input.next().ok_or(StartupError::Arguments)?;
            match key.as_str() {
                "--listen-addr" if listen.is_none() => listen = Some(value),
                "--public-origin" if origin.is_none() => origin = Some(value),
                _ => return Err(StartupError::Arguments),
            }
        }
        let input = listen.ok_or(StartupError::Arguments)?;
        let listen: SocketAddrV4 = input.parse().map_err(|_| StartupError::Arguments)?;
        if listen.to_string() != input
            || listen.port() < 1024
            || !listen.ip().is_private()
            || listen.ip().is_loopback()
            || listen.ip().is_unspecified()
            || listen.ip().is_broadcast()
        {
            return Err(StartupError::Arguments);
        }
        let public = SocketAddrV4::new(Ipv4Addr::LOCALHOST, listen.port());
        if origin.as_deref() != Some(format!("https://{public}").as_str()) {
            return Err(StartupError::Arguments);
        }
        Ok(Self { listen, public })
    }
}

pub(crate) fn secret_root() -> Result<OwnedFd, StartupError> {
    use rustix::process::{getegid, geteuid, getgid, getgroups, getuid};
    if getuid().as_raw() != OWNER.uid
        || geteuid().as_raw() != OWNER.uid
        || getgid().as_raw() != OWNER.gid
        || getegid().as_raw() != OWNER.gid
        || getgroups()
            .map_err(|_| StartupError::ProcessIdentity)?
            .iter()
            .any(|group| group.as_raw() != OWNER.gid)
    {
        return Err(StartupError::ProcessIdentity);
    }
    let root = openat2(
        CWD,
        c"/run/secrets",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|_| StartupError::Root)?;
    let metadata = fstat(&root).map_err(|_| StartupError::Root)?;
    if metadata.st_uid != 0
        || metadata.st_gid != 0
        || metadata.st_mode != (FileType::Directory.as_raw_mode() | 0o755)
    {
        return Err(StartupError::Root);
    }
    Ok(root)
}

fn database_password<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Zeroizing<String>, D::Error> {
    struct Password;
    impl Visitor<'_> for Password {
        type Value = Zeroizing<String>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a fixed-size database secret")
        }
        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.len() != 43 {
                return Err(E::custom("database secret size"));
            }
            Ok(Zeroizing::new(value.to_owned()))
        }
    }
    deserializer.deserialize_str(Password)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    installation: String,
    database_address: String,
    database_name: String,
    #[serde(deserialize_with = "database_password")]
    database_password: Zeroizing<String>,
}

pub(crate) struct Assets {
    pub(crate) endpoint: Endpoint,
    pub(crate) installation: String,
    pub(crate) blocklist: Blocklist,
    pub(crate) tls: Arc<rustls::ServerConfig>,
    pub(crate) not_after: rustls::pki_types::UnixTime,
}

pub(crate) fn utc() -> Result<rustls::pki_types::UnixTime, StartupError> {
    Ok(rustls::pki_types::UnixTime::since_unix_epoch(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| StartupError::Clock)?,
    ))
}

pub(crate) fn load(root: BorrowedFd<'_>) -> Result<Assets, StartupError> {
    load_owned(root, OWNER)
}

pub(crate) fn prepare() -> Result<Assets, StartupError> {
    let root = secret_root()?;
    load(root.as_fd())
}

fn load_owned(root: BorrowedFd<'_>, owner: Owner) -> Result<Assets, StartupError> {
    let read = |path, asset| {
        protected_file::read_asset(root, path, owner, asset).map_err(|_| StartupError::File)
    };
    let encoded = read(c"auth-config.json", Asset::Configuration)?;
    if encoded.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(StartupError::Configuration);
    }
    let configuration: Configuration =
        serde_json::from_slice(&encoded).map_err(|_| StartupError::Configuration)?;
    drop(encoded);
    if configuration.installation.is_empty()
        || configuration.installation.len() > 128
        || configuration.installation.contains('\0')
    {
        return Err(StartupError::Configuration);
    }
    let address: SocketAddr = configuration
        .database_address
        .parse()
        .map_err(|_| StartupError::Configuration)?;
    if address.to_string() != configuration.database_address {
        return Err(StartupError::Configuration);
    }
    let root_certificate = read(c"database-root.der", Asset::DatabaseRoot)?;
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            root_certificate.to_vec(),
        ))
        .map_err(|_| StartupError::Configuration)?;
    let password = Zeroizing::new(configuration.database_password.as_bytes().to_vec());
    let endpoint = Endpoint::new(
        address,
        configuration.database_name,
        password,
        root_certificate.to_vec(),
    )
    .map_err(|_| StartupError::Configuration)?;
    let corpus = read(c"password-blocklist.bin", Asset::PasswordBlocklist)?;
    let blocklist = Blocklist::policy_one(&corpus).map_err(|_| StartupError::Corpus)?;
    drop(corpus);
    let generation = bundle::admit_path(root, c"server-bundle.pem", owner, utc()?)
        .map_err(|_| StartupError::Certificate)?;
    let (tls, not_after) = generation
        .configuration()
        .map_err(|_| StartupError::Certificate)?;
    Ok(Assets {
        endpoint,
        installation: configuration.installation,
        blocklist,
        tls,
        not_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(listen: &str, origin: &str) -> Vec<String> {
        ["--listen-addr", listen, "--public-origin", origin]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
    #[test]
    fn network_arguments_are_exact_canonical_and_single_source() {
        let parsed =
            Arguments::parse(args("10.201.87.2:16291", "https://127.0.0.1:16291")).unwrap();
        assert_eq!(parsed.listen.to_string(), "10.201.87.2:16291");
        assert_eq!(parsed.public.to_string(), "127.0.0.1:16291");
        for (listen, origin) in [
            ("0.0.0.0:16291", "https://127.0.0.1:16291"),
            ("127.0.0.1:16291", "https://127.0.0.1:16291"),
            ("10.201.87.2:016291", "https://127.0.0.1:16291"),
            ("10.201.87.2:16291", "https://127.0.0.1:16291/"),
            ("10.201.87.2:16291", "https://127.0.0.1:16292"),
            ("10.201.87.2:443", "https://127.0.0.1:443"),
        ] {
            assert!(Arguments::parse(args(listen, origin)).is_err());
        }
        let mut duplicate = args("10.201.87.2:16291", "https://127.0.0.1:16291");
        duplicate.extend(["--listen-addr".to_owned(), "10.201.87.2:16291".to_owned()]);
        assert!(Arguments::parse(duplicate).is_err());
        assert!(Arguments::parse(Vec::new()).is_err());
    }

    #[test]
    #[ignore = "requires UID/GID 10001 and explicitly mounted disposable protected startup assets"]
    fn native_startup_assets() {
        let Assets {
            endpoint,
            installation,
            blocklist,
            tls,
            not_after,
        } = prepare().unwrap();
        assert_eq!(installation, "installation");
        assert_eq!(tls.alpn_protocols, vec![b"http/1.1".to_vec()]);
        assert!(not_after > utc().unwrap());
        assert!(blocklist
            .admit(
                crate::password::NormalizedPassword::new(Zeroizing::new(
                    "123456789012345".to_owned()
                ))
                .unwrap()
            )
            .is_err());
        assert!(blocklist
            .admit(
                crate::password::NormalizedPassword::new(Zeroizing::new(
                    "a newly chosen owner password".to_owned()
                ))
                .unwrap()
            )
            .is_ok());
        drop(endpoint);
    }
}
