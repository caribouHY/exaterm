use std::borrow::Cow;
use std::collections::HashSet;

use russh::keys::{Algorithm, EcdsaCurve, HashAlg};
use serde::Serialize;

use crate::config::{SshAlgorithmSelection, SshConfig};

#[derive(Debug, Clone, Serialize)]
pub struct SshAlgorithmCatalogItem {
    pub name: String,
    pub recommended: bool,
    pub compatibility: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SshAlgorithmCatalog {
    pub kex: Vec<SshAlgorithmCatalogItem>,
    pub host_key: Vec<SshAlgorithmCatalogItem>,
    pub cipher: Vec<SshAlgorithmCatalogItem>,
    pub mac: Vec<SshAlgorithmCatalogItem>,
    pub compression: Vec<SshAlgorithmCatalogItem>,
}

fn catalog_item<T: AsRef<str> + PartialEq>(
    algorithm: &T,
    recommended: &[T],
    compatibility: bool,
) -> SshAlgorithmCatalogItem {
    SshAlgorithmCatalogItem {
        name: algorithm.as_ref().to_string(),
        recommended: recommended.contains(algorithm),
        compatibility,
    }
}

fn kex_catalog() -> Vec<(russh::kex::Name, bool)> {
    vec![
        (russh::kex::MLKEM768X25519_SHA256, false),
        (russh::kex::CURVE25519, false),
        (russh::kex::CURVE25519_PRE_RFC_8731, false),
        (russh::kex::DH_GEX_SHA256, false),
        (russh::kex::DH_G18_SHA512, false),
        (russh::kex::DH_G17_SHA512, false),
        (russh::kex::DH_G16_SHA512, false),
        (russh::kex::DH_G15_SHA512, false),
        (russh::kex::DH_G14_SHA256, false),
        (russh::kex::ECDH_SHA2_NISTP521, false),
        (russh::kex::ECDH_SHA2_NISTP384, false),
        (russh::kex::ECDH_SHA2_NISTP256, false),
        (russh::kex::DH_GEX_SHA1, true),
        (russh::kex::DH_G14_SHA1, true),
        (russh::kex::DH_G1_SHA1, true),
    ]
}

fn host_key_catalog() -> Vec<(Algorithm, bool)> {
    vec![
        (Algorithm::Ed25519, false),
        (
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP256,
            },
            false,
        ),
        (
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP384,
            },
            false,
        ),
        (
            Algorithm::Ecdsa {
                curve: EcdsaCurve::NistP521,
            },
            false,
        ),
        (
            Algorithm::Rsa {
                hash: Some(HashAlg::Sha512),
            },
            false,
        ),
        (
            Algorithm::Rsa {
                hash: Some(HashAlg::Sha256),
            },
            false,
        ),
        (Algorithm::SkEd25519, false),
        (Algorithm::SkEcdsaSha2NistP256, false),
        (Algorithm::Rsa { hash: None }, true),
        (Algorithm::Dsa, true),
    ]
}

fn cipher_catalog() -> Vec<(russh::cipher::Name, bool)> {
    vec![
        (russh::cipher::CHACHA20_POLY1305, false),
        (russh::cipher::AES_256_GCM, false),
        (russh::cipher::AES_128_GCM, false),
        (russh::cipher::AES_256_CTR, false),
        (russh::cipher::AES_192_CTR, false),
        (russh::cipher::AES_128_CTR, false),
        (russh::cipher::AES_256_CBC, true),
        (russh::cipher::AES_192_CBC, true),
        (russh::cipher::AES_128_CBC, true),
        (russh::cipher::TRIPLE_DES_CBC, true),
    ]
}

fn mac_catalog() -> Vec<(russh::mac::Name, bool)> {
    vec![
        (russh::mac::HMAC_SHA512_ETM, false),
        (russh::mac::HMAC_SHA256_ETM, false),
        (russh::mac::HMAC_SHA512, false),
        (russh::mac::HMAC_SHA256, false),
        (russh::mac::HMAC_SHA1_ETM, true),
        (russh::mac::HMAC_SHA1, true),
    ]
}

fn compression_catalog() -> Vec<(russh::compression::Name, bool)> {
    vec![
        (russh::compression::NONE, false),
        (russh::compression::ZLIB, false),
        (russh::compression::ZLIB_LEGACY, false),
    ]
}

pub fn algorithm_catalog() -> SshAlgorithmCatalog {
    let preferred = russh::Preferred::default();
    SshAlgorithmCatalog {
        kex: kex_catalog()
            .iter()
            .map(|(algorithm, compatibility)| {
                catalog_item(algorithm, preferred.kex.as_ref(), *compatibility)
            })
            .collect(),
        host_key: host_key_catalog()
            .iter()
            .map(|(algorithm, compatibility)| {
                catalog_item(algorithm, preferred.key.as_ref(), *compatibility)
            })
            .collect(),
        cipher: cipher_catalog()
            .iter()
            .map(|(algorithm, compatibility)| {
                catalog_item(algorithm, preferred.cipher.as_ref(), *compatibility)
            })
            .collect(),
        mac: mac_catalog()
            .iter()
            .map(|(algorithm, compatibility)| {
                catalog_item(algorithm, preferred.mac.as_ref(), *compatibility)
            })
            .collect(),
        compression: compression_catalog()
            .iter()
            .map(|(algorithm, compatibility)| {
                catalog_item(algorithm, preferred.compression.as_ref(), *compatibility)
            })
            .collect(),
    }
}

fn recommended_selection() -> SshAlgorithmSelection {
    let catalog = algorithm_catalog();
    SshAlgorithmSelection {
        kex: catalog
            .kex
            .into_iter()
            .filter(|item| item.recommended)
            .map(|item| item.name)
            .collect(),
        host_key: catalog
            .host_key
            .into_iter()
            .filter(|item| item.recommended)
            .map(|item| item.name)
            .collect(),
        cipher: catalog
            .cipher
            .into_iter()
            .filter(|item| item.recommended)
            .map(|item| item.name)
            .collect(),
        mac: catalog
            .mac
            .into_iter()
            .filter(|item| item.recommended)
            .map(|item| item.name)
            .collect(),
        compression: catalog
            .compression
            .into_iter()
            .filter(|item| item.recommended)
            .map(|item| item.name)
            .collect(),
    }
}

pub fn legacy_algorithm_selection() -> SshAlgorithmSelection {
    let mut selection = recommended_selection();
    for name in [
        russh::kex::DH_G1_SHA1.as_ref(),
        russh::kex::DH_G14_SHA1.as_ref(),
    ] {
        selection.kex.push(name.to_string());
    }
    for name in [
        russh::cipher::AES_128_CBC.as_ref(),
        russh::cipher::AES_192_CBC.as_ref(),
        russh::cipher::AES_256_CBC.as_ref(),
        russh::cipher::TRIPLE_DES_CBC.as_ref(),
    ] {
        selection.cipher.push(name.to_string());
    }
    for name in [
        russh::mac::HMAC_SHA1.as_ref(),
        russh::mac::HMAC_SHA1_ETM.as_ref(),
    ] {
        selection.mac.push(name.to_string());
    }
    selection
}

fn validate_group(
    group: &str,
    selected: &[String],
    supported: impl Iterator<Item = String>,
) -> Result<(), String> {
    if selected.is_empty() {
        return Err(format!(
            "SSH algorithm selection for {group} must not be empty"
        ));
    }

    let supported = supported.collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for name in selected {
        if !supported.contains(name) {
            return Err(format!("Unsupported SSH {group} algorithm: {name}"));
        }
        if !seen.insert(name) {
            return Err(format!("Duplicate SSH {group} algorithm: {name}"));
        }
    }
    Ok(())
}

pub fn validate_algorithm_config(ssh_config: &SshConfig) -> Result<(), String> {
    match ssh_config.algorithm_mode.as_str() {
        "default" => return Ok(()),
        "custom" => {}
        mode => return Err(format!("Unsupported SSH algorithm mode: {mode}")),
    }

    let catalog = algorithm_catalog();
    validate_group(
        "key exchange",
        &ssh_config.algorithms.kex,
        catalog.kex.into_iter().map(|item| item.name),
    )?;
    validate_group(
        "host key",
        &ssh_config.algorithms.host_key,
        catalog.host_key.into_iter().map(|item| item.name),
    )?;
    validate_group(
        "cipher",
        &ssh_config.algorithms.cipher,
        catalog.cipher.into_iter().map(|item| item.name),
    )?;
    validate_group(
        "MAC",
        &ssh_config.algorithms.mac,
        catalog.mac.into_iter().map(|item| item.name),
    )?;
    validate_group(
        "compression",
        &ssh_config.algorithms.compression,
        catalog.compression.into_iter().map(|item| item.name),
    )
}

fn selected_in_catalog<T: Clone + AsRef<str>>(
    catalog: Vec<(T, bool)>,
    selected: &[String],
) -> Vec<T> {
    let selected = selected.iter().map(String::as_str).collect::<HashSet<_>>();
    catalog
        .into_iter()
        .map(|(algorithm, _)| algorithm)
        .filter(|algorithm| selected.contains(algorithm.as_ref()))
        .collect()
}

pub(super) fn build_client_config(ssh_config: &SshConfig) -> Result<russh::client::Config, String> {
    validate_algorithm_config(ssh_config)?;
    let mut config = russh::client::Config::default();
    if ssh_config.algorithm_mode == "default" {
        return Ok(config);
    }

    let default_preferred = russh::Preferred::default();
    let mut kex = selected_in_catalog(kex_catalog(), &ssh_config.algorithms.kex);
    kex.extend(
        default_preferred
            .kex
            .iter()
            .filter(|algorithm| {
                !russh::kex::ALL_KEX_ALGORITHMS
                    .iter()
                    .any(|candidate| candidate.as_ref() == algorithm.as_ref())
            })
            .copied(),
    );

    config.preferred = russh::Preferred {
        kex: Cow::Owned(kex),
        key: Cow::Owned(selected_in_catalog(
            host_key_catalog(),
            &ssh_config.algorithms.host_key,
        )),
        cipher: Cow::Owned(selected_in_catalog(
            cipher_catalog(),
            &ssh_config.algorithms.cipher,
        )),
        mac: Cow::Owned(selected_in_catalog(
            mac_catalog(),
            &ssh_config.algorithms.mac,
        )),
        compression: Cow::Owned(selected_in_catalog(
            compression_catalog(),
            &ssh_config.algorithms.compression,
        )),
    };
    Ok(config)
}
