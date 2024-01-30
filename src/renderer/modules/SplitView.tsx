import React, {useRef, useEffect, useState, forwardRef, useImperativeHandle} from "react";
import useEvent from "react-use-event-hook";
import {AXIS_SPACE, TIME_CANVAS_HEIGHT, TINY_MARGIN} from "renderer/prototypes/constants";
import styles from "./SplitView.module.scss";

const MARGIN = 2;
const MIN_WIDTH = 160 + 32;
const MAX_WIDTH = 480 + 32;

type SplitViewProps = {
  createLeft: (leftWidth: number) => React.ReactElement;
  right: React.ReactElement;
  setCanvasWidth: (value: number) => void;
  className?: string;
};

const SplitView = forwardRef(({className = "", ...props}: SplitViewProps, ref) => {
  const {createLeft, right, setCanvasWidth} = props;

  const [leftWidth, setLeftWidth] = useState<number>(MIN_WIDTH);
  const [separatorXPosition, setSeparatorXPosition] = useState<number>(0);
  const [rightVisibility, setRightVisibility] = useState<boolean>(true);

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

  const [rightResizeObserver, _setRightResizeObserver] = useState(
    new ResizeObserver((entries: ResizeObserverEntry[]) => {
      const {target} = entries[0];
      if (target.clientWidth > AXIS_SPACE) {
        setCanvasWidth(target.clientWidth - AXIS_SPACE);
        setRightVisibility(true);
      } else {
        setCanvasWidth(AXIS_SPACE - TINY_MARGIN);
        setRightVisibility(false);
      }
    }),
  );

  const [resizeObserver, _setResizeObserver] = useState(
    new ResizeObserver((entries: ResizeObserverEntry[]) => {
      const {target} = entries[0];
      if ((rightPaneElem.current?.clientWidth ?? 0) === 0) {
        setNormalizedLeftWidth(target.clientWidth - MARGIN);
      }
    }),
  );

  const imperativeInstanceRef = useRef<SplitViewHandleElement>({
    getBoundingClientRect: () => splitPaneElem.current?.getBoundingClientRect() ?? null,
    scrollTo: (options: ScrollToOptions) => {
      if (options.top !== undefined) {
        options.top += TIME_CANVAS_HEIGHT;
      }
      splitPaneElem.current?.scrollTo(options);
    },
    scrollTop: () => splitPaneElem.current?.scrollTop ?? 0,
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
    <div className={`${styles.SplitView} ${className}`} ref={splitPaneElem}>
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
        {rightVisibility ? right : null}
      </div>
    </div>
  );
});

SplitView.displayName = "SplitView";

export default SplitView;
