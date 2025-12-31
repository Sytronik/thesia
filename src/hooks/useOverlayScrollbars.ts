import {OverlayScrollbars} from "overlayscrollbars";
import {useEffect} from "react";

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
          OverlayScrollbars(htmlElement, {
            scrollbars: {
              theme: "os-theme-custom",
              autoHide: "scroll",
            },
          });
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
