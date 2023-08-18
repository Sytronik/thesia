import React, {useRef, useEffect, useState, useCallback} from "react";
import {AXIS_SPACE} from "renderer/prototypes/constants";
import styles from "./SplitView.scss";

const MARGIN = 2;
const MIN_WIDTH = 160 + 32;
const MAX_WIDTH = 480 + 32;

type SplitViewProps = {
  left: React.ReactElement;
  right: React.ReactElement;
  setCanvasWidth: (value: number) => void;
  className?: string;
};

type LeftPaneProps = {
  children: React.ReactElement;
  leftWidth: number | undefined;
  setLeftWidth: (value: number) => void;
};

const LeftPane = (props: LeftPaneProps) => {
  const {children, leftWidth, setLeftWidth} = props;
  const leftElem = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!leftElem.current) {
      return;
    }
    if (!leftWidth) {
      setLeftWidth(leftElem.current.clientWidth);
      return;
    }
    leftElem.current.style.width = `${leftWidth}px`;
  }, [leftElem, leftWidth, setLeftWidth]);

  return (
    <div className={styles.LeftPane} ref={leftElem}>
      {children}
    </div>
  );
};

const SplitView = (props: SplitViewProps) => {
  const {left, right, setCanvasWidth, className} = props;

  const [leftWidth, setLeftWidth] = useState<undefined | number>(MIN_WIDTH);
  const [separatorXPosition, setSeparatorXPosition] = useState<undefined | number>(undefined);
  const [dragging, setDragging] = useState(false);

  const splitPaneElem = useRef<HTMLDivElement>(null);
  const rightPaneElem = useRef<HTMLDivElement>(null);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    setSeparatorXPosition(e.clientX);
    setDragging(true);
  }, []);

  const onTouchStart = useCallback((e: React.TouchEvent) => {
    setSeparatorXPosition(e.touches[0].clientX);
    setDragging(true);
  }, []);

  const onMove = useCallback(
    (clientX: number) => {
      if (dragging && leftWidth && separatorXPosition) {
        const newLeftWidth = leftWidth + clientX - separatorXPosition;
        setSeparatorXPosition(clientX);

        if (newLeftWidth < MIN_WIDTH) {
          setLeftWidth(MIN_WIDTH);
          return;
        }

        if (splitPaneElem.current) {
          const splitPaneWidth = splitPaneElem.current.clientWidth;

          if (newLeftWidth > splitPaneWidth - MARGIN) {
            setLeftWidth(splitPaneWidth - MARGIN);
            return;
          }
        }

        if (newLeftWidth > MAX_WIDTH) {
          setLeftWidth(MAX_WIDTH);
          return;
        }

        setLeftWidth(newLeftWidth);
      }
    },
    [dragging, leftWidth, separatorXPosition],
  );

  const onMouseMove = useCallback(
    (e: MouseEvent) => {
      e.preventDefault();
      onMove(e.clientX);
    },
    [onMove],
  );

  const onTouchMove = useCallback(
    (e: TouchEvent) => {
      onMove(e.touches[0].clientX);
    },
    [onMove],
  );

  const onMouseUp = useCallback(() => {
    setDragging(false);
  }, []);

  const [resizeObserver, setResizeObserver] = useState(
    new ResizeObserver((entries: ResizeObserverEntry[]) => {
      const {target} = entries[0];
      if (target.clientWidth >= 1) {
        setCanvasWidth(target.clientWidth - AXIS_SPACE);
      }
    }),
  );

  useEffect(() => {
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("touchmove", onTouchMove);
    document.addEventListener("mouseup", onMouseUp);

    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("touchmove", onTouchMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
  });

  useEffect(() => {
    if (rightPaneElem.current) {
      resizeObserver.observe(rightPaneElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [rightPaneElem, resizeObserver]);

  return (
    <div className={`${styles.SplitView} ${className}`} ref={splitPaneElem}>
      <LeftPane leftWidth={leftWidth} setLeftWidth={setLeftWidth}>
        {left}
      </LeftPane>
      <div
        role="presentation"
        className={styles.divider}
        onMouseDown={onMouseDown}
        onTouchStart={onTouchStart}
        onTouchEnd={onMouseUp}
      >
        <div className={styles.dividerLine} />
      </div>
      <div className={styles.RightPane} ref={rightPaneElem}>
        {right}
      </div>
    </div>
  );
};

SplitView.defaultProps = {
  className: "",
};

export default SplitView;
