use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;


#[derive(Debug, Serialize)]
pub struct EventServiceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "Subscriptions")]
    pub subscriptions: ODataId,
    #[serde(rename = "ServerSentEventUri")]
    pub sse_uri: &'static str,
    #[serde(rename = "DeliveryRetryAttempts")]
    pub delivery_retry_attempts: u32,
    #[serde(rename = "DeliveryRetryIntervalSeconds")]
    pub delivery_retry_interval_seconds: u32,
    #[serde(rename = "EventFormatTypes")]
    pub event_format_types: Vec<&'static str>,
    #[serde(rename = "RegistryPrefixes")]
    pub registry_prefixes: Vec<&'static str>,
    #[serde(rename = "ResourceTypes")]
    pub resource_types: Vec<&'static str>,
    #[serde(rename = "IncludeOriginOfConditionSupported")]
    pub include_origin_of_condition_supported: bool,
    #[serde(rename = "SubordinateResourcesSupported")]
    pub subordinate_resources_supported: bool,
    #[serde(rename = "ExcludeMessageId")]
    pub exclude_message_id: bool,
    #[serde(rename = "ExcludeRegistryPrefix")]
    pub exclude_registry_prefix: bool,
    #[serde(rename = "Severities")]
    pub severities: Vec<&'static str>,
    #[serde(rename = "SSEFilterPropertiesSupported")]
    pub sse_filter_properties_supported: SseFilterProperties,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SseFilterProperties {
    #[serde(rename = "EventFormatType")]
    pub event_format_type: bool,
    #[serde(rename = "MessageId")]
    pub message_id: bool,
    #[serde(rename = "MetricReportDefinition")]
    pub metric_report_definition: bool,
    #[serde(rename = "OriginResource")]
    pub origin_resource: bool,
    #[serde(rename = "RegistryPrefix")]
    pub registry_prefix: bool,
    #[serde(rename = "ResourceType")]
    pub resource_type: bool,
    #[serde(rename = "SubordinateResources")]
    pub subordinate_resources: bool,
}

pub async fn get_event_service() -> Json<EventServiceResource> {
    Json(EventServiceResource {
        odata_id: "/redfish/v1/EventService",
        odata_type: "#EventService.v1_10_0.EventService",
        id: "EventService",
        name: "Event Service",
        description: "Redfish event delivery service",
        service_enabled: true,
        subscriptions: ODataId::new("/redfish/v1/EventService/Subscriptions"),
        sse_uri: "/redfish/v1/EventService/SSE",
        delivery_retry_attempts: 3,
        delivery_retry_interval_seconds: 30,
        event_format_types: vec!["Event"],
        registry_prefixes: vec!["ResourceEvent", "Security"],
        resource_types: vec!["ComputerSystem", "Manager", "Chassis"],
        include_origin_of_condition_supported: true,
        subordinate_resources_supported: false,
        exclude_message_id: false,
        exclude_registry_prefix: false,
        severities: vec!["OK", "Warning", "Critical"],
        sse_filter_properties_supported: SseFilterProperties {
            event_format_type: false,
            message_id: true,
            metric_report_definition: false,
            origin_resource: true,
            registry_prefix: true,
            resource_type: true,
            subordinate_resources: false,
        },
        status: Status::enabled_ok(),
    })
}

pub async fn get_subscriptions(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let subs = state.subscription_store.list();
    let members: Vec<ODataId> = subs
        .iter()
        .map(|s| {
            ODataId::new(format!(
                "/redfish/v1/EventService/Subscriptions/{}",
                s.id
            ))
        })
        .collect();

    Json(Collection::new(
        "/redfish/v1/EventService/Subscriptions",
        "#EventDestinationCollection.EventDestinationCollection",
        "Event Subscriptions",
        members,
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionRequest {
    #[serde(rename = "Destination")]
    pub destination: String,
    #[serde(rename = "Protocol")]
    pub protocol: Option<String>,
    #[serde(rename = "EventTypes", default)]
    pub event_types: Vec<String>,
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSubscriptionRequest>,
) -> Result<impl IntoResponse, RedfishApiError> {
    let sub = state.subscription_store.add(
        &body.destination,
        body.protocol.as_deref().unwrap_or("Redfish"),
        body.event_types,
    );

    // Start webhook delivery for this subscription
    let rx = state.event_bus.subscribe();
    crate::events::subscriptions::start_webhook_delivery(rx, sub.clone());

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "@odata.id": format!("/redfish/v1/EventService/Subscriptions/{}", sub.id),
            "Id": sub.id,
            "Destination": sub.destination,
            "Protocol": sub.protocol,
        })),
    ))
}

pub async fn get_subscription(
    State(state): State<Arc<AppState>>,
    Path(sub_id): Path<String>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    let sub = state
        .subscription_store
        .get(&sub_id)
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("Subscription '{sub_id}' not found"))
        })?;

    Ok(Json(serde_json::json!({
        "@odata.id": format!("/redfish/v1/EventService/Subscriptions/{}", sub.id),
        "@odata.type": "#EventDestination.v1_14_0.EventDestination",
        "Id": sub.id,
        "Name": format!("Subscription {}", sub.id),
        "Destination": sub.destination,
        "Protocol": sub.protocol,
        "EventTypes": sub.event_types,
    })))
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(sub_id): Path<String>,
) -> Result<StatusCode, RedfishApiError> {
    if state.subscription_store.remove(&sub_id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RedfishApiError::NotFound(format!(
            "Subscription '{sub_id}' not found"
        )))
    }
}

pub async fn sse_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
