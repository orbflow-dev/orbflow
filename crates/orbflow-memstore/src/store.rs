// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! In-memory implementation of `Store` and `CredentialStore` for unit testing
//! and prototyping. All collections are wrapped in `tokio::sync::RwLock` so the
//! store is safe to share across tasks. Every value returned by a read method is
//! a deep clone (via serde JSON round-trip) so callers can freely mutate the
//! result without affecting the store's internal state.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use orbflow_core::credential::{Credential, CredentialId, CredentialSummary};
use orbflow_core::error::OrbflowError;
use orbflow_core::event::DomainEvent;
use orbflow_core::execution::{Instance, InstanceId, InstanceStatus};
use orbflow_core::pagination::paginate;
use orbflow_core::ports::{
    AtomicInstanceCreator, ChangeRequestStore, CredentialStore, EventStore, InstanceStore,
    ListOptions, Store, WorkflowStore,
};
use orbflow_core::versioning::{
    ChangeRequest, ChangeRequestStatus, ReviewComment, WorkflowVersion,
};
use orbflow_core::workflow::{Workflow, WorkflowId};

/// In-memory implementation of [`Store`] and [`CredentialStore`].
///
/// Uses `tokio::sync::RwLock` for each collection so multiple readers can
/// proceed concurrently while writers hold exclusive access.
pub struct MemStore {
    workflows: RwLock<HashMap<WorkflowId, Workflow>>,
    instances: RwLock<HashMap<InstanceId, Instance>>,
    events: RwLock<HashMap<InstanceId, Vec<DomainEvent>>>,
    snapshots: RwLock<HashMap<InstanceId, Instance>>,
    credentials: RwLock<HashMap<CredentialId, Credential>>,
    change_requests: RwLock<HashMap<String, ChangeRequest>>,
    /// Version history keyed by workflow ID, ordered by version ascending.
    versions: RwLock<HashMap<WorkflowId, Vec<WorkflowVersion>>>,
}

impl MemStore {
    /// Creates a new, empty in-memory store.
    pub fn new() -> Self {
        Self {
            workflows: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
            events: RwLock::new(HashMap::new()),
            snapshots: RwLock::new(HashMap::new()),
            credentials: RwLock::new(HashMap::new()),
            change_requests: RwLock::new(HashMap::new()),
            versions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Clones a value.
///
/// Uses the standard `Clone` trait which is more efficient than the previous
/// JSON round-trip approach. Rust's ownership model already prevents shared
/// interior mutation, so a normal clone is sufficient for isolation.
fn deep_clone<T>(value: &T) -> Result<T, OrbflowError>
where
    T: Clone,
{
    Ok(value.clone())
}

// ---------------------------------------------------------------------------
// WorkflowStore
// ---------------------------------------------------------------------------

#[async_trait]
impl WorkflowStore for MemStore {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let mut map = self.workflows.write().await;
        if map.contains_key(&wf.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        let cloned = deep_clone(wf)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        let map = self.workflows.read().await;
        let wf = map.get(id).ok_or(OrbflowError::NotFound)?;
        deep_clone(wf)
    }

    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let mut map = self.workflows.write().await;
        if !map.contains_key(&wf.id) {
            return Err(OrbflowError::NotFound);
        }
        let cloned = deep_clone(wf)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError> {
        let mut map = self.workflows.write().await;
        if map.remove(id).is_none() {
            return Err(OrbflowError::NotFound);
        }
        Ok(())
    }

    async fn list_workflows(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        let map = self.workflows.read().await;
        let mut all = Vec::with_capacity(map.len());
        for wf in map.values() {
            all.push(deep_clone(wf)?);
        }
        let total = all.len() as i64;
        let page = paginate(&all, &opts);
        Ok((page, total))
    }

    async fn save_workflow_version(&self, version: &WorkflowVersion) -> Result<(), OrbflowError> {
        let cloned: WorkflowVersion = deep_clone(version)?;
        let mut map = self.versions.write().await;
        map.entry(cloned.workflow_id.clone())
            .or_default()
            .push(cloned);
        Ok(())
    }

    async fn list_workflow_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<WorkflowVersion>, i64), OrbflowError> {
        let map = self.versions.read().await;
        let empty = Vec::new();
        let versions = map.get(id).unwrap_or(&empty);
        // Return in descending version order (newest first).
        let mut all: Vec<WorkflowVersion> = versions
            .iter()
            .map(deep_clone)
            .collect::<Result<Vec<_>, _>>()?;
        all.sort_by(|a, b| b.version.cmp(&a.version));
        let total = all.len() as i64;
        let page = paginate(&all, &opts);
        Ok((page, total))
    }

    async fn get_workflow_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<WorkflowVersion, OrbflowError> {
        let map = self.versions.read().await;
        let versions = map.get(id).ok_or(OrbflowError::NotFound)?;
        let found = versions
            .iter()
            .find(|v| v.version == version)
            .ok_or(OrbflowError::NotFound)?;
        deep_clone(found)
    }
}

// ---------------------------------------------------------------------------
// InstanceStore
// ---------------------------------------------------------------------------

#[async_trait]
impl InstanceStore for MemStore {
    async fn create_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let mut map = self.instances.write().await;
        if map.contains_key(&inst.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        let cloned = deep_clone(inst)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError> {
        let map = self.instances.read().await;
        let inst = map.get(id).ok_or(OrbflowError::NotFound)?;
        deep_clone(inst)
    }

    async fn update_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let mut map = self.instances.write().await;
        if !map.contains_key(&inst.id) {
            return Err(OrbflowError::NotFound);
        }
        let cloned = deep_clone(inst)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn list_instances(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        let map = self.instances.read().await;
        let mut all = Vec::with_capacity(map.len());
        for inst in map.values() {
            all.push(deep_clone(inst)?);
        }
        let total = all.len() as i64;
        let page = paginate(&all, &opts);
        Ok((page, total))
    }

    async fn list_running_instances(&self) -> Result<Vec<Instance>, OrbflowError> {
        let map = self.instances.read().await;
        let mut running = Vec::new();
        for inst in map.values() {
            if inst.status == InstanceStatus::Running {
                running.push(deep_clone(inst)?);
            }
        }
        Ok(running)
    }
}

// ---------------------------------------------------------------------------
// AtomicInstanceCreator
// ---------------------------------------------------------------------------

#[async_trait]
impl AtomicInstanceCreator for MemStore {
    /// Creates an instance and appends the first event atomically.
    ///
    /// In the in-memory store both maps share the same `MemStore`, so we
    /// acquire both write locks sequentially. This is safe because the lock
    /// ordering is always instances-then-events (no other code path reverses
    /// it).
    async fn create_instance_tx(
        &self,
        inst: &Instance,
        event: DomainEvent,
    ) -> Result<(), OrbflowError> {
        let mut inst_map = self.instances.write().await;
        if inst_map.contains_key(&inst.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        let cloned = deep_clone(inst)?;
        let instance_id = cloned.id.clone();
        inst_map.insert(instance_id.clone(), cloned);

        let mut evt_map = self.events.write().await;
        evt_map.entry(instance_id).or_default().push(event);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EventStore
// ---------------------------------------------------------------------------

#[async_trait]
impl EventStore for MemStore {
    async fn append_event(&self, event: DomainEvent) -> Result<(), OrbflowError> {
        let mut map = self.events.write().await;
        let id = event.instance_id().clone();
        map.entry(id).or_default().push(event);
        Ok(())
    }

    async fn load_events(
        &self,
        instance_id: &InstanceId,
        after_version: i64,
    ) -> Result<Vec<DomainEvent>, OrbflowError> {
        let map = self.events.read().await;
        let events = match map.get(instance_id) {
            Some(evts) => evts,
            None => return Ok(Vec::new()),
        };

        if after_version <= 0 {
            return Ok(events.clone());
        }

        let start = after_version as usize;
        if start >= events.len() {
            return Ok(Vec::new());
        }
        Ok(events[start..].to_vec())
    }

    async fn save_snapshot(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let mut map = self.snapshots.write().await;
        let cloned = deep_clone(inst)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn load_snapshot(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<Instance>, OrbflowError> {
        let map = self.snapshots.read().await;
        match map.get(instance_id) {
            Some(snap) => Ok(Some(deep_clone(snap)?)),
            None => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Store (composite trait)
// ---------------------------------------------------------------------------

impl Store for MemStore {}

// ---------------------------------------------------------------------------
// CredentialStore
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialStore for MemStore {
    async fn create_credential(&self, cred: &Credential) -> Result<(), OrbflowError> {
        let mut map = self.credentials.write().await;
        if map.contains_key(&cred.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        let cloned = cred.clone();
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn get_credential(&self, id: &CredentialId) -> Result<Credential, OrbflowError> {
        let map = self.credentials.read().await;
        let cred = map.get(id).ok_or(OrbflowError::NotFound)?;
        Ok(cred.clone())
    }

    async fn update_credential(&self, cred: &Credential) -> Result<(), OrbflowError> {
        let mut map = self.credentials.write().await;
        if !map.contains_key(&cred.id) {
            return Err(OrbflowError::NotFound);
        }
        let cloned = cred.clone();
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn delete_credential(
        &self,
        id: &CredentialId,
        owner_id: Option<&str>,
    ) -> Result<(), OrbflowError> {
        let mut map = self.credentials.write().await;
        match map.get(id) {
            None => return Err(OrbflowError::NotFound),
            Some(cred) => {
                if let Some(oid) = owner_id {
                    if cred.owner_id.as_deref() != Some(oid) {
                        return Err(OrbflowError::NotFound);
                    }
                }
            }
        }
        map.remove(id);
        Ok(())
    }

    async fn list_credentials(&self) -> Result<Vec<CredentialSummary>, OrbflowError> {
        let map = self.credentials.read().await;
        let summaries: Vec<CredentialSummary> = map.values().map(CredentialSummary::from).collect();
        Ok(summaries)
    }
}

// ---------------------------------------------------------------------------
// ChangeRequestStore
// ---------------------------------------------------------------------------

#[async_trait]
impl ChangeRequestStore for MemStore {
    async fn create_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError> {
        let mut map = self.change_requests.write().await;
        if map.contains_key(&cr.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        let cloned = deep_clone(cr)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn get_change_request(&self, id: &str) -> Result<ChangeRequest, OrbflowError> {
        let map = self.change_requests.read().await;
        let cr = map.get(id).ok_or(OrbflowError::NotFound)?;
        deep_clone(cr)
    }

    async fn list_change_requests(
        &self,
        workflow_id: &WorkflowId,
        status: Option<ChangeRequestStatus>,
        opts: ListOptions,
    ) -> Result<(Vec<ChangeRequest>, i64), OrbflowError> {
        let map = self.change_requests.read().await;
        let mut filtered: Vec<ChangeRequest> = map
            .values()
            .filter(|cr| cr.workflow_id == *workflow_id)
            .filter(|cr| status.is_none_or(|s| cr.status == s))
            .map(deep_clone)
            .collect::<Result<Vec<_>, _>>()?;
        // Sort by created_at descending (newest first), matching Postgres behavior.
        filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let total = filtered.len() as i64;
        let page = paginate(&filtered, &opts);
        Ok((page, total))
    }

    async fn update_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError> {
        let mut map = self.change_requests.write().await;
        if !map.contains_key(&cr.id) {
            return Err(OrbflowError::NotFound);
        }
        let cloned = deep_clone(cr)?;
        map.insert(cloned.id.clone(), cloned);
        Ok(())
    }

    async fn add_comment(&self, cr_id: &str, comment: &ReviewComment) -> Result<(), OrbflowError> {
        let mut map = self.change_requests.write().await;
        let cr = map.get_mut(cr_id).ok_or(OrbflowError::NotFound)?;
        let cloned = deep_clone(comment)?;
        cr.comments.push(cloned);
        Ok(())
    }

    async fn resolve_comment(
        &self,
        cr_id: &str,
        comment_id: &str,
        resolved: bool,
    ) -> Result<(), OrbflowError> {
        let mut map = self.change_requests.write().await;
        let cr = map.get_mut(cr_id).ok_or(OrbflowError::NotFound)?;
        let comment = cr
            .comments
            .iter_mut()
            .find(|c| c.id == comment_id)
            .ok_or(OrbflowError::NotFound)?;
        comment.resolved = resolved;
        Ok(())
    }

    async fn merge_change_request(
        &self,
        cr_id: &str,
        expected_version: i32,
        new_definition: &serde_json::Value,
    ) -> Result<(), OrbflowError> {
        let mut cr_map = self.change_requests.write().await;
        let cr = cr_map.get(cr_id).ok_or(OrbflowError::NotFound)?;

        // Verify the CR is approved.
        if cr.status != ChangeRequestStatus::Approved {
            return Err(OrbflowError::Conflict);
        }

        let workflow_id = cr.workflow_id.clone();

        // Get current workflow and verify version matches (stale-version guard).
        let mut wf_map = self.workflows.write().await;
        let wf = wf_map.get(&workflow_id).ok_or(OrbflowError::NotFound)?;

        if wf.version != expected_version {
            return Err(OrbflowError::Conflict);
        }

        // Update workflow: bump version and apply the new definition.
        let mut updated_wf = deep_clone(wf)?;
        updated_wf.version = wf.version + 1;
        updated_wf.updated_at = chrono::Utc::now();
        // Apply the proposed definition: parse nodes and edges from the JSON if present.
        if let Some(nodes) = new_definition.get("nodes")
            && let Ok(parsed) = serde_json::from_value(nodes.clone())
        {
            updated_wf.nodes = parsed;
        }
        if let Some(edges) = new_definition.get("edges")
            && let Ok(parsed) = serde_json::from_value(edges.clone())
        {
            updated_wf.edges = parsed;
        }
        wf_map.insert(workflow_id, updated_wf);

        // Mark CR as merged.
        let cr = cr_map.get_mut(cr_id).ok_or(OrbflowError::NotFound)?;
        cr.status = ChangeRequestStatus::Merged;
        cr.updated_at = chrono::Utc::now();

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;

    use orbflow_core::credential::{Credential, CredentialId};
    use orbflow_core::event::{BaseEvent, DomainEvent, InstanceStartedEvent, NodeQueuedEvent};
    use orbflow_core::execution::{ExecutionContext, Instance, InstanceId, InstanceStatus};
    use orbflow_core::ports::{
        AtomicInstanceCreator, CredentialStore, EventStore, InstanceStore, ListOptions,
        WorkflowStore,
    };
    use orbflow_core::workflow::{
        DefinitionStatus, Node, NodeKind, NodeType, Position, Workflow, WorkflowId,
    };

    use super::MemStore;

    fn test_workflow(id: &str) -> Workflow {
        Workflow {
            id: WorkflowId::new(id),
            name: format!("Workflow {id}"),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![Node {
                id: "a".into(),
                name: "A".into(),
                kind: NodeKind::Action,
                node_type: NodeType::Builtin,
                plugin_ref: "builtin:log".into(),
                position: Position::default(),
                input_mapping: None,
                config: None,
                parameters: vec![],
                retry: None,
                compensate: None,
                capability_ports: vec![],
                metadata: None,
                trigger_config: None,
                requires_approval: false,
            }],
            edges: vec![],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_instance(id: &str, wf_id: &str, status: InstanceStatus) -> Instance {
        Instance {
            id: InstanceId::new(id),
            workflow_id: WorkflowId::new(wf_id),
            status,
            node_states: HashMap::new(),
            context: ExecutionContext::new(HashMap::new()),
            saga: None,
            parent_id: None,
            instance_metrics: None,
            workflow_version: None,
            owner_id: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_credential(id: &str) -> Credential {
        Credential {
            id: CredentialId::new(id).unwrap(),
            name: format!("Cred {id}"),
            credential_type: "api_key".into(),
            data: HashMap::from([("key".into(), serde_json::json!("secret"))]),
            description: Some("Test credential".into()),
            access_tier: Default::default(),
            policy: None,
            owner_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // --- Workflow tests ---

    #[tokio::test]
    async fn test_create_and_get_workflow() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let fetched = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
        assert_eq!(fetched.id, wf.id);
        assert_eq!(fetched.name, wf.name);
    }

    #[tokio::test]
    async fn test_create_workflow_duplicate() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let err = store.create_workflow(&wf).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_get_workflow_not_found() {
        let store = MemStore::new();
        let err = store
            .get_workflow(&WorkflowId::new("missing"))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_update_workflow() {
        let store = MemStore::new();
        let mut wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        wf.name = "Updated".into();
        store.update_workflow(&wf).await.unwrap();

        let fetched = store.get_workflow(&wf.id).await.unwrap();
        assert_eq!(fetched.name, "Updated");
    }

    #[tokio::test]
    async fn test_update_workflow_not_found() {
        let store = MemStore::new();
        let wf = test_workflow("missing");
        let err = store.update_workflow(&wf).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_workflow() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();
        store.delete_workflow(&wf.id).await.unwrap();

        let err = store.get_workflow(&wf.id).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_workflow_not_found() {
        let store = MemStore::new();
        let err = store
            .delete_workflow(&WorkflowId::new("missing"))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_list_workflows_pagination() {
        let store = MemStore::new();
        for i in 0..5 {
            store
                .create_workflow(&test_workflow(&format!("wf-{i}")))
                .await
                .unwrap();
        }

        let (page, total) = store
            .list_workflows(ListOptions {
                offset: 1,
                limit: 2,
            })
            .await
            .unwrap();
        assert_eq!(total, 5);
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn test_deep_clone_isolation() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        // Mutating a fetched workflow should NOT affect the stored copy.
        let mut fetched = store.get_workflow(&wf.id).await.unwrap();
        fetched.name = "Mutated".into();

        let original = store.get_workflow(&wf.id).await.unwrap();
        assert_eq!(original.name, "Workflow wf-1");
    }

    // --- Instance tests ---

    #[tokio::test]
    async fn test_create_and_get_instance() {
        let store = MemStore::new();
        let inst = test_instance("inst-1", "wf-1", InstanceStatus::Pending);
        store.create_instance(&inst).await.unwrap();

        let fetched = store
            .get_instance(&InstanceId::new("inst-1"))
            .await
            .unwrap();
        assert_eq!(fetched.id, inst.id);
    }

    #[tokio::test]
    async fn test_create_instance_duplicate() {
        let store = MemStore::new();
        let inst = test_instance("inst-1", "wf-1", InstanceStatus::Pending);
        store.create_instance(&inst).await.unwrap();

        let err = store.create_instance(&inst).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_update_instance() {
        let store = MemStore::new();
        let mut inst = test_instance("inst-1", "wf-1", InstanceStatus::Pending);
        store.create_instance(&inst).await.unwrap();

        inst.status = InstanceStatus::Running;
        store.update_instance(&inst).await.unwrap();

        let fetched = store.get_instance(&inst.id).await.unwrap();
        assert_eq!(fetched.status, InstanceStatus::Running);
    }

    #[tokio::test]
    async fn test_update_instance_not_found() {
        let store = MemStore::new();
        let inst = test_instance("missing", "wf-1", InstanceStatus::Pending);
        let err = store.update_instance(&inst).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_list_instances_pagination() {
        let store = MemStore::new();
        for i in 0..4 {
            store
                .create_instance(&test_instance(
                    &format!("inst-{i}"),
                    "wf-1",
                    InstanceStatus::Pending,
                ))
                .await
                .unwrap();
        }

        let (page, total) = store
            .list_instances(ListOptions {
                offset: 0,
                limit: 2,
            })
            .await
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn test_list_running_instances() {
        let store = MemStore::new();
        store
            .create_instance(&test_instance("inst-1", "wf-1", InstanceStatus::Running))
            .await
            .unwrap();
        store
            .create_instance(&test_instance("inst-2", "wf-1", InstanceStatus::Completed))
            .await
            .unwrap();
        store
            .create_instance(&test_instance("inst-3", "wf-1", InstanceStatus::Running))
            .await
            .unwrap();

        let running = store.list_running_instances().await.unwrap();
        assert_eq!(running.len(), 2);
        for inst in &running {
            assert_eq!(inst.status, InstanceStatus::Running);
        }
    }

    // --- AtomicInstanceCreator tests ---

    #[tokio::test]
    async fn test_create_instance_tx() {
        let store = MemStore::new();
        let inst = test_instance("inst-tx", "wf-1", InstanceStatus::Running);
        let event = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(InstanceId::new("inst-tx"), 1),
            input: HashMap::new(),
        });

        store.create_instance_tx(&inst, event).await.unwrap();

        // Instance should exist.
        let fetched = store
            .get_instance(&InstanceId::new("inst-tx"))
            .await
            .unwrap();
        assert_eq!(fetched.status, InstanceStatus::Running);

        // Event should exist.
        let events = store
            .load_events(&InstanceId::new("inst-tx"), 0)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_create_instance_tx_duplicate() {
        let store = MemStore::new();
        let inst = test_instance("inst-dup", "wf-1", InstanceStatus::Running);
        let event = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(InstanceId::new("inst-dup"), 1),
            input: HashMap::new(),
        });
        store
            .create_instance_tx(&inst, event.clone())
            .await
            .unwrap();

        let err = store.create_instance_tx(&inst, event).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    // --- EventStore tests ---

    #[tokio::test]
    async fn test_append_and_load_events() {
        let store = MemStore::new();
        let id = InstanceId::new("inst-1");

        let e1 = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(id.clone(), 1),
            input: HashMap::new(),
        });
        let e2 = DomainEvent::NodeQueued(NodeQueuedEvent {
            base: BaseEvent::new(id.clone(), 2),
            node_id: "a".into(),
        });

        store.append_event(e1).await.unwrap();
        store.append_event(e2).await.unwrap();

        let all = store.load_events(&id, 0).await.unwrap();
        assert_eq!(all.len(), 2);

        // after_version=1 should skip the first event.
        let after = store.load_events(&id, 1).await.unwrap();
        assert_eq!(after.len(), 1);
    }

    #[tokio::test]
    async fn test_load_events_empty() {
        let store = MemStore::new();
        let events = store
            .load_events(&InstanceId::new("nonexistent"), 0)
            .await
            .unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_load_events_after_version_exceeds() {
        let store = MemStore::new();
        let id = InstanceId::new("inst-1");
        let event = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(id.clone(), 1),
            input: HashMap::new(),
        });
        store.append_event(event).await.unwrap();

        // after_version beyond the event count returns empty.
        let events = store.load_events(&id, 100).await.unwrap();
        assert!(events.is_empty());
    }

    // --- Snapshot tests ---

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        let store = MemStore::new();
        let inst = test_instance("inst-1", "wf-1", InstanceStatus::Running);

        store.save_snapshot(&inst).await.unwrap();

        let snap = store
            .load_snapshot(&InstanceId::new("inst-1"))
            .await
            .unwrap();
        assert!(snap.is_some());
        assert_eq!(snap.unwrap().id, inst.id);
    }

    #[tokio::test]
    async fn test_load_snapshot_not_found() {
        let store = MemStore::new();
        let snap = store
            .load_snapshot(&InstanceId::new("missing"))
            .await
            .unwrap();
        assert!(snap.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_overwrite() {
        let store = MemStore::new();
        let inst1 = test_instance("inst-1", "wf-1", InstanceStatus::Running);
        store.save_snapshot(&inst1).await.unwrap();

        let mut inst2 = test_instance("inst-1", "wf-1", InstanceStatus::Completed);
        inst2.version = 5;
        store.save_snapshot(&inst2).await.unwrap();

        let snap = store
            .load_snapshot(&InstanceId::new("inst-1"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snap.status, InstanceStatus::Completed);
        assert_eq!(snap.version, 5);
    }

    // --- Credential tests ---

    #[tokio::test]
    async fn test_create_and_get_credential() {
        let store = MemStore::new();
        let cred = test_credential("cred-1");
        store.create_credential(&cred).await.unwrap();

        let fetched = store
            .get_credential(&CredentialId::new("cred-1").unwrap())
            .await
            .unwrap();
        assert_eq!(fetched.id, cred.id);
        assert_eq!(fetched.name, cred.name);
    }

    #[tokio::test]
    async fn test_create_credential_duplicate() {
        let store = MemStore::new();
        let cred = test_credential("cred-1");
        store.create_credential(&cred).await.unwrap();

        let err = store.create_credential(&cred).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_get_credential_not_found() {
        let store = MemStore::new();
        let err = store
            .get_credential(&CredentialId::new("missing").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_update_credential() {
        let store = MemStore::new();
        let mut cred = test_credential("cred-1");
        store.create_credential(&cred).await.unwrap();

        cred.name = "Updated".into();
        store.update_credential(&cred).await.unwrap();

        let fetched = store.get_credential(&cred.id).await.unwrap();
        assert_eq!(fetched.name, "Updated");
    }

    #[tokio::test]
    async fn test_update_credential_not_found() {
        let store = MemStore::new();
        let cred = test_credential("missing");
        let err = store.update_credential(&cred).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_credential() {
        let store = MemStore::new();
        let cred = test_credential("cred-1");
        store.create_credential(&cred).await.unwrap();
        store.delete_credential(&cred.id, None).await.unwrap();

        let err = store.get_credential(&cred.id).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_credential_not_found() {
        let store = MemStore::new();
        let err = store
            .delete_credential(&CredentialId::new("missing").unwrap(), None)
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_list_credentials() {
        let store = MemStore::new();
        store
            .create_credential(&test_credential("cred-1"))
            .await
            .unwrap();
        store
            .create_credential(&test_credential("cred-2"))
            .await
            .unwrap();

        let summaries = store.list_credentials().await.unwrap();
        assert_eq!(summaries.len(), 2);
        // Summaries should not contain the data field (verified by type).
        for s in &summaries {
            assert!(!s.name.is_empty());
        }
    }

    #[tokio::test]
    async fn test_credential_deep_clone_isolation() {
        let store = MemStore::new();
        let cred = test_credential("cred-1");
        store.create_credential(&cred).await.unwrap();

        let mut fetched = store.get_credential(&cred.id).await.unwrap();
        fetched.name = "Mutated".into();

        let original = store.get_credential(&cred.id).await.unwrap();
        assert_eq!(original.name, "Cred cred-1");
    }

    use orbflow_core::error::OrbflowError;
    use orbflow_core::ports::ChangeRequestStore;
    use orbflow_core::versioning::{ChangeRequest, ChangeRequestStatus, ReviewComment};

    fn test_change_request(id: &str, wf_id: &str) -> ChangeRequest {
        ChangeRequest {
            id: id.into(),
            workflow_id: WorkflowId::new(wf_id),
            title: format!("CR {id}"),
            description: Some("Test change request".into()),
            proposed_definition: serde_json::json!({"nodes": [], "edges": []}),
            base_version: 1,
            status: ChangeRequestStatus::Open,
            author: "alice".into(),
            reviewers: vec!["bob".into()],
            comments: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_comment(id: &str) -> ReviewComment {
        ReviewComment {
            id: id.into(),
            author: "bob".into(),
            body: format!("Comment {id}"),
            node_id: None,
            edge_ref: None,
            resolved: false,
            created_at: Utc::now(),
        }
    }

    // --- ChangeRequestStore tests ---

    #[tokio::test]
    async fn test_create_and_get_change_request() {
        let store = MemStore::new();
        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let fetched = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(fetched.id, "cr-1");
        assert_eq!(fetched.title, "CR cr-1");
        assert_eq!(fetched.status, ChangeRequestStatus::Open);
    }

    #[tokio::test]
    async fn test_create_change_request_duplicate() {
        let store = MemStore::new();
        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let err = store.create_change_request(&cr).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_get_change_request_not_found() {
        let store = MemStore::new();
        let err = store.get_change_request("missing").await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_list_change_requests_filters_by_workflow() {
        let store = MemStore::new();
        store
            .create_change_request(&test_change_request("cr-1", "wf-1"))
            .await
            .unwrap();
        store
            .create_change_request(&test_change_request("cr-2", "wf-1"))
            .await
            .unwrap();
        store
            .create_change_request(&test_change_request("cr-3", "wf-2"))
            .await
            .unwrap();

        let (items, total) = store
            .list_change_requests(&WorkflowId::new("wf-1"), None, ListOptions::default())
            .await
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_list_change_requests_filters_by_status() {
        let store = MemStore::new();
        store
            .create_change_request(&test_change_request("cr-1", "wf-1"))
            .await
            .unwrap();

        let mut cr2 = test_change_request("cr-2", "wf-1");
        cr2.status = ChangeRequestStatus::Approved;
        store.create_change_request(&cr2).await.unwrap();

        let (items, total) = store
            .list_change_requests(
                &WorkflowId::new("wf-1"),
                Some(ChangeRequestStatus::Open),
                ListOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].id, "cr-1");
    }

    #[tokio::test]
    async fn test_list_change_requests_pagination() {
        let store = MemStore::new();
        for i in 0..5 {
            store
                .create_change_request(&test_change_request(&format!("cr-{i}"), "wf-1"))
                .await
                .unwrap();
        }

        let (items, total) = store
            .list_change_requests(
                &WorkflowId::new("wf-1"),
                None,
                ListOptions {
                    offset: 1,
                    limit: 2,
                },
            )
            .await
            .unwrap();
        assert_eq!(total, 5);
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_update_change_request() {
        let store = MemStore::new();
        let mut cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        cr.status = ChangeRequestStatus::Approved;
        cr.title = "Updated title".into();
        store.update_change_request(&cr).await.unwrap();

        let fetched = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(fetched.status, ChangeRequestStatus::Approved);
        assert_eq!(fetched.title, "Updated title");
    }

    #[tokio::test]
    async fn test_update_change_request_not_found() {
        let store = MemStore::new();
        let cr = test_change_request("missing", "wf-1");
        let err = store.update_change_request(&cr).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_add_comment() {
        let store = MemStore::new();
        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let comment = test_comment("c-1");
        store.add_comment("cr-1", &comment).await.unwrap();

        let fetched = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(fetched.comments.len(), 1);
        assert_eq!(fetched.comments[0].id, "c-1");
        assert_eq!(fetched.comments[0].body, "Comment c-1");
    }

    #[tokio::test]
    async fn test_add_comment_cr_not_found() {
        let store = MemStore::new();
        let comment = test_comment("c-1");
        let err = store.add_comment("missing", &comment).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_resolve_comment() {
        let store = MemStore::new();
        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let comment = test_comment("c-1");
        store.add_comment("cr-1", &comment).await.unwrap();

        // Resolve the comment.
        store.resolve_comment("cr-1", "c-1", true).await.unwrap();
        let fetched = store.get_change_request("cr-1").await.unwrap();
        assert!(fetched.comments[0].resolved);

        // Unresolve it.
        store.resolve_comment("cr-1", "c-1", false).await.unwrap();
        let fetched = store.get_change_request("cr-1").await.unwrap();
        assert!(!fetched.comments[0].resolved);
    }

    #[tokio::test]
    async fn test_resolve_comment_not_found() {
        let store = MemStore::new();
        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let err = store
            .resolve_comment("cr-1", "missing", true)
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_merge_change_request_success() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let mut cr = test_change_request("cr-1", "wf-1");
        cr.status = ChangeRequestStatus::Approved;
        store.create_change_request(&cr).await.unwrap();

        let new_def = serde_json::json!({"nodes": [{"id": "new"}], "edges": []});
        store
            .merge_change_request("cr-1", 1, &new_def)
            .await
            .unwrap();

        let merged_cr = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(merged_cr.status, ChangeRequestStatus::Merged);

        let updated_wf = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
        assert_eq!(updated_wf.version, 2);
    }

    #[tokio::test]
    async fn test_merge_change_request_not_approved() {
        let store = MemStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let cr = test_change_request("cr-1", "wf-1");
        store.create_change_request(&cr).await.unwrap();

        let err = store
            .merge_change_request("cr-1", 1, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::Conflict));
    }

    #[tokio::test]
    async fn test_merge_change_request_stale_version_succeeds() {
        // Merging a CR whose base_version is behind the current workflow
        // version should still succeed — the proposed definition is the
        // desired end state, and the version bumps from current.
        let store = MemStore::new();
        let mut wf = test_workflow("wf-1");
        wf.version = 3;
        store.create_workflow(&wf).await.unwrap();

        let mut cr = test_change_request("cr-1", "wf-1");
        cr.status = ChangeRequestStatus::Approved;
        store.create_change_request(&cr).await.unwrap();

        let new_def = serde_json::json!({"nodes": [{"id": "merged"}], "edges": []});
        store
            .merge_change_request("cr-1", 1, &new_def)
            .await
            .unwrap();

        let merged_cr = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(merged_cr.status, ChangeRequestStatus::Merged);

        let updated_wf = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
        assert_eq!(updated_wf.version, 4); // bumped from 3 → 4
    }

    #[tokio::test]
    async fn test_merge_change_request_cr_not_found() {
        let store = MemStore::new();
        let err = store
            .merge_change_request("missing", 1, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_full_change_request_lifecycle() {
        let store = MemStore::new();

        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        // 1. Create a draft CR.
        let mut cr = test_change_request("cr-1", "wf-1");
        cr.status = ChangeRequestStatus::Draft;
        store.create_change_request(&cr).await.unwrap();

        // 2. Open it for review.
        cr.status = ChangeRequestStatus::Open;
        store.update_change_request(&cr).await.unwrap();

        // 3. Add a comment.
        let comment = test_comment("c-1");
        store.add_comment("cr-1", &comment).await.unwrap();

        // 4. Resolve the comment.
        store.resolve_comment("cr-1", "c-1", true).await.unwrap();

        // 5. Approve — re-fetch to preserve comments added above.
        let mut cr = store.get_change_request("cr-1").await.unwrap();
        cr.status = ChangeRequestStatus::Approved;
        store.update_change_request(&cr).await.unwrap();

        // 6. Merge.
        let new_def = serde_json::json!({"nodes": [{"id": "merged-node"}], "edges": []});
        store
            .merge_change_request("cr-1", 1, &new_def)
            .await
            .unwrap();

        // Verify final state.
        let final_cr = store.get_change_request("cr-1").await.unwrap();
        assert_eq!(final_cr.status, ChangeRequestStatus::Merged);
        assert_eq!(final_cr.comments.len(), 1);
        assert!(final_cr.comments[0].resolved);

        let final_wf = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
        assert_eq!(final_wf.version, 2);
    }
}
