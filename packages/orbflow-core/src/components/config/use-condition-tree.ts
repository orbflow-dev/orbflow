"use client";

import { useMemo, useCallback } from "react";
import { usePanelStore } from "../../stores/panel-store";
import { buildConditionExpression } from "../../utils/cel-builder";
import type { ConditionGroup, ConditionRule } from "../../types/schema";

export interface ConditionTreeState {
  condition: ConditionGroup | undefined;
  hasCondition: boolean;
  celPreview: string;
  addRule: () => void;
  addGroup: () => void;
  updateRule: (index: number, updated: ConditionRule | ConditionGroup) => void;
  deleteRule: (index: number) => void;
  toggleLogic: () => void;
  toggleCondition: () => void;
  setCondition: (group: ConditionGroup) => void;
}

/**
 * Manages the condition tree state for a given edge.
 * Wraps panel-store edge conditions with CRUD operations.
 */
export function useConditionTree(edgeId: string): ConditionTreeState {
  const getEdgeCondition = usePanelStore((s) => s.getEdgeCondition);
  const setEdgeCondition = usePanelStore((s) => s.setEdgeCondition);
  const removeEdgeCondition = usePanelStore((s) => s.removeEdgeCondition);

  const condition = useMemo(
    () => getEdgeCondition(edgeId),
    [getEdgeCondition, edgeId]
  );

  const hasCondition = condition !== undefined;

  const celPreview = useMemo(() => {
    if (!condition || condition.rules.length === 0) return "";
    return buildConditionExpression(condition);
  }, [condition]);

  const addRule = useCallback(() => {
    if (!condition) return;
    const rule: ConditionRule = {
      id: `rule_${Date.now()}`,
      field: "",
      operator: "==",
      value: "",
    };
    const updated: ConditionGroup = {
      ...condition,
      rules: [...condition.rules, rule],
    };
    setEdgeCondition(edgeId, updated);
  }, [condition, edgeId, setEdgeCondition]);

  const addGroup = useCallback(() => {
    if (!condition) return;
    const nested: ConditionGroup = {
      id: `group_${Date.now()}`,
      logic: "and",
      rules: [],
    };
    const updated: ConditionGroup = {
      ...condition,
      rules: [...condition.rules, nested],
    };
    setEdgeCondition(edgeId, updated);
  }, [condition, edgeId, setEdgeCondition]);

  const updateRule = useCallback(
    (index: number, updatedRule: ConditionRule | ConditionGroup) => {
      if (!condition) return;
      const newRules = condition.rules.map((r, i) =>
        i === index ? updatedRule : r
      );
      const updated: ConditionGroup = { ...condition, rules: newRules };
      setEdgeCondition(edgeId, updated);
    },
    [condition, edgeId, setEdgeCondition]
  );

  const deleteRule = useCallback(
    (index: number) => {
      if (!condition) return;
      const updated: ConditionGroup = {
        ...condition,
        rules: condition.rules.filter((_, i) => i !== index),
      };
      setEdgeCondition(edgeId, updated);
    },
    [condition, edgeId, setEdgeCondition]
  );

  const toggleLogic = useCallback(() => {
    if (!condition) return;
    const updated: ConditionGroup = {
      ...condition,
      logic: condition.logic === "and" ? "or" : "and",
    };
    setEdgeCondition(edgeId, updated);
  }, [condition, edgeId, setEdgeCondition]);

  const toggleCondition = useCallback(() => {
    if (hasCondition) {
      removeEdgeCondition(edgeId);
    } else {
      setEdgeCondition(edgeId, {
        id: edgeId,
        logic: "and",
        rules: [],
      });
    }
  }, [edgeId, hasCondition, setEdgeCondition, removeEdgeCondition]);

  const setCondition = useCallback(
    (group: ConditionGroup) => {
      setEdgeCondition(edgeId, group);
    },
    [edgeId, setEdgeCondition]
  );

  return {
    condition,
    hasCondition,
    celPreview,
    addRule,
    addGroup,
    updateRule,
    deleteRule,
    toggleLogic,
    toggleCondition,
    setCondition,
  };
}
