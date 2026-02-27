#[cfg(feature = "ssr")]
use std::{io::Write, process::Command};

#[cfg(feature = "ssr")]
pub async fn extract_text_with_pdftotext(pdf_bytes: Vec<u8>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| format!("Failed to create temp file: {e}"))?;
        temp_file
            .write_all(&pdf_bytes)
            .map_err(|e| format!("Failed to write temp file: {e}"))?;

        let output = Command::new("pdftotext")
            .arg("-layout")
            .arg("-q")
            .arg(temp_file.path())
            .arg("-")
            .output()
            .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("pdftotext failed: {stderr}"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(|e| format!("Failed to join pdftotext task: {e}"))?
}
