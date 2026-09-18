use axum::Json;
use axum::extract::Path;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct MessageRegistryFileResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "Languages")]
    pub languages: Vec<&'static str>,
    #[serde(rename = "Registry")]
    pub registry: &'static str,
    #[serde(rename = "Location")]
    pub location: Vec<RegistryLocation>,
}

#[derive(Debug, Serialize)]
pub struct RegistryLocation {
    #[serde(rename = "Language")]
    pub language: &'static str,
    #[serde(rename = "Uri")]
    pub uri: &'static str,
}

pub async fn get_registries(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new("/redfish/v1/Registries/Base")];

    Json(Collection::new(
        "/redfish/v1/Registries",
        "#MessageRegistryFileCollection.MessageRegistryFileCollection",
        "Message Registry File Collection",
        members,
    ))
}

pub async fn get_registry(
    _user: AuthenticatedUser,
    Path(registry_id): Path<String>,
) -> Result<Json<MessageRegistryFileResource>, RedfishApiError> {
    if registry_id != "Base" {
        return Err(RedfishApiError::NotFound(format!(
            "Registry '{registry_id}' not found"
        )));
    }

    Ok(Json(MessageRegistryFileResource {
        odata_id: "/redfish/v1/Registries/Base".to_string(),
        odata_type: "#MessageRegistryFile.v1_1_0.MessageRegistryFile",
        id: "Base",
        name: "Base Message Registry File",
        description: "Base Message Registry File Location",
        languages: vec!["en"],
        registry: "Base.1.18",
        location: vec![RegistryLocation {
            language: "en",
            uri: "/redfish/v1/Registries/Base",
        }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_registry_file_resource_serialization() {
        let resource = MessageRegistryFileResource {
            odata_id: "/redfish/v1/Registries/Base".to_string(),
            odata_type: "#MessageRegistryFile.v1_1_0.MessageRegistryFile",
            id: "Base",
            name: "Base Message Registry File",
            description: "Base Message Registry File Location",
            languages: vec!["en"],
            registry: "Base.1.18",
            location: vec![RegistryLocation {
                language: "en",
                uri: "/redfish/v1/Registries/Base",
            }],
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/Registries/Base");
        assert_eq!(
            value["@odata.type"],
            "#MessageRegistryFile.v1_1_0.MessageRegistryFile"
        );
        assert_eq!(value["Id"], "Base");
        assert_eq!(value["Name"], "Base Message Registry File");
        assert_eq!(value["Description"], "Base Message Registry File Location");
        assert!(value["Languages"].is_array());
        assert_eq!(value["Languages"][0], "en");
        assert_eq!(value["Registry"], "Base.1.18");
        assert!(value["Location"].is_array());
    }

    #[test]
    fn test_registry_location_serialization() {
        let location = RegistryLocation {
            language: "en",
            uri: "/redfish/v1/Registries/Base",
        };

        let value = serde_json::to_value(&location).unwrap();

        assert_eq!(value["Language"], "en");
        assert_eq!(value["Uri"], "/redfish/v1/Registries/Base");
    }

    #[test]
    fn test_message_registry_file_multiple_languages() {
        let resource = MessageRegistryFileResource {
            odata_id: "/redfish/v1/Registries/Custom".to_string(),
            odata_type: "#MessageRegistryFile.v1_1_0.MessageRegistryFile",
            id: "Custom",
            name: "Custom Message Registry File",
            description: "Custom Message Registry File Location",
            languages: vec!["en", "fr", "de"],
            registry: "Custom.1.0",
            location: vec![
                RegistryLocation {
                    language: "en",
                    uri: "/redfish/v1/Registries/Custom/en",
                },
                RegistryLocation {
                    language: "fr",
                    uri: "/redfish/v1/Registries/Custom/fr",
                },
                RegistryLocation {
                    language: "de",
                    uri: "/redfish/v1/Registries/Custom/de",
                },
            ],
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["Languages"].as_array().unwrap().len(), 3);
        assert_eq!(value["Location"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_message_registry_file_single_location() {
        let resource = MessageRegistryFileResource {
            odata_id: "/redfish/v1/Registries/Test".to_string(),
            odata_type: "#MessageRegistryFile.v1_1_0.MessageRegistryFile",
            id: "Test",
            name: "Test Registry",
            description: "Test Registry File",
            languages: vec!["en"],
            registry: "Test.1.0",
            location: vec![RegistryLocation {
                language: "en",
                uri: "/redfish/v1/Registries/Test",
            }],
        };

        let value = serde_json::to_value(&resource).unwrap();
        let locations = value["Location"].as_array().unwrap();
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0]["Language"], "en");
        assert_eq!(locations[0]["Uri"], "/redfish/v1/Registries/Test");
    }

    #[test]
    fn test_registry_location_different_languages() {
        let en_location = RegistryLocation {
            language: "en",
            uri: "/redfish/v1/Registries/Base/en",
        };

        let fr_location = RegistryLocation {
            language: "fr",
            uri: "/redfish/v1/Registries/Base/fr",
        };

        let en_value = serde_json::to_value(&en_location).unwrap();
        let fr_value = serde_json::to_value(&fr_location).unwrap();

        assert_eq!(en_value["Language"], "en");
        assert_eq!(fr_value["Language"], "fr");
        assert_ne!(en_value["Uri"], fr_value["Uri"]);
    }

    #[test]
    fn test_message_registry_file_version_format() {
        let resource = MessageRegistryFileResource {
            odata_id: "/redfish/v1/Registries/Base".to_string(),
            odata_type: "#MessageRegistryFile.v1_1_0.MessageRegistryFile",
            id: "Base",
            name: "Base Message Registry File",
            description: "Base Message Registry File Location",
            languages: vec!["en"],
            registry: "Base.1.18",
            location: vec![RegistryLocation {
                language: "en",
                uri: "/redfish/v1/Registries/Base",
            }],
        };

        let value = serde_json::to_value(&resource).unwrap();
        let registry = value["Registry"].as_str().unwrap();
        assert!(
            registry.contains('.'),
            "Registry version should contain a dot"
        );
        assert!(
            registry.starts_with("Base"),
            "Registry should start with Base"
        );
    }
}
