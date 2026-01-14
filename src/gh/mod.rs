use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;
use anyhow::{Result, anyhow};

pub async fn list_issues(context: Option<&str>) -> Result<Vec<Issue>> {
    let mut args = vec!["issue", "list", "--json", "number,title,state,author,labels,milestone,body,updatedAt"];
    if let Some(ctx) = context {
        args.push("-R");
        args.push(ctx);
    }

    let output = Command::new("gh")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("gh issue list failed: {}", err));
    }

    let issues: Vec<Issue> = serde_json::from_slice(&output.stdout)?;
    Ok(issues)
}

pub async fn list_prs(context: Option<&str>) -> Result<Vec<PullRequest>> {
    let mut args = vec!["pr", "list", "--json", "number,title,state,author,labels,body,updatedAt,mergeable"];
    if let Some(ctx) = context {
        args.push("-R");
        args.push(ctx);
    }

    let output = Command::new("gh")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("gh pr list failed: {}", err));
    }

    let prs: Vec<PullRequest> = serde_json::from_slice(&output.stdout)?;
    Ok(prs)
}

pub async fn list_projects(context: Option<&str>) -> Result<Vec<Project>> {
    let mut args = vec!["project", "list", "--json", "id,title,body,url,updatedAt"];
    if let Some(ctx) = context {
        args.push("--owner");
        args.push(ctx);
    }

    let output = Command::new("gh")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("gh project list failed: {}", err));
    }

    let projects: Vec<Project> = serde_json::from_slice(&output.stdout)?;
    Ok(projects)
}

pub async fn create_issue(title: &str, body: &str, labels: &[String], context: Option<&str>) -> Result<()> {
    let mut args = vec!["issue", "create", "-t", title, "-b", body];
    for label in labels {
        args.push("-l");
        args.push(label);
    }
    if let Some(ctx) = context {
        args.push("-R");
        args.push(ctx);
    }

    let output = Command::new("gh")
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        return Err(anyhow!("gh issue create failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(())
}

pub async fn create_pr(title: &str, body: &str, base: &str, context: Option<&str>) -> Result<()> {
    let mut args = vec!["pr", "create", "-t", title, "-b", body, "-B", base];
    if let Some(ctx) = context {
        args.push("-R");
        args.push(ctx);
    }

    let output = Command::new("gh")
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        return Err(anyhow!("gh pr create failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: Author,
    pub labels: Vec<Label>,
    pub milestone: Option<Milestone>,
    pub body: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: Author,
    pub labels: Vec<Label>,
    pub body: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "mergeable")]
    pub mergeable: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub url: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Author {
    pub login: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Label {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Milestone {
    pub title: String,
}
