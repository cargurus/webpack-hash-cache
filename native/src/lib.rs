#![allow(unused_must_use)]
#![warn(clippy::all)]

use neon::prelude::*;

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::BufReader;
use std::path::Path;
use std::time::SystemTime;

use fasthash::MetroHasher;
use jwalk::WalkDir;
use rayon::prelude::*;

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize)]
struct CachedFile {
    name: String,
    size: u64,
    modified: SystemTime,
    hash: u64,
}

impl CachedFile {
    fn from_filename(filename: &str) -> Option<CachedFile> {
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

    fn was_changed(&self) -> bool {
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

#[derive(Serialize, Deserialize)]
struct CachedEntry {
    name: String,
    files: RefCell<HashSet<CachedFile>>,
}

impl CachedEntry {
    fn new(name: String, files: HashSet<CachedFile>) -> CachedEntry {
        CachedEntry {
            name,
            files: RefCell::new(files),
        }
    }

    fn was_changed(&self) -> bool {
        for cached_file in self.files.borrow().iter() {
            if cached_file.was_changed() {
                return true;
            }
        }
        false
    }

    fn write(&self, cache_dir: &str) -> std::io::Result<()> {
        let cached_entry_filename = format!("{}.json", calculate_hash(&self.name));
        let cached_entry_path = Path::new(cache_dir).join(cached_entry_filename);
        let f = std::io::BufWriter::new(fs::File::create(&cached_entry_path).unwrap());
        serde_json::to_writer(f, self).unwrap();
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Entries {
    name: String,
    files: HashSet<String>
}

fn get_unchanged_entries(mut cx: FunctionContext) -> JsResult<JsValue> {
    // parse JS args
    let cache_dir: String = cx.argument::<JsString>(0)?.value();
    let cache_dir_path = Path::new(&cache_dir);
    // track entries and changed entries, and return the difference.
    let mut entries: HashSet<String> = HashSet::new();
    let mut changed_entries: HashSet<String> = HashSet::new();

    // iterate through all the cached files
    // or return empty array if cache dir doesn't exist
    if cache_dir_path.is_dir() {
        // jwalk::{WalkDir} uses rayon to walk the directory in parallel
        for entry in WalkDir::new(cache_dir_path).sort(false) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_dir() {
                if let Ok(file) = fs::File::open(&path) {
                    let reader = BufReader::new(file);
                    let cached_entry: CachedEntry = serde_json::from_reader(reader).unwrap();
                    entries.insert(cached_entry.name.to_string());
                    if cached_entry.was_changed() {
                        changed_entries.insert(cached_entry.name.to_string());
                    }
                }
            }
        }
    } else {
        return Ok(cx.empty_array().upcast());
    }

    let unchanged_entries: Vec<String> =
        entries.difference(&changed_entries).cloned().collect();

    Ok(neon_serde::to_value(&mut cx, &unchanged_entries).unwrap())
}

fn parse_args(cx: &mut FunctionContext) -> (String, Vec<Entries>) {
    let cache_dir = cx.argument::<JsString>(0).unwrap().value();

    let js_entries: Handle<JsValue> = cx.argument(1).unwrap();

    let entries: Vec<Entries> = neon_serde::from_value(cx, js_entries).unwrap();

    (cache_dir, entries)
}

struct BackgroundTask {
    cache_dir: String,
    entries: Vec<Entries>,
}

impl Task for BackgroundTask {
    type Output = ();
    type Error = String;
    type JsEvent = JsUndefined;

    fn perform(&self) -> Result<(), Self::Error> {
        let cached_entries: Vec<CachedEntry> = self
            .entries
            .par_iter()
            .map(|entry| {
                let cached_files: HashSet<CachedFile> = entry
                    .files
                    .par_iter()
                    .filter_map(|filename| CachedFile::from_filename(filename))
                    .collect();
                CachedEntry::new(entry.name.to_string(), cached_files)
            })
            .collect();

        // ensure directory exists.
        fs::create_dir_all(&self.cache_dir)
            .unwrap_or_else(|_| panic!("Error creating cache directory: {}", self.cache_dir));

        for cached_entry in cached_entries.iter() {
            cached_entry
                .write(&self.cache_dir)
                .unwrap_or_else(|_| panic!("Error writing cache file for: {}", cached_entry.name));
        }
        Ok(())
    }
    fn complete(
        self,
        mut cx: TaskContext,
        _result: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsEvent> {
        Ok(cx.undefined())
    }
}

// so JS can call it asynchronously
fn cache_entries(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let (cache_dir, entries) = parse_args(&mut cx);
    let f = cx.argument::<JsFunction>(2)?;
    let background_task = BackgroundTask { cache_dir, entries };
    background_task.schedule(f);
    Ok(cx.undefined())
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = MetroHasher::default();
    t.hash(&mut s);
    s.finish()
}

register_module!(mut m, {
    m.export_function("getUnchangedEntries", get_unchanged_entries);
    m.export_function("cacheEntries", cache_entries);
    Ok(())
});
