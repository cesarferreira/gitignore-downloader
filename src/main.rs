use clap::{ArgAction, Parser};
use directories::ProjectDirs;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TYPES_URL: &str = "https://api.github.com/repos/github/gitignore/contents";
const RAW_BASE_URL: &str = "https://raw.githubusercontent.com/github/gitignore/master/";
const USER_AGENT: &str = concat!("gitignore-downloader/", env!("CARGO_PKG_VERSION"));
const CACHE_FILE: &str = "types.json";

type DynError = Box<dyn std::error::Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Fetch .gitignore templates from github/gitignore")]
struct Cli {
    /// Template type(s) to fetch (e.g. rust, node). If omitted, a fuzzy picker opens.
    #[arg(value_name = "TYPE", num_args = 0..)]
    types: Vec<String>,

    /// List all available template types.
    #[arg(short, long, action = ArgAction::SetTrue)]
    list: bool,

    /// Output path (defaults to .gitignore in the current directory).
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Overwrite the output instead of appending.
    #[arg(long, action = ArgAction::SetTrue)]
    overwrite: bool,

    /// Print the template(s) instead of writing to disk.
    #[arg(long, action = ArgAction::SetTrue)]
    dry_run: bool,

    /// Ignore cached type list and hit the API.
    #[arg(long, action = ArgAction::SetTrue)]
    no_cache: bool,

    /// Cache time-to-live for the type list, in minutes (default: 1 day).
    #[arg(long, default_value_t = 60 * 24, value_name = "MINUTES")]
    cache_ttl_minutes: u64,
}

#[derive(Serialize, Deserialize)]
struct CachedTypes {
    fetched_at: u64,
    types: Vec<String>,
}

impl CachedTypes {
    fn is_fresh(&self, ttl: Duration) -> bool {
        let fetched = UNIX_EPOCH + Duration::from_secs(self.fetched_at);
        fetched.elapsed().map(|age| age <= ttl).unwrap_or(false)
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), DynError> {
    let cli = Cli::parse();
    let client = Client::builder().user_agent(USER_AGENT).build()?;
    let ttl = Duration::from_secs(cli.cache_ttl_minutes * 60);

    if cli.list {
        let types = load_types(&client, cli.no_cache, ttl)?;
        types.iter().for_each(|t| println!("{t}"));
        return Ok(());
    }

    let mut selected = cli.types;
    if selected.is_empty() {
        let available = load_types(&client, cli.no_cache, ttl)?;
        let choice = prompt_for_type(&available)?;
        selected.push(choice);
    }

    let normalized: Vec<String> = selected
        .into_iter()
        .map(normalize_type)
        .collect();

    let templates = fetch_templates(&client, &normalized)?;

    let output_path = cli.output.unwrap_or_else(|| PathBuf::from(".gitignore"));
    write_templates(&output_path, cli.overwrite, cli.dry_run, &templates)?;
    Ok(())
}

fn load_types(client: &Client, no_cache: bool, ttl: Duration) -> Result<Vec<String>, DynError> {
    if !no_cache {
        if let Some(cached) = read_cached_types(ttl)? {
            return Ok(cached);
        }
    }
    let fresh = fetch_types(client)?;
    write_cached_types(&fresh)?;
    Ok(fresh)
}

fn fetch_types(client: &Client) -> Result<Vec<String>, DynError> {
    let res = client.get(TYPES_URL).send()?;
    if res.status() != StatusCode::OK {
        return Err(format!(
            "Failed to fetch types (status {})",
            res.status()
        )
        .into());
    }
    let entries: Vec<RepoEntry> = res.json()?;
    let mut types = Vec::new();
    for entry in entries {
        if entry.name.ends_with(".gitignore") {
            let clean = entry.name.trim_end_matches(".gitignore").to_string();
            if !clean.is_empty() {
                types.push(clean);
            }
        }
    }
    types.sort();
    types.dedup();
    Ok(types)
}

#[derive(Deserialize)]
struct RepoEntry {
    name: String,
    #[serde(rename = "type")]
    _type: Option<String>,
}

fn prompt_for_type(types: &[String]) -> Result<String, DynError> {
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a gitignore template")
        .items(types)
        .default(0)
        .interact()?;
    Ok(types
        .get(selection)
        .cloned()
        .ok_or("Selection out of range")?)
}

#[derive(Debug)]
struct Template {
    name: String,
    content: String,
}

fn fetch_templates(client: &Client, types: &[String]) -> Result<Vec<Template>, DynError> {
    let mut out = Vec::new();
    for t in types {
        if let Some(snippet) = built_in_flag(t) {
            out.push(Template {
                name: t.clone(),
                content: snippet,
            });
            continue;
        }
        let url = format!("{RAW_BASE_URL}{t}.gitignore");
        let res = client.get(&url).send()?;
        if res.status() != StatusCode::OK {
            return Err(format!(
                "Template '{}' not found (status {})",
                t,
                res.status()
            )
            .into());
        }
        let content = res.text()?;
        out.push(Template {
            name: t.clone(),
            content,
        });
    }
    Ok(out)
}

fn write_templates(
    output: &Path,
    overwrite: bool,
    dry_run: bool,
    templates: &[Template],
) -> Result<(), DynError> {
    if dry_run {
        for tpl in templates {
            println!("# --- {} ---", tpl.name);
            print!("{}", tpl.content);
            if !tpl.content.ends_with('\n') {
                println!();
            }
            println!();
        }
        return Ok(());
    }

    if overwrite {
        let mut buffer = String::new();
        for tpl in templates {
            buffer.push_str(&format!("# --- {} ---\n", tpl.name));
            buffer.push_str(&tpl.content);
            if !tpl.content.ends_with('\n') {
                buffer.push('\n');
            }
            buffer.push('\n');
        }
        fs::write(output, buffer)?;
        println!("Wrote templates to {}", output.display());
        return Ok(());
    }

    let existing_content = fs::read_to_string(output).unwrap_or_default();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output)?;
    for tpl in templates {
        if !existing_content.is_empty() && existing_content.contains(&tpl.content) {
            eprintln!("Skipping {} (already present)", tpl.name);
            continue;
        }
        if needs_separator(&file)? {
            file.write_all(b"\n")?;
        }
        file.write_all(format!("# --- {} ---\n", tpl.name).as_bytes())?;
        file.write_all(tpl.content.as_bytes())?;
        if !tpl.content.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        file.write_all(b"\n")?;
        file.flush()?;
        println!("Appended {}", tpl.name);
    }
    Ok(())
}

fn needs_separator(file: &File) -> Result<bool, io::Error> {
    let meta = file.metadata()?;
    Ok(meta.len() > 0)
}

fn normalize_type(input: String) -> String {
    if input.starts_with("--") {
        return input;
    }
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => input,
    }
}

fn built_in_flag(flag: &str) -> Option<String> {
    match flag {
        "--macos" => Some("# Desktop Service Store Mac\n.DS_Store\n".to_string()),
        "--locks" => Some("# Lock Files\npackage-lock.json\nyarn.lock\n".to_string()),
        _ => None,
    }
}

fn read_cached_types(ttl: Duration) -> Result<Option<Vec<String>>, DynError> {
    let path = cache_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)?;
    let cached: CachedTypes = serde_json::from_str(&contents)?;
    if cached.is_fresh(ttl) {
        Ok(Some(cached.types))
    } else {
        Ok(None)
    }
}

fn write_cached_types(types: &[String]) -> Result<(), DynError> {
    let path = cache_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let cached = CachedTypes {
        fetched_at: now,
        types: types.to_vec(),
    };
    let serialized = serde_json::to_string(&cached)?;
    fs::write(path, serialized)?;
    Ok(())
}

fn cache_file_path() -> Result<PathBuf, DynError> {
    let proj = ProjectDirs::from("dev", "gitignore-downloader", "gitignore-downloader")
        .ok_or("Cannot determine cache directory")?;
    Ok(proj.cache_dir().join(CACHE_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    #[test]
    fn normalizes_simple_type() {
        assert_eq!(normalize_type("rust".into()), "Rust");
        assert_eq!(normalize_type("Rust".into()), "Rust");
    }

    #[test]
    fn preserves_flags() {
        assert_eq!(normalize_type("--macos".into()), "--macos");
        assert!(built_in_flag("--macos").is_some());
        assert!(built_in_flag("--locks").is_some());
        assert!(built_in_flag("--nope").is_none());
    }

    #[test]
    fn cache_staleness_checks() {
        let cached = CachedTypes {
            fetched_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            types: vec![],
        };
        assert!(cached.is_fresh(Duration::from_secs(10)));

        let stale = CachedTypes {
            fetched_at: 0,
            types: vec![],
        };
        assert!(!stale.is_fresh(Duration::from_secs(1)));
    }

    #[test]
    fn write_templates_overwrites_file() {
        let path = temp_path("overwrite");
        fs::write(&path, "old content").unwrap();

        let templates = vec![
            Template {
                name: "Rust".to_string(),
                content: "target/\n".to_string(),
            },
            Template {
                name: "Node".to_string(),
                content: "node_modules/\n".to_string(),
            },
        ];

        write_templates(&path, true, false, &templates).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let expected = "\
# --- Rust ---\n\
target/\n\n\
# --- Node ---\n\
node_modules/\n\n";
        assert_eq!(written, expected);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn write_templates_appends_and_skips_duplicates() {
        let path = temp_path("append");
        fs::write(&path, "Existing\ntarget/\n").unwrap();

        let templates = vec![
            Template {
                name: "Rust".to_string(),
                content: "target/\n".to_string(),
            },
            Template {
                name: "Node".to_string(),
                content: "node_modules/\n".to_string(),
            },
        ];

        write_templates(&path, false, false, &templates).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let expected = "Existing\ntarget/\n\n# --- Node ---\nnode_modules/\n\n";
        assert_eq!(written, expected);

        let _ = fs::remove_file(&path);
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gitignore-downloader-{name}-{unique}"))
    }
}
