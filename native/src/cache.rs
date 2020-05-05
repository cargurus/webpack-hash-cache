use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::fs;
use std::time::SystemTime;
use std::io;

use std::path::Path;
use fasthash::MetroHasher;


#[derive(Serialize, Deserialize)]
pub struct CachedEntry {
    pub name: String,
    pub files: HashSet<CachedFile>,
}

impl CachedEntry {
    pub fn new(name: String, files: HashSet<CachedFile>) -> CachedEntry {
        CachedEntry {
            name,
            files,
        }
    }

    pub fn was_changed(&self) -> io::Result<bool> {
        let mut changed = false;
        for cached_file in self.files.iter() {
            if cached_file.was_changed()? {
                changed = true;
            }
        }
        Ok(changed)
    }

    pub fn write(&self, cache_dir: &str) -> io::Result<()> {
        let cached_entry_filename = format!("{}.json", calculate_hash(&self.name));
        let cached_entry_path = Path::new(cache_dir).join(cached_entry_filename);
        let f = std::io::BufWriter::new(fs::File::create(&cached_entry_path)?);
        serde_json::to_writer(f, self)?;
        Ok(())
    }
}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CachedFile {
    name: String,
    size: u64,
    modified: SystemTime,
    hash: u64,
}

impl CachedFile {
    pub fn from_filename(filename: &str) -> io::Result<CachedFile> {
        let file_metadata = fs::metadata(filename)?;
        let size = file_metadata.len();
        let modified = file_metadata.modified()?;
        // TODO: find async way to read file.
        let hash = calculate_hash(&fs::read(filename)?);
        Ok(CachedFile {
            name: filename.to_string(),
            size,
            modified,
            hash,
        })
    }

    pub fn was_changed(&self) -> io::Result<bool> {
        let mut changed = false;
        match fs::metadata(&self.name) {
            Ok(file_metadata) => {
                let size = file_metadata.len();
                let modified = file_metadata.modified()?;
                if self.size != size || self.modified != modified {
                    let hash = calculate_hash(&fs::read(&self.name)?);
                    if self.hash != hash {
                        changed = true;
                    }
                }
            }
            // means previously cached file cannot be read
            // most likely due to the entrypoint of an npm module
            // being changed.
            Err(_) => {
                changed = true;
            }
        }
        Ok(changed)
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = MetroHasher::default();
    t.hash(&mut s);
    s.finish()
}
