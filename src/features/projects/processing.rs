#[cfg(feature = "ssr")]
use std::{path::Path, process::Stdio, time::Duration};

#[cfg(feature = "ssr")]
const PDFTOTEXT_TIMEOUT: Duration = Duration::from_secs(20);

#[cfg(feature = "ssr")]
pub async fn extract_text_with_pdftotext(pdf_path: &Path, pdf_size: u64) -> Result<String, String> {
    use crate::config::Config;

    let config = Config::global();
    if pdf_size > config.max_pdf_bytes {
        return Err(format!(
            "Uploaded PDF exceeded the size limit of {} MB",
            config.max_pdf_bytes / 1024 / 1024
        ));
    }

    let mut command = tokio::process::Command::new("pdftotext");
    command
        .arg("-layout")
        .arg("-q")
        .arg(pdf_path)
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = command
        .spawn()
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    let output = tokio::time::timeout(PDFTOTEXT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "pdftotext timed out".to_string())?
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pdftotext failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
