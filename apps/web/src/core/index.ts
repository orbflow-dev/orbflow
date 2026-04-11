// Public API for the Orbflow Workflow Builder
// This module can be embedded in any React project.

export { OrbflowWorkflowBuilderInner } from "./components/orbflow-workflow-builder";
export { NodeIcon } from "./components/icons";
export { OrbflowProvider, useOrbflow, type OrbflowConfig } from "./context/orbflow-provider";
export { NodeSchemaRegistry } from "./schemas/registry";
export { httpNodeSchema, delayNodeSchema, logNodeSchema, subWorkflowNodeSchema, transformNodeSchema, emailNodeSchema, templateNodeSchema, encodeNodeSchema, filterNodeSchema, sortNodeSchema, manualTriggerSchema, cronTriggerSchema, webhookTriggerSchema, eventTriggerSchema, postgresCapabilitySchema, builtinSchemas } from "@orbflow/core/schemas";
export type {
  FieldSchema,
  FieldType,
  NodeKind,
  ParameterMode,
  NodeTypeDefinition,
  ParameterValue,
  CapabilityPortDef,
  CapabilityEdge,
  Annotation,
  TriggerNodeConfig,
  NodeMetadata,
  FieldMapping,
  ConditionRule,
  ConditionGroup,
  CelOperator,
} from "./types/schema";
export { buildMappingExpression, buildConditionExpression, serializeMappings } from "./utils/cel-builder";
export type { OrbflowTheme } from "./styles/theme";
export { defaultDarkTheme } from "./styles/theme";
