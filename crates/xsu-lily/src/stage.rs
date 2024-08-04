use std::io::Result;
use xsu_util::fs;

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
}
