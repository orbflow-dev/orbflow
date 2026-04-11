// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mock implementation of [`Store`] for testing.
//!
//! Wraps in-memory maps (like Go's `mock.Store`) with optional callback hooks
//! so tests can intercept and validate store operations.

use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::RwLock;

use orbflow_core::error::OrbflowError;
use orbflow_core::event::DomainEvent;
use orbflow_core::execution::{Instance, InstanceId, InstanceStatus};
use orbflow_core::pagination::paginate;
use orbflow_core::ports::{EventStore, InstanceStore, ListOptions, Store, WorkflowStore};
use orbflow_core::versioning::WorkflowVersion;
use orbflow_core::workflow::{Workflow, WorkflowId};

/// Callback type for intercepting store operations.
pub type StoreHook<T> = Box<dyn Fn(&T) -> Result<(), OrbflowError> + Send + Sync>;

/// Mock store backed by in-memory maps with optional callback hooks.
///
/// All operations are thread-safe via [`RwLock`]. Hooks are invoked *before*
/// the operation proceeds, and returning an error from a hook short-circuits
/// the call.
pub struct MockStore {
    inner: RwLock<MockStoreInner>,

    // Optional hooks for intercepting calls in tests.
    pub on_create_workflow: Option<StoreHook<Workflow>>,
    pub on_update_instance: Option<StoreHook<Instance>>,
}

struct MockStoreInner {
    workflows: HashMap<WorkflowId, Workflow>,
    instances: HashMap<InstanceId, Instance>,
    events: HashMap<InstanceId, Vec<DomainEvent>>,
    snapshots: HashMap<InstanceId, Instance>,
}

impl MockStore {
    /// Creates a new, empty mock store with no hooks.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(MockStoreInner {
                workflows: HashMap::new(),
                instances: HashMap::new(),
                events: HashMap::new(),
                snapshots: HashMap::new(),
            }),
            on_create_workflow: None,
            on_update_instance: None,
        }
    }

    /// Returns all events for an instance (for test assertions).
    pub fn events(&self, instance_id: &InstanceId) -> Vec<DomainEvent> {
        self.inner
            .read()
            .events
            .get(instance_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the number of stored workflows.
    pub fn workflow_count(&self) -> usize {
        self.inner.read().workflows.len()
    }

    /// Returns the number of stored instances.
    pub fn instance_count(&self) -> usize {
        self.inner.read().instances.len()
    }
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

// Blanket Store marker — requires WorkflowStore + InstanceStore + EventStore.
impl Store for MockStore {}

#[async_trait]
impl WorkflowStore for MockStore {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        if let Some(ref hook) = self.on_create_workflow {
            hook(wf)?;
        }
        let mut inner = self.inner.write();
        if inner.workflows.contains_key(&wf.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        inner.workflows.insert(wf.id.clone(), wf.clone());
        Ok(())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        self.inner
            .read()
            .workflows
            .get(id)
            .cloned()
            .ok_or(OrbflowError::NotFound)
    }

    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let mut inner = self.inner.write();
        if !inner.workflows.contains_key(&wf.id) {
            return Err(OrbflowError::NotFound);
        }
        inner.workflows.insert(wf.id.clone(), wf.clone());
        Ok(())
    }

    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError> {
        let mut inner = self.inner.write();
        if inner.workflows.remove(id).is_none() {
            return Err(OrbflowError::NotFound);
        }
        Ok(())
    }

    async fn list_workflows(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        let inner = self.inner.read();
        let all: Vec<Workflow> = inner.workflows.values().cloned().collect();
        let total = all.len() as i64;
        Ok((paginate(&all, &opts), total))
    }

    async fn save_workflow_version(&self, _version: &WorkflowVersion) -> Result<(), OrbflowError> {
        // MockStore intentionally discards version snapshots.
        Ok(())
    }
}

#[async_trait]
impl InstanceStore for MockStore {
    async fn create_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        let mut inner = self.inner.write();
        if inner.instances.contains_key(&inst.id) {
            return Err(OrbflowError::AlreadyExists);
        }
        inner.instances.insert(inst.id.clone(), inst.clone());
        Ok(())
    }

    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError> {
        self.inner
            .read()
            .instances
            .get(id)
            .cloned()
            .ok_or(OrbflowError::NotFound)
    }

    async fn update_instance(&self, inst: &Instance) -> Result<(), OrbflowError> {
        if let Some(ref hook) = self.on_update_instance {
            hook(inst)?;
        }
        self.inner
            .write()
            .instances
            .insert(inst.id.clone(), inst.clone());
        Ok(())
    }

    async fn list_instances(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        let inner = self.inner.read();
        let all: Vec<Instance> = inner.instances.values().cloned().collect();
        let total = all.len() as i64;
        Ok((paginate(&all, &opts), total))
    }

    async fn list_running_instances(&self) -> Result<Vec<Instance>, OrbflowError> {
        let inner = self.inner.read();
        let running: Vec<Instance> = inner
            .instances
            .values()
            .filter(|inst| inst.status == InstanceStatus::Running)
            .cloned()
            .collect();
        Ok(running)
    }
}

#[async_trait]
impl EventStore for MockStore {
    async fn append_event(&self, event: DomainEvent) -> Result<(), OrbflowError> {
        let id = event.instance_id().clone();
        self.inner.write().events.entry(id).or_default().push(event);
        Ok(())
    }

    async fn load_events(
        &self,
        instance_id: &InstanceId,
        after_version: i64,
    ) -> Result<Vec<DomainEvent>, OrbflowError> {
        let inner = self.inner.read();
        let events = match inner.events.get(instance_id) {
            Some(events) => events,
            None => return Ok(Vec::new()),
        };

        if after_version <= 0 {
            return Ok(events.clone());
        }

        let skip = after_version as usize;
        if skip >= events.len() {
            return Ok(Vec::new());
        }

        Ok(events[skip..].to_vec())
    }

    async fn save_snapshot(&self, inst: &Instance) -> Result<(), OrbflowError> {
        self.inner
            .write()
            .snapshots
            .insert(inst.id.clone(), inst.clone());
        Ok(())
    }

    async fn load_snapshot(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<Instance>, OrbflowError> {
        Ok(self.inner.read().snapshots.get(instance_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;

    use orbflow_core::event::{BaseEvent, InstanceStartedEvent, NodeCompletedEvent};
    use orbflow_core::execution::{ExecutionContext, InstanceStatus};
    use orbflow_core::workflow::{DefinitionStatus, WorkflowId};

    use super::*;

    fn test_workflow(id: &str) -> Workflow {
        Workflow {
            id: WorkflowId::new(id),
            name: format!("Test {id}"),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![],
            edges: vec![],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_instance(id: &str, wf_id: &str) -> Instance {
        Instance {
            id: InstanceId::new(id),
            workflow_id: WorkflowId::new(wf_id),
            status: InstanceStatus::Running,
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

    // --- WorkflowStore tests ---

    #[tokio::test]
    async fn test_create_and_get_workflow() {
        let store = MockStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let got = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
        assert_eq!(got.id, wf.id);
        assert_eq!(got.name, wf.name);
    }

    #[tokio::test]
    async fn test_create_duplicate_workflow_fails() {
        let store = MockStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        let err = store.create_workflow(&wf).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_get_missing_workflow_fails() {
        let store = MockStore::new();
        let err = store
            .get_workflow(&WorkflowId::new("nope"))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_update_workflow() {
        let store = MockStore::new();
        let mut wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();

        wf.name = "Updated".into();
        store.update_workflow(&wf).await.unwrap();

        let got = store.get_workflow(&wf.id).await.unwrap();
        assert_eq!(got.name, "Updated");
    }

    #[tokio::test]
    async fn test_update_missing_workflow_fails() {
        let store = MockStore::new();
        let wf = test_workflow("wf-1");
        let err = store.update_workflow(&wf).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_workflow() {
        let store = MockStore::new();
        let wf = test_workflow("wf-1");
        store.create_workflow(&wf).await.unwrap();
        store.delete_workflow(&wf.id).await.unwrap();

        let err = store.get_workflow(&wf.id).await.unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_missing_workflow_fails() {
        let store = MockStore::new();
        let err = store
            .delete_workflow(&WorkflowId::new("nope"))
            .await
            .unwrap_err();
        assert!(matches!(err, OrbflowError::NotFound));
    }

    #[tokio::test]
    async fn test_list_workflows_pagination() {
        let store = MockStore::new();
        for i in 0..5 {
            store
                .create_workflow(&test_workflow(&format!("wf-{i}")))
                .await
                .unwrap();
        }

        let (all, total) = store
            .list_workflows(ListOptions {
                offset: 0,
                limit: 0,
            })
            .await
            .unwrap();
        assert_eq!(total, 5);
        assert_eq!(all.len(), 5);

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

    // --- InstanceStore tests ---

    #[tokio::test]
    async fn test_create_and_get_instance() {
        let store = MockStore::new();
        let inst = test_instance("inst-1", "wf-1");
        store.create_instance(&inst).await.unwrap();

        let got = store
            .get_instance(&InstanceId::new("inst-1"))
            .await
            .unwrap();
        assert_eq!(got.id, inst.id);
    }

    #[tokio::test]
    async fn test_create_duplicate_instance_fails() {
        let store = MockStore::new();
        let inst = test_instance("inst-1", "wf-1");
        store.create_instance(&inst).await.unwrap();

        let err = store.create_instance(&inst).await.unwrap_err();
        assert!(matches!(err, OrbflowError::AlreadyExists));
    }

    #[tokio::test]
    async fn test_update_instance() {
        let store = MockStore::new();
        let mut inst = test_instance("inst-1", "wf-1");
        store.create_instance(&inst).await.unwrap();

        inst.status = InstanceStatus::Completed;
        store.update_instance(&inst).await.unwrap();

        let got = store.get_instance(&inst.id).await.unwrap();
        assert_eq!(got.status, InstanceStatus::Completed);
    }

    #[tokio::test]
    async fn test_list_running_instances() {
        let store = MockStore::new();

        let running = test_instance("inst-1", "wf-1");
        store.create_instance(&running).await.unwrap();

        let mut completed = test_instance("inst-2", "wf-1");
        completed.status = InstanceStatus::Completed;
        store.create_instance(&completed).await.unwrap();

        let result = store.list_running_instances().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, InstanceId::new("inst-1"));
    }

    // --- EventStore tests ---

    #[tokio::test]
    async fn test_append_and_load_events() {
        let store = MockStore::new();
        let iid = InstanceId::new("inst-1");

        let event1 = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(iid.clone(), 1),
            input: HashMap::new(),
        });
        let event2 = DomainEvent::NodeCompleted(NodeCompletedEvent {
            base: BaseEvent::new(iid.clone(), 2),
            node_id: "a".into(),
            output: None,
        });

        store.append_event(event1).await.unwrap();
        store.append_event(event2).await.unwrap();

        let all = store.load_events(&iid, 0).await.unwrap();
        assert_eq!(all.len(), 2);

        let after = store.load_events(&iid, 1).await.unwrap();
        assert_eq!(after.len(), 1);

        let none = store.load_events(&iid, 10).await.unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn test_load_events_missing_instance() {
        let store = MockStore::new();
        let events = store
            .load_events(&InstanceId::new("nope"), 0)
            .await
            .unwrap();
        assert!(events.is_empty());
    }

    // --- Snapshot tests ---

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        let store = MockStore::new();
        let inst = test_instance("inst-1", "wf-1");
        store.save_snapshot(&inst).await.unwrap();

        let snap = store
            .load_snapshot(&InstanceId::new("inst-1"))
            .await
            .unwrap();
        assert!(snap.is_some());
        assert_eq!(snap.unwrap().id, inst.id);
    }

    #[tokio::test]
    async fn test_load_missing_snapshot() {
        let store = MockStore::new();
        let snap = store.load_snapshot(&InstanceId::new("nope")).await.unwrap();
        assert!(snap.is_none());
    }

    // --- Hook tests ---

    #[tokio::test]
    async fn test_on_create_workflow_hook() {
        let mut store = MockStore::new();
        store.on_create_workflow = Some(Box::new(|wf| {
            if wf.name == "blocked" {
                return Err(OrbflowError::Internal("hook blocked creation".into()));
            }
            Ok(())
        }));

        let mut wf = test_workflow("wf-1");
        wf.name = "blocked".into();
        let err = store.create_workflow(&wf).await.unwrap_err();
        assert!(matches!(err, OrbflowError::Internal(_)));

        // Not-blocked workflow succeeds.
        let wf2 = test_workflow("wf-2");
        store.create_workflow(&wf2).await.unwrap();
    }

    #[tokio::test]
    async fn test_on_update_instance_hook() {
        let mut store = MockStore::new();
        store.on_update_instance = Some(Box::new(|inst| {
            if inst.status == InstanceStatus::Failed {
                return Err(OrbflowError::Internal("hook rejected failed update".into()));
            }
            Ok(())
        }));

        let inst = test_instance("inst-1", "wf-1");
        store.create_instance(&inst).await.unwrap();

        // Normal update succeeds.
        let mut updated = inst.clone();
        updated.status = InstanceStatus::Completed;
        store.update_instance(&updated).await.unwrap();

        // Failed status blocked by hook.
        let mut failed = inst.clone();
        failed.status = InstanceStatus::Failed;
        let err = store.update_instance(&failed).await.unwrap_err();
        assert!(matches!(err, OrbflowError::Internal(_)));
    }

    // --- Helper method tests ---

    #[tokio::test]
    async fn test_events_helper() {
        let store = MockStore::new();
        let iid = InstanceId::new("inst-1");

        let event = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(iid.clone(), 1),
            input: HashMap::new(),
        });
        store.append_event(event).await.unwrap();

        let events = store.events(&iid);
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_count_helpers() {
        let store = MockStore::new();
        assert_eq!(store.workflow_count(), 0);
        assert_eq!(store.instance_count(), 0);

        store.create_workflow(&test_workflow("wf-1")).await.unwrap();
        assert_eq!(store.workflow_count(), 1);

        store
            .create_instance(&test_instance("inst-1", "wf-1"))
            .await
            .unwrap();
        assert_eq!(store.instance_count(), 1);
    }
}
