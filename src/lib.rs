#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use rayon::prelude::*;

use napi::{Task, Env, JsNumber};
use napi::bindgen_prelude::AsyncTask;

use cache::{CachedEntry, CachedFile};

mod cache;

#[napi(object)]
pub struct Entries {
    pub name: String,
    pub files: Vec<String>,
}

struct AsyncFib {
  cache_dir: String,
  entries: Vec<Entries>,
}

impl Task for AsyncFib {
    type Output = u32;
    type JsValue = JsNumber;
    fn compute(&mut self) -> napi::Result<Self::Output> {
        let cached_entries: Vec<CachedEntry> = self.entries
            .par_iter()
            .map(|entry| {
                let cached_files: HashSet<CachedFile> = entry
                    .files
                    .par_iter()
                    .map(|filename| CachedFile::from_filename(filename))
                    .filter_map(Result::ok)
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
        Ok(0)
      }

      fn resolve(&mut self, env: Env, output: u32) -> napi::Result<Self::JsValue> {
          env.create_uint32(output)
      }
}

fn walk_dir(cache_dir_path: &Path) -> io::Result<(HashSet<String>, HashSet<String>)> {
    // iterate through all the cached files
    // or return empty array if cache dir doesn't exist
    let mut entries: HashSet<String> = HashSet::new();
    let mut changed_entries: HashSet<String> = HashSet::new();
    // iterate through all the cached files
    // or return empty array if cache dir doesn't exist
    if cache_dir_path.is_dir() {
        for entry in fs::read_dir(cache_dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                if let Ok(file) = fs::File::open(&path) {
                    let reader = io::BufReader::new(file);
                    let cached_entry: CachedEntry = serde_json::from_reader(reader)?;
                    entries.insert(cached_entry.name.to_string());
                    if cached_entry.was_changed()? {
                        changed_entries.insert(cached_entry.name.to_string());
                    }
                }
            }
        }
    }
    Ok((changed_entries, entries))
}

#[napi]
fn get_unchanged_entries(cache_dir: String) -> napi::Result<Vec<String>> {
    // parse JS args
    // let cache_dir: String = cx.argument::<JsString>(0)?.value(&mut cx);
    let cache_dir_path = Path::new(&cache_dir);
    // track entries and changed entries, and return the difference.


    // iterate through all the cached files
    // or return empty array if cache dir doesn't exist
    let (changed_entries, entries) = walk_dir(cache_dir_path).unwrap();

    let unchanged_entries: Vec<String> =
        entries.difference(&changed_entries).cloned().collect();

    // Ok(neon_serde3::to_value(&mut cx, &unchanged_entries).unwrap())
    // TODO: return vec of unchanged_entries
    Ok(unchanged_entries)
}

// so JS can call it asynchronously
#[napi]
fn cache_entries(cache_dir: String, entries: Vec<Entries>) -> AsyncTask<AsyncFib> {
    AsyncTask::new(AsyncFib { cache_dir, entries })
}
