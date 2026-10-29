use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::{validate_scope, Scope, ToolKind};

use super::AdapterCommand;

#[derive(Debug, Clone)]
pub struct PreparedCommand {
    pub args: Vec<String>,
    pub preview: bool,
}

pub fn prepare(
    tool: ToolKind,
    command: &AdapterCommand,
    isolated_home: Option<&Path>,
) -> Result<PreparedCommand> {
    match tool {
        ToolKind::Claude => claude_args(command),
        ToolKind::Codex => codex_args(command, isolated_home),
        ToolKind::Grok => grok_args(command, isolated_home),
        ToolKind::Zcode => zcode_args(command, isolated_home),
        ToolKind::Pi => system_md_args(ToolKind::Pi, command, isolated_home),
        ToolKind::Omp => system_md_args(ToolKind::Omp, command, isolated_home),
        ToolKind::Dsh => dsh_args(command, isolated_home),
    }
}

fn claude_args(command: &AdapterCommand) -> Result<PreparedCommand> {
    match command {
        AdapterCommand::Version => Ok(PreparedCommand {
            args: vec!["--version".into()],
            preview: true,
        }),
        AdapterCommand::Doctor => Ok(PreparedCommand {
            args: vec!["doctor".into(), "--json".into()],
            preview: true,
        }),
        AdapterCommand::Status {
            scope,
            project_dir,
            name,
        } => {
            let mut args = vec!["status".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            // No `--runtime`: with it the adapter only counts the shell wrapper and
            // settings alignment as installed, which a normal deploy does not write,
            // so a prompt that was just deployed would read as not deployed.
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::PlanActivate {
            file,
            scope,
            project_dir,
            name,
            runtime,
            append_file,
            max_tokens,
        }
        | AdapterCommand::Activate {
            file,
            scope,
            project_dir,
            name,
            runtime,
            append_file,
            max_tokens,
            ..
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let preview = matches!(command, AdapterCommand::PlanActivate { .. });
            let mut args = vec![
                "install".into(),
                "--scope".into(),
                scope.as_str().into(),
                "--file".into(),
                abs(file)?,
            ];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            if *runtime {
                if *scope != Scope::User {
                    return Err(Error::invalid(
                        "claude --runtime is only valid for user scope",
                    ));
                }
                args.push("--runtime".into());
            }
            if let Some(append) = append_file {
                args.push("--append-file".into());
                args.push(abs(append)?);
            }
            if let Some(tokens) = max_tokens {
                if *tokens == 0 {
                    return Err(Error::invalid(
                        "claude --max-tokens must be a positive integer",
                    ));
                }
                args.push("--max-tokens".into());
                args.push(tokens.to_string());
            }
            if !preview {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand { args, preview })
        }
        AdapterCommand::PlanDeactivate {
            scope,
            project_dir,
            name,
            ..
        }
        | AdapterCommand::Deactivate {
            scope,
            project_dir,
            name,
            ..
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let preview = matches!(command, AdapterCommand::PlanDeactivate { .. });
            let mut args = vec!["uninstall".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            if !preview {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand { args, preview })
        }
        AdapterCommand::Recover {
            scope,
            project_dir,
            execute,
            ..
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let mut args = vec!["recover".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            if *execute {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: !*execute,
            })
        }
        AdapterCommand::Backups { scope, project_dir } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let mut args = vec!["backups".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::Restore {
            target,
            backup,
            scope,
            project_dir,
            execute,
        } => {
            if backup.trim().is_empty() {
                return Err(Error::invalid("claude restore requires a backup name"));
            }
            let mut args = vec![
                "restore".into(),
                "--target".into(),
                abs(target)?,
                "--backup".into(),
                backup.clone(),
            ];
            if let Some(scope) = scope {
                validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
                args.push("--scope".into());
                args.push(scope.as_str().into());
                push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            }
            if *execute {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: !*execute,
            })
        }
        AdapterCommand::Reconcile { .. } => Err(Error::unavailable("reconcile is a Grok command")),
    }
}

fn codex_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let mut args = Vec::new();
    let mut preview = true;
    match command {
        AdapterCommand::Version => args.push("--version".into()),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--dry-run".into());
        }
        AdapterCommand::Activate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { .. } => args.push("--uninstall".into()),
        AdapterCommand::Deactivate { .. } => {
            args.push("--uninstall".into());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::Recover { execute, .. } => {
            args.push("--recover".into());
            if *execute {
                args.push("--yes".into());
                preview = false;
            }
        }
        AdapterCommand::Backups { .. } | AdapterCommand::Restore { .. } => {
            return Err(Error::unavailable(
                "backups and restore are Claude commands",
            ));
        }
        AdapterCommand::Reconcile { .. } => {
            return Err(Error::unavailable("reconcile is a Grok command"));
        }
    }
    if !matches!(command, AdapterCommand::Version) {
        args.push("--lang".into());
        args.push("en".into());
        if let Some(dir) = tool_dir(home, ".codex") {
            args.push("--codex-dir".into());
            args.push(dir);
        }
    }
    Ok(PreparedCommand { args, preview })
}

fn grok_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let mut args = vec!["--json".into()];
    let mut preview = true;
    match command {
        AdapterCommand::Version => args.push("--version".into()),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--dry-run".into());
        }
        AdapterCommand::Activate {
            file,
            name,
            expected_preview_token,
            ..
        } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            push_grok_token(&mut args, expected_preview_token.as_deref())?;
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { salvage_config, .. } => {
            args.push("--uninstall".into());
            if *salvage_config {
                args.push("--salvage-config".into());
            }
        }
        AdapterCommand::Deactivate {
            expected_preview_token,
            salvage_config,
            ..
        } => {
            args.push("--uninstall".into());
            if *salvage_config {
                args.push("--salvage-config".into());
            }
            push_grok_token(&mut args, expected_preview_token.as_deref())?;
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::Recover {
            execute,
            expected_preview_token,
            ..
        } => {
            args.push("--recover".into());
            if *execute {
                push_grok_token(&mut args, expected_preview_token.as_deref())?;
                args.push("--yes".into());
                preview = false;
            }
        }
        AdapterCommand::Reconcile {
            execute,
            expected_preview_token,
        } => {
            args.push("--reconcile".into());
            if *execute {
                push_grok_token(&mut args, expected_preview_token.as_deref())?;
                args.push("--yes".into());
                preview = false;
            }
        }
        AdapterCommand::Backups { .. } | AdapterCommand::Restore { .. } => {
            return Err(Error::unavailable(
                "backups and restore are Claude commands",
            ));
        }
    }
    if let Some(dir) = tool_dir(home, ".grok") {
        args.push("--grok-dir".into());
        args.push(dir);
    }
    Ok(PreparedCommand { args, preview })
}

fn zcode_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let managed = tool_dir(home, ".zcode-keysmith");
    match command {
        AdapterCommand::Version => Ok(PreparedCommand {
            args: vec!["--version".into()],
            preview: true,
        }),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => {
            let mut args = vec!["doctor".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::PlanActivate { file, .. } => {
            let mut args = vec![
                "install".into(),
                "--system-file".into(),
                abs(file)?,
                "--dry-run".into(),
            ];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::Activate { file, .. } => {
            let mut args = vec![
                "install".into(),
                "--system-file".into(),
                abs(file)?,
                "--yes".into(),
            ];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: false,
            })
        }
        AdapterCommand::PlanDeactivate { .. } => {
            let mut args = vec!["uninstall".into(), "--dry-run".into(), "--json".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::Deactivate { .. } => {
            let mut args = vec!["uninstall".into(), "--yes".into(), "--json".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: false,
            })
        }
        AdapterCommand::Recover { .. } => {
            Err(Error::unavailable("recover is not supported for ZCode"))
        }
        AdapterCommand::Backups { .. } | AdapterCommand::Restore { .. } => Err(Error::unavailable(
            "backups and restore are Claude commands",
        )),
        AdapterCommand::Reconcile { .. } => Err(Error::unavailable("reconcile is a Grok command")),
    }
}

fn push_grok_token(args: &mut Vec<String>, token: Option<&str>) -> Result<()> {
    push_preview_token(args, token, "grok")
}

fn push_preview_token(args: &mut Vec<String>, token: Option<&str>, tool: &str) -> Result<()> {
    let Some(token) = token.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(Error::invalid(format!(
            "{tool} confirm requires the preview confirmation token"
        )));
    };
    if token.len() != 64 || !token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(Error::invalid(format!(
            "{tool} preview token must be 64 hex characters"
        )));
    }
    args.push("--expected-preview-token".into());
    args.push(token.to_ascii_lowercase());
    Ok(())
}

/// The vendored pi adapter serves both pi and OMP (oh-my-pi): OMP is a pi fork that kept
/// pi's instruction-file layout, so only the config directory differs. Every machine-readable
/// call names `--pi-dir` explicitly as an absolute path — the flag keeps the adapter's own
/// name because it is the pi CLI that reads it — so the adapter never falls back to a
/// directory of its own choosing.
fn system_md_args(
    tool: ToolKind,
    command: &AdapterCommand,
    home: Option<&Path>,
) -> Result<PreparedCommand> {
    let mut args: Vec<String> = Vec::new();
    let mut preview = true;
    let mut token = None;
    match command {
        AdapterCommand::Version => {
            return Ok(PreparedCommand {
                args: vec!["--version".into()],
                preview: true,
            });
        }
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
        }
        AdapterCommand::Activate {
            file,
            name,
            expected_preview_token,
            ..
        } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            token = Some(expected_preview_token.as_deref());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { .. } => args.push("--uninstall".into()),
        AdapterCommand::Deactivate {
            expected_preview_token,
            ..
        } => {
            args.push("--uninstall".into());
            token = Some(expected_preview_token.as_deref());
            preview = false;
        }
        AdapterCommand::Recover {
            execute,
            expected_preview_token,
            ..
        } => {
            args.push("--recover".into());
            if *execute {
                token = Some(expected_preview_token.as_deref());
                preview = false;
            }
        }
        AdapterCommand::Backups { .. } | AdapterCommand::Restore { .. } => {
            return Err(Error::unavailable(
                "backups and restore are Claude commands",
            ));
        }
        AdapterCommand::Reconcile { .. } => {
            return Err(Error::unavailable("reconcile is a Grok command"));
        }
    }
    args.push("--pi-dir".into());
    args.push(system_md_dir(tool, home)?);
    if matches!(command, AdapterCommand::PlanActivate { .. }) {
        args.push("--dry-run".into());
    }
    if let Some(token) = token {
        push_preview_token(&mut args, token, tool.as_str())?;
        args.push("--yes".into());
    }
    args.push("--json".into());
    args.push("--lang".into());
    args.push("en".into());
    Ok(PreparedCommand { args, preview })
}

/// `<home>/.pi/agent` for pi, `<home>/.omp/agent` for OMP: from an isolated home when there is
/// one, else from the user's real home.
///
/// OMP reads two variables that move that directory, and a deployment to a path OMP does not
/// read would report success and change nothing:
///
/// - `PI_CODING_AGENT_DIR` is the agent directory itself. OMP's own `getAgentDir()` honours it,
///   which is the lookup `SYSTEM.md` discovery uses, so it wins over everything else.
/// - `PI_CONFIG_DIR` renames OMP's native config directory (`.omp` by default), so the agent
///   directory becomes `~/<name>/agent`.
///
/// A value that is not a usable absolute path — or, for `PI_CONFIG_DIR`, not a plain directory
/// name — falls back to the default rather than writing somewhere arbitrary. Both are ignored
/// when an isolated home is supplied, so tests and `KEYSMITH_SWITCH_TOOL_HOME` stay hermetic.
fn system_md_dir(tool: ToolKind, home: Option<&Path>) -> Result<String> {
    let dir = match (tool, home) {
        (ToolKind::Omp, None) => omp_agent_dir_from(
            std::env::var(PI_AGENT_DIR_ENV).ok().as_deref(),
            std::env::var(PI_CONFIG_DIR_ENV).ok().as_deref(),
            &real_home(tool)?,
        ),
        (_, isolated) => {
            let name = if tool == ToolKind::Omp { ".omp" } else { ".pi" };
            match tool_dir(isolated, name) {
                Some(root) => PathBuf::from(root).join("agent"),
                None => real_home(tool)?.join(name).join("agent"),
            }
        }
    };
    if !dir.is_absolute() {
        return Err(Error::invalid(format!(
            "{} agent directory must be an absolute path",
            tool.as_str()
        )));
    }
    Ok(dir.to_string_lossy().into_owned())
}

const PI_AGENT_DIR_ENV: &str = "PI_CODING_AGENT_DIR";
const PI_CONFIG_DIR_ENV: &str = "PI_CONFIG_DIR";

fn real_home(tool: ToolKind) -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| {
        Error::invalid(format!(
            "{} needs a home folder to find its agent directory",
            tool.as_str()
        ))
    })
}

/// OMP's agent directory: `$PI_CODING_AGENT_DIR` when it names an absolute path, else
/// `~/$PI_CONFIG_DIR/agent` when that is a plain directory name, else `~/.omp/agent`.
fn omp_agent_dir_from(
    agent_dir_env: Option<&str>,
    config_dir_env: Option<&str>,
    real_home: &Path,
) -> PathBuf {
    if let Some(raw) = agent_dir_env
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = expand_tilde(Path::new(raw), real_home);
        if path.is_absolute() {
            return path;
        }
    }
    let name = config_dir_env
        .map(str::trim)
        .filter(|value| is_plain_dir_name(value))
        .unwrap_or(".omp");
    real_home.join(name).join("agent")
}

/// A single directory name under the home directory: no separators, no `.`/`..`, not empty.
fn is_plain_dir_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('\0')
}

/// dsh-keysmith, like pi: every machine-readable call names `--dsh-dir` explicitly as an
/// absolute path, so the CLI never falls back to its own `$DSH_HOME`/`~/.dsh` guess. The
/// adapter resolves that path once, in [`dsh_dir`], where the Mirasim exception lives.
///
/// One difference from pi that the shape of this function records: dsh-keysmith 0.4.0 has no
/// preview token, so a confirmation cannot be bound to the preview the user saw. `--yes` is
/// the only confirmation there is. `--file` overlays `plugin/lib/contract.md`.
fn dsh_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let mut args: Vec<String> = Vec::new();
    let mut preview = true;
    match command {
        AdapterCommand::Version => {
            return Ok(PreparedCommand {
                args: vec!["--version".into()],
                preview: true,
            });
        }
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
        }
        AdapterCommand::Activate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { .. } => args.push("--uninstall".into()),
        AdapterCommand::Deactivate { .. } => {
            args.push("--uninstall".into());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::Recover { execute, .. } => {
            args.push("--recover".into());
            if *execute {
                args.push("--yes".into());
                preview = false;
            }
        }
        AdapterCommand::Backups { .. } | AdapterCommand::Restore { .. } => {
            return Err(Error::unavailable(
                "backups and restore are Claude commands",
            ));
        }
        AdapterCommand::Reconcile { .. } => {
            return Err(Error::unavailable("reconcile is a Grok command"));
        }
    }
    args.push("--dsh-dir".into());
    args.push(dsh_dir(home)?);
    if matches!(command, AdapterCommand::PlanActivate { .. }) {
        args.push("--dry-run".into());
    }
    args.push("--json".into());
    args.push("--lang".into());
    args.push("en".into());
    Ok(PreparedCommand { args, preview })
}

/// `<home>/.dsh` from an isolated home — the shape every test uses — else the user's real
/// dsh home.
///
/// The real home is not hard-coded to `~/.dsh`: `$DSH_HOME` wins when it is set, which is what
/// dsh-keysmith 0.4.0 resolves on its own and what a relocated harness install needs. Mirasim
/// is the case that forces the two apart. It starts every dsh session with `DSH_HOME` pointing
/// at a disposable mount under `~/.mirasim/agent-homes/dsh/<id>`, assembled from symlinks back
/// into `~/.dsh`; deploying through such a mount would land in one session's scratch directory
/// and vanish with it, so a value pointing inside Mirasim's agent homes is ignored.
fn dsh_dir(home: Option<&Path>) -> Result<String> {
    let dir = match tool_dir(home, ".dsh") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let real_home = dirs::home_dir()
                .ok_or_else(|| Error::invalid("dsh needs a home folder to find ~/.dsh"))?;
            let mirasim = mirasim_home(std::env::var(MIRASIM_HOME_ENV).ok().as_deref(), &real_home);
            dsh_home_from(
                std::env::var(DSH_HOME_ENV).ok().as_deref(),
                &real_home,
                mirasim.as_deref(),
            )
        }
    };
    if !dir.is_absolute() {
        return Err(Error::invalid("dsh --dsh-dir must be an absolute path"));
    }
    Ok(dir.to_string_lossy().into_owned())
}

const DSH_HOME_ENV: &str = "DSH_HOME";
const MIRASIM_HOME_ENV: &str = "MIRASIM_HOME";

/// The user's real dsh home: `$DSH_HOME` when it names an absolute path that is not inside
/// Mirasim's agent homes, else `~/.dsh`.
fn dsh_home_from(env: Option<&str>, real_home: &Path, mirasim: Option<&Path>) -> PathBuf {
    let default = real_home.join(".dsh");
    let raw = match env.map(str::trim).filter(|value| !value.is_empty()) {
        Some(raw) => raw,
        None => return default,
    };
    let path = expand_tilde(Path::new(raw), real_home);
    if !path.is_absolute() {
        return default;
    }
    // Mirasim mounts `$DSH_HOME` per session; the canonical home it links from is the target.
    if mirasim.is_some_and(|root| path.starts_with(root.join("agent-homes"))) {
        return default;
    }
    path
}

/// Mirasim's home: `$MIRASIM_HOME` when it is set, else `~/.mirasim` when that directory
/// exists. Absent means nobody is running Mirasim here and `$DSH_HOME` needs no suspicion.
fn mirasim_home(env: Option<&str>, real_home: &Path) -> Option<PathBuf> {
    if let Some(raw) = env.map(str::trim).filter(|value| !value.is_empty()) {
        let path = expand_tilde(Path::new(raw), real_home);
        if path.is_absolute() {
            return Some(path);
        }
    }
    let candidate = real_home.join(".mirasim");
    candidate.is_dir().then_some(candidate)
}

/// `~/x` and a bare `~` mean the real home, matching what dsh-keysmith's `expanduser` does.
/// `~other/x` is another user's directory and is left untouched.
fn expand_tilde(path: &Path, real_home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => real_home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

fn push_project_dir(
    args: &mut Vec<String>,
    scope: Scope,
    project_dir: Option<&Path>,
) -> Result<()> {
    if scope.requires_project_dir() {
        let dir = project_dir
            .ok_or_else(|| Error::invalid("project/local scope requires an absolute projectDir"))?;
        if !dir.is_absolute() {
            return Err(Error::invalid(
                "project/local scope requires an absolute projectDir",
            ));
        }
        args.push("--project-dir".into());
        args.push(dir.to_string_lossy().into_owned());
    }
    Ok(())
}

fn push_name(args: &mut Vec<String>, name: Option<&str>) {
    if let Some(name) = name {
        if !name.is_empty() {
            args.push("--name".into());
            args.push(name.to_string());
        }
    }
}

fn push_flag_name(args: &mut Vec<String>, name: Option<&str>) {
    if let Some(name) = name {
        if !name.is_empty() {
            args.push("--name".into());
            args.push(name.to_string());
        }
    }
}

fn abs(path: &Path) -> Result<String> {
    if path.is_absolute() {
        return Ok(path.to_string_lossy().into_owned());
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(path).to_string_lossy().into_owned())
}

fn tool_dir(home: Option<&Path>, name: &str) -> Option<String> {
    home.map(|home| {
        let path = if home.is_absolute() {
            home.join(name)
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(home)
                .join(name)
        };
        path.to_string_lossy().into_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A platform-absolute root. `/Users` is not absolute on Windows — no drive letter — so a
    /// fixture written the Unix way would exercise the fallback instead of the branch under test.
    fn root(name: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!(r"C:\{name}"))
        } else {
            PathBuf::from(format!("/{name}"))
        }
    }

    fn home() -> PathBuf {
        root("Users").join("tester")
    }

    #[test]
    fn the_real_home_defaults_to_dot_dsh() {
        assert_eq!(dsh_home_from(None, &home(), None), home().join(".dsh"));
        // An empty or whitespace-only variable is the same as unset: dsh-keysmith trims it too.
        assert_eq!(
            dsh_home_from(Some("   "), &home(), None),
            home().join(".dsh")
        );
        assert_eq!(
            dsh_home_from(Some(""), &home(), Some(&home().join(".mirasim"))),
            home().join(".dsh")
        );
    }

    #[test]
    fn dsh_home_is_honoured_because_dsh_itself_honours_it() {
        let mirasim = home().join(".mirasim");
        let elsewhere = root("Volumes").join("work").join("dsh");
        assert_eq!(
            dsh_home_from(Some(&elsewhere.to_string_lossy()), &home(), Some(&mirasim)),
            elsewhere
        );
        // `~` is the real home, the way Python's expanduser reads it.
        assert_eq!(
            dsh_home_from(Some("~/custom-dsh"), &home(), Some(&mirasim)),
            home().join("custom-dsh")
        );
    }

    #[test]
    fn a_mirasim_session_mount_is_never_the_deployment_target() {
        let mirasim = home().join(".mirasim");
        let session = mirasim.join("agent-homes/dsh/946bece7-47e7-4afe-bcbc9d8ac186");
        // This is exactly what a Mirasim-spawned shell hands to its children.
        assert_eq!(
            dsh_home_from(Some(&session.to_string_lossy()), &home(), Some(&mirasim)),
            home().join(".dsh")
        );
        // A path that merely shares a prefix is a different directory, not a mount.
        let lookalike = mirasim.join("agent-homes-backup/dsh/x");
        assert_eq!(
            dsh_home_from(Some(&lookalike.to_string_lossy()), &home(), Some(&mirasim)),
            lookalike
        );
    }

    #[test]
    fn a_relative_dsh_home_falls_back_to_the_default() {
        assert_eq!(
            dsh_home_from(Some("dsh-home"), &home(), None),
            home().join(".dsh")
        );
        assert_eq!(
            dsh_home_from(Some("../dsh"), &home(), None),
            home().join(".dsh")
        );
    }

    #[test]
    fn another_users_tilde_path_is_left_alone() {
        // strip_prefix only eats a whole `~` component, so `~bob/x` keeps its leading `~`.
        assert_eq!(
            expand_tilde(Path::new("~bob/dsh"), &home()),
            PathBuf::from("~bob/dsh")
        );
        assert_eq!(expand_tilde(Path::new("~"), &home()), home());
    }

    #[test]
    fn omp_agent_dir_defaults_to_dot_omp() {
        assert_eq!(
            omp_agent_dir_from(None, None, &home()),
            home().join(".omp").join("agent")
        );
        // Blank values are the same as unset ones.
        assert_eq!(
            omp_agent_dir_from(Some("  "), Some(""), &home()),
            home().join(".omp").join("agent")
        );
    }

    #[test]
    fn omp_agent_dir_follows_the_agent_dir_variable_first() {
        let agent = root("omp-agent");
        // The agent directory itself wins over the config-directory name.
        assert_eq!(
            omp_agent_dir_from(Some(&agent.to_string_lossy()), Some(".custom"), &home()),
            agent
        );
        // `~` means the real home, the way Python's expanduser reads it.
        assert_eq!(
            omp_agent_dir_from(Some("~/omp-agent"), None, &home()),
            home().join("omp-agent")
        );
        // A relative value is not a usable target, so the default stands.
        assert_eq!(
            omp_agent_dir_from(Some("omp-agent"), None, &home()),
            home().join(".omp").join("agent")
        );
    }

    #[test]
    fn omp_config_dir_renames_the_directory_but_only_as_a_plain_name() {
        assert_eq!(
            omp_agent_dir_from(None, Some(".custom"), &home()),
            home().join(".custom").join("agent")
        );
        for bad in ["", ".", "..", "a/b", "a\\b", "x\0y"] {
            assert_eq!(
                omp_agent_dir_from(None, Some(bad), &home()),
                home().join(".omp").join("agent"),
                "{bad:?} must not become a directory name"
            );
        }
    }

    #[test]
    fn pi_and_omp_isolated_homes_keep_their_own_dot_directory() {
        // An isolated home is the hermetic shape: no `$PI_*` variable may reach into it.
        assert_eq!(
            system_md_dir(ToolKind::Pi, Some(&home())).unwrap(),
            home().join(".pi").join("agent").to_string_lossy()
        );
        assert_eq!(
            system_md_dir(ToolKind::Omp, Some(&home())).unwrap(),
            home().join(".omp").join("agent").to_string_lossy()
        );
    }

    #[test]
    fn omp_status_points_the_pi_adapter_at_the_omp_agent_dir() {
        let command = AdapterCommand::Status {
            scope: Scope::User,
            project_dir: None,
            name: None,
        };
        let prepared = prepare(ToolKind::Omp, &command, Some(&home())).unwrap();
        let at = prepared
            .args
            .iter()
            .position(|arg| arg == "--pi-dir")
            .expect("OMP calls the pi adapter, which takes --pi-dir");
        assert_eq!(
            prepared.args[at + 1],
            home().join(".omp").join("agent").to_string_lossy()
        );
        assert!(prepared.args.iter().any(|arg| arg == "--json"));
        // Same argv shape as pi: the only difference is the directory.
        let pi = prepare(ToolKind::Pi, &command, Some(&home())).unwrap();
        let pi_at = pi.args.iter().position(|arg| arg == "--pi-dir").unwrap();
        assert_eq!(
            pi.args[pi_at + 1],
            home().join(".pi").join("agent").to_string_lossy()
        );
        assert_eq!(prepared.args.len(), pi.args.len());
    }

    #[test]
    fn mirasim_home_prefers_an_explicit_value() {
        let explicit = root("opt").join("mirasim-home");
        assert_eq!(
            mirasim_home(Some(&explicit.to_string_lossy()), &home()),
            Some(explicit)
        );
        // No explicit value and no `~/.mirasim` on disk: nothing is running Mirasim here.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(mirasim_home(None, empty.path()), None);
        // The directory itself makes it detectable.
        std::fs::create_dir_all(empty.path().join(".mirasim")).unwrap();
        assert_eq!(
            mirasim_home(None, empty.path()),
            Some(empty.path().join(".mirasim"))
        );
    }
}
