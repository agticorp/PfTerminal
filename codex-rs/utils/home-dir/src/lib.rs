use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use std::ffi::OsStr;
use std::path::PathBuf;

const DEFAULT_CODEX_HOME_DIR: &str = ".codex";
const DEFAULT_PFTERMINAL_HOME_DIR: &str = ".pfterminal";

/// Returns the path to the Codex configuration directory, which can be specified by the
/// `CODEX_HOME` environment variable. If not set, stock Codex defaults to `~/.codex` and
/// PFTerminal defaults to `~/.pfterminal`.
///
/// - If `CODEX_HOME` is set, the value must exist and be a directory. The
///   value will be canonicalized and this function will Err otherwise.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub fn find_codex_home() -> std::io::Result<AbsolutePathBuf> {
    let codex_home_env = std::env::var("CODEX_HOME")
        .ok()
        .filter(|val| !val.is_empty());
    find_codex_home_from_env_and_default(codex_home_env.as_deref(), default_home_dir_name())
}

#[cfg(test)]
fn find_codex_home_from_env(codex_home_env: Option<&str>) -> std::io::Result<AbsolutePathBuf> {
    find_codex_home_from_env_and_default(codex_home_env, DEFAULT_CODEX_HOME_DIR)
}

fn find_codex_home_from_env_and_default(
    codex_home_env: Option<&str>,
    default_home_dir: &str,
) -> std::io::Result<AbsolutePathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    match codex_home_env {
        Some(val) => {
            let path = PathBuf::from(val);
            let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("CODEX_HOME points to {val:?}, but that path does not exist"),
                ),
                _ => std::io::Error::new(
                    err.kind(),
                    format!("failed to read CODEX_HOME {val:?}: {err}"),
                ),
            })?;

            if !metadata.is_dir() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("CODEX_HOME points to {val:?}, but that path is not a directory"),
                ))
            } else {
                let canonical = path.canonicalize().map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("failed to canonicalize CODEX_HOME {val:?}: {err}"),
                    )
                })?;
                AbsolutePathBuf::from_absolute_path(canonical)
            }
        }
        None => {
            let mut p = home_dir().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                )
            })?;
            p.push(default_home_dir);
            AbsolutePathBuf::from_absolute_path(p)
        }
    }
}

fn default_home_dir_name() -> &'static str {
    let arg0 = std::env::args_os().next();
    default_home_dir_name_for_binary(arg0.as_deref())
}

fn default_home_dir_name_for_binary(binary: Option<&OsStr>) -> &'static str {
    let file_stem = binary
        .and_then(|value| PathBuf::from(value).file_stem().map(OsStr::to_owned))
        .and_then(|value| value.into_string().ok());

    if file_stem.as_deref() == Some("pfterminal") {
        DEFAULT_PFTERMINAL_HOME_DIR
    } else {
        DEFAULT_CODEX_HOME_DIR
    }
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_PFTERMINAL_HOME_DIR;
    use super::default_home_dir_name_for_binary;
    use super::find_codex_home_from_env;
    use super::find_codex_home_from_env_and_default;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(missing_str)).expect_err("missing CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(
            err.to_string().contains("CODEX_HOME"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(file_str)).expect_err("file CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_codex_home_from_env(Some(temp_str)).expect("valid CODEX_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_default_home_dir() {
        let resolved =
            find_codex_home_from_env(/*codex_home_env*/ None).expect("default CODEX_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(".codex");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_pfterminal_default_when_requested() {
        let resolved = find_codex_home_from_env_and_default(
            /*codex_home_env*/ None,
            DEFAULT_PFTERMINAL_HOME_DIR,
        )
        .expect("default PFTerminal home");
        let mut expected = home_dir().expect("home dir");
        expected.push(".pfterminal");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn pfterminal_binary_name_uses_pfterminal_home() {
        assert_eq!(
            default_home_dir_name_for_binary(Some(OsStr::new("/usr/local/bin/pfterminal"))),
            ".pfterminal"
        );
        assert_eq!(
            default_home_dir_name_for_binary(Some(OsStr::new("pfterminal.exe"))),
            ".pfterminal"
        );
        assert_eq!(
            default_home_dir_name_for_binary(Some(OsStr::new("codex"))),
            ".codex"
        );
        assert_eq!(default_home_dir_name_for_binary(None), ".codex");
    }
}
