//! Android package identities are exact, case-sensitive names, never shell
//! commands, activity component names, URLs or intent specifications.
pub fn validate_package(package: &str) -> Result<(), String> {
    if package.is_empty() || package.len() > 255 || !package.split('.').all(|part| {
        let mut chars = part.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }) {
        return Err("Choose an Android app package, such as com.example.app".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_names_do_not_accept_launch_commands_or_intent_syntax() {
        for package in ["com.example.app", "android", "com.example.App_2"] { assert!(validate_package(package).is_ok()); }
        for package in ["", " com.example.app", "com.example.app ", ".app", "com..app", "com.1app", "com.app/.Main", "intent://app", "com.app; echo bad", "com.app\n"] {
            assert!(validate_package(package).is_err(), "{package}");
        }
    }
}
