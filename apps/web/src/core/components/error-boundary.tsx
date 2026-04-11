"use client";

import { Component, Fragment, type ReactNode, type ErrorInfo } from "react";
import { Button } from "./button";
import { NodeIcon } from "./icons";

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Label shown in the fallback UI to identify which section crashed */
  section?: string;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  retryKey: number;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, error: null, retryKey: 0 };

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(
      `[ErrorBoundary${this.props.section ? `: ${this.props.section}` : ""}]`,
      error,
      info.componentStack
    );
  }

  private handleRetry = () => {
    this.setState((s) => ({ hasError: false, error: null, retryKey: s.retryKey + 1 }));
  };

  render() {
    if (this.state.hasError) {
      return (
        <ErrorFallback
          error={this.state.error}
          section={this.props.section}
          onRetry={this.handleRetry}
        />
      );
    }
    return <Fragment key={this.state.retryKey}>{this.props.children}</Fragment>;
  }
}

function ErrorFallback({
  error,
  section,
  onRetry,
}: {
  error: Error | null;
  section?: string;
  onRetry: () => void;
}) {
  return (
    <div className="flex items-center justify-center h-full w-full p-8">
      <div className="max-w-sm text-center space-y-4">
        <div className="w-12 h-12 rounded-2xl bg-rose-500/10 flex items-center justify-center mx-auto">
          <NodeIcon name="alert-triangle" className="w-6 h-6 text-rose-400" />
        </div>

        <div>
          <h3 className="text-sm font-semibold text-orbflow-text-secondary">
            Something went wrong
          </h3>
          {section && (
            <p className="text-body text-orbflow-text-faint mt-1">
              in {section}
            </p>
          )}
        </div>

        {error?.message && (
          <p className="text-body font-mono text-rose-400/70 bg-rose-500/5 border border-rose-500/10 rounded-lg px-3 py-2 break-words">
            {error.message}
          </p>
        )}

        <Button variant="primary" onClick={onRetry}>
          Try again
        </Button>
      </div>
    </div>
  );
}
