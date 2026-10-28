//! Creates disposable files only; does not modify a trust store or print private material.
use std::{
    error::Error,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DnType, ExtendedKeyUsagePurpose,
    GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose, NameConstraints,
};
use time::{Duration, OffsetDateTime};

#[derive(Clone, Copy)]
enum Profile {
    Transport,
    Cases,
    Database,
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn leaf(
    directory: &Path,
    root: &[u8],
    issuer: &Issuer<'_, KeyPair>,
    name: &str,
    expired: bool,
    profile: Profile,
) -> Result<(), Box<dyn Error>> {
    let now = OffsetDateTime::now_utc();
    let mut params = CertificateParams::new(vec![name.to_owned()])?;
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    if matches!(profile, Profile::Database) {
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    }
    params.not_before = now - Duration::minutes(5);
    params.not_after = now + Duration::hours(12);
    if expired {
        params.not_before = rcgen::date_time_ymd(2019, 1, 1);
        params.not_after = rcgen::date_time_ymd(2020, 1, 1);
    }
    let key = KeyPair::generate()?;
    let certificate = params.signed_by(&key, issuer)?;
    write_new(&directory.join("root.cer"), root)?;
    write_new(&directory.join("server.cer"), certificate.der())?;
    write_new(&directory.join("server-key.der"), &key.serialize_der())?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(
        args.next()
            .ok_or("expected pre-created protected directory")?,
    );
    let profile = match args.next() {
        None => Profile::Transport,
        Some(flag) if flag == "--profiles" => Profile::Cases,
        Some(flag) if flag == "--database" => Profile::Database,
        Some(_) => return Err("expected optional --profiles or --database".into()),
    };
    if args.next().is_some() || !directory.is_dir() || directory.is_symlink() {
        return Err("expected an existing non-symlink protected directory".into());
    }
    let directory = directory.canonicalize()?;
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkout = manifest
        .parent()
        .and_then(|path| path.parent())
        .ok_or("cannot identify checkout boundary")?
        .canonicalize()?;
    if directory.starts_with(checkout) {
        return Err("test keys and certificates must remain outside the checkout".into());
    }
    // The caller must restrict directory ACLs before invoking this fixture helper.
    let now = OffsetDateTime::now_utc();
    let mut ca = CertificateParams::new(Vec::<String>::new())?;
    ca.distinguished_name.push(
        DnType::CommonName,
        if matches!(profile, Profile::Database) {
            "Disposable local database CA"
        } else {
            "Disposable local transport CA"
        },
    );
    ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    ca.not_before = now - Duration::minutes(5);
    ca.not_after = now + Duration::hours(24);
    ca.name_constraints = Some(NameConstraints {
        permitted_subtrees: if matches!(profile, Profile::Database) {
            vec![GeneralSubtree::DnsName("auth-db".to_owned())]
        } else {
            vec![
                GeneralSubtree::IpAddress(CidrSubnet::V4([127, 0, 0, 1], [255, 255, 255, 255])),
                GeneralSubtree::DnsName("probe.invalid".to_owned()),
            ]
        },
        excluded_subtrees: Vec::new(),
    });
    let ca_key = KeyPair::generate()?;
    let ca_certificate = ca.self_signed(&ca_key)?;
    write_new(&directory.join("root-key.der"), &ca_key.serialize_der())?;
    let issuer = Issuer::new(ca, ca_key);
    leaf(
        &directory,
        ca_certificate.der(),
        &issuer,
        if matches!(profile, Profile::Database) {
            "auth-db"
        } else {
            "127.0.0.1"
        },
        false,
        profile,
    )?;
    if matches!(profile, Profile::Cases) {
        for (profile, name, expired) in [
            ("renewed", "127.0.0.1", false),
            ("wrong-san", "probe.invalid", false),
            ("expired", "127.0.0.1", true),
        ] {
            let nested = directory.join(profile);
            std::fs::create_dir(&nested)?;
            leaf(
                &nested,
                ca_certificate.der(),
                &issuer,
                name,
                expired,
                Profile::Cases,
            )?;
        }
    }
    println!("Prepared disposable certificate fixtures; trust stores unchanged.");
    Ok(())
}
