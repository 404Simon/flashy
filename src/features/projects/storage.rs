#[cfg(feature = "ssr")]
use aws_sdk_s3::config::{Credentials, Region};
#[cfg(feature = "ssr")]
use aws_sdk_s3::Client;

#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
pub struct MinioSettings {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub force_path_style: bool,
    pub key_prefix: String,
}

#[cfg(feature = "ssr")]
impl MinioSettings {
    pub fn from_env() -> Self {
        let endpoint =
            std::env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string());
        let region = std::env::var("MINIO_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key =
            std::env::var("MINIO_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string());
        let secret_key =
            std::env::var("MINIO_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string());
        let bucket = std::env::var("MINIO_BUCKET").unwrap_or_else(|_| "flashy".to_string());
        let force_path_style = std::env::var("MINIO_FORCE_PATH_STYLE")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            != "false";
        let key_prefix =
            std::env::var("MINIO_KEY_PREFIX").unwrap_or_else(|_| "projects".to_string());

        Self {
            endpoint,
            region,
            access_key,
            secret_key,
            bucket,
            force_path_style,
            key_prefix,
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn build_minio_client(settings: &MinioSettings) -> Result<Client, String> {
    let creds = Credentials::new(
        &settings.access_key,
        &settings.secret_key,
        None,
        None,
        "static",
    );

    let s3_config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(&settings.endpoint)
        .region(Region::new(settings.region.clone()))
        .credentials_provider(creds)
        .force_path_style(settings.force_path_style)
        .build();

    let client = Client::from_conf(s3_config);

    // Check if bucket exists
    let head_result = client.head_bucket().bucket(&settings.bucket).send().await;

    let needs_create = match head_result {
        Ok(_) => false,
        Err(err) => {
            let service_err = err.into_service_error();
            if service_err.is_not_found() {
                true
            } else {
                return Err(format!("Failed to check MinIO bucket: {service_err}"));
            }
        }
    };

    if needs_create {
        client
            .create_bucket()
            .bucket(&settings.bucket)
            .send()
            .await
            .map_err(|e| format!("Failed to create MinIO bucket: {e}"))?;
    }

    Ok(client)
}
