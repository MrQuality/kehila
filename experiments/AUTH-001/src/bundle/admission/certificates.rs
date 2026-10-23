use super::*;
use std::{net::Ipv4Addr, time::Duration};

use rcgen::{
    CertificateParams, CustomExtension, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use rustls::{pki_types::CertificateDer, SignatureAlgorithm, SignatureScheme};
use x509_cert::{
    der::{
        asn1::{Any, AnyRef, BitString, SequenceRef},
        Tag, Tagged,
    },
    Certificate,
};
use zeroize::Zeroizing;

struct Fixture {
    certificate: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
    now: UnixTime,
}

impl Fixture {
    fn bundle(&self) -> super::super::DecodedBundle {
        super::super::DecodedBundle {
            certificate: Zeroizing::new(self.certificate.clone()),
            private_key: Zeroizing::new(self.private_key.to_vec()),
        }
    }

    fn reject(&self, expected: AdmissionError) {
        match admit(self.bundle(), self.now) {
            Err(actual) => assert_eq!(actual, expected),
            Ok(_) => panic!("invalid profile admitted"),
        }
    }
}

fn fixture(change: impl FnOnce(&mut CertificateParams)) -> Fixture {
    fixture_algorithms(
        &rcgen::PKCS_ECDSA_P256_SHA256,
        &rcgen::PKCS_ECDSA_P256_SHA256,
        change,
    )
}

fn fixture_algorithms(
    issuer_algorithm: &'static rcgen::SignatureAlgorithm,
    leaf_algorithm: &'static rcgen::SignatureAlgorithm,
    change: impl FnOnce(&mut CertificateParams),
) -> Fixture {
    let mut issuer = CertificateParams::new(Vec::<String>::new()).unwrap();
    issuer.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let issuer = Issuer::new(issuer, KeyPair::generate_for(issuer_algorithm).unwrap());
    let mut leaf = CertificateParams::new(Vec::<String>::new()).unwrap();
    leaf.is_ca = IsCa::ExplicitNoCa;
    leaf.subject_alt_names = vec![SanType::IpAddress(Ipv4Addr::LOCALHOST.into())];
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf.not_before = rcgen::date_time_ymd(2026, 1, 1);
    leaf.not_after = rcgen::date_time_ymd(2026, 1, 3);
    change(&mut leaf);
    let key = KeyPair::generate_for(leaf_algorithm).unwrap();
    let cert = leaf.signed_by(&key, &issuer).unwrap();
    Fixture {
        certificate: cert.der().to_vec(),
        private_key: Zeroizing::new(key.serialize_der()),
        now: UnixTime::since_unix_epoch(Duration::from_secs(1_767_312_000)), // 2026-01-02 UTC.
    }
}

fn custom(oid: &[u64], value: &[u8], critical: bool) -> CustomExtension {
    let mut extension = CustomExtension::from_oid_content(oid, value.to_vec());
    extension.set_criticality(critical);
    extension
}

fn sequence(contents: &[u8]) -> Vec<u8> {
    AnyRef::new(Tag::Sequence, contents)
        .unwrap()
        .to_der()
        .unwrap()
}

fn insert_explicit_critical_false(f: &mut Fixture) {
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let tbs = cert.tbs_certificate().to_der().unwrap();
    let body = AnyRef::from_der(&tbs).unwrap();
    let mut fields = x509_cert::der::SliceReader::new(body.value()).unwrap();
    let mut replaced = Vec::new();
    while !fields.is_finished() {
        let raw = fields.tlv_bytes().unwrap();
        let field = AnyRef::from_der(raw).unwrap();
        if field.tag()
            != (Tag::ContextSpecific {
                constructed: true,
                number: x509_cert::der::TagNumber(3),
            })
        {
            replaced.extend_from_slice(raw);
            continue;
        }
        let contents = <&SequenceRef>::from_der(field.value()).unwrap();
        let mut extensions = x509_cert::der::SliceReader::new(contents.as_bytes()).unwrap();
        let mut encoded = Vec::new();
        while !extensions.is_finished() {
            let raw = extensions.tlv_bytes().unwrap();
            let extension = x509_cert::ext::Extension::from_der(raw).unwrap();
            if extension.extn_id == ObjectIdentifier::new_unwrap("1.2.3.4") {
                let mut value = extension.extn_id.to_der().unwrap();
                value.extend_from_slice(&false.to_der().unwrap());
                value.extend_from_slice(&extension.extn_value.to_der().unwrap());
                encoded.extend_from_slice(&sequence(&value));
            } else {
                encoded.extend_from_slice(raw);
            }
        }
        replaced.extend_from_slice(
            &AnyRef::new(field.tag(), &sequence(&encoded))
                .unwrap()
                .to_der()
                .unwrap(),
        );
    }
    f.certificate = rebuild(
        &cert,
        &sequence(&replaced),
        &cert.signature_algorithm().to_der().unwrap(),
        None,
    );
}

#[test]
fn explicit_basic_constraints_default_is_rejected() {
    fixture(|p| {
        p.is_ca = IsCa::NoCa;
        p.custom_extensions
            .push(custom(&[2, 5, 29, 19], &[0x30, 3, 1, 1, 0], false));
    })
    .reject(AdmissionError::Extensions);
}

#[test]
fn explicit_critical_false_is_rejected() {
    let mut f = fixture(|p| {
        p.custom_extensions
            .push(custom(&[1, 2, 3, 4], &[5, 0], false))
    });
    insert_explicit_critical_false(&mut f);
    f.reject(AdmissionError::Extensions);
}

#[test]
fn raw_extension_projection_must_cover_all_typed_wrappers() {
    let f = fixture(|_| {});
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let extensions = cert.tbs_certificate().extensions().unwrap();
    assert_eq!(
        check_extension_encodings(&f.certificate, None),
        Err(AdmissionError::Extensions)
    );
    assert_eq!(
        check_extension_encodings(&f.certificate, Some(&extensions[..extensions.len() - 1])),
        Err(AdmissionError::Extensions)
    );
    let mut extra = extensions.clone();
    extra.push(extensions[0].clone());
    assert_eq!(
        check_extension_encodings(&f.certificate, Some(&extra)),
        Err(AdmissionError::Extensions)
    );
}

#[test]
fn undefined_key_usage_bits_are_rejected() {
    fixture(|p| {
        p.key_usages.clear();
        p.custom_extensions
            .push(custom(&[2, 5, 29, 15], &[3, 3, 6, 0x80, 0x40], true));
    })
    .reject(AdmissionError::Extensions);
}

#[test]
fn nonminimal_key_usage_bits_are_rejected() {
    fixture(|p| {
        p.key_usages.clear();
        p.custom_extensions
            .push(custom(&[2, 5, 29, 15], &[3, 3, 0, 0x80, 0], true));
    })
    .reject(AdmissionError::Extensions);
}

fn rebuild(cert: &Certificate, tbs: &[u8], algorithm: &[u8], signature: Option<&[u8]>) -> Vec<u8> {
    let mut body = tbs.to_vec();
    body.extend_from_slice(algorithm);
    body.extend_from_slice(&match signature {
        Some(bytes) => BitString::from_bytes(bytes).unwrap().to_der().unwrap(),
        None => cert.signature().to_der().unwrap(),
    });
    sequence(&body)
}

#[test]
fn valid_profile_has_a_known_matching_pair_and_expiry() {
    let f = fixture(|_| {});
    let generation = admit(f.bundle(), f.now).unwrap();
    assert!(generation.key.keys_match().is_ok());
    let cert = Certificate::from_der(&f.certificate).unwrap();
    assert_eq!(
        generation.not_after.as_secs(),
        cert.tbs_certificate()
            .validity()
            .not_after
            .to_unix_duration()
            .as_secs()
    );
}

#[test]
fn both_explicit_purpose_markers_are_required() {
    fixture(|p| p.is_ca = IsCa::NoCa).reject(AdmissionError::CertificateProfile);
    fixture(|p| p.extended_key_usages.clear()).reject(AdmissionError::CertificateProfile);
    fixture(|p| p.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth])
        .reject(AdmissionError::CertificateProfile);
    fixture(|p| p.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained))
        .reject(AdmissionError::CertificateProfile);
}

#[test]
fn ca_inconsistent_path_length_and_key_usage_are_rejected() {
    fixture(|p| {
        p.is_ca = IsCa::NoCa;
        p.custom_extensions
            .push(custom(&[2, 5, 29, 19], &[0x30, 3, 2, 1, 0], true));
    })
    .reject(AdmissionError::CertificateProfile);
    fixture(|p| p.key_usages = vec![KeyUsagePurpose::KeyEncipherment])
        .reject(AdmissionError::CertificateProfile);
    fixture(|p| p.key_usages.push(KeyUsagePurpose::KeyCertSign))
        .reject(AdmissionError::CertificateProfile);
    let f = fixture(|p| p.key_usages.clear());
    assert!(admit(f.bundle(), f.now).is_ok());
}

#[test]
fn exact_ip_identity_is_required_without_restricting_extra_names() {
    fixture(|p| p.subject_alt_names = vec![SanType::DnsName("127.0.0.1".try_into().unwrap())])
        .reject(AdmissionError::CertificateProfile);
    fixture(|p| p.subject_alt_names = vec![SanType::IpAddress(Ipv4Addr::new(127, 0, 0, 2).into())])
        .reject(AdmissionError::CertificateProfile);
    fixture(|p| {
        p.subject_alt_names.clear();
        p.custom_extensions.push(custom(
            &[2, 5, 29, 17],
            &[0x30, 5, 0x87, 3, 127, 0, 0],
            false,
        ));
    })
    .reject(AdmissionError::CertificateProfile);
    let f = fixture(|p| {
        p.subject_alt_names
            .push(SanType::DnsName("extra.invalid".try_into().unwrap()));
        p.extended_key_usages
            .push(ExtendedKeyUsagePurpose::ClientAuth);
    });
    assert!(admit(f.bundle(), f.now).is_ok());
}

#[test]
fn unknown_noncritical_is_allowed_but_critical_and_bad_envelopes_fail() {
    let f = fixture(|p| {
        p.custom_extensions
            .push(custom(&[1, 2, 3, 4], &[5, 0], false))
    });
    assert!(admit(f.bundle(), f.now).is_ok());
    fixture(|p| {
        p.custom_extensions
            .push(custom(&[1, 2, 3, 4], &[5, 0], true))
    })
    .reject(AdmissionError::Extensions);
    fixture(|p| {
        p.custom_extensions
            .push(custom(&[1, 2, 3, 4], &[0xff], false))
    })
    .reject(AdmissionError::Extensions);
}

#[test]
fn duplicate_oids_and_malformed_checked_values_are_rejected() {
    fixture(|p| {
        p.custom_extensions.extend([
            custom(&[1, 2, 3, 4], &[5, 0], false),
            custom(&[1, 2, 3, 4], &[5, 0], false),
        ])
    })
    .reject(AdmissionError::Extensions);
    fixture(|p| {
        p.custom_extensions
            .push(custom(&[2, 5, 29, 19], &[0x30, 0], false))
    })
    .reject(AdmissionError::Extensions);
    fixture(|p| {
        p.extended_key_usages.clear();
        p.custom_extensions
            .push(custom(&[2, 5, 29, 37], &[5, 0], false));
    })
    .reject(AdmissionError::Extensions);
}

#[test]
fn outer_certificate_and_tbs_tails_are_rejected() {
    let mut f = fixture(|_| {});
    f.certificate.extend_from_slice(&[5, 0]);
    f.reject(AdmissionError::CertificateDer);
    let mut f = fixture(|_| {});
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let tbs = cert.tbs_certificate().to_der().unwrap();
    let mut body = <&SequenceRef>::from_der(&tbs).unwrap().as_bytes().to_vec();
    body.extend_from_slice(&[5, 0]);
    f.certificate = rebuild(
        &cert,
        &sequence(&body),
        &cert.signature_algorithm().to_der().unwrap(),
        None,
    );
    f.reject(AdmissionError::CertificateDer);
    let mut f = fixture(|_| {});
    let mut body = <&SequenceRef>::from_der(&f.certificate)
        .unwrap()
        .as_bytes()
        .to_vec();
    body.extend_from_slice(&[5, 0]);
    f.certificate = sequence(&body);
    f.reject(AdmissionError::CertificateDer);
}

#[test]
fn ecdsa_signature_requires_exact_der_two_integer_shape() {
    for signature in [
        &[0x30, 0][..],
        &[0x30, 3, 2, 1, 1][..],
        &[0x30, 9, 2, 1, 1, 2, 1, 1, 2, 1, 1][..],
        &[0x30, 6, 2, 1, 0xff, 2, 1, 1][..],
        &[0x30, 7, 2, 2, 0, 1, 2, 1, 1][..],
        &[0x30, 6, 2, 1, 1, 2, 1, 1, 5, 0][..],
        &[0x30, 6, 4, 1, 1, 2, 1, 1][..],
    ] {
        let mut f = fixture(|_| {});
        let cert = Certificate::from_der(&f.certificate).unwrap();
        f.certificate = rebuild(
            &cert,
            &cert.tbs_certificate().to_der().unwrap(),
            &cert.signature_algorithm().to_der().unwrap(),
            Some(signature),
        );
        f.reject(AdmissionError::CertificateDer);
    }
}

#[test]
fn profile_result_does_not_claim_issuer_signature_authenticity() {
    let mut f = fixture(|_| {});
    let cert = Certificate::from_der(&f.certificate).unwrap();
    f.certificate = rebuild(
        &cert,
        &cert.tbs_certificate().to_der().unwrap(),
        &cert.signature_algorithm().to_der().unwrap(),
        Some(&[0x30, 6, 2, 1, 1, 2, 1, 1]),
    );
    assert!(admit(f.bundle(), f.now).is_ok());
}

#[test]
fn curve_and_signature_identifier_contracts_are_enforced() {
    fixture_algorithms(
        &rcgen::PKCS_ECDSA_P256_SHA256,
        &rcgen::PKCS_ECDSA_P384_SHA384,
        |_| {},
    )
    .reject(AdmissionError::CertificateProfile);
    let mut f = fixture_algorithms(
        &rcgen::PKCS_ECDSA_P384_SHA384,
        &rcgen::PKCS_ECDSA_P256_SHA256,
        |_| {},
    );
    f.reject(AdmissionError::CertificateProfile);
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let mut outer = cert.signature_algorithm().clone();
    outer.oid = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
    f.certificate = rebuild(
        &cert,
        &cert.tbs_certificate().to_der().unwrap(),
        &outer.to_der().unwrap(),
        None,
    );
    f.reject(AdmissionError::CertificateProfile); // TBS identifier still SHA-384.
    let mut f = fixture(|_| {});
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let mut outer = cert.signature_algorithm().clone();
    outer.parameters = Some(Any::from_der(&[5, 0]).unwrap());
    f.certificate = rebuild(
        &cert,
        &cert.tbs_certificate().to_der().unwrap(),
        &outer.to_der().unwrap(),
        None,
    );
    f.reject(AdmissionError::CertificateProfile);
}

#[test]
fn validity_endpoints_are_inclusive_and_bad_intervals_fail() {
    let f = fixture(|_| {});
    let cert = Certificate::from_der(&f.certificate).unwrap();
    let start = cert
        .tbs_certificate()
        .validity()
        .not_before
        .to_unix_duration();
    let end = cert
        .tbs_certificate()
        .validity()
        .not_after
        .to_unix_duration();
    for now in [start, end] {
        assert!(admit(f.bundle(), UnixTime::since_unix_epoch(now)).is_ok());
    }
    for now in [start - Duration::from_secs(1), end + Duration::from_secs(1)] {
        match admit(f.bundle(), UnixTime::since_unix_epoch(now)) {
            Err(error) => assert_eq!(error, AdmissionError::Validity),
            Ok(_) => panic!("out-of-validity generation admitted"),
        }
    }
    fixture(|p| p.not_before = rcgen::date_time_ymd(2026, 1, 4)).reject(AdmissionError::Validity);
}

#[test]
fn certificate_key_mismatch_and_unknown_consistency_are_rejected() {
    let mut f = fixture(|_| {});
    f.private_key = Zeroizing::new(KeyPair::generate().unwrap().serialize_der());
    f.reject(AdmissionError::KeyMismatch);
    #[derive(Debug)]
    struct NoPublicKey;
    impl SigningKey for NoPublicKey {
        fn choose_scheme(&self, _: &[SignatureScheme]) -> Option<Box<dyn rustls::sign::Signer>> {
            None
        }
        fn algorithm(&self) -> SignatureAlgorithm {
            SignatureAlgorithm::ECDSA
        }
    }
    let no_public = CertifiedKey::new(
        vec![CertificateDer::from(f.certificate)],
        Arc::new(NoPublicKey),
    );
    assert_eq!(require_pair(&no_public), Err(AdmissionError::KeyMismatch));
}
