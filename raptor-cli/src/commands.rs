//! Non-interactive command implementations, one function per `raptorctl`
//! subcommand. Each prints a table by default or the raw server JSON with
//! `--json`.

use crate::api;
use crate::api::ListArgs;
use crate::client::Client;
use crate::config::Config;
use crate::print::{opt, table};
use anyhow::Result;
use clap::Subcommand;
use raptor_api_types::{DsAssignment, DsCreate, ModuleRef, SmCreate, TargetCreate, TargetUpdate};

fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

// ---------------------------------------------------------------------
// login
// ---------------------------------------------------------------------

pub async fn login(url: Option<String>, user: Option<String>) -> Result<()> {
    let mut cfg = Config::load().unwrap_or_default();
    let url = url
        .or(cfg.url.clone())
        .ok_or_else(|| anyhow::anyhow!("--url required on first login"))?;
    let user = user
        .or(cfg.user.clone())
        .ok_or_else(|| anyhow::anyhow!("--user required on first login"))?;
    let pass = rpassword::prompt_password(format!("password for {user}: "))?;

    cfg.url = Some(url);
    cfg.user = Some(user);
    cfg.pass = Some(pass);
    let resolved = cfg.clone().resolve(None, None)?;
    let client = Client::new(&resolved);
    // Any authenticated GET proves the credentials work.
    api::targets::list(&client, &ListArgs::default()).await?;

    cfg.save()?;
    println!("logged in as {}", resolved.user);
    Ok(())
}

// ---------------------------------------------------------------------
// target
// ---------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TargetCmd {
    /// List targets
    List {
        #[arg(short = 'q', long)]
        query: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        limit: Option<u64>,
        #[arg(long)]
        offset: Option<u64>,
    },
    /// Show one target
    Get { controller_id: String },
    /// Register a new target
    Create {
        controller_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a target's mutable fields
    Set {
        controller_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        request_attributes: bool,
    },
    /// Delete a target
    Delete { controller_id: String },
    /// Show reported device attributes
    Attributes { controller_id: String },
    #[command(subcommand)]
    Tag(TargetTagCmd),
    /// Assign a distribution set to a target
    Assign {
        controller_id: String,
        #[arg(long)]
        ds: i64,
        /// forced (default) | soft | timeforced | downloadonly
        #[arg(long)]
        force: Option<String>,
    },
    /// List a target's actions
    Actions { controller_id: String },
}

#[derive(Subcommand)]
pub enum TargetTagCmd {
    Add { controller_id: String, tag: String },
    Rm { controller_id: String, tag: String },
}

pub async fn target(c: &Client, cmd: TargetCmd, json: bool) -> Result<()> {
    match cmd {
        TargetCmd::List {
            query,
            sort,
            limit,
            offset,
        } => {
            let args = ListArgs {
                q: query,
                sort,
                limit,
                offset,
            };
            let page = api::targets::list(c, &args).await?;
            if json {
                return print_json(&page);
            }
            let rows = page
                .content
                .iter()
                .map(|t| {
                    vec![
                        t.controller_id.clone(),
                        t.name.clone(),
                        t.update_status.clone(),
                        t.assigned_ds
                            .as_ref()
                            .map(|d| format!("{}:{}", d.name, d.version))
                            .unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["CONTROLLER_ID", "NAME", "STATUS", "ASSIGNED_DS"], &rows);
            println!("{} of {} shown", page.content.len(), page.total);
        }
        TargetCmd::Get { controller_id } => {
            let t = api::targets::get(c, &controller_id).await?;
            if json {
                return print_json(&t);
            }
            println!("controllerId  {}", t.controller_id);
            println!("name          {}", t.name);
            println!("description   {}", opt(&t.description));
            println!("updateStatus  {}", t.update_status);
            println!(
                "installedDS   {}",
                t.installed_ds
                    .map(|d| format!("{}:{}", d.name, d.version))
                    .unwrap_or_else(|| "-".into())
            );
            println!(
                "assignedDS    {}",
                t.assigned_ds
                    .map(|d| format!("{}:{}", d.name, d.version))
                    .unwrap_or_else(|| "-".into())
            );
            println!(
                "tags          {}",
                if t.tags.is_empty() {
                    "-".into()
                } else {
                    t.tags
                        .iter()
                        .map(|tag| tag.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
        }
        TargetCmd::Create {
            controller_id,
            name,
            description,
        } => {
            let body = TargetCreate {
                controller_id,
                name,
                description,
                security_token: None,
                target_type: None,
            };
            let t = api::targets::create(c, &body).await?;
            if json {
                return print_json(&t);
            }
            println!("created {}", t.controller_id);
        }
        TargetCmd::Set {
            controller_id,
            name,
            description,
            request_attributes,
        } => {
            let body = TargetUpdate {
                name,
                description,
                security_token: None,
                request_attributes: request_attributes.then_some(true),
            };
            let t = api::targets::update(c, &controller_id, &body).await?;
            if json {
                return print_json(&t);
            }
            println!("updated {}", t.controller_id);
        }
        TargetCmd::Delete { controller_id } => {
            api::targets::delete(c, &controller_id).await?;
            println!("deleted {controller_id}");
        }
        TargetCmd::Attributes { controller_id } => {
            let attrs = api::targets::attributes(c, &controller_id).await?;
            if json {
                return print_json(&attrs);
            }
            let rows = attrs
                .iter()
                .map(|(k, v)| vec![k.clone(), v.clone()])
                .collect::<Vec<_>>();
            table(&["KEY", "VALUE"], &rows);
        }
        TargetCmd::Tag(tag_cmd) => match tag_cmd {
            TargetTagCmd::Add { controller_id, tag } => {
                let id = api::tags::find_id(c, &tag).await?;
                api::tags::assign(c, id, &controller_id).await?;
                println!("tagged {controller_id} with {tag}");
            }
            TargetTagCmd::Rm { controller_id, tag } => {
                let id = api::tags::find_id(c, &tag).await?;
                api::tags::unassign(c, id, &controller_id).await?;
                println!("untagged {controller_id} from {tag}");
            }
        },
        TargetCmd::Assign {
            controller_id,
            ds,
            force,
        } => {
            let body = DsAssignment {
                id: ds,
                assign_type: force,
                forcetime: None,
            };
            let r = api::actions::assign(c, &controller_id, &body).await?;
            if json {
                return print_json(&r);
            }
            println!(
                "assigned {} (already assigned {})",
                r.assigned, r.already_assigned
            );
        }
        TargetCmd::Actions { controller_id } => {
            let page =
                api::actions::list_for_target(c, &controller_id, &ListArgs::default()).await?;
            if json {
                return print_json(&page);
            }
            let rows = page
                .content
                .iter()
                .map(|a| {
                    vec![
                        a.id.to_string(),
                        a.action_type.clone(),
                        a.status.clone(),
                        a.detail_status.clone(),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["ID", "TYPE", "STATUS", "DETAIL"], &rows);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// module
// ---------------------------------------------------------------------

#[derive(Subcommand)]
pub enum ModuleCmd {
    List {
        #[arg(short = 'q', long)]
        query: Option<String>,
    },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        version: String,
        /// os | application | firmware
        #[arg(long = "type")]
        module_type: String,
        #[arg(long)]
        vendor: Option<String>,
    },
}

pub async fn module(c: &Client, cmd: ModuleCmd, json: bool) -> Result<()> {
    match cmd {
        ModuleCmd::List { query } => {
            let args = ListArgs {
                q: query,
                ..Default::default()
            };
            let page = api::modules::list(c, &args).await?;
            if json {
                return print_json(&page);
            }
            let rows = page
                .content
                .iter()
                .map(|m| {
                    vec![
                        m.id.to_string(),
                        m.name.clone(),
                        m.version.clone(),
                        m.module_type.clone(),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["ID", "NAME", "VERSION", "TYPE"], &rows);
        }
        ModuleCmd::Create {
            name,
            version,
            module_type,
            vendor,
        } => {
            let body = SmCreate {
                name,
                version,
                module_type,
                vendor,
                description: None,
            };
            let m = api::modules::create(c, &body).await?;
            if json {
                return print_json(&m);
            }
            println!("created module {} ({})", m.id, m.name);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// artifact
// ---------------------------------------------------------------------

#[derive(Subcommand)]
pub enum ArtifactCmd {
    Upload {
        module_id: i64,
        file: std::path::PathBuf,
    },
    List {
        module_id: i64,
    },
    Delete {
        module_id: i64,
        artifact_id: i64,
    },
}

pub async fn artifact(c: &Client, cmd: ArtifactCmd, json: bool) -> Result<()> {
    match cmd {
        ArtifactCmd::Upload { module_id, file } => {
            eprintln!("uploading {}...", file.display());
            let a = api::modules::artifact_upload(c, module_id, &file).await?;
            if json {
                return print_json(&a);
            }
            println!(
                "uploaded artifact {} ({} bytes, sha256 {} verified)",
                a.id, a.size, a.hashes.sha256
            );
        }
        ArtifactCmd::List { module_id } => {
            let list = api::modules::artifact_list(c, module_id).await?;
            if json {
                return print_json(&list);
            }
            let rows = list
                .iter()
                .map(|a| {
                    vec![
                        a.id.to_string(),
                        a.provided_filename.clone(),
                        a.size.to_string(),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["ID", "FILENAME", "SIZE"], &rows);
        }
        ArtifactCmd::Delete {
            module_id,
            artifact_id,
        } => {
            api::modules::artifact_delete(c, module_id, artifact_id).await?;
            println!("deleted artifact {artifact_id}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// distribution set
// ---------------------------------------------------------------------

#[derive(Subcommand)]
pub enum DsCmd {
    List {
        #[arg(short = 'q', long)]
        query: Option<String>,
    },
    Get {
        id: i64,
    },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        version: String,
        #[arg(long = "type")]
        ds_type: String,
        /// May be repeated to bundle several modules.
        #[arg(long = "module")]
        modules: Vec<i64>,
    },
}

pub async fn ds(c: &Client, cmd: DsCmd, json: bool) -> Result<()> {
    match cmd {
        DsCmd::List { query } => {
            let args = ListArgs {
                q: query,
                ..Default::default()
            };
            let page = api::distribution_sets::list(c, &args).await?;
            if json {
                return print_json(&page);
            }
            let rows = page
                .content
                .iter()
                .map(|d| {
                    vec![
                        d.id.to_string(),
                        d.name.clone(),
                        d.version.clone(),
                        d.ds_type.clone(),
                        d.complete.to_string(),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["ID", "NAME", "VERSION", "TYPE", "COMPLETE"], &rows);
        }
        DsCmd::Get { id } => {
            let d = api::distribution_sets::get(c, id).await?;
            if json {
                return print_json(&d);
            }
            println!("id           {}", d.id);
            println!("name         {}:{}", d.name, d.version);
            println!("type         {}", d.ds_type);
            println!("complete     {}", d.complete);
            println!(
                "modules      {}",
                d.modules
                    .iter()
                    .map(|m| format!("{}:{}", m.name, m.version))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        DsCmd::Create {
            name,
            version,
            ds_type,
            modules,
        } => {
            let body = DsCreate {
                name,
                version,
                ds_type,
                description: None,
                required_migration_step: false,
                modules: modules.into_iter().map(|id| ModuleRef { id }).collect(),
            };
            let d = api::distribution_sets::create(c, &body).await?;
            if json {
                return print_json(&d);
            }
            println!(
                "created distribution set {} ({}:{})",
                d.id, d.name, d.version
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// publish
// ---------------------------------------------------------------------

/// `module create` -> `artifact upload` -> `ds create`, threaded by id. The
/// three calls are always the same for one image; this exists so a caller
/// doesn't have to carry the module id between them via `--json` + `jq`.
pub struct PublishArgs {
    pub file: std::path::PathBuf,
    pub version: String,
    pub name: Option<String>,
    pub module_type: String,
    pub vendor: Option<String>,
}

/// Defaults to the filename with a trailing `-<version>.swu` stripped —
/// how release artifacts are usually named. Falls back to the filename
/// without its extension if that suffix isn't present.
fn default_publish_name(file: &std::path::Path, version: &str) -> String {
    let filename = file.file_name().unwrap_or_default().to_string_lossy();
    filename
        .strip_suffix(&format!("-{version}.swu"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            file.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| filename.to_string())
        })
}

pub async fn publish(c: &Client, args: PublishArgs, json: bool) -> Result<()> {
    let name = args
        .name
        .unwrap_or_else(|| default_publish_name(&args.file, &args.version));

    let module = api::modules::create(
        c,
        &SmCreate {
            name: name.clone(),
            version: args.version.clone(),
            module_type: args.module_type.clone(),
            vendor: args.vendor,
            description: None,
        },
    )
    .await?;
    eprintln!("created module {} ({name})", module.id);

    eprintln!("uploading {}...", args.file.display());
    let artifact = api::modules::artifact_upload(c, module.id, &args.file).await?;
    eprintln!(
        "uploaded artifact {} ({} bytes, sha256 {} verified)",
        artifact.id, artifact.size, artifact.hashes.sha256
    );

    let ds = api::distribution_sets::create(
        c,
        &DsCreate {
            name: name.clone(),
            version: args.version,
            ds_type: args.module_type,
            description: None,
            required_migration_step: false,
            modules: vec![ModuleRef { id: module.id }],
        },
    )
    .await?;

    if json {
        return print_json(
            &serde_json::json!({"module": module, "artifact": artifact, "distributionSet": ds}),
        );
    }
    println!(
        "published {name}:{} — module {}, distribution set {}",
        ds.version, module.id, ds.id
    );
    Ok(())
}

// ---------------------------------------------------------------------
// action
// ---------------------------------------------------------------------

#[derive(Subcommand)]
pub enum ActionCmd {
    List {
        #[arg(short = 'q', long)]
        query: Option<String>,
        #[arg(long)]
        active: bool,
    },
    Status {
        controller_id: String,
        action_id: i64,
    },
    Cancel {
        controller_id: String,
        action_id: i64,
        #[arg(long)]
        force: bool,
    },
    Force {
        controller_id: String,
        action_id: i64,
    },
}

pub async fn action(c: &Client, cmd: ActionCmd, json: bool) -> Result<()> {
    match cmd {
        ActionCmd::List { query, active } => {
            let q = match (query, active) {
                (Some(q), _) => Some(q),
                (None, true) => Some("active==true".to_string()),
                (None, false) => None,
            };
            let args = ListArgs {
                q,
                ..Default::default()
            };
            let page = api::actions::list_all(c, &args).await?;
            if json {
                return print_json(&page);
            }
            let rows = page
                .content
                .iter()
                .map(|a| {
                    vec![
                        a.id.to_string(),
                        opt(&a.target),
                        a.status.clone(),
                        a.detail_status.clone(),
                    ]
                })
                .collect::<Vec<_>>();
            table(&["ID", "TARGET", "STATUS", "DETAIL"], &rows);
        }
        ActionCmd::Status {
            controller_id,
            action_id,
        } => {
            let history = api::actions::status_history(c, &controller_id, action_id).await?;
            if json {
                return print_json(&history);
            }
            for s in history {
                println!(
                    "{}  {:<10}  {}",
                    s.reported_at,
                    s.status_type,
                    s.messages.join("; ")
                );
            }
        }
        ActionCmd::Cancel {
            controller_id,
            action_id,
            force,
        } => {
            api::actions::cancel(c, &controller_id, action_id, force).await?;
            println!("cancelled action {action_id}");
        }
        ActionCmd::Force {
            controller_id,
            action_id,
        } => {
            let a = api::actions::force(c, &controller_id, action_id).await?;
            if json {
                return print_json(&a);
            }
            println!("action {} forced", a.id);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// status
// ---------------------------------------------------------------------

pub async fn status(c: &Client, json: bool) -> Result<()> {
    let s = api::system::statistics(c).await?;
    if json {
        return print_json(&s);
    }
    println!("targets              {}", s.total_targets);
    println!("distribution sets    {}", s.total_distribution_sets);
    println!("software modules     {}", s.total_software_modules);
    println!("actions              {}", s.total_actions);
    println!("rollouts             {}", s.total_rollouts);
    println!("active actions       {}", s.active_actions);
    for (k, v) in &s.targets_by_status {
        println!("  {k:<12} {v}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_publish_name_strips_version_suffix() {
        assert_eq!(
            default_publish_name(Path::new("myapp-1.2.3.swu"), "1.2.3"),
            "myapp"
        );
    }

    #[test]
    fn default_publish_name_falls_back_to_stem() {
        // version doesn't match the filename's suffix — fall back rather than error.
        assert_eq!(
            default_publish_name(Path::new("firmware.bin"), "1.2.3"),
            "firmware"
        );
    }
}
