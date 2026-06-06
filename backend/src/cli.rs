//! CLI parsing and standalone provider auth import helpers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const YOUTUBE_COOKIE_HEADER_PATH_ENV: &str = "KAULAN_YOUTUBE_COOKIE_HEADER_PATH";
const NCMDUMP_CONFIG_DIR_ENV: &str = "NCMDUMP_CONFIG_DIR";

/// Parsed command-line options for the backend binary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub music_path: Option<String>,
    pub youtube_cookie_file: Option<PathBuf>,
    pub netease_session_file: Option<PathBuf>,
    pub bilibili_session_file: Option<PathBuf>,
}

impl CliOptions {
    pub fn usage(program: &str) -> String {
        format!(
            "Usage: {program} <run|update> [music_path] [options]\n\
             \n\
             Commands:\n\
               run     Start the web server\n\
               update  Scan for new music files and update database\n\
             \n\
             Options:\n\
               --youtube-cookie-file <path>     Path to YouTube Netscape cookie jar\n\
               --netease-session-file <path>    Path to Netease session.json\n\
               --bilibili-session-file <path>   Path to Bilibili bilibili_session.json\n"
        )
    }
}

/// Parse trailing CLI arguments after the command name.
pub fn parse_cli_options(args: &[String]) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut index = 0;

    while index < args.len() {
        let current = &args[index];
        match current.as_str() {
            "--youtube-cookie-file" => {
                index += 1;
                options.youtube_cookie_file = Some(required_path_arg(args, index, current)?);
            }
            "--netease-session-file" => {
                index += 1;
                options.netease_session_file = Some(required_path_arg(args, index, current)?);
            }
            "--bilibili-session-file" => {
                index += 1;
                options.bilibili_session_file = Some(required_path_arg(args, index, current)?);
            }
            value if value.starts_with("--") => {
                return Err(format!("Unknown option: {value}"));
            }
            value => {
                if options.music_path.is_some() {
                    return Err(format!(
                        "Unexpected positional argument: {value}. Only one music_path is supported."
                    ));
                }
                options.music_path = Some(value.to_string());
            }
        }

        index += 1;
    }

    Ok(options)
}

fn required_path_arg(args: &[String], index: usize, flag: &str) -> Result<PathBuf, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("Missing value for {flag}"));
    };
    if value.starts_with("--") {
        return Err(format!("Missing value for {flag}"));
    }
    Ok(PathBuf::from(value))
}

/// Apply standalone auth file overrides before provider clients are used.
pub fn apply_standalone_auth(options: &CliOptions) -> Result<(), String> {
    if let Some(path) = options.youtube_cookie_file.as_ref() {
        ensure_readable_file(path, "--youtube-cookie-file")?;
        env::set_var(YOUTUBE_COOKIE_HEADER_PATH_ENV, path);
    }

    if let Some(path) = options.netease_session_file.as_ref() {
        copy_session_file(path, "session.json", "--netease-session-file")?;
    }

    if let Some(path) = options.bilibili_session_file.as_ref() {
        copy_session_file(path, "bilibili_session.json", "--bilibili-session-file")?;
    }

    Ok(())
}

fn copy_session_file(source: &Path, target_name: &str, flag: &str) -> Result<(), String> {
    ensure_readable_file(source, flag)?;
    let destination = ncmdump_config_dir()?.join(target_name);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create auth config dir {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::copy(source, &destination).map_err(|err| {
        format!(
            "Failed to import auth file from {} to {}: {err}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn ensure_readable_file(path: &Path, flag: &str) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("Failed to read {flag} at {}: {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "{flag} must point to a file, got {}",
            path.display()
        ));
    }
    Ok(())
}

fn ncmdump_config_dir() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(NCMDUMP_CONFIG_DIR_ENV) {
        return Ok(PathBuf::from(path));
    }

    let Some(path) = dirs::config_dir() else {
        return Err("Cannot determine config directory for provider auth".to_string());
    };
    Ok(path.join("ncmdump"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_cli_options_accepts_music_path_and_auth_files() {
        let args = vec![
            "/music".to_string(),
            "--youtube-cookie-file".to_string(),
            "/tmp/youtube.txt".to_string(),
            "--netease-session-file".to_string(),
            "/tmp/netease.json".to_string(),
            "--bilibili-session-file".to_string(),
            "/tmp/bilibili.json".to_string(),
        ];

        let parsed = parse_cli_options(&args).unwrap();

        assert_eq!(parsed.music_path.as_deref(), Some("/music"));
        assert_eq!(
            parsed.youtube_cookie_file,
            Some(PathBuf::from("/tmp/youtube.txt"))
        );
        assert_eq!(
            parsed.netease_session_file,
            Some(PathBuf::from("/tmp/netease.json"))
        );
        assert_eq!(
            parsed.bilibili_session_file,
            Some(PathBuf::from("/tmp/bilibili.json"))
        );
    }

    #[test]
    fn parse_cli_options_rejects_unknown_flag() {
        let args = vec!["--unknown".to_string(), "value".to_string()];
        let err = parse_cli_options(&args).unwrap_err();
        assert!(err.contains("Unknown option"));
    }

    #[test]
    fn apply_standalone_auth_imports_session_files() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();

        let youtube = source_dir.join("youtube.txt");
        let netease = source_dir.join("netease.json");
        let bilibili = source_dir.join("bilibili.json");
        fs::write(
            &youtube,
            "# Netscape HTTP Cookie File\n.youtube.com\tTRUE\t/\tTRUE\t0\tSID\tvalue\n",
        )
        .unwrap();
        fs::write(&netease, "{\n  \"MUSIC_U\": \"abc\"\n}\n").unwrap();
        fs::write(
            &bilibili,
            "{\n  \"sessdata\": \"abc\",\n  \"bili_jct\": \"csrf\"\n}\n",
        )
        .unwrap();

        let original_ncmdump = env::var_os(NCMDUMP_CONFIG_DIR_ENV);
        let original_youtube = env::var_os(YOUTUBE_COOKIE_HEADER_PATH_ENV);
        env::set_var(NCMDUMP_CONFIG_DIR_ENV, &target_dir);

        let options = CliOptions {
            music_path: None,
            youtube_cookie_file: Some(youtube.clone()),
            netease_session_file: Some(netease),
            bilibili_session_file: Some(bilibili),
        };

        apply_standalone_auth(&options).unwrap();

        assert_eq!(
            env::var_os(YOUTUBE_COOKIE_HEADER_PATH_ENV),
            Some(youtube.into())
        );
        assert!(target_dir.join("session.json").exists());
        assert!(target_dir.join("bilibili_session.json").exists());

        restore_env(NCMDUMP_CONFIG_DIR_ENV, original_ncmdump);
        restore_env(YOUTUBE_COOKIE_HEADER_PATH_ENV, original_youtube);
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }
}
