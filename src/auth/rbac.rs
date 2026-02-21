use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Privilege {
    Login,
    ConfigureManager,
    ConfigureUsers,
    ConfigureComponents,
    ConfigureSelf,
}

pub fn role_privileges(role: &str) -> Vec<Privilege> {
    match role {
        "Administrator" => vec![
            Privilege::Login,
            Privilege::ConfigureManager,
            Privilege::ConfigureUsers,
            Privilege::ConfigureComponents,
            Privilege::ConfigureSelf,
        ],
        "Operator" => vec![
            Privilege::Login,
            Privilege::ConfigureComponents,
            Privilege::ConfigureSelf,
        ],
        "ReadOnly" => vec![Privilege::Login, Privilege::ConfigureSelf],
        _ => vec![Privilege::Login],
    }
}

pub fn has_privilege(role: &str, required: Privilege) -> bool {
    role_privileges(role).contains(&required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_privileges() {
        assert!(has_privilege("Administrator", Privilege::Login));
        assert!(has_privilege("Administrator", Privilege::ConfigureManager));
        assert!(has_privilege("Administrator", Privilege::ConfigureUsers));
        assert!(has_privilege("Administrator", Privilege::ConfigureComponents));
        assert!(has_privilege("Administrator", Privilege::ConfigureSelf));
    }

    #[test]
    fn test_readonly_limited() {
        assert!(has_privilege("ReadOnly", Privilege::Login));
        assert!(!has_privilege("ReadOnly", Privilege::ConfigureManager));
        assert!(!has_privilege("ReadOnly", Privilege::ConfigureComponents));
    }
}
