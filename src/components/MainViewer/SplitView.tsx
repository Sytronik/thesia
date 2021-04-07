import React, { createRef, useRef, useEffect, useLayoutEffect, useState } from "react";
import "./SplitView.scss"

const MARGIN = 2;
const MIN_WIDTH = 160 + 32;
const MAX_WIDTH = 480 + 32;
interface SplitViewProps {
  left: React.ReactElement;
  right: React.ReactElement;
  setCanvasWidth: (value: number) => void;
  className?: string;
}

const LeftPane: React.FunctionComponent<{
  leftWidth: number | undefined;
  setLeftWidth: (value: number) => void;
}> = ({ children, leftWidth, setLeftWidth }) => {
  const leftRef = createRef<HTMLDivElement>();

  useEffect(() => {
    if (leftRef.current) {
      if (!leftWidth) {
        setLeftWidth(leftRef.current.clientWidth);
        return;
      }

      leftRef.current.style.width = `${leftWidth}px`;
    }
  }, [leftRef, leftWidth, setLeftWidth]);

  return <div className="LeftPane" ref={leftRef}>{children}</div>;
};

export const SplitView: React.FunctionComponent<SplitViewProps> = ({
  left,
  right,
  setCanvasWidth,
  className
}) => {
  const [leftWidth, setLeftWidth] = useState<undefined | number>(MIN_WIDTH);
  const [separatorXPosition, setSeparatorXPosition] = useState< undefined | number >(undefined);
  const [dragging, setDragging] = useState(false);

  const splitPaneRef = createRef<HTMLDivElement>();
  const rightPaneRef = useRef<HTMLDivElement>(null);

  const onMouseDown = (e: React.MouseEvent) => {
    setSeparatorXPosition(e.clientX);
    setDragging(true);
  };

  const onTouchStart = (e: React.TouchEvent) => {
    setSeparatorXPosition(e.touches[0].clientX);
    setDragging(true);
  };

  const onMove = (clientX: number) => {
    if (dragging && leftWidth && separatorXPosition) {
      const newLeftWidth = leftWidth + clientX - separatorXPosition;
      setSeparatorXPosition(clientX);

      if (newLeftWidth < MIN_WIDTH) {
        setLeftWidth(MIN_WIDTH);
        return;
      }

      if (splitPaneRef.current) {
        const splitPaneWidth = splitPaneRef.current.clientWidth;

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
  };

  const onMouseMove = (e: MouseEvent) => {
    e.preventDefault();
    onMove(e.clientX);
  };

  const onTouchMove = (e: TouchEvent) => {
    onMove(e.touches[0].clientX);
  };

  const onMouseUp = () => {
    setDragging(false);
  };

  const [resizeObserver, _] = useState(new ResizeObserver((entries: ResizeObserverEntry[]) => {
    const target = entries[0].target
      setCanvasWidth(target.clientWidth);
  }));

  React.useEffect(() => {
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("touchmove", onTouchMove);
    document.addEventListener("mouseup", onMouseUp);

    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("touchmove", onTouchMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
  });

  useLayoutEffect(() => {
    if(rightPaneRef.current) {
      resizeObserver.observe(rightPaneRef.current);
    }

    return (() => {
      resizeObserver.disconnect();
    });
  }, [rightPaneRef, resizeObserver]);

  return (
    <div className={`SplitView ${className ?? ""}`} ref={splitPaneRef}>
      <LeftPane leftWidth={leftWidth} setLeftWidth={setLeftWidth}>
        {left}
      </LeftPane>
      <div
        className="divider-hitbox"
        onMouseDown={onMouseDown}
        onTouchStart={onTouchStart}
        onTouchEnd={onMouseUp}
      >
        <div className="divider" />
      </div>
      <div className="rightPane" ref={rightPaneRef}>{right}</div>
    </div>
  );
};
