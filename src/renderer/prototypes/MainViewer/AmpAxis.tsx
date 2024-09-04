import React, {RefObject, forwardRef, useCallback, useMemo, useRef, useState} from "react";
import AxisCanvas, {getAxisHeight} from "renderer/modules/AxisCanvas";
import styles from "renderer/modules/AxisCanvas.module.scss";
import Draggable, {CursorStateInfo} from "renderer/modules/Draggable";
import useEvent from "react-use-event-hook";
import FloatingUserInput from "renderer/modules/FloatingUserInput";
import {
  AMP_CANVAS_WIDTH,
  AMP_MARKER_POS,
  DEFAULT_AMP_RANGE,
  MIN_ABS_AMP_RANGE,
  MAX_ABS_AMP_RANGE,
  VERTICAL_AXIS_PADDING,
  MIN_DIST_FROM_0_FOR_DRAG,
} from "../constants/tracks";

type AmpAxisProps = {
  height: number;
  ampRangeRef: RefObject<[number, number]>;
  setAmpRange: (newRange: [number, number]) => void;
  resetAmpRange: () => void;
  enableInteraction: boolean;
};

type AmpAxisCursorState = "positive" | "negative";
const determineCursorStates = (cursorPos: number, rect: DOMRect) => {
  if (cursorPos < rect.height / 2) return "positive";
  return "negative";
};

type AmpAxisDragAnchor = {
  cursorRatio: number;
  ampRange: [number, number];
};
const DEFAULT_DRAG_ANCHOR: AmpAxisDragAnchor = {cursorRatio: 0.5, ampRange: DEFAULT_AMP_RANGE};

const calcIntervalZeroRatio = (ampRange: [number, number]) => {
  const interval = ampRange[1] - ampRange[0];
  const zeroRatio = ampRange[1] / interval;
  return [interval, zeroRatio];
};
const clampAmpRange = (ampRange: [number, number]) => {
  return [
    Math.min(Math.max(ampRange[0], -MAX_ABS_AMP_RANGE), -MIN_ABS_AMP_RANGE),
    Math.min(Math.max(ampRange[1], MIN_ABS_AMP_RANGE), MAX_ABS_AMP_RANGE),
  ] as [number, number];
};

const AmpAxis = forwardRef((props: AmpAxisProps, ref) => {
  const {height, ampRangeRef, setAmpRange, resetAmpRange, enableInteraction} = props;
  const wrapperDivElem = useRef<HTMLDivElement | null>(null);
  const [floatingInputHidden, setFloatingInputHidden] = useState<boolean>(true);

  const calcLimitedCursorRatio = (
    cursorState: AmpAxisCursorState,
    cursorPos: number,
    rect: DOMRect,
  ) => {
    const cursorRatio = cursorPos / getAxisHeight(rect);
    const [_interval, zeroRatio] = calcIntervalZeroRatio(ampRangeRef.current ?? DEFAULT_AMP_RANGE);
    if (cursorState === "positive") {
      return Math.min(cursorRatio, zeroRatio - MIN_DIST_FROM_0_FOR_DRAG);
    }
    return Math.max(cursorRatio, zeroRatio + MIN_DIST_FROM_0_FOR_DRAG);
  };

  const calcDragAnchor = useEvent(
    (cursorState: AmpAxisCursorState, cursorPos: number, rect: DOMRect) => {
      return {
        cursorRatio: calcLimitedCursorRatio(cursorState, cursorPos, rect),
        ampRange: (ampRangeRef.current ?? DEFAULT_AMP_RANGE).slice(),
      } as AmpAxisDragAnchor;
    },
  );

  const handleDragging = useEvent(
    (
      cursorState: AmpAxisCursorState,
      cursorPos: number,
      dragAnchorValue: AmpAxisDragAnchor,
      rect: DOMRect,
    ) => {
      const {cursorRatio: anchorRatio, ampRange: anchorAmpRange} = dragAnchorValue;
      const [anchorInterval, zeroRatio] = calcIntervalZeroRatio(anchorAmpRange);
      const cursorRatio = calcLimitedCursorRatio(cursorState, cursorPos, rect);
      const newInterval = (anchorInterval * (anchorRatio - zeroRatio)) / (cursorRatio - zeroRatio);
      setAmpRange(clampAmpRange([newInterval * (zeroRatio - 1), newInterval * zeroRatio]));
    },
  );

  const onWheel = useEvent((e: WheelEvent) => {
    if (!enableInteraction) return;
    if (e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;

      const [interval, zeroRatio] = calcIntervalZeroRatio(ampRangeRef.current ?? DEFAULT_AMP_RANGE);
      const newInterval = interval * Math.max(1 - e.deltaY / 500, 0);
      setAmpRange(clampAmpRange([newInterval * (zeroRatio - 1), newInterval * zeroRatio]));
    }
  });

  const onClick = useEvent((e: MouseEvent) => {
    if (!enableInteraction) return;
    if (e.button === 0 && e.altKey && e.detail === 1) {
      e.preventDefault();
      resetAmpRange();
    }
    if (e.button === 0 && e.detail === 2) {
      e.preventDefault();
      setFloatingInputHidden(false);
    }
  });

  const cursorStateInfos: Map<
    AmpAxisCursorState,
    CursorStateInfo<AmpAxisCursorState, AmpAxisDragAnchor>
  > = useMemo(
    () =>
      new Map([
        [
          "positive",
          {
            cursor: "n-resize",
            cursorClassNameForBody: "nResizeCursor",
            handleDragging,
          },
        ],
        [
          "negative",
          {
            cursor: "s-resize",
            cursorClassNameForBody: "sResizeCursor",
            handleDragging,
          },
        ],
      ]),
    [handleDragging],
  );

  const wrapperDivElemCallback = useCallback(
    (elem: HTMLDivElement) => {
      if (!elem) {
        wrapperDivElem.current?.removeEventListener("wheel", onWheel);
        wrapperDivElem.current?.removeEventListener("click", onClick);
        wrapperDivElem.current = null;
        return;
      }
      elem.addEventListener("wheel", onWheel, {passive: false});
      elem.addEventListener("click", onClick);
      wrapperDivElem.current = elem;
    },
    [onWheel, onClick],
  );

  const onEndEditingFloatingInput = useEvent((v: string | null) => {
    if (v !== null) {
      const num = Number(v);
      if (!Number.isNaN(num)) {
        const absValue = Math.abs(num);
        if (absValue > MIN_ABS_AMP_RANGE) setAmpRange(clampAmpRange([-absValue, absValue]));
      }
    }
    setFloatingInputHidden(true);
  });

  const axisCanvas = (
    <>
      <FloatingUserInput
        value={ampRangeRef.current?.[1].toFixed(1) ?? "0.0"}
        onEndEditing={onEndEditingFloatingInput}
        hidden={floatingInputHidden}
        top="0px"
        left="0px"
      />
      <AxisCanvas
        ref={ref}
        width={AMP_CANVAS_WIDTH}
        height={height}
        axisPadding={VERTICAL_AXIS_PADDING}
        markerPos={AMP_MARKER_POS}
        direction="V"
        className="ampAxis"
        endInclusive
      />
    </>
  );
  return (
    <div
      ref={wrapperDivElemCallback}
      className={styles.ampAxisWrapper}
      role="presentation"
      onMouseDown={(e) => {
        if ((e.button === 0 && e.detail === 1) || e.button !== 0) {
          setFloatingInputHidden(true);
        }
      }}
    >
      {enableInteraction ? (
        <Draggable
          cursorStateInfos={cursorStateInfos}
          calcCursorPos="y"
          determineCursorStates={determineCursorStates}
          calcDragAnchor={calcDragAnchor}
          dragAnchorDefault={DEFAULT_DRAG_ANCHOR}
        >
          {axisCanvas}
        </Draggable>
      ) : (
        axisCanvas
      )}
    </div>
  );
});

AmpAxis.displayName = "AmpAxis";

export default AmpAxis;
