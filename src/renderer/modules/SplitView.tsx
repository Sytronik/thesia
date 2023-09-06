import React, {useRef, useEffect, useState, forwardRef, useImperativeHandle} from "react";
import useEvent from "react-use-event-hook";
import {AXIS_SPACE} from "renderer/prototypes/constants";
import styles from "./SplitView.scss";

const MARGIN = 2;
const MIN_WIDTH = 160 + 32;
const MAX_WIDTH = 480 + 32;

type SplitViewProps = {
  createLeft: (leftWidth: number) => React.ReactElement;
  right: React.ReactElement;
  setCanvasWidth: (value: number) => void;
  className?: string;
};

const SplitView = forwardRef((props: SplitViewProps, ref) => {
  const {createLeft, right, setCanvasWidth, className} = props;

  const [leftWidth, setLeftWidth] = useState<number>(MIN_WIDTH);
  const [separatorXPosition, setSeparatorXPosition] = useState<undefined | number>(undefined);
  const [dragging, setDragging] = useState(false);

  const splitPaneElem = useRef<HTMLDivElement>(null);
  const rightPaneElem = useRef<HTMLDivElement>(null);

  const onMouseDown = (e: React.MouseEvent) => {
    setSeparatorXPosition(e.clientX - leftWidth);
    setDragging(true);
  };

  const onTouchStart = (e: React.TouchEvent) => {
    setSeparatorXPosition(e.touches[0].clientX - leftWidth);
    setDragging(true);
  };

  const onMove = useEvent((clientX: number) => {
    if (dragging && separatorXPosition) {
      let newLeftWidth = Math.max(clientX - separatorXPosition, MIN_WIDTH);
      if (splitPaneElem.current && newLeftWidth > splitPaneElem.current.clientWidth - MARGIN) {
        setLeftWidth(splitPaneElem.current.clientWidth - MARGIN);
        return;
      }
      newLeftWidth = Math.min(newLeftWidth, MAX_WIDTH);

      setLeftWidth(newLeftWidth);
    }
  });

  const onMouseMove = useEvent((e: MouseEvent) => {
    e.preventDefault();
    onMove(e.clientX);
  });

  const onTouchMove = useEvent((e: TouchEvent) => {
    onMove(e.touches[0].clientX);
  });

  const onMouseUp = useEvent(() => {
    setDragging(false);
  });

  const [resizeObserver, setResizeObserver] = useState(
    new ResizeObserver((entries: ResizeObserverEntry[]) => {
      const {target} = entries[0];
      if (target.clientWidth >= 1) {
        setCanvasWidth(target.clientWidth - AXIS_SPACE);
      }
    }),
  );

  const imperativeInstanceRef = useRef<SplitViewHandleElement>({
    getBoundingClientY: () => splitPaneElem.current?.getBoundingClientRect().y ?? 0,
    scrollTo: (options: ScrollToOptions) => splitPaneElem.current?.scrollTo(options),
  });
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  useEffect(() => {
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("touchmove", onTouchMove);
    document.addEventListener("mouseup", onMouseUp);
    document.addEventListener("touchend", onMouseUp);

    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("touchmove", onTouchMove);
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("touchend", onMouseUp);
    };
  }, [onMouseMove, onMouseUp, onTouchMove]);

  useEffect(() => {
    if (rightPaneElem.current) {
      resizeObserver.observe(rightPaneElem.current);
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
        {right}
      </div>
    </div>
  );
});

SplitView.displayName = "SplitView";
SplitView.defaultProps = {
  className: "",
};

export default SplitView;
