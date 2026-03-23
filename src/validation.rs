use leptos::prelude::ServerFnError;

/// Validates a username according to security best practices.
///
/// Rules:
/// - Length: 3-50 characters
/// - Allowed characters: alphanumeric, underscore, hyphen
/// - Must start and end with alphanumeric
/// - No consecutive special characters
///
/// # Examples
///
/// ```
/// use flashy::validation::validate_username;
///
/// assert!(validate_username("john_doe").is_ok());
/// assert!(validate_username("user-123").is_ok());
/// assert!(validate_username("ab").is_err()); // Too short
/// ```
pub fn validate_username(username: &str) -> Result<String, ServerFnError> {
    let username = username.trim();

    if username.len() < 3 {
        return Err(ServerFnError::new("Username must be at least 3 characters"));
    }

    if username.len() > 50 {
        return Err(ServerFnError::new("Username must not exceed 50 characters"));
    }

    // Check allowed characters (alphanumeric, underscore, hyphen)
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ServerFnError::new(
            "Username can only contain letters, numbers, underscores, and hyphens",
        ));
    }

    // Must start and end with alphanumeric
    if !username.chars().next().unwrap().is_alphanumeric()
        || !username.chars().last().unwrap().is_alphanumeric()
    {
        return Err(ServerFnError::new(
            "Username must start and end with a letter or number",
        ));
    }

    // No consecutive special characters
    if username.contains("__")
        || username.contains("--")
        || username.contains("_-")
        || username.contains("-_")
    {
        return Err(ServerFnError::new(
            "Username cannot contain consecutive special characters",
        ));
    }

    Ok(username.to_string())
}

/// Validates a password according to security best practices.
///
/// Rules:
/// - Minimum 8 characters
/// - Maximum 128 characters (bcrypt limit)
/// - Must contain at least one letter (case insensitive)
/// - Cannot be empty or only whitespace
///
/// # Examples
///
/// ```
/// use flashy::validation::validate_password;
///
/// assert!(validate_password("password123").is_ok());
/// assert!(validate_password("Pass1234").is_ok());
/// assert!(validate_password("short1").is_err()); // Too short
/// ```
pub fn validate_password(password: &str) -> Result<(), ServerFnError> {
    if password.is_empty() || password.trim().is_empty() {
        return Err(ServerFnError::new("Password cannot be empty"));
    }

    if password.len() < 8 {
        return Err(ServerFnError::new("Password must be at least 8 characters"));
    }

    if password.len() > 128 {
        return Err(ServerFnError::new(
            "Password must not exceed 128 characters (bcrypt limit)",
        ));
    }

    // Must contain at least one letter
    if !password.chars().any(|c| c.is_alphabetic()) {
        return Err(ServerFnError::new(
            "Password must contain at least one letter",
        ));
    }

    Ok(())
}

/// Validates an email address.
///
/// Rules:
/// - Must contain exactly one '@' symbol
/// - Must have characters before and after '@'
/// - Domain part must contain at least one '.'
/// - Returns normalized (trimmed and lowercased) email
///
/// # Examples
///
/// ```
/// use flashy::validation::validate_email;
///
/// assert_eq!(validate_email("User@Example.COM").unwrap(), "user@example.com");
/// assert_eq!(validate_email("  user@test.com  ").unwrap(), "user@test.com");
/// ```
pub fn validate_email(email: &str) -> Result<String, ServerFnError> {
    let email = email.trim().to_lowercase();

    if email.is_empty() {
        return Err(ServerFnError::new("Email cannot be empty"));
    }

    if email.len() > 254 {
        return Err(ServerFnError::new("Email is too long"));
    }

    // Basic email validation
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err(ServerFnError::new("Invalid email format"));
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() || domain.is_empty() {
        return Err(ServerFnError::new("Invalid email format"));
    }

    if !domain.contains('.') {
        return Err(ServerFnError::new("Invalid email domain"));
    }

    // Check for invalid characters in local part
    if local.contains("..") {
        return Err(ServerFnError::new("Invalid email format"));
    }

    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_username() {
        assert!(validate_username("john_doe").is_ok());
        assert!(validate_username("user-123").is_ok());
        assert!(validate_username("User123").is_ok());
        assert!(validate_username("ab").is_err()); // Too short
        assert!(validate_username(&"a".repeat(51)).is_err()); // Too long
        assert!(validate_username("user@email").is_err()); // Invalid chars
        assert!(validate_username("user name").is_err()); // Space not allowed
        assert!(validate_username("-username").is_err()); // Starts with special char
        assert!(validate_username("username-").is_err()); // Ends with special char
        assert!(validate_username("user__name").is_err()); // Consecutive special chars
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("password123").is_ok());
        assert!(validate_password("Pass1234").is_ok());
        assert!(validate_password("12345abcd").is_ok());
        assert!(validate_password("noNumbersHere").is_ok()); // Letters only is fine
        assert!(validate_password("short1").is_err()); // Too short
        assert!(validate_password("12345678").is_err()); // No letters
        assert!(validate_password("").is_err()); // Empty
        assert!(validate_password("   ").is_err()); // Only whitespace
        assert!(validate_password(&"a".repeat(129)).is_err()); // Too long
    }

    #[test]
    fn test_validate_email() {
        assert_eq!(
            validate_email("User@Example.COM").unwrap(),
            "user@example.com"
        );
        assert_eq!(
            validate_email("  user@test.com  ").unwrap(),
            "user@test.com"
        );
        assert!(validate_email("").is_err());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("user@").is_err());
        assert!(validate_email("user@@example.com").is_err());
        assert!(validate_email("user..name@example.com").is_err());
        assert!(validate_email("user@example").is_err()); // No TLD
    }
}
