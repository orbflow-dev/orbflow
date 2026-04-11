// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Engine configuration options (builder pattern replaces Go functional options).

use std::sync::Arc;

use crate::metering::Budget;
use crate::ports::{BudgetStore, Bus, CredentialStore, MetricsStore, Store};
use crate::rbac::RbacPolicy;

/// Configuration for constructing an Engine.
pub struct EngineOptions {
    pub store: Arc<dyn Store>,
    pub bus: Arc<dyn Bus>,
    pub credential_store: Option<Arc<dyn CredentialStore>>,
    pub metrics_store: Option<Arc<dyn MetricsStore>>,
    pub pool_name: String,
    pub snapshot_interval: i64,
    pub enable_resume: bool,
    /// Optional budget for per-execution cost/resource enforcement.
    pub budget: Option<Budget>,
    /// Optional RBAC policy for permission enforcement.
    pub rbac: Option<RbacPolicy>,
    /// Optional persistent budget store for org-level cost enforcement.
    pub budget_store: Option<Arc<dyn BudgetStore>>,
}

impl std::fmt::Debug for EngineOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineOptions")
            .field("pool_name", &self.pool_name)
            .field("snapshot_interval", &self.snapshot_interval)
            .field("enable_resume", &self.enable_resume)
            .finish_non_exhaustive()
    }
}

/// Builder for [`EngineOptions`].
pub struct EngineOptionsBuilder {
    store: Option<Arc<dyn Store>>,
    bus: Option<Arc<dyn Bus>>,
    credential_store: Option<Arc<dyn CredentialStore>>,
    metrics_store: Option<Arc<dyn MetricsStore>>,
    pool_name: String,
    snapshot_interval: i64,
    enable_resume: bool,
    budget: Option<Budget>,
    rbac: Option<RbacPolicy>,
    budget_store: Option<Arc<dyn BudgetStore>>,
}

impl Default for EngineOptionsBuilder {
    fn default() -> Self {
        Self {
            store: None,
            bus: None,
            credential_store: None,
            metrics_store: None,
            pool_name: "default".into(),
            snapshot_interval: 10,
            enable_resume: true,
            budget: None,
            rbac: None,
            budget_store: None,
        }
    }
}

impl EngineOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(mut self, store: Arc<dyn Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn bus(mut self, bus: Arc<dyn Bus>) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn credential_store(mut self, store: Arc<dyn CredentialStore>) -> Self {
        self.credential_store = Some(store);
        self
    }

    pub fn metrics_store(mut self, store: Arc<dyn MetricsStore>) -> Self {
        self.metrics_store = Some(store);
        self
    }

    pub fn pool_name(mut self, name: impl Into<String>) -> Self {
        self.pool_name = name.into();
        self
    }

    pub fn snapshot_interval(mut self, interval: i64) -> Self {
        self.snapshot_interval = interval;
        self
    }

    pub fn enable_resume(mut self, enable: bool) -> Self {
        self.enable_resume = enable;
        self
    }

    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn rbac(mut self, policy: RbacPolicy) -> Self {
        self.rbac = Some(policy);
        self
    }

    pub fn budget_store(mut self, store: Arc<dyn BudgetStore>) -> Self {
        self.budget_store = Some(store);
        self
    }

    /// Builds the options. Returns an error if required fields are missing
    /// or if values are invalid.
    pub fn build(self) -> Result<EngineOptions, crate::error::OrbflowError> {
        let store = self.store.ok_or_else(|| {
            crate::error::OrbflowError::InvalidNodeConfig("store is required".into())
        })?;
        let bus = self.bus.ok_or_else(|| {
            crate::error::OrbflowError::InvalidNodeConfig("bus is required".into())
        })?;
        if self.pool_name.is_empty() {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(
                "pool_name must not be empty".into(),
            ));
        }
        if self.snapshot_interval < 1 {
            return Err(crate::error::OrbflowError::InvalidNodeConfig(
                "snapshot_interval must be >= 1".into(),
            ));
        }
        Ok(EngineOptions {
            store,
            bus,
            credential_store: self.credential_store,
            metrics_store: self.metrics_store,
            pool_name: self.pool_name,
            snapshot_interval: self.snapshot_interval,
            enable_resume: self.enable_resume,
            budget: self.budget,
            rbac: self.rbac,
            budget_store: self.budget_store,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::OrbflowError;
    use crate::event::DomainEvent;
    use crate::execution::{Instance, InstanceId};
    use crate::metering::Budget;
    use crate::ports::ListOptions;
    use crate::ports::MsgHandler;
    use crate::rbac::RbacPolicy;
    use crate::versioning::WorkflowVersion;
    use crate::workflow::{Workflow, WorkflowId};
    use async_trait::async_trait;

    // --- Stub Store ---

    struct StubStore;

    #[async_trait]
    impl crate::ports::WorkflowStore for StubStore {
        async fn create_workflow(&self, _wf: &Workflow) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn get_workflow(&self, _id: &WorkflowId) -> Result<Workflow, OrbflowError> {
            unimplemented!()
        }
        async fn update_workflow(&self, _wf: &Workflow) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn delete_workflow(&self, _id: &WorkflowId) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn list_workflows(
            &self,
            _opts: ListOptions,
        ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
            unimplemented!()
        }
        async fn save_workflow_version(&self, _v: &WorkflowVersion) -> Result<(), OrbflowError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl crate::ports::InstanceStore for StubStore {
        async fn create_instance(&self, _inst: &Instance) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn get_instance(&self, _id: &InstanceId) -> Result<Instance, OrbflowError> {
            unimplemented!()
        }
        async fn update_instance(&self, _inst: &Instance) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn list_instances(
            &self,
            _opts: ListOptions,
        ) -> Result<(Vec<Instance>, i64), OrbflowError> {
            unimplemented!()
        }
        async fn list_running_instances(&self) -> Result<Vec<Instance>, OrbflowError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl crate::ports::EventStore for StubStore {
        async fn append_event(&self, _event: DomainEvent) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn load_events(
            &self,
            _id: &InstanceId,
            _after: i64,
        ) -> Result<Vec<DomainEvent>, OrbflowError> {
            unimplemented!()
        }
        async fn save_snapshot(&self, _inst: &Instance) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn load_snapshot(&self, _id: &InstanceId) -> Result<Option<Instance>, OrbflowError> {
            unimplemented!()
        }
    }

    impl Store for StubStore {}

    // --- Stub Bus ---

    struct StubBus;

    #[async_trait]
    impl Bus for StubBus {
        async fn publish(&self, _subject: &str, _data: &[u8]) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn subscribe(
            &self,
            _subject: &str,
            _handler: MsgHandler,
        ) -> Result<(), OrbflowError> {
            unimplemented!()
        }
        async fn close(&self) -> Result<(), OrbflowError> {
            unimplemented!()
        }
    }

    fn stub_store() -> Arc<dyn Store> {
        Arc::new(StubStore)
    }

    fn stub_bus() -> Arc<dyn Bus> {
        Arc::new(StubBus)
    }

    #[test]
    fn default_builder_has_expected_defaults() {
        let builder = EngineOptionsBuilder::new();
        let opts = builder.store(stub_store()).bus(stub_bus()).build().unwrap();

        assert_eq!(opts.pool_name, "default");
        assert_eq!(opts.snapshot_interval, 10);
        assert!(opts.enable_resume);
        assert!(opts.credential_store.is_none());
        assert!(opts.metrics_store.is_none());
        assert!(opts.budget.is_none());
        assert!(opts.rbac.is_none());
        assert!(opts.budget_store.is_none());
    }

    #[test]
    fn builder_sets_pool_name() {
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .pool_name("my-pool")
            .build()
            .unwrap();

        assert_eq!(opts.pool_name, "my-pool");
    }

    #[test]
    fn builder_sets_snapshot_interval() {
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .snapshot_interval(42)
            .build()
            .unwrap();

        assert_eq!(opts.snapshot_interval, 42);
    }

    #[test]
    fn builder_sets_enable_resume_false() {
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .enable_resume(false)
            .build()
            .unwrap();

        assert!(!opts.enable_resume);
    }

    #[test]
    fn builder_sets_budget() {
        let budget = Budget {
            limit_usd: Some(10.0),
            limit_tokens: Some(5000),
            limit_wall_time_ms: None,
        };
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .budget(budget)
            .build()
            .unwrap();

        let b = opts.budget.expect("budget should be set");
        assert_eq!(b.limit_usd, Some(10.0));
        assert_eq!(b.limit_tokens, Some(5000));
    }

    #[test]
    fn builder_sets_rbac() {
        let policy = RbacPolicy::new();
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .rbac(policy)
            .build()
            .unwrap();

        assert!(opts.rbac.is_some());
    }

    #[test]
    fn builder_pool_name_accepts_string() {
        let name = String::from("owned-pool");
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .pool_name(name)
            .build()
            .unwrap();

        assert_eq!(opts.pool_name, "owned-pool");
    }

    #[test]
    fn build_fails_without_store() {
        let result = EngineOptionsBuilder::new().bus(stub_bus()).build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("store is required")
        );
    }

    #[test]
    fn build_fails_without_bus() {
        let result = EngineOptionsBuilder::new().store(stub_store()).build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bus is required"));
    }

    #[test]
    fn build_fails_with_empty_pool_name() {
        let result = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .pool_name("")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pool_name"));
    }

    #[test]
    fn build_fails_with_zero_snapshot_interval() {
        let result = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .snapshot_interval(0)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("snapshot_interval")
        );
    }

    #[test]
    fn build_fails_with_negative_snapshot_interval() {
        let result = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .snapshot_interval(-1)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_chaining_all_options() {
        let opts = EngineOptionsBuilder::new()
            .store(stub_store())
            .bus(stub_bus())
            .pool_name("full")
            .snapshot_interval(99)
            .enable_resume(false)
            .budget(Budget {
                limit_usd: Some(1.0),
                limit_tokens: None,
                limit_wall_time_ms: Some(60_000),
            })
            .rbac(RbacPolicy::new())
            .build()
            .unwrap();

        assert_eq!(opts.pool_name, "full");
        assert_eq!(opts.snapshot_interval, 99);
        assert!(!opts.enable_resume);
        assert!(opts.budget.is_some());
        assert!(opts.rbac.is_some());
    }
}
