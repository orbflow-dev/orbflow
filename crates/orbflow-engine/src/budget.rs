// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Budget enforcement checks for workflow execution.

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::BudgetStore;

/// Checks whether a workflow has exceeded its budget before allowing execution.
///
/// Returns `Ok(())` if no budget is configured or the budget has not been exceeded.
/// Returns `Err(OrbflowError::BudgetExceeded)` if the workflow's accumulated cost
/// has reached or exceeded the configured limit.
pub async fn check_budget_before_start(
    budget_store: &dyn BudgetStore,
    workflow_id: &str,
) -> Result<(), OrbflowError> {
    if let Some(budget) = budget_store.check_budget(workflow_id).await?
        && budget.current_usd >= budget.limit_usd
    {
        return Err(OrbflowError::BudgetExceeded(format!(
            "Budget exceeded for workflow {}: ${:.2} / ${:.2}",
            workflow_id, budget.current_usd, budget.limit_usd
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use orbflow_core::metering::{AccountBudget, BudgetPeriod};

    struct MockBudgetStore {
        budget: Option<AccountBudget>,
    }

    #[async_trait]
    impl BudgetStore for MockBudgetStore {
        async fn create_budget(&self, _budget: &AccountBudget) -> Result<(), OrbflowError> {
            Ok(())
        }
        async fn get_budget(&self, _id: &str) -> Result<AccountBudget, OrbflowError> {
            self.budget.clone().ok_or(OrbflowError::NotFound)
        }
        async fn list_budgets(&self) -> Result<Vec<AccountBudget>, OrbflowError> {
            Ok(self.budget.iter().cloned().collect())
        }
        async fn update_budget(&self, _budget: &AccountBudget) -> Result<(), OrbflowError> {
            Ok(())
        }
        async fn delete_budget(&self, _id: &str) -> Result<(), OrbflowError> {
            Ok(())
        }
        async fn check_budget(
            &self,
            _workflow_id: &str,
        ) -> Result<Option<AccountBudget>, OrbflowError> {
            Ok(self.budget.clone())
        }
        async fn increment_cost(
            &self,
            _workflow_id: &str,
            _cost_usd: f64,
        ) -> Result<(), OrbflowError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn allows_when_no_budget() {
        let store = MockBudgetStore { budget: None };
        let result = check_budget_before_start(&store, "wf-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn allows_when_under_budget() {
        let store = MockBudgetStore {
            budget: Some(AccountBudget {
                id: "b-1".into(),
                workflow_id: Some("wf-1".into()),
                team: None,
                period: BudgetPeriod::Monthly,
                limit_usd: 100.0,
                current_usd: 50.0,
                reset_at: Utc::now(),
                created_at: Utc::now(),
            }),
        };
        let result = check_budget_before_start(&store, "wf-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn rejects_when_budget_exceeded() {
        let store = MockBudgetStore {
            budget: Some(AccountBudget {
                id: "b-1".into(),
                workflow_id: Some("wf-1".into()),
                team: None,
                period: BudgetPeriod::Monthly,
                limit_usd: 100.0,
                current_usd: 100.0,
                reset_at: Utc::now(),
                created_at: Utc::now(),
            }),
        };
        let result = check_budget_before_start(&store, "wf-1").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_budget_exceeded());
    }
}
