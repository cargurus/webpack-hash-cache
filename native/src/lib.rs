#![allow(unused_must_use)]
#![warn(clippy::all)]

use neon::prelude::*;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use rayon::prelude::*;

use cache::{CachedEntry, CachedFile};

mod cache;

#[derive(Serialize, Deserialize)]
struct Entries {
    name: String,
    files: HashSet<String>
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

fn get_unchanged_entries(mut cx: FunctionContext) -> JsResult<JsValue> {
    // parse JS args
    let cache_dir: String = cx.argument::<JsString>(0)?.value(&mut cx);
    let cache_dir_path = Path::new(&cache_dir);
    // track entries and changed entries, and return the difference.


    // iterate through all the cached files
    // or return empty array if cache dir doesn't exist
    let (changed_entries, entries) = walk_dir(cache_dir_path).unwrap();

    let unchanged_entries: Vec<String> =
        entries.difference(&changed_entries).cloned().collect();

    Ok(neon_serde3::to_value(&mut cx, &unchanged_entries).unwrap())
}

// so JS can call it asynchronously
fn cache_entries(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let cache_dir = cx.argument::<JsString>(0)?.value(&mut cx);

    let js_entries: Handle<JsValue> = cx.argument(1)?;

    let entries: Vec<Entries> = neon_serde3::from_value(&mut cx, js_entries).unwrap();

    let _f = cx.argument::<JsFunction>(2)?;

    let cached_entries: Vec<CachedEntry> = entries
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
    fs::create_dir_all(&cache_dir)
        .unwrap_or_else(|_| panic!("Error creating cache directory: {}", cache_dir));

    for cached_entry in cached_entries.iter() {
        cached_entry
            .write(&cache_dir)
            .unwrap_or_else(|_| panic!("Error writing cache file for: {}", cached_entry.name));
    }
    Ok(cx.undefined())
}

register_module!(mut m, {
    m.export_function("getUnchangedEntries", get_unchanged_entries);
    m.export_function("cacheEntries", cache_entries);
    Ok(())
});