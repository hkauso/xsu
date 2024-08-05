use std::{fs::File, collections::HashMap, io::prelude::*};
use xsu_util::fs::fstat;

use tar::{Builder, Archive};
use flate2::Compression;
use flate2::write::GzEncoder;

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

    /// Read a [`Pack`] from its hash
    pub fn from_hash(hash: String) -> HashMap<String, String> {
        let mut archive = Archive::new(File::open(format!(".garden/objects/{hash}")).unwrap());
        let mut out = HashMap::new();

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
}
