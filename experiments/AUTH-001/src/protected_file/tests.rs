use super::*;
fn read_bundle(
    root: BorrowedFd<'_>,
    path: &CStr,
    owner: Owner,
) -> Result<Zeroizing<Vec<u8>>, FileError> {
    read_asset(root, path, owner, Asset::TlsBundle)
}
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::{
        fd::AsFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    root: File,
    owner: Owner,
}

impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        assert!(
            !base.starts_with(checkout),
            "test artifacts must be outside the checkout"
        );
        let path = loop {
            let path = base.join(format!(
                "kehila-reader-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("external fixture directory creation failed"),
            }
        };
        let root = File::open(&path).unwrap();
        let metadata = root.metadata().unwrap();
        Self {
            path,
            root,
            owner: Owner {
                uid: metadata.uid(),
                gid: metadata.gid(),
            },
        }
    }

    fn file(&self, name: &str, bytes: &[u8], mode: u32) {
        let path = self.path.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o400)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.set_permissions(fs::Permissions::from_mode(mode))
            .unwrap();
    }

    fn read(&self, name: &CStr) -> Result<Zeroizing<Vec<u8>>, FileError> {
        read_bundle(self.root.as_fd(), name, self.owner)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The unique 0700 directory is owned by this fixture, outside the checkout.
        if fs::remove_dir_all(&self.path).is_err() && !std::thread::panicking() {
            panic!("owned fixture cleanup failed");
        }
    }
}

#[test]
fn exact_owner_mode_and_bytes_are_required() {
    let f = Fixture::new();
    f.file("bundle", b"fixture", 0o400);
    assert_eq!(f.read(c"bundle").unwrap().as_slice(), b"fixture");
    for mode in [0o440, 0o600, 0o401, 0o4400, 0o2400, 0o1400] {
        fs::set_permissions(f.path.join("bundle"), fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(f.read(c"bundle").err(), Some(FileError::Mode));
    }
    fs::set_permissions(f.path.join("bundle"), fs::Permissions::from_mode(0o400)).unwrap();
    for owner in [
        Owner {
            uid: f.owner.uid ^ 1,
            ..f.owner
        },
        Owner {
            gid: f.owner.gid ^ 1,
            ..f.owner
        },
    ] {
        assert_eq!(
            read_bundle(f.root.as_fd(), c"bundle", owner).err(),
            Some(FileError::Owner)
        );
    }
}

#[test]
fn parent_and_final_symlinks_cannot_substitute_a_file() {
    let f = Fixture::new();
    fs::create_dir(f.path.join("inner")).unwrap();
    f.file("inner/bundle", b"fixture", 0o400);
    std::os::unix::fs::symlink("inner", f.path.join("parent")).unwrap();
    std::os::unix::fs::symlink("inner/bundle", f.path.join("final")).unwrap();
    for path in [c"parent/bundle", c"final"] {
        assert!(matches!(f.read(path), Err(FileError::Open(_))));
    }
    assert_eq!(f.read(c"inner/bundle").unwrap().as_slice(), b"fixture");
}

#[test]
fn existing_outside_paths_and_nonregular_objects_are_rejected() {
    let f = Fixture::new();
    let other = Fixture::new();
    other.file("bundle", b"outside", 0o400);
    let absolute =
        std::ffi::CString::new(other.path.join("bundle").as_os_str().as_encoded_bytes()).unwrap();
    let relative = std::ffi::CString::new(format!(
        "../{}/bundle",
        other.path.file_name().unwrap().to_str().unwrap()
    ))
    .unwrap();
    for path in [absolute.as_c_str(), relative.as_c_str(), c"missing"] {
        assert!(matches!(f.read(path), Err(FileError::Open(_))));
    }
    assert_eq!(f.read(c".").err(), Some(FileError::ObjectType));
}

#[test]
fn fifo_open_has_an_owned_process_deadline() {
    const CHILD: &str = "KEHILA_READER_FIFO_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        let root = File::open(path).unwrap();
        let metadata = root.metadata().unwrap();
        let owner = Owner {
            uid: metadata.uid(),
            gid: metadata.gid(),
        };
        assert_eq!(
            read_bundle(root.as_fd(), c"fifo", owner).err(),
            Some(FileError::ObjectType)
        );
        return;
    }
    // Parent owns cleanup even when a missing NONBLOCK strands the child in open.
    let fixture = Fixture::new();
    rustix::fs::mkfifoat(fixture.root.as_fd(), c"fifo", Mode::RUSR).unwrap();
    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = OwnedChild(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bundle::reader::tests::fifo_open_has_an_owned_process_deadline",
                "--nocapture",
            ])
            .env(CHILD, &fixture.path)
            .spawn()
            .unwrap(),
    );
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "FIFO child failed");
            return;
        }
        assert!(
            std::time::Instant::now() < end,
            "FIFO open exceeded its process deadline"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn opened_descriptor_does_not_follow_a_replaced_path() {
    let f = Fixture::new();
    f.file("bundle", b"original", 0o400);
    let fd = open_checked(f.root.as_fd(), c"bundle", f.owner).unwrap();
    fs::rename(f.path.join("bundle"), f.path.join("old")).unwrap();
    f.file("bundle", b"replacement", 0o400);
    let bytes = read_bounded(Asset::TlsBundle, |output| rustix::io::read(&fd, output)).unwrap();
    assert_eq!(bytes.as_slice(), b"original");
    assert_eq!(f.read(c"bundle").unwrap().as_slice(), b"replacement");
}

#[test]
fn writable_alias_is_not_immutable_generation_evidence() {
    let f = Fixture::new();
    f.file("bundle", b"old", 0o600);
    let mut writer = OpenOptions::new()
        .write(true)
        .open(f.path.join("bundle"))
        .unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(0o400))
        .unwrap();
    let fd = open_checked(f.root.as_fd(), c"bundle", f.owner).unwrap();
    writer.write_all(b"new").unwrap();
    let bytes = read_bounded(Asset::TlsBundle, |output| rustix::io::read(&fd, output)).unwrap();
    assert_eq!(bytes.as_slice(), b"new");
}

#[test]
fn byte_cap_is_measured_from_reads_not_metadata() {
    let f = Fixture::new();
    f.file("exact", &vec![b' '; crate::bundle::MAX_ENCODED], 0o400);
    assert_eq!(f.read(c"exact").unwrap().len(), crate::bundle::MAX_ENCODED);
    f.file(
        "overflow",
        &vec![b' '; crate::bundle::MAX_ENCODED + 1],
        0o400,
    );
    assert_eq!(f.read(c"overflow").err(), Some(FileError::TooLarge));
    f.file("growing", &[], 0o600);
    let mut writer = OpenOptions::new()
        .write(true)
        .open(f.path.join("growing"))
        .unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(0o400))
        .unwrap();
    let fd = open_checked(f.root.as_fd(), c"growing", f.owner).unwrap();
    assert_eq!(fstat(&fd).unwrap().st_size, 0);
    writer
        .write_all(&vec![b' '; crate::bundle::MAX_ENCODED + 1])
        .unwrap();
    assert_eq!(
        read_bounded(Asset::TlsBundle, |output| rustix::io::read(&fd, output)).err(),
        Some(FileError::TooLarge)
    );
}

#[test]
fn short_reads_interruptions_and_read_errors_preserve_the_bound() {
    let mut attempts = 0;
    let mut input = b"fixture".as_slice();
    let bytes = read_bounded(Asset::TlsBundle, |output| {
        attempts += 1;
        if attempts % 2 == 1 {
            return Err(Errno::INTR);
        }
        let take = input.len().min(2);
        output[..take].copy_from_slice(&input[..take]);
        input = &input[take..];
        Ok(take)
    })
    .unwrap();
    assert_eq!(bytes.as_slice(), b"fixture");
    assert_eq!(
        read_bounded(Asset::TlsBundle, |_| Err(Errno::IO)).err(),
        Some(FileError::Read(Errno::IO))
    );
}

#[test]
fn opened_file_descriptors_are_released_after_success_and_rejection() {
    let f = Fixture::new();
    f.file("bundle", b"fixture", 0o400);
    let inode = fs::metadata(f.path.join("bundle")).unwrap();
    let count = || {
        fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| fs::metadata(entry.path()).ok())
            .filter(|m| m.ino() == inode.ino() && m.dev() == inode.dev())
            .count()
    };
    let held = open_checked(f.root.as_fd(), c"bundle", f.owner).unwrap();
    assert_eq!(
        count(),
        1,
        "independent descriptor oracle must observe the held file"
    );
    drop(held);
    assert_eq!(count(), 0);
    assert!(f.read(c"bundle").is_ok());
    assert_eq!(count(), 0);
    fs::set_permissions(f.path.join("bundle"), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(f.read(c"bundle").err(), Some(FileError::Mode));
    assert_eq!(count(), 0);
    assert!(f.root.metadata().is_ok(), "caller root remains owned");
}

#[test]
fn private_composition_preserves_file_decode_and_profile_errors() {
    use crate::bundle::{admit_path, BundleDecodeError, BundleLoadError};
    use rustls::pki_types::UnixTime;
    let f = Fixture::new();
    let now = UnixTime::since_unix_epoch(std::time::Duration::from_secs(1_767_312_000));
    assert!(matches!(
        admit_path(f.root.as_fd(), c"missing", f.owner, now),
        Err(BundleLoadError::File(_))
    ));
    f.file("empty", b"", 0o400);
    assert_eq!(
        admit_path(f.root.as_fd(), c"empty", f.owner, now).err(),
        Some(BundleLoadError::Decode(BundleDecodeError::Framing))
    );
    let text = b"-----BEGIN CERTIFICATE-----\nTQ==\n-----END CERTIFICATE-----\n-----BEGIN PRIVATE KEY-----\nTQ==\n-----END PRIVATE KEY-----";
    f.file("der", text, 0o400);
    assert!(matches!(
        admit_path(f.root.as_fd(), c"der", f.owner, now),
        Err(BundleLoadError::Profile(_))
    ));
    let mut params = rcgen::CertificateParams::new(vec!["127.0.0.1".to_owned()]).unwrap();
    params.is_ca = rcgen::IsCa::ExplicitNoCa;
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    params.not_after = rcgen::date_time_ymd(2026, 1, 3);
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    fn base64_lines(input: &[u8]) -> Zeroizing<Vec<u8>> {
        let codec = data_encoding::BASE64;
        let mut output = Zeroizing::new(vec![0; codec.encode_len(input.len())]);
        codec.encode_mut(input, &mut output);
        let mut lines = Zeroizing::new(Vec::new());
        for chunk in output.chunks(64) {
            lines.extend_from_slice(chunk);
            lines.push(b'\n');
        }
        lines
    }
    let key_der = Zeroizing::new(key.serialize_der());
    let key_text = base64_lines(&key_der);
    let mut encoded = Zeroizing::new(b"-----BEGIN CERTIFICATE-----\n".to_vec());
    encoded.extend_from_slice(&base64_lines(cert.der()));
    encoded.extend_from_slice(b"-----END CERTIFICATE-----\n-----BEGIN PRIVATE KEY-----\n");
    encoded.extend_from_slice(&key_text);
    encoded.extend_from_slice(b"-----END PRIVATE KEY-----\n");
    f.file("valid", &encoded, 0o400);
    assert!(admit_path(f.root.as_fd(), c"valid", f.owner, now).is_ok());
    encoded.resize(crate::bundle::MAX_ENCODED, b' ');
    f.file("exact", &encoded, 0o400);
    assert!(admit_path(f.root.as_fd(), c"exact", f.owner, now).is_ok());
    encoded.push(b' ');
    f.file("overflow", &encoded, 0o400);
    assert!(matches!(
        admit_path(f.root.as_fd(), c"overflow", f.owner, now),
        Err(BundleLoadError::File(FileError::TooLarge))
    ));
}

#[test]
fn startup_asset_caps_do_not_expand_the_tls_bundle_boundary() {
    let f = Fixture::new();
    let size = crate::password::blocklist::ENCODED_LEN;
    assert!(size > crate::bundle::MAX_ENCODED);
    f.file("corpus", &vec![0; size], 0o400);
    assert_eq!(
        read_asset(f.root.as_fd(), c"corpus", f.owner, Asset::PasswordBlocklist)
            .unwrap()
            .len(),
        size
    );
    assert_eq!(
        read_asset(f.root.as_fd(), c"corpus", f.owner, Asset::TlsBundle).err(),
        Some(FileError::TooLarge)
    );
    assert_eq!(
        read_asset(f.root.as_fd(), c"corpus", f.owner, Asset::DatabaseRoot).err(),
        Some(FileError::TooLarge)
    );
    f.file("config", &vec![b' '; 4096], 0o400);
    assert_eq!(
        read_asset(f.root.as_fd(), c"config", f.owner, Asset::Configuration)
            .unwrap()
            .len(),
        4096
    );
    f.file("oversized-config", &vec![b' '; 4097], 0o400);
    assert_eq!(
        read_asset(
            f.root.as_fd(),
            c"oversized-config",
            f.owner,
            Asset::Configuration
        )
        .err(),
        Some(FileError::TooLarge)
    );
}
