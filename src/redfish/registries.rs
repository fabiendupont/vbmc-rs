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
