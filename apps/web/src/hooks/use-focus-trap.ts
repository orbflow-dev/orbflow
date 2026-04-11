import { useEffect, useRef, type RefObject } from "react";

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface FocusTrapOptions {
  /** When true, restores focus to the previously active element on unmount. Defaults to true. */
  restoreFocus?: boolean;
}

/**
 * Traps focus within a container element while it's mounted.
 * Press Tab / Shift+Tab to cycle through focusable children.
 * Optionally restores focus to the previously focused element on unmount.
 */
export function useFocusTrap(ref: RefObject<HTMLElement | null>, options: FocusTrapOptions = {}) {
  const { restoreFocus = true } = options;
  const previouslyFocused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (restoreFocus) {
      previouslyFocused.current = document.activeElement as HTMLElement | null;
    }

    const container = ref.current;
    if (!container) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;

      const focusable = Array.from(
        container.querySelectorAll<HTMLElement>(FOCUSABLE)
      ).filter((el) => el.offsetParent !== null); // visible only

      if (focusable.length === 0) {
        e.preventDefault();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    container.addEventListener("keydown", handler);
    return () => {
      container.removeEventListener("keydown", handler);
      if (restoreFocus && previouslyFocused.current && typeof previouslyFocused.current.focus === "function") {
        previouslyFocused.current.focus();
      }
    };
  }, [ref, restoreFocus]);
}
