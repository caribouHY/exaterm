use std::borrow::Cow;

use russh::keys::Algorithm;

use crate::config::{config_load, SshConfig};

fn append_missing<T: Clone + PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

pub(super) fn build_client_config(ssh_config: &SshConfig) -> russh::client::Config {
    let mut config = russh::client::Config::default();
    if !ssh_config.allow_legacy_algorithms {
        return config;
    }

    let default_preferred = russh::Preferred::default();
    let mut kex = default_preferred.kex.to_vec();
    append_missing(&mut kex, russh::kex::DH_G1_SHA1);
    append_missing(&mut kex, russh::kex::DH_G14_SHA1);

    let mut cipher = default_preferred.cipher.to_vec();
    append_missing(&mut cipher, russh::cipher::AES_128_CBC);
    append_missing(&mut cipher, russh::cipher::AES_192_CBC);
    append_missing(&mut cipher, russh::cipher::AES_256_CBC);
    append_missing(&mut cipher, russh::cipher::TRIPLE_DES_CBC);

    let mut mac = default_preferred.mac.to_vec();
    append_missing(&mut mac, russh::mac::HMAC_SHA1);
    append_missing(&mut mac, russh::mac::HMAC_SHA1_ETM);

    let mut key = default_preferred.key.to_vec();
    append_missing(&mut key, Algorithm::Rsa { hash: None });

    config.preferred = russh::Preferred {
        kex: Cow::Owned(kex),
        key: Cow::Owned(key),
        cipher: Cow::Owned(cipher),
        mac: Cow::Owned(mac),
        compression: default_preferred.compression,
    };
    config
}

pub(super) fn load_client_config() -> Result<russh::client::Config, String> {
    let app_config = config_load()?;
    Ok(build_client_config(&app_config.ssh))
}
