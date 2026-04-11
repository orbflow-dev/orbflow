import { useMemo } from "react";
import { STATUS_BADGE } from "../../execution/constants";

export interface StatusBadgeData {
  icon: string;
  spinning: boolean;
  cssModifier: string;
}

/** Resolves a node execution status to badge display data. Returns null for unknown/missing status. */
export function useStatusBadge(status: string | undefined): StatusBadgeData | null {
  return useMemo(() => {
    if (!status) return null;
    const badge = STATUS_BADGE[status];
    if (!badge) return null;
    return {
      icon: badge.icon,
      spinning: badge.spin === true,
      cssModifier: badge.cssModifier,
    };
  }, [status]);
}

export interface StatusBadgeProps {
  status: string | undefined;
  children: (data: StatusBadgeData) => React.ReactNode;
}

/** Headless status badge — renders nothing when status has no badge config. */
export function StatusBadge({ status, children }: StatusBadgeProps): React.ReactNode {
  const data = useStatusBadge(status);
  if (!data) return null;
  return children(data);
}
