//! Maps user-supplied package names to a concrete backend + native name.

use anyhow::Result;

use crate::backend::Backend;
use crate::error::{EXIT_NOT_FOUND, EXIT_USAGE, exit};
use crate::exec::Runner;
use crate::models::BackendKind;
use crate::registry::AliasTable;
use crate::validate;

#[derive(Debug, Clone)]
pub struct Resolved {
    /// The name the user typed.
    pub requested: String,
    pub backend: Backend,
    /// The native package name for that backend (may include `cask:`).
    pub package: String,
}

impl Resolved {
    pub fn display_target(&self) -> String {
        if self.package.eq_ignore_ascii_case(&self.requested) {
            self.package.clone()
        } else {
            format!("{} ({})", self.requested, self.package)
        }
    }
}

/// Resolve every requested name.
///
/// Order of preference:
/// 1. `--backend` if given (raw name unless an alias exists for that backend).
/// 2. Highest-priority available backend with an alias entry.
/// 3. Highest-priority available backend that reports the package in `search`.
/// 4. The highest-priority available backend with the raw name (let it explain
///    the failure).
pub fn resolve(
    names: &[String],
    forced: Option<BackendKind>,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
) -> Result<Vec<Resolved>> {
    if names.is_empty() {
        return Err(exit(EXIT_USAGE, "no packages given"));
    }
    if backends.is_empty() {
        return Err(exit(
            EXIT_NOT_FOUND,
            "no supported package manager was found on this system (try `upm doctor`)",
        ));
    }

    let candidates: Vec<Backend> = match forced {
        Some(kind) => match backends.iter().find(|b| b.kind() == kind) {
            Some(backend) => vec![*backend],
            None => {
                return Err(exit(
                    EXIT_USAGE,
                    format!("backend `{kind}` is not available on this system"),
                ));
            }
        },
        None => backends.to_vec(),
    };

    let mut resolved = Vec::with_capacity(names.len());
    for name in names {
        validate::package_name(name)?;

        // 1. Alias entry for any candidate, in priority order.
        let mut chosen = candidates.iter().find_map(|backend| {
            table.lookup(name, backend.kind()).map(|package| Resolved {
                requested: name.clone(),
                backend: *backend,
                package: package.to_string(),
            })
        });

        // 2. Ask the backends whether they carry it.
        if chosen.is_none() {
            for backend in &candidates {
                if backend.has_package(name, runner).unwrap_or(false) {
                    chosen = Some(Resolved {
                        requested: name.clone(),
                        backend: *backend,
                        package: name.clone(),
                    });
                    break;
                }
            }
        }

        // 3. Fall back to the preferred backend with the raw name.
        let resolved_package = chosen.unwrap_or_else(|| Resolved {
            requested: name.clone(),
            backend: candidates[0],
            package: name.clone(),
        });

        resolved.push(resolved_package);
    }

    Ok(resolved)
}

/// Validate and parse a forced backend string.
pub fn parse_backend(value: &str) -> Result<BackendKind> {
    value
        .parse::<BackendKind>()
        .map_err(|message| exit(EXIT_USAGE, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn no_backends() -> Vec<Backend> {
        Vec::new()
    }

    #[test]
    fn rejects_empty_input() {
        let table = AliasTable::load(&Config::default()).unwrap();
        let runner = Runner {
            dry_run: true,
            assume_yes: true,
            quiet: true,
        };
        assert!(resolve(&[], None, &table, &no_backends(), &runner).is_err());
    }

    #[test]
    fn rejects_bad_names_before_touching_backends() {
        let table = AliasTable::load(&Config::default()).unwrap();
        let runner = Runner {
            dry_run: true,
            assume_yes: true,
            quiet: true,
        };
        let names = vec!["--force".to_string()];
        assert!(resolve(&names, None, &table, &no_backends(), &runner).is_err());
    }

    #[test]
    fn display_target_hides_identical_names() {
        let resolved = Resolved {
            requested: "git".into(),
            backend: Backend::new(BackendKind::Brew),
            package: "git".into(),
        };
        assert_eq!(resolved.display_target(), "git");
    }
}
