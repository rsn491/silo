//! Utilities for working with Git branch names.

/// Validates a branch name against a safe allowlist: `[a-zA-Z0-9._/\-]+`.
///
/// Rejects any name containing characters outside this set to prevent shell/AppleScript
/// injection when branch names are embedded in commands passed to iTerm2 or other terminals.
///
/// # Errors
///
/// Returns a `String` describing the first invalid character, or that the name is empty.
pub fn validate_branch_name(branch: &str) -> Result<(), String> {
    if branch.is_empty() {
        return Err("branch name must not be empty".to_string());
    }
    let invalid_char = branch
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '/' | '-'));
    if let Some(c) = invalid_char {
        return Err(format!(
            "branch name contains invalid character {:?}; only a-z, A-Z, 0-9, '.', '_', '/', '-' are allowed",
            c
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_branch_name_accepts_valid_names() {
        let valid = [
            "main",
            "feature/my-branch",
            "fix-123",
            "release/1.0.0",
            "user_feature",
            "UPPER/lower_123",
            "a",
        ];
        for name in valid {
            assert!(
                validate_branch_name(name).is_ok(),
                "should accept {:?}",
                name
            );
        }
    }

    #[test]
    fn test_validate_branch_name_rejects_injection_attempts() {
        let invalid = [
            "feature'; do shell script \"rm -rf ~\"",
            "branch$(whoami)",
            "branch`id`",
            "branch name",
            "branch\nnewline",
            "branch!bang",
            "",
        ];
        for name in invalid {
            assert!(
                validate_branch_name(name).is_err(),
                "should reject {:?}",
                name
            );
        }
    }
}
