// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fine-grained Role-Based Access Control with step-level permissions.
//!
//! Supports permissions at three granularity levels:
//! - **Global**: applies to all workflows and nodes
//! - **Workflow**: applies to all nodes within a specific workflow
//! - **Node**: applies to a specific node within a specific workflow
//!
//! Permissions are resolved from most specific to least specific (node > workflow > global).

use crate::error::OrbflowError;
use serde::{Deserialize, Serialize};

/// IDs of the four builtin roles that cannot be deleted or modified.
pub const BUILTIN_ROLE_IDS: &[&str] = &["viewer", "editor", "operator", "admin"];

/// Actions that can be controlled by RBAC policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// View workflow definitions and execution results.
    View,
    /// Edit workflow definitions (add/remove/modify nodes).
    Edit,
    /// Execute/start a workflow.
    Execute,
    /// Approve nodes in WaitingApproval state.
    Approve,
    /// Delete workflows.
    Delete,
    /// Manage credentials used by nodes.
    ManageCredentials,
    /// Administer RBAC policies.
    Admin,
}

/// A role groups a set of permissions under a named identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Unique role identifier (e.g., "viewer", "editor", "operator", "admin").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Permissions granted by this role.
    pub permissions: Vec<Permission>,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this role is a builtin role (non-editable, non-deletable).
    #[serde(default)]
    pub builtin: bool,
}

/// The scope at which a policy applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyScope {
    /// Applies globally to all workflows.
    Global,
    /// Applies to a specific workflow.
    Workflow { workflow_id: String },
    /// Applies to a specific node within a specific workflow.
    Node {
        workflow_id: String,
        node_id: String,
    },
}

/// A policy binding: assigns a role to a subject (user/team) at a scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBinding {
    /// The subject this policy applies to (user ID, team ID, email).
    pub subject: String,
    /// The role ID to grant.
    pub role_id: String,
    /// The scope at which this binding applies.
    pub scope: PolicyScope,
}

/// The RBAC policy store — holds all role definitions and policy bindings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RbacPolicy {
    /// Available role definitions.
    pub roles: Vec<Role>,
    /// Active policy bindings.
    pub bindings: Vec<PolicyBinding>,
}

impl RbacPolicy {
    /// Creates a new empty policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a default policy with standard roles (viewer, editor, operator, admin).
    pub fn with_defaults() -> Self {
        Self {
            roles: vec![
                Role {
                    id: "viewer".into(),
                    name: "Viewer".into(),
                    permissions: vec![Permission::View],
                    description: Some("Can view workflows and execution results".into()),
                    builtin: true,
                },
                Role {
                    id: "editor".into(),
                    name: "Editor".into(),
                    permissions: vec![Permission::View, Permission::Edit],
                    description: Some("Can view and edit workflow definitions".into()),
                    builtin: true,
                },
                Role {
                    id: "operator".into(),
                    name: "Operator".into(),
                    permissions: vec![Permission::View, Permission::Execute, Permission::Approve],
                    description: Some("Can view, execute workflows, and approve nodes".into()),
                    builtin: true,
                },
                Role {
                    id: "admin".into(),
                    name: "Admin".into(),
                    permissions: vec![
                        Permission::View,
                        Permission::Edit,
                        Permission::Execute,
                        Permission::Approve,
                        Permission::Delete,
                        Permission::ManageCredentials,
                        Permission::Admin,
                    ],
                    description: Some("Full access to all operations".into()),
                    builtin: true,
                },
            ],
            bindings: Vec::new(),
        }
    }

    /// Ensures all builtin roles are present, prepending any that are missing.
    pub fn ensure_defaults(&mut self) {
        let defaults = Self::with_defaults();
        for builtin in defaults.roles.into_iter().rev() {
            if !self.roles.iter().any(|r| r.id == builtin.id) {
                self.roles.insert(0, builtin);
            }
        }
    }

    /// Finds a role by ID.
    pub fn get_role(&self, role_id: &str) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == role_id)
    }

    /// Checks if a subject has a specific permission at the given scope.
    ///
    /// Resolution order (most specific wins):
    /// 1. Node-level binding for this workflow + node
    /// 2. Workflow-level binding for this workflow
    /// 3. Global binding
    pub fn has_permission(
        &self,
        subject: &str,
        permission: Permission,
        workflow_id: &str,
        node_id: Option<&str>,
    ) -> bool {
        // Collect all applicable bindings for this subject, ordered by specificity.
        let mut applicable_bindings: Vec<&PolicyBinding> = Vec::new();

        for binding in &self.bindings {
            if binding.subject != subject {
                continue;
            }

            let applies = match &binding.scope {
                PolicyScope::Node {
                    workflow_id: wid,
                    node_id: nid,
                } => wid == workflow_id && node_id.is_some_and(|n| n == nid),
                PolicyScope::Workflow { workflow_id: wid } => wid == workflow_id,
                PolicyScope::Global => true,
            };

            if applies {
                applicable_bindings.push(binding);
            }
        }

        // Sort by specificity: Node > Workflow > Global.
        applicable_bindings.sort_by_key(|b| match &b.scope {
            PolicyScope::Node { .. } => 0,
            PolicyScope::Workflow { .. } => 1,
            PolicyScope::Global => 2,
        });

        // Check if any applicable role grants the permission.
        for binding in &applicable_bindings {
            if let Some(role) = self.get_role(&binding.role_id)
                && role.permissions.contains(&permission)
            {
                return true;
            }
        }

        false
    }

    /// Returns true if the policy has at least one binding whose role includes Admin permission.
    pub fn has_admin_binding(&self) -> bool {
        self.bindings.iter().any(|binding| {
            self.get_role(&binding.role_id)
                .is_some_and(|role| role.permissions.contains(&Permission::Admin))
        })
    }

    /// Validates a proposed policy update.
    ///
    /// Ensures builtin roles are preserved with their original permissions,
    /// no duplicate role IDs, valid custom role IDs, every role has at least
    /// one permission, and every binding references an existing role.
    pub fn validate_update(new_policy: &RbacPolicy) -> Result<(), OrbflowError> {
        use std::sync::LazyLock;

        static BUILTIN_DEFAULTS: LazyLock<RbacPolicy> = LazyLock::new(RbacPolicy::with_defaults);
        static ROLE_ID_REGEX: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"^[a-z0-9-]+$").expect("hard-coded pattern"));

        let defaults = &*BUILTIN_DEFAULTS;
        let role_id_regex = &*ROLE_ID_REGEX;

        // 1. All 4 builtin roles must be present with matching IDs.
        for builtin in &defaults.roles {
            let found = new_policy.roles.iter().find(|r| r.id == builtin.id);
            match found {
                None => {
                    return Err(OrbflowError::InvalidPolicy(format!(
                        "rbac: builtin role '{}' must not be removed",
                        builtin.id
                    )));
                }
                Some(role) => {
                    // 2. Builtin roles must have builtin = true.
                    if !role.builtin {
                        return Err(OrbflowError::InvalidPolicy(format!(
                            "rbac: builtin role '{}' must have builtin = true",
                            builtin.id
                        )));
                    }
                    // 3. Builtin role permissions must match defaults exactly.
                    if role.permissions != builtin.permissions {
                        return Err(OrbflowError::InvalidPolicy(format!(
                            "rbac: permissions for builtin role '{}' cannot be modified",
                            builtin.id
                        )));
                    }
                }
            }
        }

        // Collect role IDs for duplicate and binding checks.
        let mut seen_ids = std::collections::HashSet::new();

        for role in &new_policy.roles {
            // For custom roles, validate format FIRST (before ID appears in error messages).
            if !BUILTIN_ROLE_IDS.contains(&role.id.as_str()) {
                if role.id.is_empty() || role.id.len() > 64 || !role_id_regex.is_match(&role.id) {
                    return Err(OrbflowError::InvalidPolicy(
                        "rbac: custom role ID must be 1-64 characters matching [a-z0-9-]".into(),
                    ));
                }
                // Custom roles must not claim builtin status.
                if role.builtin {
                    return Err(OrbflowError::InvalidPolicy(format!(
                        "rbac: custom role '{}' must not have builtin = true",
                        role.id
                    )));
                }
                // Custom roles may not include the Admin permission.
                if role.permissions.contains(&Permission::Admin) {
                    return Err(OrbflowError::InvalidPolicy(format!(
                        "rbac: custom role '{}' may not include the Admin permission; \
                         use a binding to the builtin 'admin' role instead",
                        role.id
                    )));
                }
            }

            // No duplicate role IDs.
            if !seen_ids.insert(&role.id) {
                return Err(OrbflowError::InvalidPolicy(format!(
                    "rbac: duplicate role ID '{}'",
                    role.id
                )));
            }

            // Every role must have at least one permission.
            if role.permissions.is_empty() {
                return Err(OrbflowError::InvalidPolicy(format!(
                    "rbac: role '{}' must have at least one permission",
                    role.id
                )));
            }
        }

        // 7. Every binding must reference an existing role ID.
        for binding in &new_policy.bindings {
            if !seen_ids.contains(&binding.role_id) {
                return Err(OrbflowError::InvalidPolicy(format!(
                    "rbac: binding references non-existent role '{}'",
                    binding.role_id
                )));
            }
        }

        Ok(())
    }

    /// Adds a policy binding. Returns an error if the role does not exist.
    pub fn add_binding(
        &mut self,
        binding: PolicyBinding,
    ) -> Result<(), crate::error::OrbflowError> {
        if self.get_role(&binding.role_id).is_none() {
            return Err(crate::error::OrbflowError::InvalidPolicy(format!(
                "rbac: role '{}' does not exist; create the role before binding it",
                binding.role_id
            )));
        }
        self.bindings.push(binding);
        Ok(())
    }

    /// Lists all permissions a subject has for a given workflow/node.
    pub fn effective_permissions(
        &self,
        subject: &str,
        workflow_id: &str,
        node_id: Option<&str>,
    ) -> Vec<Permission> {
        let all_perms = [
            Permission::View,
            Permission::Edit,
            Permission::Execute,
            Permission::Approve,
            Permission::Delete,
            Permission::ManageCredentials,
            Permission::Admin,
        ];

        all_perms
            .iter()
            .filter(|p| self.has_permission(subject, **p, workflow_id, node_id))
            .copied()
            .collect()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_policy() -> RbacPolicy {
        let mut policy = RbacPolicy::with_defaults();

        // Alice is a global viewer.
        policy
            .add_binding(PolicyBinding {
                subject: "alice".into(),
                role_id: "viewer".into(),
                scope: PolicyScope::Global,
            })
            .unwrap();

        // Bob is an editor on workflow "wf-1".
        policy
            .add_binding(PolicyBinding {
                subject: "bob".into(),
                role_id: "editor".into(),
                scope: PolicyScope::Workflow {
                    workflow_id: "wf-1".into(),
                },
            })
            .unwrap();

        // Carol is an operator on node "sensitive-node" in workflow "wf-1".
        policy
            .add_binding(PolicyBinding {
                subject: "carol".into(),
                role_id: "operator".into(),
                scope: PolicyScope::Node {
                    workflow_id: "wf-1".into(),
                    node_id: "sensitive-node".into(),
                },
            })
            .unwrap();

        // Dave is a global admin.
        policy
            .add_binding(PolicyBinding {
                subject: "dave".into(),
                role_id: "admin".into(),
                scope: PolicyScope::Global,
            })
            .unwrap();

        policy
    }

    #[test]
    fn test_global_viewer() {
        let policy = setup_policy();
        assert!(policy.has_permission("alice", Permission::View, "wf-1", None));
        assert!(policy.has_permission("alice", Permission::View, "wf-2", None));
        assert!(!policy.has_permission("alice", Permission::Edit, "wf-1", None));
        assert!(!policy.has_permission("alice", Permission::Execute, "wf-1", None));
    }

    #[test]
    fn test_workflow_editor() {
        let policy = setup_policy();
        assert!(policy.has_permission("bob", Permission::View, "wf-1", None));
        assert!(policy.has_permission("bob", Permission::Edit, "wf-1", None));
        assert!(!policy.has_permission("bob", Permission::Edit, "wf-2", None));
        assert!(!policy.has_permission("bob", Permission::Execute, "wf-1", None));
    }

    #[test]
    fn test_node_level_operator() {
        let policy = setup_policy();
        // Carol can approve on the specific node.
        assert!(policy.has_permission(
            "carol",
            Permission::Approve,
            "wf-1",
            Some("sensitive-node")
        ));
        // Carol cannot approve on other nodes.
        assert!(!policy.has_permission("carol", Permission::Approve, "wf-1", Some("other-node")));
        // Carol cannot approve at workflow level.
        assert!(!policy.has_permission("carol", Permission::Approve, "wf-1", None));
    }

    #[test]
    fn test_global_admin() {
        let policy = setup_policy();
        assert!(policy.has_permission("dave", Permission::View, "any-wf", None));
        assert!(policy.has_permission("dave", Permission::Edit, "any-wf", None));
        assert!(policy.has_permission("dave", Permission::Execute, "any-wf", None));
        assert!(policy.has_permission("dave", Permission::Delete, "any-wf", None));
        assert!(policy.has_permission("dave", Permission::Admin, "any-wf", None));
    }

    #[test]
    fn test_unknown_subject_has_no_permissions() {
        let policy = setup_policy();
        assert!(!policy.has_permission("unknown", Permission::View, "wf-1", None));
    }

    #[test]
    fn test_effective_permissions() {
        let policy = setup_policy();
        let perms = policy.effective_permissions("bob", "wf-1", None);
        assert!(perms.contains(&Permission::View));
        assert!(perms.contains(&Permission::Edit));
        assert!(!perms.contains(&Permission::Execute));
    }

    #[test]
    fn test_default_roles() {
        let policy = RbacPolicy::with_defaults();
        assert_eq!(policy.roles.len(), 4);
        assert!(policy.get_role("viewer").is_some());
        assert!(policy.get_role("editor").is_some());
        assert!(policy.get_role("operator").is_some());
        assert!(policy.get_role("admin").is_some());
    }

    #[test]
    fn test_policy_serde_roundtrip() {
        let policy = setup_policy();
        let json = serde_json::to_string(&policy).unwrap();
        let policy2: RbacPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy2.roles.len(), policy.roles.len());
        assert_eq!(policy2.bindings.len(), policy.bindings.len());
    }

    // ─── validate_update tests ─────────────────────────────────────────

    #[test]
    fn test_builtin_roles_are_marked() {
        let policy = RbacPolicy::with_defaults();
        for role in &policy.roles {
            assert!(role.builtin, "role '{}' should be marked builtin", role.id);
        }
    }

    #[test]
    fn test_validate_rejects_missing_builtin_role() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.retain(|r| r.id != "viewer");
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(
            err.to_string()
                .contains("builtin role 'viewer' must not be removed")
        );
    }

    #[test]
    fn test_validate_rejects_modified_builtin_permissions() {
        let mut policy = RbacPolicy::with_defaults();
        let editor = policy.roles.iter_mut().find(|r| r.id == "editor").unwrap();
        editor.permissions.push(Permission::Delete);
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(
            err.to_string()
                .contains("permissions for builtin role 'editor' cannot be modified")
        );
    }

    #[test]
    fn test_validate_rejects_duplicate_role_ids() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "custom-a".into(),
            name: "Custom A".into(),
            permissions: vec![Permission::View],
            description: None,
            builtin: false,
        });
        policy.roles.push(Role {
            id: "custom-a".into(),
            name: "Custom A Dup".into(),
            permissions: vec![Permission::View],
            description: None,
            builtin: false,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(err.to_string().contains("duplicate role ID 'custom-a'"));
    }

    #[test]
    fn test_validate_rejects_empty_permissions() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "empty-role".into(),
            name: "Empty".into(),
            permissions: vec![],
            description: None,
            builtin: false,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(
            err.to_string()
                .contains("must have at least one permission")
        );
    }

    #[test]
    fn test_validate_rejects_builtin_id_collision() {
        // A custom role cannot use a builtin role ID. This is caught because
        // the builtin role already occupies that ID, creating a duplicate.
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "admin".into(),
            name: "Fake Admin".into(),
            permissions: vec![Permission::View],
            description: None,
            builtin: false,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(err.to_string().contains("duplicate role ID 'admin'"));
    }

    #[test]
    fn test_validate_allows_custom_roles() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "ci-bot".into(),
            name: "CI Bot".into(),
            permissions: vec![Permission::View, Permission::Execute],
            description: Some("Automated CI runner".into()),
            builtin: false,
        });
        // Add a binding for the custom role so orphan check is exercised.
        policy.bindings.push(PolicyBinding {
            subject: "bot@ci".into(),
            role_id: "ci-bot".into(),
            scope: PolicyScope::Global,
        });
        RbacPolicy::validate_update(&policy).unwrap();
    }

    #[test]
    fn test_validate_rejects_orphaned_bindings() {
        let mut policy = RbacPolicy::with_defaults();
        policy.bindings.push(PolicyBinding {
            subject: "alice".into(),
            role_id: "nonexistent-role".into(),
            scope: PolicyScope::Global,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(
            err.to_string()
                .contains("binding references non-existent role 'nonexistent-role'")
        );
    }

    #[test]
    fn test_custom_role_permissions_work() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "deployer".into(),
            name: "Deployer".into(),
            permissions: vec![Permission::View, Permission::Execute],
            description: None,
            builtin: false,
        });
        policy
            .add_binding(PolicyBinding {
                subject: "deploy-bot".into(),
                role_id: "deployer".into(),
                scope: PolicyScope::Global,
            })
            .unwrap();
        assert!(policy.has_permission("deploy-bot", Permission::View, "wf-1", None));
        assert!(policy.has_permission("deploy-bot", Permission::Execute, "wf-1", None));
        assert!(!policy.has_permission("deploy-bot", Permission::Edit, "wf-1", None));
        assert!(!policy.has_permission("deploy-bot", Permission::Admin, "wf-1", None));
    }

    #[test]
    fn test_validate_rejects_builtin_with_wrong_flag() {
        let mut policy = RbacPolicy::with_defaults();
        policy
            .roles
            .iter_mut()
            .find(|r| r.id == "viewer")
            .unwrap()
            .builtin = false;
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(err.to_string().contains("builtin = true"));
    }

    #[test]
    fn test_validate_rejects_custom_role_claiming_builtin() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "deployer".into(),
            name: "Deployer".into(),
            permissions: vec![Permission::View, Permission::Execute],
            description: None,
            builtin: true,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(err.to_string().contains("must not have builtin = true"));
    }

    #[test]
    fn test_validate_rejects_custom_role_with_admin_permission() {
        let mut policy = RbacPolicy::with_defaults();
        policy.roles.push(Role {
            id: "super-editor".into(),
            name: "Super Editor".into(),
            permissions: vec![Permission::View, Permission::Edit, Permission::Admin],
            description: None,
            builtin: false,
        });
        let err = RbacPolicy::validate_update(&policy).unwrap_err();
        assert!(
            err.to_string()
                .contains("may not include the Admin permission")
        );
    }
}
