use serde::Serialize;

use crate::models::BackendKind;

#[derive(Debug, Clone, Serialize)]
pub struct RawHit {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawOutdated {
    pub name: String,
    pub current: Option<String>,
    pub available: Option<String>,
    pub security: bool,
}

pub fn search(kind: BackendKind, stdout: &str) -> Vec<RawHit> {
    match kind {
        BackendKind::Brew => brew_search(stdout),
        BackendKind::Apt => apt_search(stdout),
        BackendKind::Dnf => dnf_search(stdout),
        BackendKind::Pacman => pacman_search(stdout),
        BackendKind::Zypper => zypper_search(stdout),
        BackendKind::Winget => winget_search(stdout),
        BackendKind::Scoop => scoop_search(stdout),
        BackendKind::Choco => choco_search(stdout),
    }
}

pub fn installed(kind: BackendKind, stdout: &str) -> Vec<(String, Option<String>)> {
    match kind {
        BackendKind::Brew => lines_versions(stdout),
        BackendKind::Apt => apt_installed(stdout),
        BackendKind::Dnf | BackendKind::Zypper => rpm_installed(stdout),
        BackendKind::Pacman => lines_versions(stdout),
        BackendKind::Winget => winget_installed(stdout),
        BackendKind::Scoop => scoop_installed(stdout),
        BackendKind::Choco => choco_installed(stdout),
    }
}

pub fn outdated(kind: BackendKind, stdout: &str) -> Vec<RawOutdated> {
    match kind {
        BackendKind::Brew => brew_outdated(stdout),
        BackendKind::Apt => apt_outdated(stdout),
        BackendKind::Dnf => dnf_outdated(stdout),
        BackendKind::Pacman => pacman_outdated(stdout),
        BackendKind::Zypper => zypper_outdated(stdout),
        BackendKind::Winget => winget_outdated(stdout),
        BackendKind::Scoop => scoop_outdated(stdout),
        BackendKind::Choco => choco_outdated(stdout),
    }
}

pub fn rank(term: &str, hit: &RawHit) -> i64 {
    let term = term.to_ascii_lowercase();
    let name = hit.name.to_ascii_lowercase();
    let mut score = 0;

    if name == term {
        score = 1000;
    } else if name.starts_with(&term) {
        score = 800;
    } else if name.contains(&term) {
        score = 650;
    }

    if let Some(description) = &hit.description {
        if description.to_ascii_lowercase().contains(&term) {
            score = score.max(400);
        }
    }

    if score == 0 && is_subsequence(&term, &name) {
        score = 350;
    }

    if score == 0 {
        let distance = levenshtein(&term, &name);
        if distance <= 2 {
            score = 300 - (distance as i64 * 50);
        }
    }

    score - (name.len() as i64 / 20)
}

pub fn matches_fuzzy(term: &str, text: &str) -> bool {
    let term = term.to_ascii_lowercase();
    let text = text.to_ascii_lowercase();
    text.contains(&term) || is_subsequence(&term, &text)
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle
        .chars()
        .all(|wanted| chars.any(|candidate| candidate == wanted))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, left) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let cost = usize::from(left != right);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

fn clean(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('\u{feff}');
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn strip_arch(value: &str) -> String {
    const ARCHES: [&str; 14] = [
        "x86_64",
        "noarch",
        "i686",
        "i386",
        "aarch64",
        "armv7hl",
        "armv6hl",
        "ppc64le",
        "s390x",
        "riscv64",
        "x86_64_v2",
        "x86_64_v3",
        "x86_64_v4",
        "amd64",
    ];
    match value.rsplit_once('.') {
        Some((base, arch)) if ARCHES.contains(&arch) => base.to_string(),
        _ => value.to_string(),
    }
}

fn split_name_version(value: &str) -> (String, Option<String>) {
    let mut words = value.split_whitespace();
    let name = words.next().unwrap_or_default().to_string();
    let version = words.next().map(str::to_string);
    (name, version)
}

fn lines_versions(stdout: &str) -> Vec<(String, Option<String>)> {
    stdout
        .lines()
        .filter_map(clean)
        .filter(|line| !line.starts_with("==>"))
        .map(|line| split_name_version(&line))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}

fn brew_search(stdout: &str) -> Vec<RawHit> {
    let mut hits = Vec::new();
    for line in stdout.lines() {
        let Some(line) = clean(line) else { continue };
        if line.starts_with("==>") {
            continue;
        }
        for name in line.split_whitespace() {
            hits.push(RawHit {
                name: name.to_string(),
                version: None,
                description: None,
            });
        }
    }
    hits
}

fn apt_search(stdout: &str) -> Vec<RawHit> {
    stdout
        .lines()
        .filter_map(clean)
        .filter(|line| !line.starts_with("WARNING"))
        .map(|line| match line.split_once(" - ") {
            Some((name, description)) => RawHit {
                name: name.trim().to_string(),
                version: None,
                description: Some(description.trim().to_string()),
            },
            None => RawHit {
                name: line.to_string(),
                version: None,
                description: None,
            },
        })
        .filter(|hit| !hit.name.is_empty())
        .collect()
}

fn dnf_search(stdout: &str) -> Vec<RawHit> {
    let mut hits = Vec::new();
    for line in stdout.lines() {
        let Some(line) = clean(line) else { continue };
        if line.starts_with("Last metadata")
            || line.starts_with('=')
            || line.starts_with("Matched")
            || line.starts_with("Name")
            || line.starts_with("Summary")
        {
            continue;
        }
        if let Some((left, right)) = line.split_once(" : ") {
            let name = strip_arch(left.trim());
            if name.is_empty() {
                continue;
            }
            hits.push(RawHit {
                name,
                version: None,
                description: Some(right.trim().to_string()),
            });
        }
    }

    if hits.is_empty() {
        for row in table_rows(stdout, &["Name"], &["Version", "Summary", "Repository"]) {
            if let Some(name) = field(&row, "Name") {
                hits.push(RawHit {
                    name: name.to_string(),
                    version: field(&row, "Version").map(str::to_string),
                    description: field(&row, "Summary").map(str::to_string),
                });
            }
        }
    }
    hits
}

fn pacman_search(stdout: &str) -> Vec<RawHit> {
    let mut hits: Vec<RawHit> = Vec::new();
    for line in stdout.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = hits.last_mut() {
                if last.description.is_none() {
                    last.description = clean(line);
                }
            }
            continue;
        }
        let Some(line) = clean(line) else { continue };
        let mut words = line.split_whitespace();
        let Some(first) = words.next() else { continue };
        let name = first
            .rsplit_once('/')
            .map(|(_, name)| name)
            .unwrap_or(first);
        let version = words
            .next()
            .map(|value| value.trim_end_matches("[installed]").to_string());
        hits.push(RawHit {
            name: name.to_string(),
            version,
            description: None,
        });
    }
    hits
}

fn zypper_search(stdout: &str) -> Vec<RawHit> {
    let mut hits = Vec::new();
    for row in pipe_rows(stdout, &["Name"]) {
        let Some(name) = pipe_field(&row, "Name") else {
            continue;
        };
        hits.push(RawHit {
            name: name.to_string(),
            version: pipe_field(&row, "Version").map(str::to_string),
            description: pipe_field(&row, "Summary").map(str::to_string),
        });
    }
    hits
}

fn winget_search(stdout: &str) -> Vec<RawHit> {
    let mut hits = Vec::new();
    for row in table_rows(stdout, &["Name", "Id", "Version"], &["Source", "Match"]) {
        let display = field(&row, "Name");
        let id = field(&row, "Id").or(display);
        let Some(name) = id else { continue };
        hits.push(RawHit {
            name: name.to_string(),
            version: field(&row, "Version").map(str::to_string),
            description: match display {
                Some(display) if !display.eq_ignore_ascii_case(name) => Some(display.to_string()),
                _ => None,
            },
        });
    }
    hits
}

fn scoop_search(stdout: &str) -> Vec<RawHit> {
    let mut hits = Vec::new();
    for row in table_rows(stdout, &["Name"], &["Version", "Source", "Updated", "Info"]) {
        let Some(name) = field(&row, "Name") else {
            continue;
        };
        hits.push(RawHit {
            name: name.to_string(),
            version: field(&row, "Version").map(str::to_string),
            description: field(&row, "Info").map(str::to_string),
        });
    }
    if hits.is_empty() {
        for line in stdout.lines().filter_map(clean) {
            let lower = line.to_ascii_lowercase();
            if lower.contains("results from")
                || lower.starts_with("name")
                || lower.starts_with('-')
                || lower.contains("no matches")
            {
                continue;
            }
            hits.push(RawHit {
                name: split_name_version(&line).0,
                version: None,
                description: None,
            });
        }
    }
    hits
}

fn choco_search(stdout: &str) -> Vec<RawHit> {
    stdout
        .lines()
        .filter_map(clean)
        .filter_map(|line| {
            let mut parts = line.split('|');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(RawHit {
                name: name.to_string(),
                version: parts.next().map(|value| value.trim().to_string()),
                description: None,
            })
        })
        .collect()
}

fn apt_installed(stdout: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or_default().trim();
        let name = parts.next().unwrap_or_default().trim();
        let version = parts.next().unwrap_or_default().trim();
        if name.is_empty() {
            continue;
        }
        if !status.is_empty() && !status.starts_with("ii") {
            continue;
        }
        out.push((
            name.to_string(),
            if version.is_empty() {
                None
            } else {
                Some(version.to_string())
            },
        ));
    }
    out
}

fn rpm_installed(stdout: &str) -> Vec<(String, Option<String>)> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            let version = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            Some((name.to_string(), version.map(str::to_string)))
        })
        .collect()
}

fn winget_installed(stdout: &str) -> Vec<(String, Option<String>)> {
    table_rows(stdout, &["Name", "Id", "Version"], &["Source", "Available"])
        .into_iter()
        .filter_map(|row| {
            let name = field(&row, "Id").or_else(|| field(&row, "Name"))?;
            Some((name.to_string(), field(&row, "Version").map(str::to_string)))
        })
        .collect()
}

fn scoop_installed(stdout: &str) -> Vec<(String, Option<String>)> {
    table_rows(stdout, &["Name"], &["Version", "Source", "Updated", "Info"])
        .into_iter()
        .filter_map(|row| {
            let name = field(&row, "Name")?;
            Some((name.to_string(), field(&row, "Version").map(str::to_string)))
        })
        .collect()
}

fn choco_installed(stdout: &str) -> Vec<(String, Option<String>)> {
    stdout
        .lines()
        .filter_map(clean)
        .filter_map(|line| {
            let mut parts = line.split('|');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some((
                name.to_string(),
                parts
                    .next()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            ))
        })
        .collect()
}

fn brew_outdated(stdout: &str) -> Vec<RawOutdated> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout) {
        let mut out = Vec::new();
        for key in ["formulae", "casks"] {
            let Some(items) = value.get(key).and_then(|items| items.as_array()) else {
                continue;
            };
            for item in items {
                let Some(name) = item.get("name").and_then(|name| name.as_str()) else {
                    continue;
                };
                let current = item
                    .get("installed_versions")
                    .and_then(|versions| versions.as_array())
                    .and_then(|versions| versions.last())
                    .and_then(|version| version.as_str())
                    .map(str::to_string);
                let available = item
                    .get("current_version")
                    .and_then(|version| version.as_str())
                    .map(str::to_string);
                out.push(RawOutdated {
                    name: name.to_string(),
                    current,
                    available,
                    security: false,
                });
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    stdout
        .lines()
        .filter_map(clean)
        .filter(|line| !line.starts_with("==>"))
        .filter_map(|line| {
            let name = line.split_whitespace().next()?.to_string();
            let current = line
                .split_once('(')
                .and_then(|(_, rest)| rest.split_once(')'))
                .map(|(value, _)| value.to_string());
            let available = if line.contains("->") {
                line.split("->")
                    .nth(1)
                    .and_then(|value| value.split_whitespace().next())
            } else {
                line.split('<')
                    .nth(1)
                    .and_then(|value| value.split_whitespace().next())
            }
            .map(str::to_string);
            Some(RawOutdated {
                name,
                current,
                available,
                security: false,
            })
        })
        .collect()
}

fn apt_outdated(stdout: &str) -> Vec<RawOutdated> {
    stdout
        .lines()
        .filter_map(clean)
        .filter(|line| !line.starts_with("Listing") && !line.starts_with("WARNING"))
        .filter_map(|line| {
            let (left, rest) = line.split_once(' ')?;
            let (name, repository) = left.split_once('/')?;
            let available = rest.split_whitespace().next().map(str::to_string);
            let current = line
                .split_once("upgradable from:")
                .map(|(_, value)| value.trim().trim_end_matches(']').to_string());
            Some(RawOutdated {
                name: name.to_string(),
                current,
                available,
                security: repository.to_ascii_lowercase().contains("security"),
            })
        })
        .collect()
}

fn dnf_outdated(stdout: &str) -> Vec<RawOutdated> {
    let mut out = Vec::new();
    for row in table_rows(stdout, &["Name"], &["Version", "Repository"]) {
        let Some(name) = field(&row, "Name") else {
            continue;
        };
        let repository = field(&row, "Repository").unwrap_or_default();
        out.push(RawOutdated {
            name: strip_arch(name),
            current: None,
            available: field(&row, "Version").map(str::to_string),
            security: repository.to_ascii_lowercase().contains("security"),
        });
    }
    if !out.is_empty() {
        return out;
    }

    for line in stdout.lines() {
        let Some(line) = clean(line) else { continue };
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("last metadata")
            || lower.starts_with("available upgrades")
            || lower.starts_with("dependencies resolved")
            || lower.starts_with("nothing to do")
        {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(name) = words.next() else { continue };
        let (rest, repository) = match (words.next(), words.next()) {
            (Some(version), Some(repository)) => (version, repository),
            _ => continue,
        };
        out.push(RawOutdated {
            name: strip_arch(name),
            current: None,
            available: Some(rest.to_string()),
            security: repository.to_ascii_lowercase().contains("security"),
        });
    }
    out
}

fn pacman_outdated(stdout: &str) -> Vec<RawOutdated> {
    stdout
        .lines()
        .filter_map(clean)
        .filter_map(|line| {
            let (left, right) = line.split_once("->")?;
            let mut left = left.split_whitespace();
            let name = left.next()?.to_string();
            let current = left.last().map(str::to_string);
            let available = right.split_whitespace().next().map(str::to_string);
            Some(RawOutdated {
                name,
                current,
                available,
                security: false,
            })
        })
        .collect()
}

fn zypper_outdated(stdout: &str) -> Vec<RawOutdated> {
    let mut out = Vec::new();
    for row in pipe_rows(stdout, &["Name"]) {
        let Some(name) = pipe_field(&row, "Name") else {
            continue;
        };
        let repository = pipe_field(&row, "Repository").unwrap_or_default();
        out.push(RawOutdated {
            name: name.to_string(),
            current: pipe_field(&row, "Current Version").map(str::to_string),
            available: pipe_field(&row, "Available Version").map(str::to_string),
            security: repository.to_ascii_lowercase().contains("security"),
        });
    }
    out
}

fn winget_outdated(stdout: &str) -> Vec<RawOutdated> {
    table_rows(stdout, &["Name", "Id", "Version"], &["Available", "Source"])
        .into_iter()
        .filter_map(|row| {
            let available = field(&row, "Available")?;
            let name = field(&row, "Id").or_else(|| field(&row, "Name"))?;
            Some(RawOutdated {
                name: name.to_string(),
                current: field(&row, "Version").map(str::to_string),
                available: Some(available.to_string()),
                security: false,
            })
        })
        .collect()
}

fn scoop_outdated(stdout: &str) -> Vec<RawOutdated> {
    table_rows(
        stdout,
        &["Name"],
        &["Version", "Latest", "Source", "Updated", "Info"],
    )
    .into_iter()
    .filter_map(|row| {
        let name = field(&row, "Name")?;
        let available = field(&row, "Latest")?;
        Some(RawOutdated {
            name: name.to_string(),
            current: field(&row, "Version").map(str::to_string),
            available: Some(available.to_string()),
            security: false,
        })
    })
    .collect()
}

fn choco_outdated(stdout: &str) -> Vec<RawOutdated> {
    stdout
        .lines()
        .filter_map(clean)
        .filter_map(|line| {
            let mut parts = line.split('|');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            let current = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let available = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            Some(RawOutdated {
                name: name.to_string(),
                current: current.map(str::to_string),
                available: available.map(str::to_string),
                security: false,
            })
        })
        .collect()
}

fn table_rows(
    stdout: &str,
    required: &[&'static str],
    optional: &[&'static str],
) -> Vec<Vec<(&'static str, String)>> {
    let lines: Vec<&str> = stdout.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !required.iter().all(|label| line.contains(label)) {
            continue;
        }
        let mut labels: Vec<&'static str> = required
            .iter()
            .chain(optional.iter())
            .copied()
            .filter(|label| line.contains(label))
            .collect();
        if labels.len() < 2 {
            continue;
        }
        labels.sort_by_key(|label| line.find(label).unwrap_or(usize::MAX));
        let starts: Vec<usize> = labels
            .iter()
            .map(|label| line.find(label).unwrap_or(0))
            .collect();

        let mut rows = Vec::new();
        for row in &lines[index + 1..] {
            let trimmed = row.trim();
            if trimmed.is_empty()
                || trimmed.starts_with('-')
                || trimmed.starts_with('=')
                || trimmed.starts_with('+')
            {
                continue;
            }
            let mut values = Vec::new();
            for (position, label) in labels.iter().enumerate() {
                let start = starts[position];
                let end = starts.get(position + 1).copied().unwrap_or(row.len());
                let mut value = if start < row.len() {
                    row.get(start..end).unwrap_or("")
                } else {
                    ""
                };
                if value.trim().is_empty() {
                    value = row.split_whitespace().nth(position).unwrap_or("");
                }
                values.push((*label, value.trim().to_string()));
            }
            if values.first().is_some_and(|(_, value)| !value.is_empty()) {
                rows.push(values);
            }
        }
        return rows;
    }
    Vec::new()
}

fn field<'a>(row: &'a [(&'static str, String)], label: &str) -> Option<&'a str> {
    row.iter()
        .find(|(name, _)| *name == label)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

fn pipe_rows(stdout: &str, required: &[&str]) -> Vec<Vec<(String, String)>> {
    let mut headers: Option<Vec<String>> = None;
    let mut rows = Vec::new();
    for line in stdout.lines() {
        if !line.contains('|') {
            continue;
        }
        let cells: Vec<String> = line
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();
        match &headers {
            None => {
                if required
                    .iter()
                    .all(|needle| cells.iter().any(|cell| cell.contains(needle)))
                {
                    headers = Some(cells);
                }
            }
            Some(headers) => {
                if cells
                    .iter()
                    .all(|cell| cell.is_empty() || cell.chars().all(|c| c == '-' || c == '+'))
                {
                    continue;
                }
                let mut row = Vec::new();
                for (index, cell) in cells.iter().enumerate() {
                    if let Some(name) = headers.get(index) {
                        row.push((name.clone(), cell.clone()));
                    }
                }
                rows.push(row);
            }
        }
    }
    rows
}

fn pipe_field<'a>(row: &'a [(String, String)], label: &str) -> Option<&'a str> {
    row.iter()
        .find(|(name, _)| name == label)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_search_splits_description() {
        let hits = search(
            BackendKind::Apt,
            "git - fast, scalable, distributed revision control system\nripgrep - line-oriented search tool\n",
        );
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].name, "git");
        assert!(hits[0].description.as_deref().unwrap().contains("revision"));
    }

    #[test]
    fn pacman_search_reads_version_and_description() {
        let hits = search(
            BackendKind::Pacman,
            "extra/git 2.47.1-1\n    the fast distributed version control system\n",
        );
        assert_eq!(hits[0].name, "git");
        assert_eq!(hits[0].version.as_deref(), Some("2.47.1-1"));
        assert!(hits[0].description.as_deref().unwrap().contains("fast"));
    }

    #[test]
    fn winget_search_uses_id_as_package_name() {
        let output = "\
Name               Id                    Version   Source
--------------------------------------------------------
Mozilla Firefox    Mozilla.Firefox       128.0     winget
";
        let hits = search(BackendKind::Winget, output);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Mozilla.Firefox");
        assert_eq!(hits[0].version.as_deref(), Some("128.0"));
        assert_eq!(hits[0].description.as_deref(), Some("Mozilla Firefox"));
    }

    #[test]
    fn choco_installed_reads_limit_output() {
        let pkgs = installed(BackendKind::Choco, "git|2.47.1\n7zip|24.8\n");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0], ("git".to_string(), Some("2.47.1".to_string())));
    }

    #[test]
    fn pacman_outdated_parses_arrow() {
        let items = outdated(BackendKind::Pacman, "git 2.46.0-1 -> 2.47.1-1\n");
        assert_eq!(items[0].name, "git");
        assert_eq!(items[0].current.as_deref(), Some("2.46.0-1"));
        assert_eq!(items[0].available.as_deref(), Some("2.47.1-1"));
    }

    #[test]
    fn brew_outdated_reads_json() {
        let output = r#"{"formulae":[{"name":"git","installed_versions":["2.46.0"],"current_version":"2.47.1"}],"casks":[]}"#;
        let items = outdated(BackendKind::Brew, output);
        assert_eq!(items[0].name, "git");
        assert_eq!(items[0].current.as_deref(), Some("2.46.0"));
        assert_eq!(items[0].available.as_deref(), Some("2.47.1"));
    }

    #[test]
    fn ranking_prefers_exact_names() {
        let exact = RawHit {
            name: "firefox".into(),
            version: None,
            description: None,
        };
        let prefix = RawHit {
            name: "firefox-esr".into(),
            version: None,
            description: None,
        };
        assert!(rank("firefox", &exact) > rank("firefox", &prefix));
    }

    #[test]
    fn fuzzy_matching_accepts_subsequences() {
        assert!(matches_fuzzy("rg", "ripgrep"));
        assert!(matches_fuzzy("fire", "Mozilla.Firefox"));
        assert!(!matches_fuzzy("zzz", "ripgrep"));
    }
}
