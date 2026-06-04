import {
  type InitializationTarget,
  OverlayScrollbars,
  type OverlayScrollbars as OverlayScrollbarsInstance,
  type PartialOptions,
} from "overlayscrollbars";
import { RefObject, useCallback, useEffect, useRef } from "react";

const DEFAULT_OPTIONS: PartialOptions = {
  scrollbars: {
    theme: "os-theme-custom",
    autoHide: "scroll",
  },
};

type OverlayScrollbarElements = {
  viewport?: RefObject<HTMLElement | null>;
  content?: RefObject<HTMLElement | null>;
  scrollbarSlot?: RefObject<HTMLElement | null>;
};

/**
 * Hook to apply OverlayScrollbars to a single element with optional onScroll handler
 * You don't need to add the class "overflow-y-auto" to the element.
 * If you want to apply OverlayScrollbars globally (w/o onScroll handler), use useOverlayScrollbars hook instead.
 */
export const useOverlayScrollbar = (
  ref: RefObject<HTMLElement | null>,
  onScroll?: () => void,
  elements?: OverlayScrollbarElements,
) => {
  const viewportRef = useRef<HTMLElement | null>(null);
  const instanceRef = useRef<OverlayScrollbarsInstance | null>(null);
  const updateRequestRef = useRef<number | null>(null);

  const update = useCallback((force = false) => {
    if (updateRequestRef.current !== null) {
      cancelAnimationFrame(updateRequestRef.current);
    }
    updateRequestRef.current = requestAnimationFrame(() => {
      updateRequestRef.current = null;
      instanceRef.current?.update(force);
    });
  }, []);

  useEffect(() => {
    const target = ref.current;
    if (!target) return;

    const viewportElement = elements?.viewport?.current;
    const contentElement = elements?.content?.current;
    const scrollbarSlotElement = elements?.scrollbarSlot?.current;
    if (
      (elements?.viewport && !viewportElement) ||
      (elements?.content && !contentElement) ||
      (elements?.scrollbarSlot && !scrollbarSlotElement)
    ) {
      return;
    }

    const initializationTarget: InitializationTarget = elements
      ? {
          target,
          elements: {
            viewport: viewportElement,
            content: contentElement,
          },
          scrollbars: {
            slot: scrollbarSlotElement,
          },
        }
      : target;

    const osInstance = OverlayScrollbars(initializationTarget, DEFAULT_OPTIONS);
    const viewport = osInstance.elements().viewport;
    viewportRef.current = viewport;
    instanceRef.current = osInstance;
    update(true);

    if (onScroll) {
      viewport.addEventListener("scroll", onScroll);
    }

    return () => {
      if (updateRequestRef.current !== null) {
        cancelAnimationFrame(updateRequestRef.current);
        updateRequestRef.current = null;
      }
      if (onScroll) {
        viewport.removeEventListener("scroll", onScroll);
      }
      viewportRef.current = null;
      instanceRef.current = null;
      osInstance.destroy();
    };
  }, [elements, ref, onScroll, update]);
  return { viewportRef, update };
};

/**
 * Hook to automatically apply OverlayScrollbars to all scrollable elements in the document
 * To make an element scrollable, add the class "overflow-y-auto" to the element
 * You can't use onScroll event handler with this hook.
 */
export const useOverlayScrollbars = () => {
  useEffect(() => {
    // Initialize OverlayScrollbars on all scrollable elements
    const initScrollbars = () => {
      // Target the main scrollable areas
      const scrollableElements = [
        document.body,
        document.querySelector('[data-scrollable="true"]'),
        ...document.querySelectorAll(".overflow-y-auto"),
        ...document.querySelectorAll(".overflow-auto"),
      ].filter(Boolean) as Element[];

      scrollableElements.forEach((element) => {
        const htmlElement = element as HTMLElement;
        if (htmlElement && !htmlElement.hasAttribute("data-overlayscrollbars-initialize")) {
          OverlayScrollbars(htmlElement, DEFAULT_OPTIONS);
          htmlElement.setAttribute("data-overlayscrollbars-initialize", "true");
        }
      });
    };

    // Initialize immediately
    initScrollbars();

    // Re-initialize when DOM changes (for dynamic content)
    const observer = new MutationObserver(() => {
      setTimeout(initScrollbars, 100);
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      observer.disconnect();
      // Clean up all OverlayScrollbars instances
      document.querySelectorAll("[data-overlayscrollbars-initialize]").forEach((element) => {
        const instance = OverlayScrollbars(element as HTMLElement);
        if (instance) {
          instance.destroy();
        }
      });
    };
  }, []);
};
