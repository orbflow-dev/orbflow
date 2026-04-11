// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cron-based trigger: schedules periodic workflow starts.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use orbflow_core::workflow::WorkflowId;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::TriggerCallback;

/// Schedules cron jobs that fire workflow triggers on a schedule.
pub struct CronScheduler {
    /// Maps workflow ID -> list of job UUIDs registered for that workflow.
    jobs: DashMap<String, Vec<Uuid>>,
    scheduler: JobScheduler,
    fire: TriggerCallback,
}

impl CronScheduler {
    /// Creates a new cron scheduler.
    pub async fn new(fire: TriggerCallback) -> Result<Self, JobSchedulerError> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            jobs: DashMap::new(),
            scheduler,
            fire,
        })
    }

    /// Registers a cron job for a workflow.
    ///
    /// The `cron_expr` should be a standard cron expression (5 or 6 fields).
    pub async fn add(&self, workflow_id: &WorkflowId, cron_expr: &str) {
        let wf_id = workflow_id.clone();
        let fire = Arc::clone(&self.fire);

        let job = match Job::new_async(cron_expr, move |_uuid, _lock| {
            let wf_id = wf_id.clone();
            let fire = Arc::clone(&fire);
            Box::pin(async move {
                info!(workflow = %wf_id, "cron trigger fired");
                let payload = HashMap::new();
                fire(wf_id, orbflow_core::TriggerType::Schedule, payload).await;
            })
        }) {
            Ok(job) => job,
            Err(e) => {
                error!(
                    workflow = %workflow_id,
                    cron = %cron_expr,
                    error = %e,
                    "failed to create cron job"
                );
                return;
            }
        };

        let job_id = job.guid();
        match self.scheduler.add(job).await {
            Ok(_) => {
                self.jobs
                    .entry(workflow_id.to_string())
                    .or_default()
                    .push(job_id);
                info!(
                    workflow = %workflow_id,
                    cron = %cron_expr,
                    "cron job registered"
                );
            }
            Err(e) => {
                error!(
                    workflow = %workflow_id,
                    error = %e,
                    "failed to add cron job to scheduler"
                );
            }
        }
    }

    /// Removes all cron jobs for a workflow.
    pub async fn remove(&self, workflow_id: &WorkflowId) {
        if let Some((_, job_ids)) = self.jobs.remove(&workflow_id.to_string()) {
            for job_id in job_ids {
                if let Err(e) = self.scheduler.remove(&job_id).await {
                    warn!(
                        workflow = %workflow_id,
                        job = %job_id,
                        error = %e,
                        "failed to remove cron job"
                    );
                }
            }
        }
    }

    /// Starts the cron scheduler.
    pub async fn start(&self) -> Result<(), JobSchedulerError> {
        self.scheduler.start().await?;
        info!("cron scheduler started");
        Ok(())
    }

    /// Stops the cron scheduler.
    pub async fn stop(&mut self) -> Result<(), JobSchedulerError> {
        self.scheduler.shutdown().await?;
        info!("cron scheduler stopped");
        Ok(())
    }
}
