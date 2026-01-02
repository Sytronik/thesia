import {OverlayScrollbars, PartialOptions} from "overlayscrollbars";
import {RefObject, useEffect} from "react";

const DEFAULT_OPTIONS: PartialOptions = {
  scrollbars: {
    theme: "os-theme-custom",
    autoHide: "scroll",
  },
};

/**
 * Hook to apply OverlayScrollbars to a single element with optional onScroll handler
 * You don't need to add the class "overflow-y-auto" to the element.
 * If you want to apply OverlayScrollbars globally (w/o onScroll handler), use useOverlayScrollbars hook instead.
 */
export const useOverlayScrollbar = (ref: RefObject<HTMLElement | null>, onScroll?: () => void) => {
  useEffect(() => {
    if (!ref.current) return;

    const osInstance = OverlayScrollbars(ref.current, DEFAULT_OPTIONS);
    const viewport = osInstance.elements().viewport;

    if (onScroll) {
      viewport.addEventListener("scroll", onScroll);
    }

    return () => {
      if (onScroll) {
        viewport.removeEventListener("scroll", onScroll);
      }
      osInstance.destroy();
    };
  }, [ref, onScroll]);
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
