//! Fill a template, then hand the HTML to WeasyPrint.

use handlebars::Handlebars;
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug)]
pub enum ReportError {
    /// The template is missing, or names a field the view lacks.
    Template(String),
    /// The `weasyprint` program is not where it was asked to be.
    NoWeasyPrint(PathBuf),
    /// WeasyPrint ran and refused: its own words, verbatim.
    WeasyPrint {
        status: String,
        stderr: String,
    },
    /// Postgres would not give the rows the report needs.
    Database(sqlx::Error),
    /// The record is not in the database (deleted since the click).
    Missing(String),
    Io(std::io::Error),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Template(why) => {
                write!(f, "template: {why}")
            }
            ReportError::NoWeasyPrint(p) => {
                write!(f, "WeasyPrint not found at {}", p.display())
            }
            ReportError::WeasyPrint { status, stderr } => {
                write!(f, "WeasyPrint {status}: {}", stderr.trim())
            }
            ReportError::Database(e) => write!(f, "{e}"),
            ReportError::Missing(what) => {
                write!(f, "{what} is no longer there")
            }
            ReportError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<std::io::Error> for ReportError {
    fn from(e: std::io::Error) -> Self {
        ReportError::Io(e)
    }
}

/// Where the templates live, where the PDFs go, and which
/// `weasyprint` to run.
#[derive(Debug, Clone)]
pub struct Renderer {
    pub templates: PathBuf,
    pub out: PathBuf,
    pub weasyprint: PathBuf,
}

impl Renderer {
    pub fn new(
        templates: impl Into<PathBuf>,
        out: impl Into<PathBuf>,
    ) -> Self {
        Renderer {
            templates: templates.into(),
            out: out.into(),
            weasyprint: "weasyprint".into(),
        }
    }

    pub fn with_weasyprint(
        mut self,
        program: impl Into<PathBuf>,
    ) -> Self {
        self.weasyprint = program.into();
        self
    }

    /// The template, read from disk now (so a changed file takes effect
    /// on the next print), filled from `view`. Strict: a field the view
    /// does not have is an error, not a blank.
    pub fn html(
        &self,
        name: &str,
        view: &impl Serialize,
    ) -> Result<String, ReportError> {
        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);
        let file = self.templates.join(format!("{name}.hbs"));
        hb.register_template_file(name, &file)
            .map_err(|e| ReportError::Template(e.to_string()))?;
        hb.render(name, view)
            .map_err(|e| ReportError::Template(e.to_string()))
    }

    /// HTML to a PDF at `to`. The templates folder is the base URL, so
    /// the page finds its stylesheet and logo beside the template.
    pub async fn bake(
        &self,
        html: &str,
        to: &Path,
    ) -> Result<(), ReportError> {
        let mut child = Command::new(&self.weasyprint)
            .arg("--base-url")
            .arg(&self.templates)
            .arg("-")
            .arg(to)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    ReportError::NoWeasyPrint(self.weasyprint.clone())
                }
                _ => ReportError::Io(e),
            })?;
        let mut stdin = child.stdin.take().expect("piped");
        stdin.write_all(html.as_bytes()).await?;
        drop(stdin);
        let out = child.wait_with_output().await?;
        if out.status.success() {
            Ok(())
        } else {
            Err(ReportError::WeasyPrint {
                status: out.status.to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).into(),
            })
        }
    }
}
