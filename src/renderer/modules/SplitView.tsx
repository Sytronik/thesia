import React, {useRef, useEffect, useState, forwardRef, useImperativeHandle, useMemo} from "react";
import useEvent from "react-use-event-hook";
import {AXIS_SPACE, TIME_CANVAS_HEIGHT} from "renderer/prototypes/constants/tracks";
import {NativeTypes} from "react-dnd-html5-backend";
import {DropTargetMonitor, useDrop} from "react-dnd";
import styles from "./SplitView.module.scss";

const MARGIN = 2;
const MIN_WIDTH = 140 + 32;
const INIT_WIDTH = 230 + 32;
const MAX_WIDTH = 480 + 32;

type SplitViewProps = {
  createLeft: (leftWidth: number) => React.ReactElement;
  right: React.ReactElement;
  setCanvasWidth: (value: number) => void;
  className?: string;
  onFileHover?: (item: any, monitor: DropTargetMonitor) => void;
  onFileHoverLeave?: () => void;
  onFileDrop?: (item: any) => void;
  onVerticalViewportChange?: () => void;
};

const SplitView = forwardRef(
  (
    {
      className = "",
      onFileHover = () => {},
      onFileHoverLeave = () => {},
      onFileDrop = () => {},
      onVerticalViewportChange = () => {},
      ...props
    }: SplitViewProps,
    ref,
  ) => {
    const {createLeft, right, setCanvasWidth} = props;

    const [leftWidth, setLeftWidth] = useState<number>(INIT_WIDTH);
    const [separatorXPosition, setSeparatorXPosition] = useState<number>(0);

    const splitPaneElem = useRef<HTMLDivElement>(null);
    const rightPaneElem = useRef<HTMLDivElement>(null);

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
      document.addEventListener("mouseup", onMouseUp, {once: true});
      document.addEventListener("touchend", onMouseUp, {once: true});
    };

    const onTouchStart = (e: React.TouchEvent) => {
      setSeparatorXPosition(e.touches[0].clientX - leftWidth);
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("touchmove", onTouchMove);
      document.addEventListener("mouseup", onMouseUp, {once: true});
      document.addEventListener("touchend", onMouseUp, {once: true});
    };

    const rightResizeObserver = useMemo(
      () =>
        new ResizeObserver((entries: ResizeObserverEntry[]) => {
          const {target} = entries[0];
          if (target.clientWidth > AXIS_SPACE) {
            setCanvasWidth(target.clientWidth - AXIS_SPACE);
          } else {
            setCanvasWidth(0);
          }
        }),
      [setCanvasWidth],
    );

    const heightRef = useRef(0);
    const resizeObserver = useMemo(
      () =>
        new ResizeObserver((entries: ResizeObserverEntry[]) => {
          const {target} = entries[0];
          if ((rightPaneElem.current?.clientWidth ?? 0) === 0) {
            setNormalizedLeftWidth(target.clientWidth - MARGIN);
          }
          if (heightRef.current !== target.clientHeight) {
            heightRef.current = target.clientHeight;
            onVerticalViewportChange();
          }
        }),
      [onVerticalViewportChange, setNormalizedLeftWidth],
    );

    const [{isOver}, drop] = useDrop({
      accept: NativeTypes.FILE,
      drop(item) {
        onFileDrop(item);
      },
      hover(item: any, monitor) {
        onFileHover(item, monitor);
      },
      collect: (monitor) => ({
        isOver: monitor.isOver(),
      }),
    });

    useEffect(() => {
      drop(splitPaneElem);
    }, [drop]);

    useEffect(() => {
      if (!isOver) {
        onFileHoverLeave();
      }
    }, [isOver, onFileHoverLeave]);

    const imperativeInstanceRef = useRef<SplitViewHandleElement>({
      getBoundingClientRect: () => splitPaneElem.current?.getBoundingClientRect() ?? null,
      scrollTo: (options: ScrollToOptions) => {
        if (options.top !== undefined) {
          options.top += TIME_CANVAS_HEIGHT;
        }
        if (options.top !== splitPaneElem.current?.scrollTop) {
          splitPaneElem.current?.scrollTo(options);
        }
      },
      scrollTop: () => splitPaneElem.current?.scrollTop ?? 0,
      hasScrollBar: () =>
        (splitPaneElem.current?.scrollHeight ?? 0) > (splitPaneElem.current?.clientHeight ?? 0),
    });
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    useEffect(() => {
      if (rightPaneElem.current) {
        rightResizeObserver.observe(rightPaneElem.current);
      }

      return () => {
        rightResizeObserver.disconnect();
      };
    }, [rightResizeObserver]);

    useEffect(() => {
      if (splitPaneElem.current) {
        resizeObserver.observe(splitPaneElem.current);
      }

      return () => {
        resizeObserver.disconnect();
      };
    }, [resizeObserver]);

    return (
      <div
        className={`${styles.SplitView} ${className}`}
        ref={splitPaneElem}
        onScroll={onVerticalViewportChange}
      >
        <div className={styles.Scrolled}>
          <div className={styles.LeftPane} style={{width: leftWidth}}>
            {createLeft(leftWidth)}
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
    );
  },
);

SplitView.displayName = "SplitView";

export default SplitView;
