//! R0: pure contracts and validation, no network, disk, or desktop side effects.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u16 = 1;
pub type Check = Result<(), ContractError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractError {
    UnsupportedVersion, InvalidIdentity, InvalidDefinition, InvalidParameters,
    InvalidInputs, MissingResource, UnsupportedKind, CapabilityMismatch,
    ConnectionUnavailable, VersionConflict, InvalidTransition, OutputsNotArchived,
}

fn identity(s: &str) -> Check {
    if s.is_empty() || s.len() > 128 || !s.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c)) {
        Err(ContractError::InvalidIdentity)
    } else { Ok(()) }
}
fn version(v: u16) -> Check {
    if v == SCHEMA_VERSION { Ok(()) } else { Err(ContractError::UnsupportedVersion) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind { Image, Text, File, Link }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Media {
    Image { width: u32, height: u32 },
    Text { encoding: String },
    File,
    Link,
}
impl Media {
    pub fn kind(&self) -> ResourceKind {
        match self { Self::Image {..} => ResourceKind::Image, Self::Text {..} => ResourceKind::Text,
            Self::File => ResourceKind::File, Self::Link => ResourceKind::Link }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance { pub task_id: Option<String>, pub parent_ids: Vec<String> }

/// blob_id is an opaque managed key, never a user path or a provider URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub schema_version: u16, pub id: String, pub blob_id: String,
    pub mime: String, pub byte_length: u64, pub media: Media,
    pub provenance: Provenance,
}
impl Resource {
    pub fn validate(&self) -> Check {
        version(self.schema_version)?; identity(&self.id)?; identity(&self.blob_id)?;
        if self.mime.trim().is_empty() { return Err(ContractError::InvalidDefinition); }
        match &self.media {
            Media::Image {width, height} if *width == 0 || *height == 0 || self.byte_length == 0
                || !["image/png", "image/jpeg", "image/webp"].contains(&self.mime.as_str()) => return Err(ContractError::InvalidDefinition),
            Media::Text {encoding} if encoding != "utf-8" || !self.mime.starts_with("text/") => return Err(ContractError::InvalidDefinition),
            _ => (),
        }
        if let Some(id) = &self.provenance.task_id { identity(id)?; }
        for id in &self.provenance.parent_ids { identity(id)?; }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRef { pub connection_id: String, pub capability_id: String, pub revision: String }
impl CapabilityRef {
    pub fn validate(&self) -> Check {
        identity(&self.connection_id)?; identity(&self.capability_id)?; identity(&self.revision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus { Unverified, Ready, Offline, Revoked }

/// Host-managed profile points at an adapter-specific configuration, not a command from an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub schema_version: u16, pub id: String, pub adapter_id: String,
    pub profile_id: String, pub credential_ref: Option<String>, pub status: ConnectionStatus,
}
impl Connection {
    pub fn validate(&self) -> Check {
        version(self.schema_version)?; identity(&self.id)?; identity(&self.adapter_id)?; identity(&self.profile_id)?;
        if let Some(id) = &self.credential_ref { identity(id)?; } Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleSpec { pub role: String, pub kinds: Vec<ResourceKind>, pub min: u16, pub max: u16 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParameterSpec {
    Text { required: bool, max_length: u32 },
    Integer { required: bool, min: i64, max: i64 },
    Boolean { required: bool },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue { Text(String), Integer(i64), Boolean(bool) }
pub type Parameters = BTreeMap<String, ParameterValue>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionTraits {
    pub progress: bool, pub cancel: bool, pub query: bool, pub idempotency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub schema_version: u16, pub reference: CapabilityRef,
    pub inputs: Vec<RoleSpec>, pub outputs: Vec<RoleSpec>,
    pub parameters: BTreeMap<String, ParameterSpec>,
    pub permissions: BTreeSet<String>, pub execution: ExecutionTraits,
}
fn role_specs(roles: &[RoleSpec]) -> Check {
    let mut seen = BTreeSet::new();
    for spec in roles {
        identity(&spec.role)?;
        let kinds: BTreeSet<_> = spec.kinds.iter().collect();
        if !seen.insert(&spec.role) || spec.min > spec.max || spec.max == 0 || kinds.is_empty() || kinds.len() != spec.kinds.len() {
            return Err(ContractError::InvalidDefinition);
        }
    }
    Ok(())
}
impl Capability {
    pub fn validate(&self) -> Check {
        version(self.schema_version)?; self.reference.validate()?;
        role_specs(&self.inputs)?; role_specs(&self.outputs)?;
        for permission in &self.permissions { identity(permission)?; }
        for (name, spec) in &self.parameters {
            identity(name)?;
            match spec {
                ParameterSpec::Text {max_length, ..} if *max_length == 0 => return Err(ContractError::InvalidDefinition),
                ParameterSpec::Integer {min, max, ..} if min > max => return Err(ContractError::InvalidDefinition),
                _ => (),
            }
        }
        Ok(())
    }
    pub fn validate_parameters(&self, values: &Parameters) -> Check {
        if values.keys().any(|k| !self.parameters.contains_key(k)) { return Err(ContractError::InvalidParameters); }
        for (name, spec) in &self.parameters {
            let value = values.get(name);
            let valid = match (spec, value) {
                (ParameterSpec::Text {required, ..} | ParameterSpec::Integer {required, ..} | ParameterSpec::Boolean {required}, None) => !required,
                (ParameterSpec::Text {max_length, ..}, Some(ParameterValue::Text(s))) => !s.trim().is_empty() && s.chars().count() <= *max_length as usize,
                (ParameterSpec::Integer {min,max,..}, Some(ParameterValue::Integer(n))) => n >= min && n <= max,
                (ParameterSpec::Boolean {..}, Some(ParameterValue::Boolean(_))) => true,
                _ => false,
            };
            if !valid { return Err(ContractError::InvalidParameters); }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub schema_version: u16, pub id: String, pub name: String,
    pub capability: CapabilityRef, pub parameters: Parameters, pub cover_resource_id: Option<String>,
}
impl Preset {
    pub fn validate(&self, capability: &Capability) -> Check {
        version(self.schema_version)?; identity(&self.id)?; capability.validate()?;
        if self.name.trim().is_empty() || self.name.chars().count() > 24 { return Err(ContractError::InvalidDefinition); }
        if self.capability != capability.reference { return Err(ContractError::CapabilityMismatch); }
        if let Some(id) = &self.cover_resource_id { identity(id)?; }
        capability.validate_parameters(&self.parameters)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration { pub schema_version: u16, pub capability: Capability, pub preset: Preset }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationAction { Create, KeepExisting }
impl Registration {
    /// Caller authenticates the connection and checks permissions before calling this pure validator.
    pub fn validate(&self, connection: &Connection, existing: Option<&Capability>) -> Result<RegistrationAction, ContractError> {
        version(self.schema_version)?; connection.validate()?;
        self.preset.validate(&self.capability)?;
        if connection.id != self.capability.reference.connection_id { return Err(ContractError::CapabilityMismatch); }
        if connection.status != ConnectionStatus::Ready { return Err(ContractError::ConnectionUnavailable); }
        match existing {
            Some(old) if old.reference != self.capability.reference => Err(ContractError::CapabilityMismatch),
            Some(old) if old != &self.capability => Err(ContractError::VersionConflict),
            Some(_) => Ok(RegistrationAction::KeepExisting),
            None => Ok(RegistrationAction::Create),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef { pub role: String, pub resource_id: String }
fn references(items: &[ResourceRef], specs: &[RoleSpec], resources: &BTreeMap<String, Resource>, supported: &BTreeSet<ResourceKind>) -> Check {
    let mut seen = BTreeSet::new();
    for item in items {
        identity(&item.resource_id)?;
        let spec = specs.iter().find(|s| s.role == item.role).ok_or(ContractError::InvalidInputs)?;
        if !seen.insert((&item.role, &item.resource_id)) { return Err(ContractError::InvalidInputs); }
        let resource = resources.get(&item.resource_id).ok_or(ContractError::MissingResource)?;
        resource.validate()?;
        if resource.id != item.resource_id { return Err(ContractError::InvalidIdentity); }
        if !supported.contains(&resource.media.kind()) { return Err(ContractError::UnsupportedKind); }
        if !spec.kinds.contains(&resource.media.kind()) { return Err(ContractError::InvalidInputs); }
    }
    for spec in specs {
        let count = items.iter().filter(|i| i.role == spec.role).count();
        if count < spec.min as usize || count > spec.max as usize { return Err(ContractError::InvalidInputs); }
    }
    Ok(())
}

/// One request is ONE task. UI `each` expands a batch before admission, preserving roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRequest {
    pub schema_version: u16, pub submission_id: String, pub batch_id: Option<String>,
    pub preset_id: String, pub capability: CapabilityRef,
    pub inputs: Vec<ResourceRef>, pub parameters: Parameters,
}
impl TaskRequest {
    pub fn validate(&self, capability: &Capability, connection: &Connection, resources: &BTreeMap<String,Resource>, supported: &BTreeSet<ResourceKind>) -> Check {
        version(self.schema_version)?; identity(&self.submission_id)?; identity(&self.preset_id)?;
        if let Some(id) = &self.batch_id { identity(id)?; }
        capability.validate()?; connection.validate()?;
        if self.capability != capability.reference || connection.id != self.capability.connection_id { return Err(ContractError::CapabilityMismatch); }
        if connection.status != ConnectionStatus::Ready { return Err(ContractError::ConnectionUnavailable); }
        capability.validate_parameters(&self.parameters)?;
        references(&self.inputs, &capability.inputs, resources, supported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState { Queued, Running, WaitingUser, Finalizing, Succeeded, Failed, Interrupted, Cancelled }
impl TaskState {
    /// Retry creates a new attempt; terminal attempts never transition back to running.
    pub fn transition(self, next: Self, outputs_archived: bool) -> Check {
        use TaskState::*;
        let allowed = matches!((self,next),
            (Queued, Running | Cancelled | Failed) |
            (Running, WaitingUser | Finalizing | Failed | Interrupted | Cancelled) |
            (WaitingUser, Running | Failed | Interrupted | Cancelled) |
            (Finalizing, Succeeded | Failed | Interrupted));
        if !allowed { return Err(ContractError::InvalidTransition); }
        if next == Succeeded && !outputs_archived { return Err(ContractError::OutputsNotArchived); }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub schema_version: u16, pub id: String, pub request: TaskRequest,
    pub capability_snapshot: Capability, pub attempt: u32, pub state: TaskState,
    pub event_version: u64, pub external_run_id: Option<String>, pub outputs: Vec<ResourceRef>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub failure: Option<TaskFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResult { pub schema_version: u16, pub outputs: Vec<ResourceRef>, pub summary: Option<String> }
impl TaskResult {
    /// Validates archived references, not remote URLs or executor temp files.
    pub fn validate(&self, capability: &Capability, resources: &BTreeMap<String,Resource>, supported: &BTreeSet<ResourceKind>) -> Check {
        version(self.schema_version)?; capability.validate()?;
        references(&self.outputs, &capability.outputs, resources, supported)
    }
}

/// Adapter transfers are scoped to an attempt, never arbitrary paths in imported JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferRef {
    pub handle: String, pub mime: String, pub byte_length: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingOutput { pub role: String, pub transfer: TransferRef }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionOutput { pub outputs: Vec<PendingOutput>, pub summary: Option<String> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage { Admission, Execution, Archive }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFailure {
    pub code: String, pub stage: FailureStage, pub message: String,
    pub outcome_unknown: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEvent {
    pub schema_version: u16, pub task_id: String, pub attempt: u32,
    pub event_version: u64, pub state: TaskState, pub failure: Option<TaskFailure>,
}
impl TaskEvent {
    /// Only apply a later event for the exact task/attempt being observed.
    pub fn is_newer_than(&self, current: &Self) -> bool {
        self.schema_version == SCHEMA_VERSION && current.schema_version == SCHEMA_VERSION
            && self.task_id == current.task_id && self.attempt == current.attempt
            && self.event_version > current.event_version
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement { pub id: String, pub resource_id: String, pub x: f64, pub y: f64 }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardScene {
    pub schema_version: u16, pub id: String, pub revision: u64,
    pub items: Vec<Placement>, pub camera_x: f64, pub camera_y: f64, pub zoom: f64,
    pub selected_item_ids: Vec<String>,
}
impl BoardScene {
    pub fn validate(&self, resources: &BTreeMap<String,Resource>) -> Check {
        version(self.schema_version)?; identity(&self.id)?;
        if !self.camera_x.is_finite() || !self.camera_y.is_finite() || !self.zoom.is_finite() || self.zoom <= 0.0 { return Err(ContractError::InvalidDefinition); }
        let mut ids = BTreeSet::new();
        for item in &self.items {
            identity(&item.id)?;
            if !ids.insert(&item.id) || !item.x.is_finite() || !item.y.is_finite() { return Err(ContractError::InvalidDefinition); }
            if !resources.contains_key(&item.resource_id) { return Err(ContractError::MissingResource); }
        }
        if self.selected_item_ids.iter().any(|id| !ids.contains(id)) { return Err(ContractError::InvalidInputs); }
        Ok(())
    }
}
