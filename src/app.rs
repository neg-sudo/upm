//! Command implementations.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::backend::Backend;
use crate::capabilities::Capabilities;
use crate::cli::{Cli, Command, ConfigCmd, HistoryCmd, ProfileCmd, SnapshotCmd};
use crate::config::Config;
use crate::detect;
use crate::error::{
    EXIT_ERROR, EXIT_NO_BACKEND, EXIT_NOT_FOUND, EXIT_UNSUPPORTED, EXIT_USAGE, exit,
};
use crate::exec::{self, Runner};
use crate::history::{self, HistoryPackage};
use crate::manifest::{self, Manifest, Snapshot, SnapshotPackage};
use crate::models::BackendKind;
use crate::output;
use crate::parse;
use crate::paths;
use crate::plan::{Plan, PlanPackage};
use crate::registry::AliasTable;
use crate::resolver::{self, Reason, Resolved};
use crate::validate;

pub fn run(cli: Cli, cfg: Config) -> Result<()> {
    let forced = match &cli.backend {
        Some(value) => Some(resolver::parse_backend(value)?),
        None => None,
    };
    let backends = detect::detected(&cfg);
    let table = AliasTable::load(&cfg)?;
    let runner = Runner {
        dry_run: cli.dry_run,
        assume_yes: cli.yes || cli.non_interactive,
        quiet: cli.quiet,
        keep_going: cli.keep_going,
        no_elevate: cli.no_elevate,
    };

    match &cli.command {
        Command::Install { packages } => {
            cmd_install(&cli, &cfg, &table, &backends, &runner, packages, forced)
        }
        Command::Remove { packages } => {
            cmd_remove(&cli, &cfg, &table, &backends, &runner, packages, forced)
        }
        Command::Upgrade {
            security,
            interactive,
            packages,
        } => cmd_upgrade(
            &cli,
            &cfg,
            &table,
            &backends,
            &runner,
            packages,
            forced,
            *security,
            *interactive,
        ),
        Command::Update => cmd_update(&cli, &cfg, &backends, &runner, forced),
        Command::Search {
            term,
            exact,
            installed,
            outdated,
            limit,
            interactive,
        } => cmd_search(
            &cli,
            &cfg,
            &backends,
            &runner,
            term,
            forced,
            *exact,
            *installed,
            *outdated,
            *limit,
            *interactive,
        ),
        Command::List {
            outdated,
            search,
            limit,
        } => cmd_list(
            &cli,
            &backends,
            &runner,
            forced,
            *outdated,
            search.as_deref(),
            *limit,
        ),
        Command::Outdated { security } => cmd_outdated(&cli, &backends, &runner, forced, *security),
        Command::Info { package } => cmd_info(&cli, &table, &backends, &runner, package, forced),
        Command::Which => cmd_which(&cli, &cfg, &table, &backends),
        Command::Doctor { fix } => cmd_doctor(&cli, &cfg, &table, &backends, *fix),
        Command::Resolve { explain, packages } => {
            cmd_resolve(&cli, &table, &backends, &runner, packages, forced, *explain)
        }
        Command::Explain { action, packages } => cmd_explain(
            &cli, &cfg, &table, &backends, &runner, action, packages, forced,
        ),
        Command::Capabilities { backend } => cmd_capabilities(&cli, backend.as_deref()),
        Command::Clean { all } => cmd_clean(&cli, &cfg, &backends, &runner, forced, *all),
        Command::Pin { packages } => cmd_pin(
            &cli, &cfg, &table, &backends, &runner, packages, forced, true,
        ),
        Command::Unpin { packages } => cmd_pin(
            &cli, &cfg, &table, &backends, &runner, packages, forced, false,
        ),
        Command::Pins => cmd_pins(&cli, &backends, &runner, forced),
        Command::Why { package } => {
            cmd_why(&cli, &table, &backends, &runner, package, forced, true)
        }
        Command::Depends { package } => {
            cmd_why(&cli, &table, &backends, &runner, package, forced, false)
        }
        Command::History { cmd } => cmd_history(&cli, cmd.as_ref()),
        Command::Rollback { id } => cmd_rollback(&cli, &cfg, &backends, &runner, *id, forced),
        Command::Init { from_system, file } => cmd_init(
            &cli,
            &backends,
            &runner,
            &table,
            file.as_deref(),
            *from_system,
        ),
        Command::Export { file } => cmd_export(&cli, &backends, &runner, &table, file.as_deref()),
        Command::Sync { file } => cmd_sync(
            &cli,
            &cfg,
            &table,
            &backends,
            &runner,
            file.as_deref(),
            forced,
        ),
        Command::Plan {
            action,
            packages,
            out,
        } => cmd_plan(
            &cli,
            &table,
            &backends,
            &runner,
            action,
            packages,
            forced,
            out.as_deref(),
        ),
        Command::Apply { file } => cmd_apply(&cli, &cfg, &backends, &runner, file, forced),
        Command::Profile { cmd } => {
            cmd_profile(&cli, &cfg, &table, &backends, &runner, cmd, forced)
        }
        Command::Snapshot { cmd } => cmd_snapshot(&cli, &cfg, &backends, &runner, cmd, forced),
        Command::Config { cmd } => cmd_config(&cli, &cfg, cmd.as_ref()),
        Command::Completions { shell } => cmd_completions(shell),
    }
}

// ---------------------------------------------------------------------------
// install / remove / upgrade
// ---------------------------------------------------------------------------

fn cmd_install(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
) -> Result<()> {
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    let mut plan = Vec::new();
    for item in resolved {
        if !cli.force {
            if let Some(true) = installed_state(&item, runner)? {
                output::info(&format!(
                    "{} is already installed via {} (use --force to reinstall)",
                    item.requested,
                    item.backend.id()
                ));
                continue;
            }
        }
        plan.push(item);
    }
    run_action("install", &plan, cli, cfg, runner)
}

fn cmd_remove(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
) -> Result<()> {
    let plan = resolver::resolve(packages, forced, table, backends, runner)?;
    run_action("remove", &plan, cli, cfg, runner)
}

#[allow(clippy::too_many_arguments)]
fn cmd_upgrade(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
    security: bool,
    interactive: bool,
) -> Result<()> {
    if !packages.is_empty() {
        if security || interactive {
            return Err(exit(
                EXIT_USAGE,
                "--security and --interactive cannot be combined with explicit packages",
            ));
        }
        let plan = resolver::resolve(packages, forced, table, backends, runner)?;
        return run_action("upgrade", &plan, cli, cfg, runner);
    }

    if interactive {
        let scope = select_backends(backends, forced)?;
        let outdated = collect_outdated(&scope, runner);
        if outdated.is_empty() {
            output::success("everything is up to date");
            return Ok(());
        }
        let selected = prompt_outdated(&outdated)?;
        if selected.is_empty() {
            output::info("aborted");
            return Ok(());
        }
        let plan: Vec<Resolved> = selected
            .into_iter()
            .map(|row| Resolved {
                requested: row.name.clone(),
                backend: row.backend,
                package: row.name,
                reason: Reason::Fallback,
            })
            .collect();
        return run_action("upgrade", &plan, cli, cfg, runner);
    }

    if security {
        let scope = select_backends(backends, forced)?;
        let mut tasks = Vec::new();
        for backend in scope {
            if let Some(argv) = backend.security_upgrade_cmd(runner.assume_yes) {
                tasks.push((backend, argv));
            }
        }
        if tasks.is_empty() {
            return Err(exit(
                EXIT_UNSUPPORTED,
                "no available backend supports security-only upgrades (try dnf or zypper)",
            ));
        }
        return run_tasks("upgrade", tasks, cli, cfg, runner, Vec::new());
    }

    cmd_upgrade_all(cli, cfg, backends, runner, forced)
}

fn run_action(
    action: &str,
    plan: &[Resolved],
    cli: &Cli,
    cfg: &Config,
    runner: &Runner,
) -> Result<()> {
    let history_packages: Vec<HistoryPackage> = plan
        .iter()
        .map(|item| HistoryPackage {
            requested: item.requested.clone(),
            backend: item.backend.id().to_string(),
            package: item.package.clone(),
        })
        .collect();

    if plan.is_empty() {
        if cli.json {
            output::json(&ActionReport {
                schema_version: 1,
                action: action.to_string(),
                dry_run: cli.dry_run,
                status: "empty",
                packages: Vec::new(),
                commands: Vec::new(),
                failures: Vec::new(),
            })?;
        } else {
            output::success("nothing to do");
        }
        return Ok(());
    }

    output::header(&format!(
        "Plan: {action} {} package{}",
        plan.len(),
        plural(plan.len())
    ));
    for item in plan {
        output::detail(&format!(
            "  {} {:<8} {}",
            output::green("•"),
            item.backend.id(),
            item.display_target()
        ));
    }

    if !confirm(cli, cfg)? {
        output::info("aborted");
        return Ok(());
    }

    let mut commands = Vec::new();
    let mut failures = Vec::new();
    let mut first_error: Option<anyhow::Error> = None;

    for item in plan {
        let argv = action_command(action, item, runner.assume_yes);
        commands.push(output::shell_join(&argv));
        match runner.run(&argv, item.backend.needs_elevation()) {
            Ok(()) => {
                if !cli.dry_run {
                    let verb = match action {
                        "install" => "installed",
                        "remove" => "removed",
                        "upgrade" => "upgraded",
                        _ => "done",
                    };
                    output::success(&format!(
                        "{verb} {} via {}",
                        item.requested,
                        item.backend.id()
                    ));
                }
            }
            Err(err) => {
                output::warn(&format!(
                    "{} {} via {}: {err}",
                    action,
                    item.requested,
                    item.backend.id()
                ));
                failures.push(format!("{}: {err:#}", item.requested));
                if first_error.is_none() {
                    first_error = Some(err);
                }
                if !runner.keep_going {
                    break;
                }
            }
        }
    }

    let status = if failures.is_empty() {
        if cli.dry_run { "planned" } else { "success" }
    } else if failures.len() < plan.len() {
        "partial"
    } else {
        "failed"
    };

    if !cli.dry_run {
        record_history(cfg, action, status, history_packages, commands.clone());
    }

    if cli.json {
        output::json(&ActionReport {
            schema_version: 1,
            action: action.to_string(),
            dry_run: cli.dry_run,
            status,
            packages: plan.iter().map(PackageRecord::from_resolved).collect(),
            commands,
            failures: failures.clone(),
        })?;
    }

    if let Some(err) = first_error {
        if !runner.keep_going {
            return Err(err);
        }
    }
    if !failures.is_empty() {
        return Err(exit(
            EXIT_ERROR,
            format!(
                "{} of {} operations failed; first failure: {}",
                failures.len(),
                plan.len(),
                failures[0]
            ),
        ));
    }
    Ok(())
}

fn run_tasks(
    action: &'static str,
    tasks: Vec<(Backend, Vec<String>)>,
    cli: &Cli,
    cfg: &Config,
    runner: &Runner,
    history_packages: Vec<HistoryPackage>,
) -> Result<()> {
    if tasks.is_empty() {
        if cli.json {
            output::json(&AllReport {
                schema_version: 1,
                action,
                dry_run: cli.dry_run,
                status: "empty",
                backends: Vec::new(),
                commands: Vec::new(),
                failures: Vec::new(),
            })?;
        } else {
            output::info("nothing to do");
        }
        return Ok(());
    }

    output::header(&format!("Plan: {action}"));
    for (backend, argv) in &tasks {
        output::detail(&format!(
            "  {} {:<8} {}",
            output::green("•"),
            backend.id(),
            output::dim(&output::shell_join(argv))
        ));
    }

    if !confirm(cli, cfg)? {
        output::info("aborted");
        return Ok(());
    }

    let mut commands = Vec::new();
    let mut backends = Vec::new();
    let mut failures = Vec::new();
    let mut first_error: Option<anyhow::Error> = None;

    for (backend, argv) in &tasks {
        commands.push(output::shell_join(argv));
        if !backends.contains(&backend.id().to_string()) {
            backends.push(backend.id().to_string());
        }
        match runner.run(argv, backend.needs_elevation()) {
            Ok(()) => {
                if !cli.dry_run {
                    output::success(&format!("{action} via {}", backend.id()));
                }
            }
            Err(err) => {
                output::warn(&format!("{action} via {}: {err}", backend.id()));
                failures.push(format!("{}: {err:#}", backend.id()));
                if first_error.is_none() {
                    first_error = Some(err);
                }
                if !runner.keep_going {
                    break;
                }
            }
        }
    }

    let status = if failures.is_empty() {
        if cli.dry_run { "planned" } else { "success" }
    } else if failures.len() < tasks.len() {
        "partial"
    } else {
        "failed"
    };

    if !cli.dry_run {
        record_history(cfg, action, status, history_packages, commands.clone());
    }

    if cli.json {
        output::json(&AllReport {
            schema_version: 1,
            action,
            dry_run: cli.dry_run,
            status,
            backends,
            commands,
            failures: failures.clone(),
        })?;
    }

    if let Some(err) = first_error {
        if !runner.keep_going {
            return Err(err);
        }
    }
    if !failures.is_empty() {
        return Err(exit(
            EXIT_ERROR,
            format!(
                "{} of {} operations failed; first failure: {}",
                failures.len(),
                tasks.len(),
                failures[0]
            ),
        ));
    }
    Ok(())
}

fn action_command(action: &str, item: &Resolved, yes: bool) -> Vec<String> {
    match action {
        "install" => item.backend.install_cmd(&item.package, yes),
        "remove" => item.backend.remove_cmd(&item.package, yes),
        "upgrade" => item.backend.upgrade_cmd(&item.package, yes),
        _ => unreachable!("unknown action"),
    }
}

fn validate_action(action: &str) -> Result<()> {
    match action {
        "install" | "remove" | "upgrade" => Ok(()),
        other => Err(exit(
            EXIT_USAGE,
            format!("unknown action `{other}` (expected install, remove or upgrade)"),
        )),
    }
}

fn cmd_upgrade_all(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let outdated = collect_outdated(&scope, runner);

    if !cli.json {
        output::header("System update");
        if outdated.is_empty() {
            output::success("everything is up to date");
            return Ok(());
        }
        let security = outdated.iter().filter(|row| row.security).count();
        output::detail(&format!(
            "  {} package{} can be upgraded ({} security)",
            outdated.len(),
            plural(outdated.len()),
            security
        ));
        let mut per_backend: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &outdated {
            *per_backend.entry(row.backend.id()).or_default() += 1;
        }
        for (backend, count) in per_backend {
            output::detail(&format!("  {backend:<8} {count}"));
        }
    }

    if outdated.is_empty() {
        if cli.json {
            output::json(&AllReport {
                schema_version: 1,
                action: "upgrade",
                dry_run: cli.dry_run,
                status: "empty",
                backends: Vec::new(),
                commands: Vec::new(),
                failures: Vec::new(),
            })?;
        }
        return Ok(());
    }

    let tasks: Vec<(Backend, Vec<String>)> = scope
        .iter()
        .map(|backend| (*backend, backend.upgrade_all_cmd(runner.assume_yes)))
        .collect();
    let history_packages = outdated
        .iter()
        .map(|row| HistoryPackage {
            requested: row.name.clone(),
            backend: row.backend.id().to_string(),
            package: row.name.clone(),
        })
        .collect();
    run_tasks("upgrade", tasks, cli, cfg, runner, history_packages)
}

// ---------------------------------------------------------------------------
// update / search / list / outdated / info
// ---------------------------------------------------------------------------

fn cmd_update(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let mut tasks = Vec::new();
    for backend in &scope {
        match backend.update_cmd() {
            Some(argv) => tasks.push((*backend, argv)),
            None => output::info(&format!("{} has no metadata refresh step", backend.id())),
        }
    }
    run_tasks("update", tasks, cli, cfg, runner, Vec::new())
}

#[allow(clippy::too_many_arguments)]
fn cmd_search(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    term: &str,
    forced: Option<BackendKind>,
    exact: bool,
    installed_only: bool,
    outdated_only: bool,
    limit: usize,
    interactive: bool,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let installed = collect_installed_sets(&scope, runner);
    let outdated = if outdated_only {
        collect_outdated_set(&scope, runner)
    } else {
        HashSet::new()
    };

    let commands: Vec<(Backend, Vec<String>)> = scope
        .iter()
        .map(|backend| (*backend, backend.search_cmd(term)))
        .collect();
    let outputs = capture_parallel(&commands, runner);

    let mut rows = Vec::new();
    for (backend, result) in outputs {
        match result {
            Ok(out) => {
                if !out.status.success() {
                    output::log(&format!(
                        "{} search exited with {}",
                        backend.id(),
                        out.status
                    ));
                }
                let stdout = String::from_utf8_lossy(&out.stdout);
                for raw in parse::search(backend.kind(), &stdout) {
                    let key = (backend.id().to_string(), raw.name.to_ascii_lowercase());
                    let is_installed = installed.get(backend.id()).is_some_and(|names| {
                        names
                            .iter()
                            .any(|(name, _)| name == &raw.name.to_ascii_lowercase())
                    });
                    let is_outdated = outdated.contains(&key);
                    let score = parse::rank(term, &raw);
                    rows.push(SearchRow {
                        name: raw.name,
                        version: raw.version,
                        description: raw.description,
                        backend,
                        installed: is_installed,
                        outdated: is_outdated,
                        score,
                    });
                }
            }
            Err(err) => output::warn(&format!("{}: {err}", backend.id())),
        }
    }

    if exact {
        rows.retain(|row| row.name.eq_ignore_ascii_case(term));
    }
    if installed_only {
        rows.retain(|row| row.installed);
    }
    if outdated_only {
        rows.retain(|row| row.outdated);
    }

    rows.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.installed.cmp(&left.installed))
            .then_with(|| left.name.len().cmp(&right.name.len()))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.backend.id().cmp(right.backend.id()))
    });

    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert((row.backend.id().to_string(), row.name.to_ascii_lowercase())));
    rows.truncate(limit);

    if cli.json {
        let results: Vec<SearchResult> = rows.iter().map(SearchResult::from_row).collect();
        output::json(&SearchReport {
            schema_version: 1,
            term: term.to_string(),
            best_match: results.first().cloned(),
            results,
        })?;
    } else {
        output::header(&format!("Search results for \"{term}\""));
        if rows.is_empty() {
            output::info("no matches");
            return Ok(());
        }
        output::detail(&format!(
            "  {:<8} {:<34} {:<14} {}",
            "BACKEND", "PACKAGE", "VERSION", "STATUS"
        ));
        for row in &rows {
            let status = if row.outdated {
                "update available"
            } else if row.installed {
                "installed"
            } else {
                ""
            };
            output::detail(&format!(
                "  {:<8} {:<34} {:<14} {}",
                row.backend.id(),
                truncate(&row.name, 34),
                row.version.as_deref().unwrap_or("-"),
                status
            ));
            if let Some(description) = &row.description {
                output::detail(&format!("  {:<8} {}", "", output::dim(description)));
            }
        }
        if let Some(best) = rows.first() {
            output::detail(&format!(
                "\nBest match: {} via {}",
                output::bold(&best.name),
                best.backend.id()
            ));
        }
    }

    if interactive {
        interactive_install(&rows, cli, cfg, runner)?;
    }
    Ok(())
}

fn cmd_list(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
    outdated_only: bool,
    search: Option<&str>,
    limit: Option<usize>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let installed = collect_installed_sets(&scope, runner);
    let outdated = if outdated_only {
        collect_outdated_set(&scope, runner)
    } else {
        HashSet::new()
    };

    let mut rows = Vec::new();
    for backend in &scope {
        let Some(packages) = installed.get(backend.id()) else {
            continue;
        };
        for (name, version) in packages {
            let is_outdated =
                outdated.contains(&(backend.id().to_string(), name.to_ascii_lowercase()));
            if outdated_only && !is_outdated {
                continue;
            }
            if let Some(term) = search {
                if !parse::matches_fuzzy(term, name) {
                    continue;
                }
            }
            rows.push(ListPackage {
                name: name.clone(),
                version: version.clone(),
                backend: backend.id().to_string(),
                outdated: is_outdated,
            });
        }
    }

    rows.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.backend.cmp(&right.backend))
    });
    if let Some(limit) = limit {
        rows.truncate(limit);
    }

    if cli.json {
        output::json(&ListReport {
            schema_version: 1,
            total: rows.len(),
            packages: rows,
        })?;
        return Ok(());
    }

    output::header("Installed packages");
    if rows.is_empty() {
        output::info("no packages matched");
        return Ok(());
    }
    output::detail(&format!(
        "  {:<34} {:<16} {:<8} {}",
        "PACKAGE", "VERSION", "BACKEND", "STATUS"
    ));
    for row in &rows {
        output::detail(&format!(
            "  {:<34} {:<16} {:<8} {}",
            truncate(&row.name, 34),
            row.version.as_deref().unwrap_or("-"),
            row.backend,
            if row.outdated { "update available" } else { "" }
        ));
    }
    output::detail(&format!("\n{} package{}", rows.len(), plural(rows.len())));
    Ok(())
}

fn cmd_outdated(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
    security_only: bool,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let mut rows = collect_outdated(&scope, runner);
    if security_only {
        rows.retain(|row| row.security);
    }

    if cli.json {
        let packages: Vec<OutdatedPackage> = rows.iter().map(OutdatedPackage::from_row).collect();
        output::json(&OutdatedReport {
            schema_version: 1,
            total: packages.len(),
            security: packages.iter().filter(|row| row.security).count(),
            packages,
        })?;
        return Ok(());
    }

    output::header("Available updates");
    if rows.is_empty() {
        output::success("everything is up to date");
        return Ok(());
    }
    output::detail(&format!(
        "  {:<8} {:<30} {:<16} {:<16} {}",
        "BACKEND", "PACKAGE", "CURRENT", "AVAILABLE", "SECURITY"
    ));
    for row in &rows {
        output::detail(&format!(
            "  {:<8} {:<30} {:<16} {:<16} {}",
            row.backend.id(),
            truncate(&row.name, 30),
            row.current.as_deref().unwrap_or("-"),
            row.available.as_deref().unwrap_or("-"),
            if row.security { "yes" } else { "" }
        ));
    }
    output::detail(&format!(
        "\n{} update{} ({} security)",
        rows.len(),
        plural(rows.len()),
        rows.iter().filter(|row| row.security).count()
    ));
    Ok(())
}

fn cmd_info(
    cli: &Cli,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    package: &str,
    forced: Option<BackendKind>,
) -> Result<()> {
    let names = vec![package.to_string()];
    let resolved = resolver::resolve(&names, forced, table, backends, runner)?;
    let item = &resolved[0];
    let Some(argv) = item.backend.info_cmd(&item.package) else {
        return Err(exit(
            EXIT_UNSUPPORTED,
            format!("{} cannot show package details", item.backend.id()),
        ));
    };
    let out = runner.capture(&argv)?;
    let text = combined_output(&out);

    if cli.json {
        output::json(&InfoReport {
            schema_version: 1,
            requested: item.requested.clone(),
            backend: item.backend.id().to_string(),
            package: item.package.clone(),
            success: out.status.success(),
            output: text.trim_end().to_string(),
        })?;
    } else {
        output::header(&format!("{} — {}", item.requested, item.backend.id()));
        if text.trim().is_empty() {
            output::warn("the backend returned no details");
        } else {
            output::detail(text.trim_end());
        }
    }

    if !out.status.success() {
        return Err(exit(
            EXIT_NOT_FOUND,
            format!(
                "no package named `{}` via {}",
                item.requested,
                item.backend.id()
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// which / doctor / resolve / explain / capabilities
// ---------------------------------------------------------------------------

fn cmd_which(cli: &Cli, cfg: &Config, table: &AliasTable, backends: &[Backend]) -> Result<()> {
    let report = environment(cfg, backends, cli.backend.as_deref(), table.len());

    if cli.json {
        return output::json(&report);
    }

    output::header("Environment");
    output::detail(&format!("  {:<9} {}", "OS", report.os));
    if let Some(distro) = &report.distro {
        output::detail(&format!("  {:<9} {}", "Distro", distro));
    }
    if let Some(container) = &report.container {
        output::detail(&format!("  {:<9} {}", "Container", container));
    }
    output::detail(&format!("  {:<9} {}", "Arch", report.arch));
    output::detail(&format!(
        "  {:<9} {}",
        "Config",
        report.config.as_deref().unwrap_or("(none)")
    ));
    output::detail(&format!(
        "  {:<9} {}",
        "State",
        report.state.as_deref().unwrap_or("(none)")
    ));
    output::detail(&format!("  {:<9} {}", "Aliases", report.aliases));
    output::detail(&format!(
        "  {:<9} {}",
        "Admin",
        if report.root { "yes" } else { "no" }
    ));
    if let Some(forced) = &report.forced {
        output::detail(&format!("  {:<9} {}", "Forced", forced));
    }

    output::header("Backends");
    for info in &report.backends {
        let mark = if info.available {
            output::green("✓")
        } else {
            output::red("✗")
        };
        let scope = if info.selected {
            "selected"
        } else if !info.enabled {
            "disabled"
        } else {
            "not available"
        };
        let elevation = if info.requires_elevation {
            "root"
        } else {
            "user"
        };
        output::detail(&format!(
            "  {mark} {:<8} {:<10} {:<8} {}",
            info.id,
            scope,
            elevation,
            info.path.as_deref().unwrap_or("-")
        ));
        if let Some(version) = &info.version {
            output::detail(&format!("    {}", output::dim(version)));
        }
    }
    Ok(())
}

fn cmd_doctor(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    fix: bool,
) -> Result<()> {
    let report = environment(cfg, backends, cli.backend.as_deref(), table.len());
    let mut problems: Vec<Problem> = Vec::new();

    if backends.is_empty() {
        problems.push(Problem {
            message: "no supported package manager is installed or enabled".to_string(),
            hint: Some(
                "install a supported package manager or enable one in the config".to_string(),
            ),
        });
    }
    if !report.root
        && backends.iter().any(|backend| backend.needs_elevation())
        && which::which("sudo").is_err()
        && cfg!(unix)
    {
        problems.push(Problem {
            message: "some backends need root but `sudo` is not installed".to_string(),
            hint: Some("run upm as root or install sudo".to_string()),
        });
    }
    if table.is_empty() {
        problems.push(Problem {
            message: "the alias registry is empty; package names will be passed through"
                .to_string(),
            hint: Some(
                "run `upm config init` and add `[packages.*]` overrides if needed".to_string(),
            ),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(path) = Config::path() {
            if let Ok(meta) = std::fs::metadata(&path) {
                let mode = meta.permissions().mode() & 0o777;
                if mode & 0o077 != 0 {
                    problems.push(Problem {
                        message: format!(
                            "config file {} is readable by other users",
                            path.display()
                        ),
                        hint: Some(format!("chmod 600 {}", path.display())),
                    });
                }
            }
        }
        if std::env::var_os("UPM_CONFIG").is_some() {
            if let Some(default) = dirs::config_dir()
                .map(|dir| dir.join("upm").join("config.toml"))
                .filter(|path| path.exists())
            {
                problems.push(Problem {
                    message: format!("UPM_CONFIG is set while {} also exists", default.display()),
                    hint: Some("unset UPM_CONFIG or remove the duplicate config".to_string()),
                });
            }
        }
    }

    if cli.json {
        output::json(&DoctorReport {
            schema_version: 1,
            environment: report,
            problems: problems.clone(),
        })?;
    } else {
        output::header("Diagnostics");
        output::detail(&format!(
            "  {} OS        {} ({})",
            output::green("✓"),
            detect::os_name(),
            detect::arch()
        ));
        if let Some(container) = &report.container {
            output::detail(&format!(
                "  {} Container {}",
                output::yellow("•"),
                container
            ));
        }
        output::detail(&format!(
            "  {} Admin     {}",
            if report.root {
                output::green("✓")
            } else {
                output::yellow("•")
            },
            if report.root {
                "yes"
            } else {
                "no (elevation will use sudo)"
            }
        ));
        output::detail(&format!(
            "  {} Config    {}",
            output::green("✓"),
            report.config.as_deref().unwrap_or("(none)")
        ));

        output::header("Backends");
        for info in &report.backends {
            let mark = if info.selected {
                output::green("✓")
            } else if info.available {
                output::yellow("•")
            } else {
                output::red("✗")
            };
            let status = if info.selected {
                "ready"
            } else if !info.enabled {
                "disabled in config"
            } else if info.available {
                "available"
            } else {
                "not installed"
            };
            output::detail(&format!(
                "  {mark} {:<8} {}{}",
                info.id,
                status,
                info.version
                    .as_deref()
                    .map(|version| format!("  {}", output::dim(version)))
                    .unwrap_or_default()
            ));
        }

        for problem in &problems {
            output::warn(&problem.message);
        }
        if problems.is_empty() {
            output::success("everything looks good");
        } else if fix {
            for problem in &problems {
                if let Some(hint) = &problem.hint {
                    output::detail(&format!("  {} {hint}", output::dim("fix:")));
                }
            }
        }
    }

    if backends.is_empty() {
        return Err(exit(
            EXIT_NO_BACKEND,
            "no supported package manager detected",
        ));
    }
    Ok(())
}

fn cmd_resolve(
    cli: &Cli,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
    explain: bool,
) -> Result<()> {
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    if cli.json {
        return output::json(
            &resolved
                .iter()
                .map(PackageRecord::from_resolved)
                .collect::<Vec<_>>(),
        );
    }

    if explain {
        output::header("Resolution");
        output::detail(&format!("  {:<10} {}", "OS", detect::os_name()));
        output::detail(&format!(
            "  {:<10} {}",
            "Distro",
            detect::distro().unwrap_or_else(|| "-".into())
        ));
        output::detail(&format!("  {:<10} {}", "Arch", detect::arch()));
        let available = backends
            .iter()
            .map(|backend| backend.id())
            .collect::<Vec<_>>()
            .join(", ");
        output::detail(&format!("  {:<10} {available}", "Backends"));
        for item in &resolved {
            output::detail(&format!("\n  Requested: {}", item.requested));
            output::detail(&format!(
                "  Selected:  {} -> {}",
                item.backend.id(),
                item.package
            ));
            output::detail(&format!("  Reason:    {}", item.reason.as_str()));
            output::detail(&format!(
                "  Command:   {}",
                output::dim(&output::shell_join(&action_command(
                    "install",
                    item,
                    runner.assume_yes
                )))
            ));
        }
        return Ok(());
    }

    output::header("Resolution");
    for item in &resolved {
        output::detail(&format!(
            "  {} {:<8} {}  {}",
            output::green("•"),
            item.backend.id(),
            item.display_target(),
            output::dim(item.reason.as_str())
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_explain(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    action: &str,
    packages: &[String],
    forced: Option<BackendKind>,
) -> Result<()> {
    validate_action(action)?;
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    let mut items = Vec::new();

    for item in &resolved {
        let installed = installed_state(item, runner).unwrap_or(None);
        let command = output::shell_join(&action_command(action, item, runner.assume_yes));
        items.push(ExplainItem {
            requested: item.requested.clone(),
            backend: item.backend.id().to_string(),
            package: item.package.clone(),
            reason: item.reason.as_str().to_string(),
            installed,
            command,
        });
    }

    if cli.json {
        return output::json(&ExplainReport {
            schema_version: 1,
            action: action.to_string(),
            os: detect::os_name().to_string(),
            arch: detect::arch().to_string(),
            distro: detect::distro(),
            packages: items,
            executed: false,
        });
    }

    output::header(&format!("Explain: {action}"));
    output::detail(&format!("  {:<10} {}", "OS", detect::os_name()));
    if let Some(container) = detect::container() {
        output::detail(&format!("  {:<10} {}", "Container", container));
    }
    output::detail(&format!("  {:<10} {}", "Arch", detect::arch()));
    for item in &items {
        output::detail(&format!("\n  Request:   {}", item.requested));
        output::detail(&format!(
            "  Resolution {} -> {} ({})",
            item.backend, item.package, item.reason
        ));
        if let Some(installed) = item.installed {
            output::detail(&format!(
                "  Installed: {}",
                if installed { "yes" } else { "no" }
            ));
        }
        output::detail(&format!("  Command:   {}", output::dim(&item.command)));
    }
    output::header("Safety");
    output::detail(&format!("  {} package names validated", output::green("✓")));
    output::detail(&format!(
        "  {} shell not used (argv execution)",
        output::green("✓")
    ));
    output::detail(&format!(
        "  {} confirmation required unless --yes/--non-interactive",
        output::green("✓")
    ));
    output::detail("\nExecution: not executed (explain)");
    let _ = cfg;
    Ok(())
}

fn cmd_capabilities(cli: &Cli, backend: Option<&str>) -> Result<()> {
    let kinds: Vec<BackendKind> = match backend {
        Some(value) => vec![resolver::parse_backend(value)?],
        None => BackendKind::ALL.to_vec(),
    };

    let rows: Vec<CapabilityRow> = kinds
        .iter()
        .map(|kind| CapabilityRow {
            backend: kind.id().to_string(),
            available: Backend::new(*kind).available(),
            capabilities: Capabilities::of(*kind),
        })
        .collect();

    if cli.json {
        return output::json(&CapabilityReport {
            schema_version: 1,
            backends: rows,
        });
    }

    output::header("Capabilities");
    output::detail(&format!(
        "  {:<8} {:<10} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4}",
        "BACKEND",
        "AVAILABLE",
        "INST",
        "REM",
        "UPG",
        "UPA",
        "UPD",
        "SRC",
        "INF",
        "LST",
        "OUT",
        "CLN",
        "PIN",
        "PNS",
        "WHY",
        "DEP",
        "REP",
        "RBK",
        "SEC",
        "INT"
    ));
    for row in &rows {
        let marks: Vec<&str> = row
            .capabilities
            .entries()
            .iter()
            .map(|(_, supported)| if *supported { "✓" } else { "✗" })
            .collect();
        output::detail(&format!(
            "  {:<8} {:<10} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4} {:<4}",
            row.backend,
            if row.available { "yes" } else { "no" },
            marks[0],
            marks[1],
            marks[2],
            marks[3],
            marks[4],
            marks[5],
            marks[6],
            marks[7],
            marks[8],
            marks[9],
            marks[10],
            marks[11],
            marks[12],
            marks[13],
            marks[14],
            marks[15],
            marks[16],
            marks[17]
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// clean / pin / why / history / rollback
// ---------------------------------------------------------------------------

fn cmd_clean(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
    all: bool,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let mut tasks = Vec::new();
    for backend in &scope {
        for argv in backend.clean_cmds(all) {
            tasks.push((*backend, argv));
        }
    }
    if tasks.is_empty() {
        return Err(exit(
            EXIT_UNSUPPORTED,
            "no available backend supports cleanup",
        ));
    }
    run_tasks("clean", tasks, cli, cfg, runner, Vec::new())
}

#[allow(clippy::too_many_arguments)]
fn cmd_pin(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
    pin: bool,
) -> Result<()> {
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    let action = if pin { "pin" } else { "unpin" };
    let mut tasks = Vec::new();
    let mut unsupported = Vec::new();
    for item in &resolved {
        let argv = if pin {
            item.backend.pin_cmd(&item.package)
        } else {
            item.backend.unpin_cmd(&item.package)
        };
        match argv {
            Some(argv) => tasks.push((item.backend, argv)),
            None => unsupported.push(item.backend.id().to_string()),
        }
    }
    if !unsupported.is_empty() {
        return Err(exit(
            EXIT_UNSUPPORTED,
            format!(
                "{} does not support {action} (try `upm capabilities`)",
                unsupported.join(", ")
            ),
        ));
    }
    let history_packages = resolved
        .iter()
        .map(|item| HistoryPackage {
            requested: item.requested.clone(),
            backend: item.backend.id().to_string(),
            package: item.package.clone(),
        })
        .collect();
    run_tasks(action, tasks, cli, cfg, runner, history_packages)
}

fn cmd_pins(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let commands: Vec<(Backend, Vec<String>)> = scope
        .iter()
        .filter_map(|backend| backend.pins_cmd().map(|argv| (*backend, argv)))
        .collect();

    if commands.is_empty() {
        return Err(exit(
            EXIT_UNSUPPORTED,
            "no available backend can list pinned packages",
        ));
    }

    let outputs = capture_parallel(&commands, runner);
    let mut results = Vec::new();
    for (backend, result) in outputs {
        match result {
            Ok(out) => {
                let text = combined_output(&out).trim_end().to_string();
                if cli.json {
                    results.push(BackendOutput {
                        backend: backend.id().to_string(),
                        success: out.status.success(),
                        output: text,
                    });
                } else {
                    output::header(&format!("{} — {}", backend.label(), backend.id()));
                    if text.trim().is_empty() {
                        output::info("no pinned packages");
                    } else {
                        output::detail(&text);
                    }
                }
            }
            Err(err) => {
                if cli.json {
                    results.push(BackendOutput {
                        backend: backend.id().to_string(),
                        success: false,
                        output: err.to_string(),
                    });
                } else {
                    output::warn(&format!("{}: {err}", backend.id()));
                }
            }
        }
    }
    if cli.json {
        output::json(&PinsReport {
            schema_version: 1,
            results,
        })?;
    }
    Ok(())
}

fn cmd_why(
    cli: &Cli,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    package: &str,
    forced: Option<BackendKind>,
    reverse: bool,
) -> Result<()> {
    let names = vec![package.to_string()];
    let resolved = resolver::resolve(&names, forced, table, backends, runner)?;
    let item = &resolved[0];
    let capabilities = Capabilities::of(item.backend.kind());
    let supported = if reverse {
        capabilities.why
    } else {
        capabilities.depends
    };
    let argv = if reverse {
        item.backend.why_cmd(&item.package)
    } else {
        item.backend.depends_cmd(&item.package)
    };
    let Some(argv) = argv.filter(|_| supported) else {
        return Err(exit(
            EXIT_UNSUPPORTED,
            format!(
                "{} cannot report {} (try `upm capabilities`)",
                item.backend.id(),
                if reverse {
                    "reverse dependencies"
                } else {
                    "dependencies"
                }
            ),
        ));
    };

    let out = runner.capture(&argv)?;
    let text = combined_output(&out);

    if cli.json {
        output::json(&WhyReport {
            schema_version: 1,
            package: item.requested.clone(),
            backend: item.backend.id().to_string(),
            native: item.package.clone(),
            success: out.status.success(),
            output: text.trim_end().to_string(),
        })?;
    } else {
        output::header(&format!(
            "{} — {}",
            item.requested,
            if reverse { "required by" } else { "depends on" }
        ));
        if text.trim().is_empty() {
            output::info("no results");
        } else {
            output::detail(text.trim_end());
        }
    }

    if !out.status.success() {
        return Err(exit(
            EXIT_ERROR,
            format!(
                "{} could not report dependencies for {}",
                item.backend.id(),
                item.requested
            ),
        ));
    }
    Ok(())
}

fn cmd_history(cli: &Cli, command: Option<&HistoryCmd>) -> Result<()> {
    match command {
        None => {
            let entries = history::read_all()?;
            let start = entries.len().saturating_sub(50);
            let shown = &entries[start..];
            if cli.json {
                return output::json(&HistoryReport {
                    schema_version: 1,
                    entries,
                });
            }
            output::header("History");
            if shown.is_empty() {
                output::info("no transactions recorded yet");
                return Ok(());
            }
            output::detail(&format!(
                "  {:<6} {:<10} {:<10} {:<8} {}",
                "ID", "ACTION", "STATUS", "PACKAGES", "TIME"
            ));
            for entry in shown {
                output::detail(&format!(
                    "  {:<6} {:<10} {:<10} {:<8} {}{}",
                    entry.id,
                    entry.action,
                    entry.status,
                    entry.packages.len(),
                    history::age(entry.timestamp_unix),
                    if entry.dry_run { " (dry-run)" } else { "" }
                ));
            }
            if let Some(path) = history::history_path() {
                output::detail(&format!("\nHistory file: {}", path.display()));
            }
            Ok(())
        }
        Some(HistoryCmd::Show { id }) => {
            let Some(entry) = history::find(*id)? else {
                return Err(exit(
                    EXIT_NOT_FOUND,
                    format!("transaction {id} was not found"),
                ));
            };
            if cli.json {
                return output::json(&entry);
            }
            output::header(&format!("Transaction {}", entry.id));
            output::detail(&format!("  Action:  {}", entry.action));
            output::detail(&format!("  Status:  {}", entry.status));
            output::detail(&format!("  Dry run: {}", entry.dry_run));
            output::detail(&format!(
                "  When:    {}",
                history::age(entry.timestamp_unix)
            ));
            if entry.packages.is_empty() {
                output::detail("  Packages: (none)");
            } else {
                output::detail("  Packages:");
                for package in &entry.packages {
                    output::detail(&format!(
                        "    - {} -> {} ({})",
                        package.requested, package.package, package.backend
                    ));
                }
            }
            if !entry.commands.is_empty() {
                output::detail("  Commands:");
                for command in &entry.commands {
                    output::detail(&format!("    {}", output::dim(command)));
                }
            }
            Ok(())
        }
        Some(HistoryCmd::Export { path }) => {
            let path = path
                .clone()
                .unwrap_or_else(|| PathBuf::from("upm-history.jsonl"));
            let count = history::export(&path)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "path": path.display().to_string(),
                    "transactions": count,
                }))
            } else {
                output::success(&format!(
                    "exported {count} transaction{} to {}",
                    plural(count),
                    path.display()
                ));
                Ok(())
            }
        }
    }
}

fn cmd_rollback(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    id: Option<u64>,
    _forced: Option<BackendKind>,
) -> Result<()> {
    let entry = match id {
        Some(id) => history::find(id)?,
        None => history::last_reversible()?,
    };
    let Some(entry) = entry else {
        return Err(exit(
            EXIT_NOT_FOUND,
            "no reversible transaction found (rollback supports install and remove)",
        ));
    };
    if entry.dry_run {
        return Err(exit(
            EXIT_UNSUPPORTED,
            "dry-run transactions cannot be rolled back",
        ));
    }
    let inverse = match entry.action.as_str() {
        "install" => "remove",
        "remove" => "install",
        other => {
            return Err(exit(
                EXIT_UNSUPPORTED,
                format!("cannot roll back a `{other}` transaction"),
            ));
        }
    };

    let mut plan = Vec::new();
    for package in &entry.packages {
        let kind = package
            .backend
            .parse::<BackendKind>()
            .map_err(|message| exit(EXIT_USAGE, message))?;
        let Some(backend) = backends
            .iter()
            .copied()
            .find(|backend| backend.kind() == kind)
        else {
            return Err(exit(
                EXIT_NOT_FOUND,
                format!("backend `{kind}` is not available for rollback"),
            ));
        };
        validate::package_name(&package.package)?;
        plan.push(Resolved {
            requested: package.requested.clone(),
            backend,
            package: package.package.clone(),
            reason: Reason::Fallback,
        });
    }

    output::info(&format!(
        "Rolling back transaction {} ({} {})",
        entry.id,
        inverse,
        entry.packages.len()
    ));
    run_action(inverse, &plan, cli, cfg, runner)
}

// ---------------------------------------------------------------------------
// manifests / profiles / snapshots
// ---------------------------------------------------------------------------

fn manifest_path(file: Option<&Path>) -> PathBuf {
    file.map(Path::to_path_buf)
        .unwrap_or_else(manifest::default_manifest_path)
}

fn cmd_init(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    table: &AliasTable,
    file: Option<&Path>,
    from_system: bool,
) -> Result<()> {
    let path = manifest_path(file);
    if path.exists() && !cli.force {
        return Err(exit(
            EXIT_USAGE,
            format!(
                "{} already exists (use --force to overwrite)",
                path.display()
            ),
        ));
    }
    let mut manifest = if from_system {
        capture_manifest(backends, runner, table)
    } else {
        Manifest::default()
    };
    manifest.environment.name = std::env::current_dir().ok().and_then(|dir| {
        dir.file_name()
            .map(|name| name.to_string_lossy().to_string())
    });
    manifest.save(&path)?;

    if cli.json {
        output::json(&serde_json::json!({
            "schema_version": 1,
            "path": path.display().to_string(),
            "packages": manifest.packages.len(),
            "from_system": from_system,
        }))
    } else {
        output::success(&format!(
            "created {} with {} package{}",
            path.display(),
            manifest.packages.len(),
            plural(manifest.packages.len())
        ));
        Ok(())
    }
}

fn cmd_export(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    table: &AliasTable,
    file: Option<&Path>,
) -> Result<()> {
    let path = manifest_path(file);
    if path.exists() && !cli.force {
        return Err(exit(
            EXIT_USAGE,
            format!(
                "{} already exists (use --force to overwrite)",
                path.display()
            ),
        ));
    }
    let manifest = capture_manifest(backends, runner, table);
    manifest.save(&path)?;

    if cli.json {
        output::json(&serde_json::json!({
            "schema_version": 1,
            "path": path.display().to_string(),
            "packages": manifest.packages.len(),
        }))
    } else {
        output::success(&format!(
            "exported {} package{} to {}",
            manifest.packages.len(),
            plural(manifest.packages.len()),
            path.display()
        ));
        Ok(())
    }
}

fn cmd_sync(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    file: Option<&Path>,
    forced: Option<BackendKind>,
) -> Result<()> {
    let path = manifest_path(file);
    if !path.exists() {
        return Err(exit(
            EXIT_NOT_FOUND,
            format!(
                "{} does not exist (create it with `upm init`)",
                path.display()
            ),
        ));
    }
    let manifest = Manifest::load(&path)?;
    let packages = manifest.packages_for_current_platform();
    if packages.is_empty() {
        output::info("the manifest declares no packages");
        return Ok(());
    }
    if packages.values().any(|version| version != "*") {
        output::warn("version constraints are recorded but not enforced by backend managers");
    }
    install_names(
        &packages.keys().cloned().collect::<Vec<_>>(),
        forced,
        cli,
        cfg,
        table,
        backends,
        runner,
    )
}

fn cmd_profile(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    command: &ProfileCmd,
    forced: Option<BackendKind>,
) -> Result<()> {
    match command {
        ProfileCmd::List => {
            let profiles = manifest::list_profiles()?;
            if cli.json {
                let rows: Vec<_> = profiles
                    .iter()
                    .map(|(name, manifest)| {
                        serde_json::json!({
                            "name": name,
                            "packages": manifest.packages.len(),
                            "description": manifest.environment.description,
                        })
                    })
                    .collect();
                return output::json(&serde_json::json!({
                    "schema_version": 1,
                    "profiles": rows,
                }));
            }
            output::header("Profiles");
            if profiles.is_empty() {
                output::info("no profiles yet (try `upm profile create developer`)");
                return Ok(());
            }
            output::detail(&format!(
                "  {:<20} {:<8} {}",
                "NAME", "PACKAGES", "DESCRIPTION"
            ));
            for (name, manifest) in &profiles {
                output::detail(&format!(
                    "  {:<20} {:<8} {}",
                    name,
                    manifest.packages.len(),
                    manifest.environment.description.as_deref().unwrap_or("")
                ));
            }
            Ok(())
        }
        ProfileCmd::Create { name } => {
            if manifest::profile_path(name)?.exists() && !cli.force {
                return Err(exit(
                    EXIT_USAGE,
                    format!("profile `{name}` already exists (use --force to overwrite)"),
                ));
            }
            let mut manifest = Manifest::default();
            manifest.environment.name = Some(name.clone());
            let path = manifest::save_profile(name, &manifest)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "path": path.display().to_string(),
                }))
            } else {
                output::success(&format!("created profile `{name}` at {}", path.display()));
                Ok(())
            }
        }
        ProfileCmd::Delete { name } => {
            let removed = manifest::delete_profile(name)?;
            if !removed {
                return Err(exit(
                    EXIT_NOT_FOUND,
                    format!("profile `{name}` was not found"),
                ));
            }
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "deleted": true,
                }))
            } else {
                output::success(&format!("deleted profile `{name}`"));
                Ok(())
            }
        }
        ProfileCmd::Add { name, packages } => {
            let mut manifest = manifest::load_profile(name)?;
            for package in packages {
                validate::package_name(package)?;
            }
            manifest.add(packages);
            manifest::save_profile(name, &manifest)?;
            output::success(&format!(
                "added {} package{} to profile `{name}`",
                packages.len(),
                plural(packages.len())
            ));
            Ok(())
        }
        ProfileCmd::Remove { name, packages } => {
            let mut manifest = manifest::load_profile(name)?;
            let removed = manifest.remove(packages);
            manifest::save_profile(name, &manifest)?;
            if removed.is_empty() {
                output::info("none of those packages were in the profile");
            } else {
                output::success(&format!(
                    "removed {} package{} from profile `{name}`",
                    removed.len(),
                    plural(removed.len())
                ));
            }
            Ok(())
        }
        ProfileCmd::Show { name } => {
            let manifest = manifest::load_profile(name)?;
            if cli.json {
                output::json(&manifest)
            } else {
                print!("{}", toml::to_string_pretty(&manifest)?);
                Ok(())
            }
        }
        ProfileCmd::Install { name } => {
            let manifest = manifest::load_profile(name)?;
            let packages = manifest.packages_for_current_platform();
            if packages.is_empty() {
                output::info("the profile declares no packages");
                return Ok(());
            }
            install_names(
                &packages.keys().cloned().collect::<Vec<_>>(),
                forced,
                cli,
                cfg,
                table,
                backends,
                runner,
            )
        }
        ProfileCmd::Export { name, path } => {
            let manifest = manifest::load_profile(name)?;
            let path = path
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{name}.toml")));
            manifest.save(&path)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "path": path.display().to_string(),
                }))
            } else {
                output::success(&format!("exported profile `{name}` to {}", path.display()));
                Ok(())
            }
        }
        ProfileCmd::Import { path, name } => {
            let mut manifest = manifest::Manifest::load(path)?;
            let name = name
                .clone()
                .or_else(|| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().to_string())
                })
                .ok_or_else(|| exit(EXIT_USAGE, "could not infer a profile name"))?;
            if manifest::profile_path(&name)?.exists() && !cli.force {
                return Err(exit(
                    EXIT_USAGE,
                    format!("profile `{name}` already exists (use --force to overwrite)"),
                ));
            }
            manifest.environment.name = Some(name.clone());
            let path = manifest::save_profile(&name, &manifest)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "path": path.display().to_string(),
                }))
            } else {
                output::success(&format!(
                    "imported profile `{name}` from {}",
                    path.display()
                ));
                Ok(())
            }
        }
    }
}

fn cmd_snapshot(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    command: &SnapshotCmd,
    forced: Option<BackendKind>,
) -> Result<()> {
    match command {
        SnapshotCmd::Create { name } => {
            let name = name
                .clone()
                .unwrap_or_else(|| format!("snapshot-{}", history::now_unix()));
            let path = manifest::snapshot_path(&name)?;
            if path.exists() && !cli.force {
                return Err(exit(
                    EXIT_USAGE,
                    format!("snapshot `{name}` already exists (use --force to overwrite)"),
                ));
            }
            let mut snapshot = capture_snapshot(&name, backends, runner);
            snapshot.name = name.clone();
            let path = manifest::save_snapshot(&snapshot)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "path": path.display().to_string(),
                    "packages": snapshot.packages.len(),
                }))
            } else {
                output::success(&format!(
                    "captured {} package{} into snapshot `{name}`",
                    snapshot.packages.len(),
                    plural(snapshot.packages.len())
                ));
                Ok(())
            }
        }
        SnapshotCmd::List => {
            let snapshots = manifest::list_snapshots()?;
            if cli.json {
                return output::json(&serde_json::json!({
                    "schema_version": 1,
                    "snapshots": snapshots,
                }));
            }
            output::header("Snapshots");
            if snapshots.is_empty() {
                output::info("no snapshots yet (try `upm snapshot create`)");
                return Ok(());
            }
            output::detail(&format!(
                "  {:<24} {:<8} {:<8} {}",
                "NAME", "PACKAGES", "CREATED", "BACKENDS"
            ));
            for snapshot in &snapshots {
                output::detail(&format!(
                    "  {:<24} {:<8} {:<8} {}",
                    snapshot.name,
                    snapshot.packages.len(),
                    history::age(snapshot.created_unix),
                    snapshot.backends.join(", ")
                ));
            }
            Ok(())
        }
        SnapshotCmd::Show { name } => {
            let snapshot = manifest::load_snapshot(name)?;
            output::json(&snapshot)
        }
        SnapshotCmd::Delete { name } => {
            let removed = manifest::delete_snapshot(name)?;
            if !removed {
                return Err(exit(
                    EXIT_NOT_FOUND,
                    format!("snapshot `{name}` was not found"),
                ));
            }
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "deleted": true,
                }))
            } else {
                output::success(&format!("deleted snapshot `{name}`"));
                Ok(())
            }
        }
        SnapshotCmd::Export { name, path } => {
            let snapshot = manifest::load_snapshot(name)?;
            let path = path
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{name}.json")));
            snapshot.save(&path)?;
            if cli.json {
                output::json(&serde_json::json!({
                    "schema_version": 1,
                    "name": name,
                    "path": path.display().to_string(),
                }))
            } else {
                output::success(&format!("exported snapshot `{name}` to {}", path.display()));
                Ok(())
            }
        }
        SnapshotCmd::Diff { a, b } => {
            let left = load_snapshot_arg(a)?;
            let right = load_snapshot_arg(b)?;
            let diff = left.diff(&right);
            if cli.json {
                return output::json(&serde_json::json!({
                    "schema_version": 1,
                    "a": left.name,
                    "b": right.name,
                    "diff": diff,
                }));
            }
            output::header(&format!("Diff: {} -> {}", left.name, right.name));
            if diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty() {
                output::success("snapshots are identical");
                return Ok(());
            }
            for package in &diff.added {
                output::detail(&format!(
                    "  {} {} ({})",
                    output::green("+"),
                    package.name,
                    package.backend
                ));
            }
            for package in &diff.removed {
                output::detail(&format!(
                    "  {} {} ({})",
                    output::red("-"),
                    package.name,
                    package.backend
                ));
            }
            for change in &diff.changed {
                output::detail(&format!(
                    "  {} {} ({}) {} -> {}",
                    output::yellow("~"),
                    change.name,
                    change.backend,
                    change.from.as_deref().unwrap_or("-"),
                    change.to.as_deref().unwrap_or("-")
                ));
            }
            Ok(())
        }
        SnapshotCmd::Restore { name } => {
            let snapshot = manifest::load_snapshot(name)?;
            let scope = select_backends(backends, forced)?;
            let installed = collect_installed_sets(&scope, runner);
            let mut plan = Vec::new();
            for package in &snapshot.packages {
                let kind = package
                    .backend
                    .parse::<BackendKind>()
                    .map_err(|message| exit(EXIT_USAGE, message))?;
                let Some(backend) = scope.iter().copied().find(|backend| backend.kind() == kind)
                else {
                    output::warn(&format!(
                        "skipping {}: backend `{kind}` is not available",
                        package.name
                    ));
                    continue;
                };
                validate::package_name(&package.name)?;
                if installed.get(backend.id()).is_some_and(|names| {
                    names
                        .iter()
                        .any(|(name, _)| name == &package.name.to_ascii_lowercase())
                }) {
                    continue;
                }
                plan.push(Resolved {
                    requested: package.name.clone(),
                    backend,
                    package: package.name.clone(),
                    reason: Reason::Fallback,
                });
            }
            if plan.is_empty() {
                output::success("all snapshot packages are already installed");
                return Ok(());
            }
            run_action("install", &plan, cli, cfg, runner)
        }
    }
}

fn load_snapshot_arg(value: &str) -> Result<Snapshot> {
    let path = manifest::resolve_snapshot_path(value);
    if !path.exists() {
        return Err(exit(
            EXIT_NOT_FOUND,
            format!("snapshot `{value}` was not found"),
        ));
    }
    Snapshot::load(&path)
}

fn capture_manifest(backends: &[Backend], runner: &Runner, table: &AliasTable) -> Manifest {
    let installed = collect_installed_sets(backends, runner);
    let mut manifest = Manifest::default();
    for backend in backends {
        let Some(packages) = installed.get(backend.id()) else {
            continue;
        };
        for (name, _) in packages {
            let canonical = table
                .canonical(backend.kind(), name)
                .unwrap_or(name)
                .to_string();
            manifest
                .packages
                .entry(canonical)
                .or_insert_with(|| "*".to_string());
        }
    }
    manifest
}

fn capture_snapshot(name: &str, backends: &[Backend], runner: &Runner) -> Snapshot {
    let installed = collect_installed_sets(backends, runner);
    let mut packages = Vec::new();
    let mut used = Vec::new();
    for backend in backends {
        let Some(entries) = installed.get(backend.id()) else {
            continue;
        };
        used.push(backend.id().to_string());
        for (package, version) in entries {
            packages.push(SnapshotPackage {
                name: package.clone(),
                version: version.clone(),
                backend: backend.id().to_string(),
            });
        }
    }
    packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.backend.cmp(&right.backend))
    });
    Snapshot {
        schema_version: manifest::SNAPSHOT_SCHEMA,
        name: name.to_string(),
        created_unix: history::now_unix(),
        os: detect::os_name().to_string(),
        arch: detect::arch().to_string(),
        distro: detect::distro(),
        backends: used,
        packages,
    }
}

// ---------------------------------------------------------------------------
// plan / apply
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn cmd_plan(
    cli: &Cli,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    action: &str,
    packages: &[String],
    forced: Option<BackendKind>,
    out: Option<&Path>,
) -> Result<()> {
    validate_action(action)?;
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    let mut planned = Vec::new();
    for item in resolved {
        if action == "install" && !cli.force {
            if let Some(true) = installed_state(&item, runner)? {
                continue;
            }
        }
        planned.push(item);
    }

    let plan = Plan {
        schema_version: crate::plan::SCHEMA_VERSION,
        action: action.to_string(),
        dry_run: true,
        packages: planned
            .iter()
            .map(|item| PlanPackage {
                requested: item.requested.clone(),
                backend: item.backend.id().to_string(),
                package: item.package.clone(),
            })
            .collect(),
        commands: planned
            .iter()
            .map(|item| output::shell_join(&action_command(action, item, runner.assume_yes)))
            .collect(),
    };

    if let Some(path) = out {
        plan.save(path)?;
    }

    if cli.json {
        output::json(&plan)?;
    } else {
        output::header(&format!("Plan: {action}"));
        if plan.packages.is_empty() {
            output::info("nothing to do");
        }
        for (item, command) in planned.iter().zip(&plan.commands) {
            output::detail(&format!(
                "  {} {:<8} {}\n    {} {}",
                output::green("•"),
                item.backend.id(),
                item.display_target(),
                output::dim("→"),
                output::dim(command)
            ));
        }
        if let Some(path) = out {
            output::success(&format!("wrote {}", path.display()));
        }
    }
    Ok(())
}

fn cmd_apply(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    file: &Path,
    _forced: Option<BackendKind>,
) -> Result<()> {
    let plan = Plan::load(file)?;
    if plan.schema_version > crate::plan::SCHEMA_VERSION {
        return Err(exit(
            EXIT_UNSUPPORTED,
            format!(
                "plan schema {} is newer than supported schema {}",
                plan.schema_version,
                crate::plan::SCHEMA_VERSION
            ),
        ));
    }
    validate_action(&plan.action)?;

    let mut resolved = Vec::new();
    for package in &plan.packages {
        let kind = package
            .backend
            .parse::<BackendKind>()
            .map_err(|message| exit(EXIT_USAGE, message))?;
        let Some(backend) = backends
            .iter()
            .copied()
            .find(|backend| backend.kind() == kind)
        else {
            return Err(exit(
                EXIT_NOT_FOUND,
                format!("backend `{kind}` is not available on this system"),
            ));
        };
        validate::package_name(&package.package)?;
        resolved.push(Resolved {
            requested: package.requested.clone(),
            backend,
            package: package.package.clone(),
            reason: Reason::Fallback,
        });
    }

    run_action(&plan.action, &resolved, cli, cfg, runner)
}

// ---------------------------------------------------------------------------
// config / completions
// ---------------------------------------------------------------------------

fn cmd_config(cli: &Cli, cfg: &Config, command: Option<&ConfigCmd>) -> Result<()> {
    match command {
        None | Some(ConfigCmd::Show) => {
            if cli.json {
                output::json(cfg)
            } else {
                print!("{}", toml::to_string_pretty(cfg)?);
                Ok(())
            }
        }
        Some(ConfigCmd::Path) => {
            let path = Config::path();
            if cli.json {
                output::json(&serde_json::json!({
                    "path": path.as_ref().map(|p| p.display().to_string()),
                }))
            } else {
                match path {
                    Some(path) => {
                        println!("{}", path.display());
                        Ok(())
                    }
                    None => {
                        output::warn("could not determine a config directory");
                        Ok(())
                    }
                }
            }
        }
        Some(ConfigCmd::Init) => {
            let existed = Config::path().map(|path| path.exists()).unwrap_or(false);
            let path = Config::write_default()?;
            if cli.json {
                output::json(&serde_json::json!({
                    "path": path.display().to_string(),
                    "created": !existed,
                }))
            } else if existed {
                output::info(&format!("config already exists at {}", path.display()));
                Ok(())
            } else {
                output::success(&format!("created {}", path.display()));
                Ok(())
            }
        }
    }
}

fn cmd_completions(shell: &str) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::generate;
    use clap_complete::shells::{Bash, Elvish, Fish, PowerShell, Zsh};

    let mut command = Cli::command();
    let mut stdout = io::stdout();
    match shell.to_ascii_lowercase().as_str() {
        "bash" => generate(Bash, &mut command, "upm", &mut stdout),
        "zsh" => generate(Zsh, &mut command, "upm", &mut stdout),
        "fish" => generate(Fish, &mut command, "upm", &mut stdout),
        "powershell" | "pwsh" => generate(PowerShell, &mut command, "upm", &mut stdout),
        "elvish" => generate(Elvish, &mut command, "upm", &mut stdout),
        other => {
            return Err(exit(
                EXIT_USAGE,
                format!("unsupported shell `{other}` (try bash, zsh, fish, powershell, elvish)"),
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn select_backends(backends: &[Backend], forced: Option<BackendKind>) -> Result<Vec<Backend>> {
    match forced {
        Some(kind) => {
            let selected: Vec<Backend> = backends
                .iter()
                .copied()
                .filter(|backend| backend.kind() == kind)
                .collect();
            if selected.is_empty() {
                Err(exit(
                    EXIT_USAGE,
                    format!("backend `{kind}` is not available on this system"),
                ))
            } else {
                Ok(selected)
            }
        }
        None => {
            if backends.is_empty() {
                Err(exit(
                    EXIT_NO_BACKEND,
                    "no supported package manager detected (try `upm doctor`)",
                ))
            } else {
                Ok(backends.to_vec())
            }
        }
    }
}

fn installed_state(item: &Resolved, runner: &Runner) -> Result<Option<bool>> {
    let Some(argv) = item.backend.probe_cmd(&item.package) else {
        return Ok(None);
    };
    match runner.capture(&argv) {
        Ok(out) => Ok(item.backend.interpret_installed(
            &item.package,
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
        )),
        Err(_) => Ok(None),
    }
}

fn combined_output(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if stdout.trim().is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        stdout
    }
}

fn confirm(cli: &Cli, cfg: &Config) -> Result<bool> {
    if cli.yes || cli.non_interactive || cli.dry_run || !cfg.general.confirm {
        return Ok(true);
    }
    if output::is_json() || output::is_quiet() {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Ok(true);
    }
    print!("{} ", output::bold("Proceed? [Y/n]"));
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut out: String = value.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn environment(
    cfg: &Config,
    selected: &[Backend],
    forced: Option<&str>,
    aliases: usize,
) -> EnvReport {
    let backends = detect::considered()
        .into_iter()
        .enumerate()
        .map(|(priority, kind)| {
            let backend = Backend::new(kind);
            let available = backend.available();
            BackendInfo {
                id: kind.id().to_string(),
                label: kind.label().to_string(),
                binary: kind.binary().to_string(),
                path: backend.path().map(|path| path.display().to_string()),
                available,
                enabled: cfg.backend_enabled(kind),
                selected: selected.iter().any(|item| item.kind() == kind),
                priority,
                requires_elevation: backend.needs_elevation(),
                version: if available {
                    exec::version_of(kind.binary())
                } else {
                    None
                },
                capabilities: Capabilities::of(kind),
            }
        })
        .collect();

    EnvReport {
        schema_version: 1,
        os: detect::os_name().to_string(),
        arch: detect::arch().to_string(),
        distro: detect::distro(),
        container: detect::container(),
        root: exec::is_admin(),
        config: Config::path().map(|path| path.display().to_string()),
        state: paths::state_dir().map(|path| path.display().to_string()),
        history: history::history_path().map(|path| path.display().to_string()),
        forced: forced.map(str::to_string),
        aliases,
        backends,
    }
}

fn capture_parallel(
    commands: &[(Backend, Vec<String>)],
    runner: &Runner,
) -> Vec<(Backend, Result<Output>)> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = commands
            .iter()
            .map(|(backend, argv)| {
                let backend = *backend;
                let argv = argv.clone();
                let runner = *runner;
                (backend, scope.spawn(move || runner.capture(&argv)))
            })
            .collect();
        handles
            .into_iter()
            .map(|(backend, handle)| {
                let result = handle
                    .join()
                    .unwrap_or_else(|_| Err(anyhow!("backend probe panicked")));
                (backend, result)
            })
            .collect()
    })
}

fn collect_installed_sets(
    backends: &[Backend],
    runner: &Runner,
) -> HashMap<String, Vec<(String, Option<String>)>> {
    let commands: Vec<(Backend, Vec<String>)> = backends
        .iter()
        .map(|backend| (*backend, backend.installed_cmd()))
        .collect();
    let mut out = HashMap::new();
    for (backend, result) in capture_parallel(&commands, runner) {
        match result {
            Ok(output) => {
                if !output.status.success() {
                    output::log(&format!(
                        "{} list exited with {}",
                        backend.id(),
                        output.status
                    ));
                }
                let text = String::from_utf8_lossy(&output.stdout);
                out.insert(
                    backend.id().to_string(),
                    parse::installed(backend.kind(), &text),
                );
            }
            Err(err) => output::warn(&format!("{}: {err}", backend.id())),
        }
    }
    out
}

#[derive(Debug, Clone)]
struct OutdatedRow {
    backend: Backend,
    name: String,
    current: Option<String>,
    available: Option<String>,
    security: bool,
}

fn collect_outdated(backends: &[Backend], runner: &Runner) -> Vec<OutdatedRow> {
    let commands: Vec<(Backend, Vec<String>)> = backends
        .iter()
        .map(|backend| (*backend, backend.outdated_cmd()))
        .collect();
    let mut rows = Vec::new();
    for (backend, result) in capture_parallel(&commands, runner) {
        match result {
            Ok(output) => {
                if !output.status.success() {
                    output::log(&format!(
                        "{} outdated check exited with {}",
                        backend.id(),
                        output.status
                    ));
                }
                let text = String::from_utf8_lossy(&output.stdout);
                for item in parse::outdated(backend.kind(), &text) {
                    rows.push(OutdatedRow {
                        backend,
                        name: item.name,
                        current: item.current,
                        available: item.available,
                        security: item.security,
                    });
                }
            }
            Err(err) => output::warn(&format!("{}: {err}", backend.id())),
        }
    }
    rows.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.backend.id().cmp(right.backend.id()))
    });
    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert((row.backend.id().to_string(), row.name.to_ascii_lowercase())));
    rows
}

fn collect_outdated_set(backends: &[Backend], runner: &Runner) -> HashSet<(String, String)> {
    collect_outdated(backends, runner)
        .into_iter()
        .map(|row| (row.backend.id().to_string(), row.name.to_ascii_lowercase()))
        .collect()
}

fn prompt_outdated(rows: &[OutdatedRow]) -> Result<Vec<OutdatedRow>> {
    if output::is_json() || output::is_quiet() || !io::stdin().is_terminal() {
        return Err(exit(
            EXIT_USAGE,
            "--interactive needs a terminal and cannot be used with --json/--quiet",
        ));
    }
    output::header("Select packages to upgrade");
    for (index, row) in rows.iter().enumerate() {
        output::detail(&format!(
            "  {:>3}. {:<30} {:<8} {} -> {}",
            index + 1,
            row.name,
            row.backend.id(),
            row.current.as_deref().unwrap_or("-"),
            row.available.as_deref().unwrap_or("-")
        ));
    }
    print!(
        "{} ",
        output::bold("Selection (e.g. 1,3,5 or 'all', empty to cancel):")
    );
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
        return Ok(Vec::new());
    }
    if line.eq_ignore_ascii_case("all") {
        return Ok(rows.to_vec());
    }
    let mut selected = Vec::new();
    for part in line.split(',') {
        let Ok(index) = part.trim().parse::<usize>() else {
            continue;
        };
        if let Some(row) = rows.get(index.saturating_sub(1)) {
            selected.push(row.clone());
        }
    }
    Ok(selected)
}

fn interactive_install(rows: &[SearchRow], cli: &Cli, cfg: &Config, runner: &Runner) -> Result<()> {
    if output::is_json() || output::is_quiet() || !io::stdin().is_terminal() {
        return Err(exit(
            EXIT_USAGE,
            "--interactive needs a terminal and cannot be used with --json/--quiet",
        ));
    }
    print!(
        "{} ",
        output::bold("Install which results? (e.g. 1,3 or empty to cancel):")
    );
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
        output::info("aborted");
        return Ok(());
    }

    let mut plan = Vec::new();
    for part in line.split(',') {
        let Ok(index) = part.trim().parse::<usize>() else {
            continue;
        };
        let Some(row) = rows.get(index.saturating_sub(1)) else {
            continue;
        };
        if row.installed && !cli.force {
            output::info(&format!("{} is already installed", row.name));
            continue;
        }
        plan.push(Resolved {
            requested: row.name.clone(),
            backend: row.backend,
            package: row.name.clone(),
            reason: Reason::Fallback,
        });
    }
    if plan.is_empty() {
        output::info("nothing selected");
        return Ok(());
    }
    let _ = runner;
    run_action(
        "install",
        &plan,
        cli,
        cfg,
        &Runner {
            dry_run: cli.dry_run,
            assume_yes: cli.yes || cli.non_interactive,
            quiet: cli.quiet,
            keep_going: cli.keep_going,
            no_elevate: cli.no_elevate,
        },
    )
}

fn install_names(
    names: &[String],
    forced: Option<BackendKind>,
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
) -> Result<()> {
    if names.is_empty() {
        output::info("nothing to install");
        return Ok(());
    }
    let resolved = resolver::resolve(names, forced, table, backends, runner)?;
    let mut plan = Vec::new();
    for item in resolved {
        if !cli.force {
            if let Some(true) = installed_state(&item, runner)? {
                output::info(&format!(
                    "{} is already installed via {}",
                    item.requested,
                    item.backend.id()
                ));
                continue;
            }
        }
        plan.push(item);
    }
    run_action("install", &plan, cli, cfg, runner)
}

fn record_history(
    cfg: &Config,
    action: &str,
    status: &str,
    packages: Vec<HistoryPackage>,
    commands: Vec<String>,
) {
    let _ = cfg;
    if let Err(err) = history::append(action, false, status, packages, commands) {
        output::warn(&format!("could not record history: {err}"));
    }
}

// ---------------------------------------------------------------------------
// JSON report shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PackageRecord {
    requested: String,
    backend: String,
    package: String,
    reason: String,
}

impl PackageRecord {
    fn from_resolved(item: &Resolved) -> Self {
        Self {
            requested: item.requested.clone(),
            backend: item.backend.id().to_string(),
            package: item.package.clone(),
            reason: item.reason.as_str().to_string(),
        }
    }
}

#[derive(Serialize)]
struct ActionReport {
    schema_version: u32,
    action: String,
    dry_run: bool,
    status: &'static str,
    packages: Vec<PackageRecord>,
    commands: Vec<String>,
    failures: Vec<String>,
}

#[derive(Serialize)]
struct AllReport {
    schema_version: u32,
    action: &'static str,
    dry_run: bool,
    status: &'static str,
    backends: Vec<String>,
    commands: Vec<String>,
    failures: Vec<String>,
}

#[derive(Serialize)]
struct BackendOutput {
    backend: String,
    success: bool,
    output: String,
}

#[derive(Serialize)]
struct PinsReport {
    schema_version: u32,
    results: Vec<BackendOutput>,
}

#[derive(Clone, Serialize)]
struct SearchResult {
    name: String,
    version: Option<String>,
    description: Option<String>,
    backend: String,
    installed: bool,
    outdated: bool,
    score: i64,
}

impl SearchResult {
    fn from_row(row: &SearchRow) -> Self {
        Self {
            name: row.name.clone(),
            version: row.version.clone(),
            description: row.description.clone(),
            backend: row.backend.id().to_string(),
            installed: row.installed,
            outdated: row.outdated,
            score: row.score,
        }
    }
}

#[derive(Serialize)]
struct SearchReport {
    schema_version: u32,
    term: String,
    best_match: Option<SearchResult>,
    results: Vec<SearchResult>,
}

#[derive(Clone)]
struct SearchRow {
    name: String,
    version: Option<String>,
    description: Option<String>,
    backend: Backend,
    installed: bool,
    outdated: bool,
    score: i64,
}

#[derive(Serialize)]
struct ListPackage {
    name: String,
    version: Option<String>,
    backend: String,
    outdated: bool,
}

#[derive(Serialize)]
struct ListReport {
    schema_version: u32,
    total: usize,
    packages: Vec<ListPackage>,
}

#[derive(Serialize)]
struct OutdatedPackage {
    name: String,
    current: Option<String>,
    available: Option<String>,
    backend: String,
    security: bool,
}

impl OutdatedPackage {
    fn from_row(row: &OutdatedRow) -> Self {
        Self {
            name: row.name.clone(),
            current: row.current.clone(),
            available: row.available.clone(),
            backend: row.backend.id().to_string(),
            security: row.security,
        }
    }
}

#[derive(Serialize)]
struct OutdatedReport {
    schema_version: u32,
    total: usize,
    security: usize,
    packages: Vec<OutdatedPackage>,
}

#[derive(Serialize)]
struct InfoReport {
    schema_version: u32,
    requested: String,
    backend: String,
    package: String,
    success: bool,
    output: String,
}

#[derive(Serialize)]
struct WhyReport {
    schema_version: u32,
    package: String,
    backend: String,
    native: String,
    success: bool,
    output: String,
}

#[derive(Serialize)]
struct HistoryReport {
    schema_version: u32,
    entries: Vec<history::Entry>,
}

#[derive(Serialize)]
struct BackendInfo {
    id: String,
    label: String,
    binary: String,
    path: Option<String>,
    available: bool,
    enabled: bool,
    selected: bool,
    priority: usize,
    requires_elevation: bool,
    version: Option<String>,
    capabilities: Capabilities,
}

#[derive(Serialize)]
struct EnvReport {
    schema_version: u32,
    os: String,
    arch: String,
    distro: Option<String>,
    container: Option<String>,
    root: bool,
    config: Option<String>,
    state: Option<String>,
    history: Option<String>,
    forced: Option<String>,
    aliases: usize,
    backends: Vec<BackendInfo>,
}

#[derive(Clone, Serialize)]
struct Problem {
    message: String,
    hint: Option<String>,
}

#[derive(Serialize)]
struct DoctorReport {
    schema_version: u32,
    environment: EnvReport,
    problems: Vec<Problem>,
}

#[derive(Serialize)]
struct CapabilityRow {
    backend: String,
    available: bool,
    capabilities: Capabilities,
}

#[derive(Serialize)]
struct CapabilityReport {
    schema_version: u32,
    backends: Vec<CapabilityRow>,
}

#[derive(Serialize)]
struct ExplainItem {
    requested: String,
    backend: String,
    package: String,
    reason: String,
    installed: Option<bool>,
    command: String,
}

#[derive(Serialize)]
struct ExplainReport {
    schema_version: u32,
    action: String,
    os: String,
    arch: String,
    distro: Option<String>,
    packages: Vec<ExplainItem>,
    executed: bool,
}
