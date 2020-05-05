use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::fs;
use std::time::SystemTime;

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

    pub fn was_changed(&self) -> bool {
        for cached_file in self.files.iter() {
            if cached_file.was_changed() {
                return true;
            }
        }
        false
    }

    pub fn write(&self, cache_dir: &str) -> std::io::Result<()> {
        let cached_entry_filename = format!("{}.json", calculate_hash(&self.name));
        let cached_entry_path = Path::new(cache_dir).join(cached_entry_filename);
        let f = std::io::BufWriter::new(fs::File::create(&cached_entry_path).unwrap());
        serde_json::to_writer(f, self).unwrap();
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
    pub fn from_filename(filename: &str) -> Option<CachedFile> {
        match fs::metadata(filename) {
            Ok(file_metadata) => {
                let size = file_metadata.len();
                let modified = file_metadata.modified().unwrap();
                // TODO: find async way to read file.
                let hash = calculate_hash(&fs::read(filename).unwrap());
                Some(CachedFile {
                    name: filename.to_string(),
                    size,
                    modified,
                    hash,
                })
            }
            Err(_) => None,
        }
    }

    pub fn was_changed(&self) -> bool {
        match fs::metadata(&self.name) {
            Ok(file_metadata) => {
                let size = file_metadata.len();
                let modified = file_metadata.modified().unwrap();
                if self.size != size || self.modified != modified {
                    let hash = calculate_hash(&fs::read(&self.name).unwrap());
                    if self.hash != hash {
                        return true;
                    }
                }
            }
            // means previously cached file cannot be read
            // most likely due to the entrypoint of an npm module
            // being changed.
            Err(_) => {
                return true;
            }
        }
        false
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = MetroHasher::default();
    t.hash(&mut s);
    s.finish()
}
