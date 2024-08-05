use std::io::Result;
use xsu_util::fs;

use walkdir::WalkDir;
use globset::{Glob, GlobSetBuilder};

/// The stage is where files that we want to change but haven't yet committed are referenced
#[derive(Debug, Clone)]
pub struct Stage(pub String);

impl Stage {
    /// Make sure the stagefile exists
    pub fn init(&self) -> Result<()> {
        if let Err(_) = fs::read(&self.0) {
            fs::touch(&self.0)?;
        }

        Ok(())
    }

    /// Get the list of files that are in the stage
    pub fn get_files(&self) -> Result<Vec<String>> {
        match fs::read(&self.0) {
            Ok(r) => {
                let mut out = Vec::new();

                for slice in r.split("\n") {
                    if slice.is_empty() {
                        continue;
                    }

                    out.push(slice.to_string())
                }

                Ok(out)
            }
            Err(e) => Err(e),
        }
    }

    /// Clear the stage
    pub fn clear(&self) -> Result<()> {
        fs::touch(&self.0)
    }

    /// Add to the stage
    pub fn add(&self, file: String) -> Result<()> {
        fs::append(&self.0, format!("\n{file}"))
    }

    /// Add everything that isn't matched by one of the provided globs
    pub fn add_glob(&self, mut ignore_globs: Vec<String>) -> Result<()> {
        let mut builder = GlobSetBuilder::new();

        ignore_globs.push(".git/**/*".to_string()); // ignore .git
        ignore_globs.push(".garden/**/*".to_string()); // ignore .garden

        for glob in ignore_globs {
            builder.add(Glob::new(&glob).unwrap());
        }

        // match
        let mut out = String::new();
        let glob_match = builder.build().unwrap();

        for entry in WalkDir::new(".").into_iter() {
            match entry {
                Ok(p) => {
                    let path = p.path().to_str().unwrap().replace("./", "");

                    if path.is_empty() {
                        continue;
                    }

                    if p.metadata().unwrap().is_dir() {
                        // we cannot do anything with directories
                        continue;
                    }

                    if glob_match.matches(&path).len() != 0 {
                        // any matches to the ignored globs means we need to skip this file
                        continue;
                    }

                    out.push_str(&format!("\n{path}"))
                }
                Err(e) => panic!("{e}"),
            }
        }

        fs::append(&self.0, format!("\n{out}"))
    }
}
