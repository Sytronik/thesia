import React, {
  useRef,
  useEffect,
  useState,
  forwardRef,
  useImperativeHandle,
  ReactNode,
  useMemo,
} from "react";
import useEvent from "react-use-event-hook";
import { useOverlayScrollbar } from "src/hooks/useOverlayScrollbars";
import { AXIS_SPACE } from "src/prototypes/constants/tracks";
import styles from "./SplitView.module.scss";

const MARGIN = 2;
const MIN_WIDTH = 140 + 32;
const INIT_WIDTH = 230 + 32;
const MAX_WIDTH = 480 + 32;

type SplitViewProps = {
  left: ReactNode;
  right: ReactNode;
  setCanvasWidth: (value: number) => void;
  className?: string;
  onVerticalViewportChange?: () => void;
  onVerticalViewportResize?: () => void;
};

const SplitView = forwardRef(
  (
    {
      className = "",
      onVerticalViewportChange,
      onVerticalViewportResize = () => {},
      ...props
    }: SplitViewProps,
    ref,
  ) => {
    const { left, right, setCanvasWidth } = props;

    const [leftWidth, setLeftWidth] = useState<number>(INIT_WIDTH);
    const [separatorXPosition, setSeparatorXPosition] = useState<number>(0);

    const splitPaneElem = useRef<HTMLDivElement>(null);
    const scrollBoxElem = useRef<HTMLDivElement>(null);
    const scrolledElem = useRef<HTMLDivElement>(null);
    const rightPaneElem = useRef<HTMLDivElement>(null);
    const scrollbarElements = useMemo(
      () => ({
        viewport: scrollBoxElem,
        content: scrolledElem,
        scrollbarSlot: splitPaneElem,
      }),
      [],
    );
    const { viewportRef: scrollViewportElem, update: updateScrollbar } = useOverlayScrollbar(
      splitPaneElem,
      onVerticalViewportChange,
      scrollbarElements,
    );

    const setNormalizedLeftWidth = useEvent((value: number) => {
      let newLeftWidth = Math.max(value, MIN_WIDTH);
      if (splitPaneElem.current && newLeftWidth >= splitPaneElem.current.clientWidth - MARGIN) {
        setLeftWidth(splitPaneElem.current.clientWidth - MARGIN);
        return;
      }
      newLeftWidth = Math.min(
        newLeftWidth,
        MAX_WIDTH,
        (splitPaneElem.current?.clientWidth ?? 0) * 0.7,
      );

      setLeftWidth(newLeftWidth);
    });

    const onMove = useEvent((e: MouseEvent | TouchEvent, clientX: number) => {
      e.preventDefault();
      setNormalizedLeftWidth(clientX - separatorXPosition);
    });

    const onMouseMove = useEvent((e: MouseEvent) => {
      onMove(e, e.clientX);
    });

    const onTouchMove = useEvent((e: TouchEvent) => {
      onMove(e, e.touches[0].clientX);
    });

    const onMouseUp = useEvent(() => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("touchmove", onTouchMove);
    });

    const onMouseDown = (e: React.MouseEvent) => {
      setSeparatorXPosition(e.clientX - leftWidth);
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("touchmove", onTouchMove);
      document.addEventListener("mouseup", onMouseUp, { once: true });
      document.addEventListener("touchend", onMouseUp, { once: true });
    };

    const onTouchStart = (e: React.TouchEvent) => {
      setSeparatorXPosition(e.touches[0].clientX - leftWidth);
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("touchmove", onTouchMove);
      document.addEventListener("mouseup", onMouseUp, { once: true });
      document.addEventListener("touchend", onMouseUp, { once: true });
    };

    const heightRef = useRef(0);

    const imperativeInstanceRef = useRef<SplitViewHandleElement>({
      getBoundingClientRect: () => splitPaneElem.current?.getBoundingClientRect() ?? null,
      scrollTo: (options: ScrollToOptions) => {
        const viewport = scrollViewportElem.current;
        if (!viewport) return;
        if (options.top !== undefined) {
          if (options.top !== viewport.scrollTop) viewport.scrollTo({ top: options.top });
          return;
        }
        if (options.left !== undefined || options.behavior !== undefined) {
          viewport.scrollTo(options);
        }
      },
      scrollTop: () => scrollViewportElem.current?.scrollTop ?? 0,
    });
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    useEffect(() => {
      const rightPane = rightPaneElem.current;
      if (!rightPane) return;
      const rightResizeObserver = new ResizeObserver((entries: ResizeObserverEntry[]) => {
        const { target } = entries[0];
        if (target.clientWidth > AXIS_SPACE) {
          setCanvasWidth(target.clientWidth - AXIS_SPACE);
        } else {
          setCanvasWidth(0);
        }
      });
      rightResizeObserver.observe(rightPane);

      return () => {
        rightResizeObserver.disconnect();
      };
    }, [setCanvasWidth]);

    useEffect(() => {
      const splitPane = splitPaneElem.current;
      if (!splitPane) return;
      const resizeObserver = new ResizeObserver((entries: ResizeObserverEntry[]) => {
        const { target } = entries[0];
        if ((rightPaneElem.current?.clientWidth ?? 0) === 0) {
          setNormalizedLeftWidth(target.clientWidth - MARGIN);
        }
        updateScrollbar(true);
        if (heightRef.current !== target.clientHeight) {
          heightRef.current = target.clientHeight;
          onVerticalViewportResize();
        }
      });
      resizeObserver.observe(splitPane);

      return () => {
        resizeObserver.disconnect();
      };
    }, [onVerticalViewportResize, setNormalizedLeftWidth, updateScrollbar]);

    useEffect(() => {
      const scrolled = scrolledElem.current;
      if (!scrolled) return;
      const resizeObserver = new ResizeObserver(() => {
        updateScrollbar(true);
      });
      resizeObserver.observe(scrolled);

      return () => {
        resizeObserver.disconnect();
      };
    }, [updateScrollbar]);

    return (
      <div className={`${styles.SplitView} ${className}`} ref={splitPaneElem}>
        <div className={styles.Scrollbox} ref={scrollBoxElem}>
          <div className={styles.Scrolled} ref={scrolledElem}>
            <div className={styles.LeftPane} style={{ width: leftWidth }}>
              {left}
            </div>
            <div
              role="presentation"
              className={styles.divider}
              onMouseDown={onMouseDown}
              onTouchStart={onTouchStart}
            >
              <div className={styles.dividerLine} />
            </div>
            <div className={styles.RightPane} ref={rightPaneElem}>
              {right}
            </div>
          </div>
        </div>
      </div>
    );
  },
);

SplitView.displayName = "SplitView";

export default SplitView;
