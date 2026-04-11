import { create } from "zustand";
import type { FieldMapping, ConditionGroup, ParameterValue } from "../types/schema";

interface PanelStore {
  panelOpen: boolean;
  panelMode: "node" | "edge" | null;

  // Node input mappings: nodeId -> { fieldKey -> FieldMapping }
  inputMappings: Record<string, Record<string, FieldMapping>>;
  // Node parameter values: nodeId -> { paramKey -> ParameterValue }
  parameterValues: Record<string, Record<string, ParameterValue>>;
  // Edge conditions: edgeId -> ConditionGroup
  edgeConditions: Record<string, ConditionGroup>;

  openNodePanel: (nodeId: string) => void;
  openEdgePanel: (edgeId: string) => void;
  closePanel: () => void;

  setInputMapping: (
    nodeId: string,
    fieldKey: string,
    mapping: FieldMapping
  ) => void;
  getNodeMappings: (nodeId: string) => Record<string, FieldMapping>;

  setParameterValue: (
    nodeId: string,
    paramKey: string,
    value: ParameterValue
  ) => void;
  getNodeParameters: (nodeId: string) => Record<string, ParameterValue>;

  setEdgeCondition: (edgeId: string, condition: ConditionGroup) => void;
  removeEdgeCondition: (edgeId: string) => void;
  getEdgeCondition: (edgeId: string) => ConditionGroup | undefined;

  // Load existing mappings when opening a node that already has data
  loadNodeMappings: (
    nodeId: string,
    mappings: Record<string, FieldMapping>
  ) => void;

  // Load existing parameter values
  loadNodeParameters: (
    nodeId: string,
    params: Record<string, ParameterValue>
  ) => void;

  // Reset all state (used when switching workflows)
  clearAll: () => void;
}

export const usePanelStore = create<PanelStore>((set, get) => ({
  panelOpen: false,
  panelMode: null,
  inputMappings: {},
  parameterValues: {},
  edgeConditions: {},

  openNodePanel: () => set({ panelOpen: true, panelMode: "node" }),
  openEdgePanel: () => set({ panelOpen: true, panelMode: "edge" }),
  closePanel: () => set({ panelOpen: false, panelMode: null }),

  setInputMapping: (nodeId, fieldKey, mapping) =>
    set((s) => ({
      inputMappings: {
        ...s.inputMappings,
        [nodeId]: {
          ...s.inputMappings[nodeId],
          [fieldKey]: mapping,
        },
      },
    })),

  getNodeMappings: (nodeId) => get().inputMappings[nodeId] || {},

  setParameterValue: (nodeId, paramKey, value) =>
    set((s) => ({
      parameterValues: {
        ...s.parameterValues,
        [nodeId]: {
          ...s.parameterValues[nodeId],
          [paramKey]: value,
        },
      },
    })),

  getNodeParameters: (nodeId) => get().parameterValues[nodeId] || {},

  setEdgeCondition: (edgeId, condition) =>
    set((s) => ({
      edgeConditions: { ...s.edgeConditions, [edgeId]: condition },
    })),

  removeEdgeCondition: (edgeId) =>
    set((s) => {
      const { [edgeId]: _, ...rest } = s.edgeConditions;
      return { edgeConditions: rest };
    }),

  getEdgeCondition: (edgeId) => get().edgeConditions[edgeId],

  loadNodeMappings: (nodeId, mappings) =>
    set((s) => ({
      inputMappings: { ...s.inputMappings, [nodeId]: mappings },
    })),

  loadNodeParameters: (nodeId, params) =>
    set((s) => ({
      parameterValues: { ...s.parameterValues, [nodeId]: params },
    })),

  clearAll: () =>
    set({
      panelOpen: false,
      panelMode: null,
      inputMappings: {},
      parameterValues: {},
      edgeConditions: {},
    }),
}));
