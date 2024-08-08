use std::collections::BTreeMap;
use std::{fs::File, io::prelude::*};
use xsu_util::fs::fstat;

use tar::{Builder, Archive};
use flate2::Compression;
use flate2::{read::GzDecoder, write::GzEncoder};

/// A pack is a `.tar.gz` file of the entire working tree at the point of a commit
pub struct Pack(pub String);

impl Pack {
    /// Create a new [`Pack`]
    pub fn new(files: Vec<String>, hash: String) -> Self {
        let path = format!(".garden/objects/{hash}");
        let file = File::create(&path).unwrap();

        let enc = GzEncoder::new(file, Compression::default());
        let mut archive = Builder::new(enc);

        for file in files {
            if file.is_empty() {
                continue;
            }

            let stat = fstat(&file).unwrap();

            if stat.is_dir() {
                archive.append_dir_all(&file, &file).unwrap();
            } else {
                archive.append_path(&file).unwrap();
            }
        }

        archive.finish().unwrap(); // finish the pack

        // return the pack
        Self(path)
    }

    /// Create a [`Pack`] of the entire `.garden` directory
    pub async fn from_repo(name: String) -> Self {
        let path = format!("{name}.repo");
        let file = File::create(&path).unwrap();

        let enc = GzEncoder::new(file, Compression::default());
        let mut archive = Builder::new(enc);

        // get files
        archive
            .append_dir_all("objects", ".garden/objects")
            .unwrap();
        // archive.append_dir_all("www", ".garden/www").unwrap();
        archive.append_dir_all("bin", ".garden/bin").unwrap();

        archive.finish().unwrap(); // finish the pack

        // return the pack
        Self(path)
    }

    /// Read a [`Pack`] from its hash
    pub fn from_hash(hash: String) -> BTreeMap<String, String> {
        Pack::from_file(File::open(format!(".garden/objects/{hash}")).unwrap())
    }

    /// Read a [`Pack`] from a [`File`]
    pub fn from_file(file: File) -> BTreeMap<String, String> {
        let mut archive = Archive::new(GzDecoder::new(file));

        let mut out = BTreeMap::new();

        for file in archive.entries().unwrap() {
            let mut file = file.unwrap();

            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();

            out.insert(
                file.path().unwrap().to_str().unwrap().to_string(),
                content.to_string(),
            );
        }

        out
    }

    /// Pack a single input [`String`]
    pub fn from_string(input: String) -> Vec<u8> {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(input.as_bytes()).unwrap();
        enc.finish().unwrap()
    }

    /// Decode a [`Vec<u8>`] into a [`String`]
    pub fn decode_vec(input: Vec<u8>) -> String {
        let mut dec = GzDecoder::new(&input[..]);
        let mut string = String::new();
        dec.read_to_string(&mut string).unwrap();
        string
    }
}
