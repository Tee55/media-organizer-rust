use chrono::Utc;
use regex::Regex;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

/// Slugifies a given string: removes non-alphanumeric characters, trims, replaces spaces with hyphens, and normalizes hyphens.
fn custom_slugify(input: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\s]").unwrap();
    let slug = re
        .replace_all(input, "")
        .to_lowercase()
        .trim()
        .replace(" ", "-");
    let re_hyphens = Regex::new(r"-{2,}").unwrap();
    re_hyphens
        .replace_all(slug.trim_matches('-'), "-")
        .to_string()
}

/// Cleans a given name string by removing unwanted patterns, slugifying, and handling edge cases.
fn sanitize_filename(mut name: String) -> String {
    name = name.trim().to_string();

    // Remove patterns inside [...], (...), {...}
    for pattern in [r"\[.*?\]", r"\(.*?\)", r"\{.*?\}"] {
        let re = Regex::new(pattern).unwrap();
        name = re.replace_all(&name, "").to_string();
    }

    let mut cleaned_name = custom_slugify(&name);

    // Append datetime if name is empty
    if cleaned_name.is_empty() {
        cleaned_name = format!("unknown {}", Utc::now().format("%y%m%d-%H%M%S"));
        sleep(Duration::from_secs(1)); // Simulate delay to ensure unique timestamps
    }

    // Remove common end words
    let remove_list = [
        "chapter",
        "chapters",
        "english",
        "digital",
        "fakku",
        "comic",
        "comics",
        "decensored",
        "x3200",
    ];
    for word in remove_list {
        let re = Regex::new(&format!(r"(\s*{}[-]*)+$", word)).unwrap();
        cleaned_name = re.replace_all(&cleaned_name, "").to_string();
    }

    // Remove datetime suffix if present
    let datetime_re = Regex::new(r"\d{6}-\d{6}$").unwrap();
    cleaned_name = datetime_re.replace(&cleaned_name, "").to_string();

    // Normalize whitespace
    let re_whitespace = Regex::new(r"\s+").unwrap();
    re_whitespace
        .replace_all(&cleaned_name, " ")
        .trim()
        .to_string()
}

/// Cleans all entries in the given directory path.
pub fn clean(content_path: &Path) -> io::Result<()> {
    for (processed, entry) in fs::read_dir(content_path)?
        .filter_map(Result::ok)
        .enumerate()
    {
        let path = entry.path();
        println!(
            "Progress: {}/{} ({}%) - Processing: {}",
            processed + 1,
            content_path.read_dir()?.count(),
            (processed + 1) * 100 / content_path.read_dir()?.count(),
            path.display()
        );

        if path.is_dir() {
            handle_directory(&path)?;
        } else {
            eprintln!("Error: Not a directory - {}", path.display());
        }
    }
    println!("Cleaning complete.");
    Ok(())
}

/// Handles directory cleaning by checking contents, renaming, and recursively cleaning.
fn handle_directory(path: &Path) -> io::Result<()> {
    if fs::read_dir(path)?.next().is_none() {
        fs::remove_dir(path)?;
        println!("Removed empty folder: {}", path.display());
    } else {
        let old_name = path
            .file_name()
            .unwrap_or(&OsString::new())
            .to_string_lossy()
            .to_string();
        let new_name = sanitize_filename(old_name.clone());

        if new_name != old_name {
            let new_path = path.with_file_name(new_name);
            fs::rename(&path, &new_path)?;
            println!("Renamed folder: {} -> {}", old_name, new_path.display());
            clean(&new_path)?;
        } else {
            clean(path)?;
        }
    }
    Ok(())
}

fn sep_author_name(name: &str) -> (Option<String>, String) {
    let bracket_re = Regex::new(r"\[(.*?)\]").unwrap();
    let paren_re = Regex::new(r"\((.*?)\)").unwrap();

    let author = bracket_re.captures(name).and_then(|caps| {
        let author_text = &caps[1];
        // Check for nested parentheses within brackets
        paren_re.captures(author_text).map_or_else(
            || Some(author_text.to_string()),
            |inner_caps| Some(inner_caps[1].to_string()),
        )
    });

    let item_name = bracket_re.replace(name, "").trim().to_string();
    let cleaned_item_name = sanitize_filename(item_name);

    let cleaned_author = author.map(sanitize_filename);
    (cleaned_author, cleaned_item_name)
}

pub fn main() {
    let example_name = "  [Special](example) {test} title chapter 01 20231128 123456  ".to_string();
    let cleaned_name = sanitize_filename(example_name);
    println!("Cleaned name: {}", cleaned_name);

    // Adjust path accordingly
    if let Err(e) = clean(Path::new("path/to/content")) {
        eprintln!("Error during cleaning: {}", e);
    }
}
