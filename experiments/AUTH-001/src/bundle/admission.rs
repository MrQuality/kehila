//! Private profile checks. No listener, issuer trust, file access or system clock.
use std::sync::Arc;

use rustls::sign::SigningKey;
use x509_cert::der::{asn1::ObjectIdentifier, Decode, Encode};

const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");

#[derive(Debug, PartialEq, Eq)]
enum AdmissionError {
    KeyDer,
    UnsupportedKeyProfile,
    KeyLoad,
    Allocation,
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
    // The library discards attributes/additional fields. Equality enforces our accepted profile.
    let size = usize::try_from(
        info.encoded_len()
            .map_err(|_| AdmissionError::UnsupportedKeyProfile)?,
    )
    .map_err(|_| AdmissionError::UnsupportedKeyProfile)?;
    if size != input.len() {
        return Err(AdmissionError::UnsupportedKeyProfile);
    }
    let mut encoded = super::buffer(size).map_err(|_| AdmissionError::Allocation)?;
    encoded.resize(size, 0);
    if info
        .encode_to_slice(&mut encoded)
        .map_err(|_| AdmissionError::UnsupportedKeyProfile)?
        != input
    {
        return Err(AdmissionError::UnsupportedKeyProfile);
    }
    drop(encoded);
    // Borrow the protected input; no application-owned unprotected key copy is needed.
    let key =
        rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(input));
    rustls::crypto::ring::sign::any_ecdsa_type(&key).map_err(|_| AdmissionError::KeyLoad)
}

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
