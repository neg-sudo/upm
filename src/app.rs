//! Command implementations.

use std::io::{self, IsTerminal, Write};
use std::process::Output;

use anyhow::Result;
use serde::Serialize;

use crate::backend::Backend;
use crate::cli::{Cli, Command, ConfigCmd};
use crate::config::Config;
use crate::detect;
use crate::error::{EXIT_NO_BACKEND, EXIT_NOT_FOUND, EXIT_UNSUPPORTED, EXIT_USAGE, exit};
use crate::exec::{self, Runner};
use crate::models::BackendKind;
use crate::output;
use crate::registry::AliasTable;
use crate::resolver::{self, Resolved};

pub fn run(cli: Cli, cfg: Config) -> Result<()> {
    let forced = match &cli.backend {
        Some(value) => Some(resolver::parse_backend(value)?),
        None => None,
    };
    let backends = detect::detected(&cfg);
    let table = AliasTable::load(&cfg)?;
    let runner = Runner {
        dry_run: cli.dry_run,
        assume_yes: cli.yes,
        quiet: cli.quiet,
    };

    match &cli.command {
        Command::Install { packages } => {
            cmd_install(&cli, &cfg, &table, &backends, &runner, packages, forced)
        }
        Command::Remove { packages } => {
            cmd_remove(&cli, &cfg, &table, &backends, &runner, packages, forced)
        }
        Command::Upgrade { packages } => {
            cmd_upgrade(&cli, &cfg, &table, &backends, &runner, packages, forced)
        }
        Command::Update => cmd_update(&cli, &cfg, &backends, &runner, forced),
        Command::Search { term } => cmd_search(&cli, &backends, &runner, term, forced),
        Command::List => cmd_list(&cli, &backends, &runner, forced),
        Command::Info { package } => cmd_info(&cli, &table, &backends, &runner, package, forced),
        Command::Which => cmd_which(&cli, &cfg, &table, &backends),
        Command::Doctor => cmd_doctor(&cli, &cfg, &table, &backends),
        Command::Resolve { packages } => {
            cmd_resolve(&cli, &table, &backends, &runner, packages, forced)
        }
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

fn cmd_upgrade(
    cli: &Cli,
    cfg: &Config,
    table: &AliasTable,
    backends: &[Backend],
    runner: &Runner,
    packages: &[String],
    forced: Option<BackendKind>,
) -> Result<()> {
    if packages.is_empty() {
        cmd_upgrade_all(cli, cfg, backends, runner, forced)
    } else {
        let plan = resolver::resolve(packages, forced, table, backends, runner)?;
        run_action("upgrade", &plan, cli, cfg, runner)
    }
}

fn run_action(
    action: &'static str,
    plan: &[Resolved],
    cli: &Cli,
    cfg: &Config,
    runner: &Runner,
) -> Result<()> {
    if plan.is_empty() {
        if cli.json {
            output::json(&ActionReport {
                action,
                dry_run: cli.dry_run,
                packages: Vec::new(),
                commands: Vec::new(),
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
    for item in plan {
        let argv = action_command(action, item, runner.assume_yes);
        commands.push(output::shell_join(&argv));
        runner.run(&argv, item.backend.needs_elevation())?;
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

    if cli.json {
        output::json(&ActionReport {
            action,
            dry_run: cli.dry_run,
            packages: plan
                .iter()
                .map(|item| PackageRecord {
                    requested: item.requested.clone(),
                    backend: item.backend.id().to_string(),
                    package: item.package.clone(),
                })
                .collect(),
            commands,
        })?;
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

fn cmd_upgrade_all(
    cli: &Cli,
    cfg: &Config,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let tasks: Vec<(Backend, Vec<String>)> = scope
        .iter()
        .map(|backend| (*backend, backend.upgrade_all_cmd(runner.assume_yes)))
        .collect();

    output::header("Plan: upgrade all packages");
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
    let mut used = Vec::new();
    for (backend, argv) in &tasks {
        commands.push(output::shell_join(argv));
        used.push(backend.id().to_string());
        runner.run(argv, backend.needs_elevation())?;
        if !cli.dry_run {
            output::success(&format!("upgraded packages via {}", backend.id()));
        }
    }

    if cli.json {
        output::json(&AllReport {
            action: "upgrade",
            dry_run: cli.dry_run,
            backends: used,
            commands,
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// update / search / list / info
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

    if tasks.is_empty() {
        if cli.json {
            output::json(&AllReport {
                action: "update",
                dry_run: cli.dry_run,
                backends: Vec::new(),
                commands: Vec::new(),
            })?;
        } else {
            output::info("nothing to update");
        }
        return Ok(());
    }

    output::header("Plan: refresh package metadata");
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
    let mut used = Vec::new();
    for (backend, argv) in &tasks {
        commands.push(output::shell_join(argv));
        used.push(backend.id().to_string());
        runner.run(argv, backend.needs_elevation())?;
        if !cli.dry_run {
            output::success(&format!("refreshed {}", backend.id()));
        }
    }

    if cli.json {
        output::json(&AllReport {
            action: "update",
            dry_run: cli.dry_run,
            backends: used,
            commands,
        })?;
    }
    Ok(())
}

fn cmd_search(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    term: &str,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let mut results = Vec::new();

    for backend in &scope {
        let argv = backend.search_cmd(term);
        match runner.capture(&argv) {
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
                        output::info("no results");
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
        output::json(&SearchReport {
            term: term.to_string(),
            results,
        })?;
    }
    Ok(())
}

fn cmd_list(
    cli: &Cli,
    backends: &[Backend],
    runner: &Runner,
    forced: Option<BackendKind>,
) -> Result<()> {
    let scope = select_backends(backends, forced)?;
    let mut results = Vec::new();

    for backend in &scope {
        let argv = backend.list_cmd();
        match runner.capture(&argv) {
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
                        output::info("no packages installed");
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
        output::json(&ListReport { results })?;
    }
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
// which / doctor / resolve
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
    output::detail(&format!("  {:<9} {}", "Arch", report.arch));
    output::detail(&format!(
        "  {:<9} {}",
        "Config",
        report.config.as_deref().unwrap_or("(none)")
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

fn cmd_doctor(cli: &Cli, cfg: &Config, table: &AliasTable, backends: &[Backend]) -> Result<()> {
    let report = environment(cfg, backends, cli.backend.as_deref(), table.len());
    let mut problems = Vec::new();

    if backends.is_empty() {
        problems.push("no supported package manager is installed or enabled".to_string());
    }
    if !report.root
        && backends.iter().any(|backend| backend.needs_elevation())
        && which::which("sudo").is_err()
        && cfg!(unix)
    {
        problems.push(
            "some backends need root but `sudo` is not installed (run upm as root)".to_string(),
        );
    }
    if table.is_empty() {
        problems
            .push("the alias registry is empty; package names will be passed through".to_string());
    }

    if cli.json {
        output::json(&DoctorReport {
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
                    .map(|v| format!("  {}", output::dim(v)))
                    .unwrap_or_default()
            ));
        }

        for problem in &problems {
            output::warn(problem);
        }
        if problems.is_empty() {
            output::success("everything looks good");
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
) -> Result<()> {
    let resolved = resolver::resolve(packages, forced, table, backends, runner)?;
    if cli.json {
        return output::json(
            &resolved
                .iter()
                .map(|item| PackageRecord {
                    requested: item.requested.clone(),
                    backend: item.backend.id().to_string(),
                    package: item.package.clone(),
                })
                .collect::<Vec<_>>(),
        );
    }

    output::header("Resolution");
    for item in &resolved {
        output::detail(&format!(
            "  {} {:<8} {}",
            output::green("•"),
            item.backend.id(),
            item.display_target()
        ));
    }
    Ok(())
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
    if cli.yes || cli.dry_run || !cfg.general.confirm {
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
            }
        })
        .collect();

    EnvReport {
        os: detect::os_name().to_string(),
        arch: detect::arch().to_string(),
        distro: detect::distro(),
        root: exec::is_admin(),
        config: Config::path().map(|path| path.display().to_string()),
        forced: forced.map(str::to_string),
        aliases,
        backends,
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
}

#[derive(Serialize)]
struct ActionReport {
    action: &'static str,
    dry_run: bool,
    packages: Vec<PackageRecord>,
    commands: Vec<String>,
}

#[derive(Serialize)]
struct AllReport {
    action: &'static str,
    dry_run: bool,
    backends: Vec<String>,
    commands: Vec<String>,
}

#[derive(Serialize)]
struct BackendOutput {
    backend: String,
    success: bool,
    output: String,
}

#[derive(Serialize)]
struct SearchReport {
    term: String,
    results: Vec<BackendOutput>,
}

#[derive(Serialize)]
struct ListReport {
    results: Vec<BackendOutput>,
}

#[derive(Serialize)]
struct InfoReport {
    requested: String,
    backend: String,
    package: String,
    success: bool,
    output: String,
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
}

#[derive(Serialize)]
struct EnvReport {
    os: String,
    arch: String,
    distro: Option<String>,
    root: bool,
    config: Option<String>,
    forced: Option<String>,
    aliases: usize,
    backends: Vec<BackendInfo>,
}

#[derive(Serialize)]
struct DoctorReport {
    environment: EnvReport,
    problems: Vec<String>,
}
