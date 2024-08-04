//! Hashing and IDs
use hex_fmt::HexFmt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ids
#[allow(dead_code)]
pub fn uuid() -> String {
    let uuid = Uuid::new_v4();
    return uuid.to_string();
}

#[allow(dead_code)]
pub fn hash(input: String) -> String {
    let mut hasher = <Sha256 as Digest>::new();
    hasher.update(input.into_bytes());

    let res = hasher.finalize();
    return HexFmt(res).to_string();
}

#[allow(dead_code)]
pub fn random_id() -> String {
    return hash(uuid());
}
