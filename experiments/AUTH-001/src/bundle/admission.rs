//! Private profile checks. No listener, issuer trust, file access or system clock.
use std::{sync::Arc, time::Duration};

use rustls::{
    pki_types::UnixTime,
    sign::{CertifiedKey, SigningKey},
};
use x509_cert::{
    der::{
        asn1::{AnyRef, ObjectIdentifier, SequenceRef, UintRef},
        oid::AssociatedOid,
        Decode, Encode, Reader, SliceReader, Tag, Tagged,
    },
    ext::pkix::{name::GeneralName, BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAltName},
    Certificate, Version,
};

const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
const ECDSA_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
const SERVER_AUTH: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.3.1");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    CertificateDer,
    CertificateProfile,
    Extensions,
    Validity,
    KeyMismatch,
    KeyDer,
    UnsupportedKeyProfile,
    KeyLoad,
    Allocation,
}

/// A profile/key-pair snapshot, not proof of issuer-signature authenticity or trust.
/// TLS clients must independently perform ordinary certificate-path verification.
pub(crate) struct ProfileCheckedGeneration {
    key: CertifiedKey,
    #[cfg(target_os = "linux")]
    not_before: UnixTime,
    not_after: UnixTime,
}

impl ProfileCheckedGeneration {
    #[cfg(target_os = "linux")]
    pub(crate) fn configuration(
        self,
    ) -> Result<(Arc<rustls::ServerConfig>, UnixTime, UnixTime), AdmissionError> {
        struct Fixed(Arc<CertifiedKey>);
        impl std::fmt::Debug for Fixed {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("Fixed admitted certificate generation")
            }
        }
        impl rustls::server::ResolvesServerCert for Fixed {
            fn resolve(&self, _: rustls::server::ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
                Some(self.0.clone())
            }
        }
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| AdmissionError::KeyLoad)?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(Fixed(Arc::new(self.key))));
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok((Arc::new(config), self.not_before, self.not_after))
    }
}

/// Checks the disposable profile and key pair at the supplied time.
/// Success does not authenticate the issuer signature or establish certificate trust;
/// ongoing expiry and resume enforcement remain separate integration requirements.
pub(super) fn admit(
    mut bundle: super::DecodedBundle,
    now: UnixTime,
) -> Result<ProfileCheckedGeneration, AdmissionError> {
    let certificate =
        Certificate::from_der(&bundle.certificate).map_err(|_| AdmissionError::CertificateDer)?;
    check_extension_encodings(
        &bundle.certificate,
        certificate
            .tbs_certificate()
            .extensions()
            .map(|values| values.as_slice()),
    )?;
    let expiry = check_certificate(&certificate, now)?;
    #[cfg(target_os = "linux")]
    let not_before = UnixTime::since_unix_epoch(
        certificate
            .tbs_certificate()
            .validity()
            .not_before
            .to_unix_duration(),
    );
    let signing = load_key(&bundle.private_key)?;
    drop(certificate);
    let mut chain = Vec::new();
    chain
        .try_reserve_exact(1)
        .map_err(|_| AdmissionError::Allocation)?;
    chain.push(rustls::pki_types::CertificateDer::from(std::mem::take(
        &mut *bundle.certificate,
    )));
    let key = CertifiedKey::new(chain, signing);
    require_pair(&key)?;
    Ok(ProfileCheckedGeneration {
        key,
        #[cfg(target_os = "linux")]
        not_before,
        not_after: UnixTime::since_unix_epoch(expiry),
    })
}

fn require_pair(key: &CertifiedKey) -> Result<(), AdmissionError> {
    key.keys_match().map_err(|_| AdmissionError::KeyMismatch)
}

fn require_canonical<T: Encode>(
    value: &T,
    original: &[u8],
    category: AdmissionError,
) -> Result<(), AdmissionError> {
    let size = usize::try_from(value.encoded_len().map_err(|_| category)?).map_err(|_| category)?;
    if size != original.len() {
        return Err(category);
    }
    let mut encoded = super::buffer(size).map_err(|_| AdmissionError::Allocation)?;
    encoded.resize(size, 0);
    if value.encode_to_slice(&mut encoded).map_err(|_| category)? != original {
        return Err(category);
    }
    Ok(())
}

fn check_extension_encodings(
    input: &[u8],
    expected: Option<&[x509_cert::ext::Extension]>,
) -> Result<(), AdmissionError> {
    // Project original wrappers only AFTER typed certificate validation. Other TBS fields
    // remain schema-owned; library readers handle all tags, lengths and wire decoding.
    let root = <&SequenceRef>::from_der(input).map_err(|_| AdmissionError::Extensions)?;
    let mut outer = SliceReader::new(root.as_bytes()).map_err(|_| AdmissionError::Extensions)?;
    let tbs = <&SequenceRef>::decode(&mut outer).map_err(|_| AdmissionError::Extensions)?;
    let mut fields = SliceReader::new(tbs.as_bytes()).map_err(|_| AdmissionError::Extensions)?;
    let tag = Tag::ContextSpecific {
        constructed: true,
        number: x509_cert::der::TagNumber(3),
    };
    let mut found = false;
    while !fields.is_finished() {
        let field = AnyRef::decode(&mut fields).map_err(|_| AdmissionError::Extensions)?;
        if field.tag() != tag {
            continue;
        }
        if found {
            return Err(AdmissionError::Extensions);
        }
        found = true;
        let mut typed = expected.ok_or(AdmissionError::Extensions)?.iter();
        let list =
            <&SequenceRef>::from_der(field.value()).map_err(|_| AdmissionError::Extensions)?;
        let mut entries =
            SliceReader::new(list.as_bytes()).map_err(|_| AdmissionError::Extensions)?;
        while !entries.is_finished() {
            let raw = entries
                .tlv_bytes()
                .map_err(|_| AdmissionError::Extensions)?;
            let value = typed.next().ok_or(AdmissionError::Extensions)?;
            require_canonical(value, raw, AdmissionError::Extensions)?;
        }
        if typed.next().is_some() {
            return Err(AdmissionError::Extensions);
        }
    }
    if found != expected.is_some() {
        return Err(AdmissionError::Extensions);
    }
    Ok(())
}

fn canonical_extension<T>(certificate: &Certificate) -> Result<Option<T>, AdmissionError>
where
    T: for<'a> Decode<'a> + Encode + AssociatedOid,
{
    let tbs = certificate.tbs_certificate();
    let Some((_, value)) = tbs
        .get_extension::<T>()
        .map_err(|_| AdmissionError::Extensions)?
    else {
        return Ok(None);
    };
    let raw = tbs
        .extensions()
        .and_then(|items| items.iter().find(|item| item.extn_id == T::OID))
        .ok_or(AdmissionError::Extensions)?;
    require_canonical(
        &value,
        raw.extn_value.as_bytes(),
        AdmissionError::Extensions,
    )?;
    Ok(Some(value))
}

fn check_signature_shape(bytes: &[u8]) -> Result<(), AdmissionError> {
    // The library bounds the sequence and checks each INTEGER; no scalar or issuer verification.
    let mut reader = SliceReader::new(bytes).map_err(|_| AdmissionError::CertificateDer)?;
    reader
        .sequence(|nested| -> x509_cert::der::Result<()> {
            UintRef::decode(nested)?;
            UintRef::decode(nested)?;
            Ok(())
        })
        .map_err(|_| AdmissionError::CertificateDer)?;
    reader.finish().map_err(|_| AdmissionError::CertificateDer)
}

fn check_certificate(certificate: &Certificate, now: UnixTime) -> Result<Duration, AdmissionError> {
    let tbs = certificate.tbs_certificate();
    let signature = certificate.signature_algorithm();
    if tbs.version() != Version::V3
        || signature != tbs.signature()
        || signature.oid != ECDSA_SHA256
        || signature.parameters.is_some()
    {
        return Err(AdmissionError::CertificateProfile);
    }
    let spki = tbs.subject_public_key_info();
    if !p256_algorithm(
        spki.algorithm.oid,
        spki.algorithm
            .parameters
            .as_ref()
            .and_then(|value| value.decode_as::<ObjectIdentifier>().ok()),
    ) || spki.subject_public_key.as_bytes().is_none()
    {
        return Err(AdmissionError::CertificateProfile);
    }
    check_signature_shape(
        certificate
            .signature()
            .as_bytes()
            .ok_or(AdmissionError::CertificateDer)?,
    )?;

    let extensions = tbs.extensions().map_or(&[][..], |values| values.as_slice());
    let mut ids: Vec<&ObjectIdentifier> = Vec::new();
    ids.try_reserve_exact(extensions.len())
        .map_err(|_| AdmissionError::Allocation)?;
    ids.extend(extensions.iter().map(|extension| &extension.extn_id));
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AdmissionError::Extensions);
    }
    let interpreted = [
        BasicConstraints::OID,
        ExtendedKeyUsage::OID,
        SubjectAltName::OID,
        KeyUsage::OID,
    ];
    for extension in extensions {
        // Unknown noncritical values remain opaque after this single-TLV envelope check.
        AnyRef::from_der(extension.extn_value.as_bytes())
            .map_err(|_| AdmissionError::Extensions)?;
        if extension.critical && !interpreted.contains(&extension.extn_id) {
            return Err(AdmissionError::Extensions);
        }
    }
    let basic = canonical_extension::<BasicConstraints>(certificate)?
        .ok_or(AdmissionError::CertificateProfile)?;
    if basic.ca || basic.path_len_constraint.is_some() {
        return Err(AdmissionError::CertificateProfile);
    }
    let (_, usage) = tbs
        .get_extension::<ExtendedKeyUsage>()
        .map_err(|_| AdmissionError::Extensions)?
        .ok_or(AdmissionError::CertificateProfile)?;
    if !usage.0.contains(&SERVER_AUTH) {
        return Err(AdmissionError::CertificateProfile);
    }
    if let Some(usage) = canonical_extension::<KeyUsage>(certificate)? {
        if !usage.digital_signature() || usage.key_cert_sign() {
            return Err(AdmissionError::CertificateProfile);
        }
    }
    let (_, names) = tbs
        .get_extension::<SubjectAltName>()
        .map_err(|_| AdmissionError::Extensions)?
        .ok_or(AdmissionError::CertificateProfile)?;
    let mut localhost = false;
    for name in names.0 {
        if let GeneralName::IpAddress(address) = name {
            if !matches!(address.as_bytes().len(), 4 | 16) {
                return Err(AdmissionError::CertificateProfile);
            }
            localhost |= address.as_bytes() == [127, 0, 0, 1];
        }
    }
    if !localhost {
        return Err(AdmissionError::CertificateProfile);
    }
    let validity = tbs.validity();
    let start = validity.not_before.to_unix_duration();
    let end = validity.not_after.to_unix_duration();
    let current = Duration::from_secs(now.as_secs());
    // RFC 5280's validity endpoints are inclusive. Runtime expiry/drain remains a separate gate.
    if start > end || current < start || current > end {
        return Err(AdmissionError::Validity);
    }
    Ok(end)
}

fn p256_algorithm(oid: ObjectIdentifier, parameters: Option<ObjectIdentifier>) -> bool {
    oid == EC_PUBLIC_KEY && parameters == Some(P256)
}

fn load_key(input: &[u8]) -> Result<Arc<dyn SigningKey>, AdmissionError> {
    if input.len() > super::MAX_ENCODED * 3 / 4 {
        return Err(AdmissionError::UnsupportedKeyProfile);
    }
    let info = pkcs8::PrivateKeyInfoRef::from_der(input).map_err(|_| AdmissionError::KeyDer)?;
    let parameters = info
        .algorithm
        .parameters
        .and_then(|value| value.decode_as::<ObjectIdentifier>().ok());
    if info.version() != pkcs8::Version::V1 || !p256_algorithm(info.algorithm.oid, parameters) {
        return Err(AdmissionError::UnsupportedKeyProfile);
    }
    let inner = sec1::EcPrivateKey::from_der(info.private_key.as_bytes())
        .map_err(|_| AdmissionError::KeyDer)?;
    if inner.public_key.is_none()
        || inner
            .parameters
            .is_some_and(|value| value != sec1::EcParameters::NamedCurve(P256))
    {
        return Err(AdmissionError::UnsupportedKeyProfile);
    }
    // The library discards metadata; compare its encoding to enforce the accepted profile.
    require_canonical(&info, input, AdmissionError::UnsupportedKeyProfile)?;
    // Borrow the protected input; no application-owned unprotected key copy is needed.
    let key =
        rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(input));
    rustls::crypto::ring::sign::any_ecdsa_type(&key).map_err(|_| AdmissionError::KeyLoad)
}

#[cfg(test)]
mod certificates;

#[cfg(test)]
mod tests {
    use super::*;
    use x509_cert::der::{
        asn1::{AnyRef, BitStringRef, OctetStringRef, SequenceRef},
        Tag,
    };
    use zeroize::Zeroizing;

    fn key() -> Zeroizing<Vec<u8>> {
        Zeroizing::new(rcgen::KeyPair::generate().unwrap().serialize_der())
    }

    fn reject(input: &[u8], expected: AdmissionError) {
        match load_key(input) {
            Err(actual) => assert_eq!(actual, expected),
            Ok(_) => panic!("unsupported key loaded"),
        }
    }

    fn append_field(input: &[u8], field: &[u8]) -> Zeroizing<Vec<u8>> {
        let sequence = <&SequenceRef>::from_der(input).unwrap();
        let mut body = Zeroizing::new(sequence.as_bytes().to_vec());
        body.extend_from_slice(field);
        Zeroizing::new(AnyRef::new(Tag::Sequence, &body).unwrap().to_der().unwrap())
    }

    fn replace_inner(input: &[u8], inner: &[u8]) -> Zeroizing<Vec<u8>> {
        let info = pkcs8::PrivateKeyInfoRef::from_der(input).unwrap();
        Zeroizing::new(
            pkcs8::PrivateKeyInfoRef::new(info.algorithm, OctetStringRef::new(inner).unwrap())
                .to_der()
                .unwrap(),
        )
    }

    #[test]
    fn producer_key_loads_with_public_identity() {
        let signing = load_key(&key()).unwrap();
        assert!(signing.public_key().is_some());
    }

    #[test]
    fn optional_metadata_and_extra_fields_are_unsupported() {
        let input = key();
        for field in [
            &[0xa0, 0][..],       // Valid empty attributes.
            &[0xa0, 1, 0xff][..], // Opaque malformed attribute contents.
            &[0xa2, 2, 5, 0][..], // Additional context-specific field.
        ] {
            reject(
                &append_field(&input, field),
                AdmissionError::UnsupportedKeyProfile,
            );
        }
    }

    #[test]
    fn outer_and_inner_tails_are_not_ignored() {
        let mut input = key();
        input.extend_from_slice(&[5, 0]);
        reject(&input, AdmissionError::KeyDer);
        let input = key();
        let info = pkcs8::PrivateKeyInfoRef::from_der(&input).unwrap();
        let malformed = append_field(info.private_key.as_bytes(), &[5, 0]);
        reject(&replace_inner(&input, &malformed), AdmissionError::KeyDer);
        let unknown = append_field(info.private_key.as_bytes(), &[0xa2, 2, 5, 0]);
        reject(&replace_inner(&input, &unknown), AdmissionError::KeyDer);
        reject(&[], AdmissionError::KeyDer);
    }

    #[test]
    fn size_and_algorithm_parameters_fail_before_provider_loading() {
        reject(
            &vec![0; super::super::MAX_ENCODED * 3 / 4 + 1],
            AdmissionError::UnsupportedKeyProfile,
        );
        let input = key();
        for parameters in [
            None,
            Some(AnyRef::NULL),
            Some(AnyRef::new(Tag::ObjectIdentifier, &[0x80]).unwrap()),
        ] {
            let mut info = pkcs8::PrivateKeyInfoRef::from_der(&input).unwrap();
            info.algorithm.parameters = parameters;
            let encoded = Zeroizing::new(info.to_der().unwrap());
            reject(&encoded, AdmissionError::UnsupportedKeyProfile);
        }
    }

    #[test]
    fn wrong_curve_and_pkcs8_v2_are_unsupported() {
        let other = Zeroizing::new(
            rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P384_SHA384)
                .unwrap()
                .serialize_der(),
        );
        reject(&other, AdmissionError::UnsupportedKeyProfile);
        let input = key();
        let mut info = pkcs8::PrivateKeyInfoRef::from_der(&input).unwrap();
        info.public_key = Some(BitStringRef::from_bytes(&[4; 65]).unwrap());
        let encoded = Zeroizing::new(info.to_der().unwrap());
        reject(&encoded, AdmissionError::UnsupportedKeyProfile);
    }

    #[test]
    fn inner_public_key_and_curve_contract_is_explicit() {
        let input = key();
        let info = pkcs8::PrivateKeyInfoRef::from_der(&input).unwrap();
        let mut inner = sec1::EcPrivateKey::from_der(info.private_key.as_bytes()).unwrap();
        inner.parameters = Some(sec1::EcParameters::NamedCurve(P256));
        let encoded = Zeroizing::new(inner.to_der().unwrap());
        let signing = load_key(&replace_inner(&input, &encoded)).unwrap();
        assert_eq!(signing.public_key(), load_key(&input).unwrap().public_key());

        inner.parameters = Some(sec1::EcParameters::NamedCurve(
            ObjectIdentifier::new_unwrap("1.3.132.0.34"),
        ));
        let encoded = Zeroizing::new(inner.to_der().unwrap());
        reject(
            &replace_inner(&input, &encoded),
            AdmissionError::UnsupportedKeyProfile,
        );
        inner.parameters = None;
        inner.public_key = None;
        let encoded = Zeroizing::new(inner.to_der().unwrap());
        reject(
            &replace_inner(&input, &encoded),
            AdmissionError::UnsupportedKeyProfile,
        );
    }

    #[test]
    fn provider_rejects_inconsistent_secret_and_public_components() {
        let input = key();
        let info = pkcs8::PrivateKeyInfoRef::from_der(&input).unwrap();
        let mut inner = sec1::EcPrivateKey::from_der(info.private_key.as_bytes()).unwrap();
        let mut public = inner.public_key.unwrap().to_vec();
        public[64] ^= 1;
        inner.public_key = Some(&public);
        let encoded = Zeroizing::new(inner.to_der().unwrap());
        reject(&replace_inner(&input, &encoded), AdmissionError::KeyLoad);
    }
}
