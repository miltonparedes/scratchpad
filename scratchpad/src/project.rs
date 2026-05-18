use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::models::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub slug: String,
    pub source: ProjectSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSource {
    Flag,
    Env,
    GitConfig,
    Alias {
        alias_name: String,
        repo: String,
    },
    GitRemoteOrigin {
        remote_url: String,
        repo_root: PathBuf,
    },
    RepoBasename {
        repo_root: PathBuf,
    },
    Shared,
}

impl ProjectSource {
    pub fn label(&self) -> &'static str {
        match self {
            ProjectSource::Flag => "flag",
            ProjectSource::Env => "env",
            ProjectSource::GitConfig => "git_config",
            ProjectSource::Alias { .. } => "alias",
            ProjectSource::GitRemoteOrigin { .. } => "git_remote_origin",
            ProjectSource::RepoBasename { .. } => "repo_basename",
            ProjectSource::Shared => "shared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteIdent {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

impl RemoteIdent {
    pub fn canonical_slug(&self, short_form_hosts: &[String]) -> String {
        if short_form_hosts
            .iter()
            .any(|h| h.eq_ignore_ascii_case(&self.host))
        {
            format!("{}/{}", self.owner, self.repo)
        } else {
            format!("{}/{}/{}", self.host, self.owner, self.repo)
        }
    }
}

pub fn default_short_form_hosts() -> Vec<String> {
    vec![
        "github.com".to_string(),
        "gitlab.com".to_string(),
        "bitbucket.org".to_string(),
        "codeberg.org".to_string(),
    ]
}

pub fn parse_remote_url(url: &str) -> Option<RemoteIdent> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    let rest = if let Some(rest) = url.strip_prefix("git@") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("https://") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("git://") {
        rest.to_string()
    } else {
        url.to_string()
    };

    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);

    let (host_part, path_part) = if let Some(idx) = rest.find(':') {
        let host_candidate = &rest[..idx];
        let path_candidate = &rest[idx + 1..];
        if host_candidate.contains('/') {
            split_first_segment(rest)?
        } else {
            (host_candidate.to_string(), path_candidate.to_string())
        }
    } else {
        split_first_segment(rest)?
    };

    let host = host_part.to_lowercase();
    let host = host.split('@').next_back().unwrap_or(&host).to_string();
    let host = host.split(':').next().unwrap_or(&host).to_string();

    let mut segments: Vec<&str> = path_part.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return None;
    }
    let repo = segments.pop()?.to_string();
    let owner = segments.join("/");

    Some(RemoteIdent { host, owner, repo })
}

fn split_first_segment(s: &str) -> Option<(String, String)> {
    let (head, tail) = s.split_once('/')?;
    Some((head.to_string(), tail.to_string()))
}

pub fn resolve_project(cwd: &Path, config: &Config, override_name: Option<&str>) -> Project {
    if let Some(name) = override_name {
        return Project {
            slug: name.to_string(),
            source: ProjectSource::Flag,
        };
    }
    if let Ok(env_name) = std::env::var("SP_PROJECT")
        && !env_name.is_empty()
    {
        return Project {
            slug: env_name,
            source: ProjectSource::Env,
        };
    }

    let repo_root = find_git_repo_root(cwd);

    if let Some(root) = repo_root.as_ref()
        && let Some(name) = git_config_value(root, "sp.project")
    {
        return Project {
            slug: name,
            source: ProjectSource::GitConfig,
        };
    }

    let Some(root) = repo_root else {
        return Project {
            slug: "shared".to_string(),
            source: ProjectSource::Shared,
        };
    };

    let remote_urls = git_remote_urls(&root);
    let short_form = effective_short_form(config);

    for url in &remote_urls {
        if let Some(ident) = parse_remote_url(url) {
            let candidate = ident.canonical_slug(&short_form);
            if let Some(alias) = match_alias(config, &candidate, url) {
                return Project {
                    slug: alias.clone(),
                    source: ProjectSource::Alias {
                        alias_name: alias,
                        repo: candidate.clone(),
                    },
                };
            }
        }
    }

    if let Some(origin) = remote_urls.first()
        && let Some(ident) = parse_remote_url(origin)
    {
        let slug = ident.canonical_slug(&short_form);
        return Project {
            slug,
            source: ProjectSource::GitRemoteOrigin {
                remote_url: origin.clone(),
                repo_root: root,
            },
        };
    }

    let basename = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "shared".to_string());
    Project {
        slug: basename,
        source: ProjectSource::RepoBasename { repo_root: root },
    }
}

fn effective_short_form(config: &Config) -> Vec<String> {
    if let Some(hosts) = &config.hosts
        && !hosts.short_form.is_empty()
    {
        return hosts.short_form.clone();
    }
    default_short_form_hosts()
}

fn match_alias(config: &Config, canonical: &str, raw_url: &str) -> Option<String> {
    let parsed = parse_remote_url(raw_url);
    for alias in &config.projects {
        for repo in &alias.repos {
            if repo == canonical {
                return Some(alias.name.clone());
            }
            if let Some(parsed_alias) = parse_remote_url(repo)
                && let Some(parsed_raw) = parsed.as_ref()
                && parsed_alias.host.eq_ignore_ascii_case(&parsed_raw.host)
                && parsed_alias.owner == parsed_raw.owner
                && parsed_alias.repo == parsed_raw.repo
            {
                return Some(alias.name.clone());
            }
        }
    }
    None
}

pub fn find_git_repo_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

fn git_remote_urls(repo_root: &Path) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(origin) = git_remote_url(repo_root, "origin") {
        urls.push(origin);
    }
    let output = Command::new("git")
        .args(["remote"])
        .current_dir(repo_root)
        .output();
    let Ok(output) = output else { return urls };
    if !output.status.success() {
        return urls;
    }
    let remotes = String::from_utf8_lossy(&output.stdout);
    for name in remotes.lines() {
        let name = name.trim();
        if name.is_empty() || name == "origin" {
            continue;
        }
        if let Some(url) = git_remote_url(repo_root, name) {
            urls.push(url);
        }
    }
    urls
}

fn git_remote_url(repo_root: &Path, name: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", name])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

fn git_config_value(repo_root: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", key])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(host: &str, owner: &str, repo: &str) -> RemoteIdent {
        RemoteIdent {
            host: host.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    #[test]
    fn parse_https_url() {
        assert_eq!(
            parse_remote_url("https://github.com/acme/api.git"),
            Some(ident("github.com", "acme", "api"))
        );
    }

    #[test]
    fn parse_https_url_without_git_suffix() {
        assert_eq!(
            parse_remote_url("https://github.com/acme/api"),
            Some(ident("github.com", "acme", "api"))
        );
    }

    #[test]
    fn parse_ssh_short_form() {
        assert_eq!(
            parse_remote_url("git@github.com:acme/api.git"),
            Some(ident("github.com", "acme", "api"))
        );
    }

    #[test]
    fn parse_ssh_explicit_protocol() {
        assert_eq!(
            parse_remote_url("ssh://git@github.com/acme/api.git"),
            Some(ident("github.com", "acme", "api"))
        );
    }

    #[test]
    fn parse_gitlab_subgroup() {
        assert_eq!(
            parse_remote_url("https://gitlab.com/acme/team/api.git"),
            Some(ident("gitlab.com", "acme/team", "api"))
        );
    }

    #[test]
    fn parse_invalid_url_returns_none() {
        assert!(parse_remote_url("not-a-url").is_none());
        assert!(parse_remote_url("").is_none());
    }

    #[test]
    fn canonical_slug_short_form() {
        let hosts = vec!["github.com".to_string()];
        assert_eq!(
            ident("github.com", "acme", "api").canonical_slug(&hosts),
            "acme/api"
        );
    }

    #[test]
    fn canonical_slug_long_form() {
        let hosts = vec!["github.com".to_string()];
        assert_eq!(
            ident("gitlab.empresa.com", "team", "api").canonical_slug(&hosts),
            "gitlab.empresa.com/team/api"
        );
    }

    #[test]
    fn canonical_slug_is_case_insensitive_on_host() {
        let hosts = vec!["GitHub.com".to_string()];
        assert_eq!(
            ident("github.com", "acme", "api").canonical_slug(&hosts),
            "acme/api"
        );
    }
}
