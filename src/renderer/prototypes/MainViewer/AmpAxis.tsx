import React, {useEffect, useMemo, useState} from "react";
import AxisCanvas, {getAxisHeight} from "renderer/modules/AxisCanvas";
import styles from "renderer/modules/AxisCanvas.module.scss";
import Draggable, {CursorStateInfo} from "renderer/modules/Draggable";
import useEvent from "react-use-event-hook";
import FloatingUserInput from "renderer/modules/FloatingUserInput";
import {ipcRenderer} from "electron";
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
  id: number;
  height: number;
  markersAndLength: [Markers, number];
  ampRange: [number, number];
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

function AmpAxis(props: AmpAxisProps) {
  const {id, height, markersAndLength, ampRange, setAmpRange, resetAmpRange, enableInteraction} =
    props;
  const [floatingInputHidden, setFloatingInputHidden] = useState<boolean>(true);

  const calcLimitedCursorRatio = (
    cursorState: AmpAxisCursorState,
    cursorPos: number,
    rect: DOMRect,
  ) => {
    const cursorRatio = cursorPos / getAxisHeight(rect);
    const [_interval, zeroRatio] = calcIntervalZeroRatio(ampRange);
    if (cursorState === "positive") {
      return Math.min(cursorRatio, zeroRatio - MIN_DIST_FROM_0_FOR_DRAG);
    }
    return Math.max(cursorRatio, zeroRatio + MIN_DIST_FROM_0_FOR_DRAG);
  };

  const calcDragAnchor = useEvent(
    (cursorState: AmpAxisCursorState, cursorPos: number, rect: DOMRect) => {
      return {
        cursorRatio: calcLimitedCursorRatio(cursorState, cursorPos, rect),
        ampRange: ampRange.slice(),
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
      if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;

      const [interval, zeroRatio] = calcIntervalZeroRatio(ampRange);
      const newInterval = interval * Math.max(1 - e.deltaY / 500, 0);
      setAmpRange(clampAmpRange([newInterval * (zeroRatio - 1), newInterval * zeroRatio]));
    }
  });

  const onClick = useEvent((e: React.MouseEvent) => {
    if (!enableInteraction) return;
    if (e.button === 0 && e.altKey && e.detail === 1) {
      e.preventDefault();
      resetAmpRange();
    }
    if (e.button === 0 && e.detail === 2) {
      e.preventDefault();
      setFloatingInputHidden(false);
    }
    if ((e.button === 0 && e.detail === 1) || e.button !== 0) {
      setFloatingInputHidden(true);
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

  const onEditAxisRangeMenu = useEvent(() => setFloatingInputHidden(false));

  useEffect(() => {
    ipcRenderer.on(`edit-ampAxis-range-${id}`, onEditAxisRangeMenu);
    return () => {
      ipcRenderer.removeListener(`edit-ampAxis-range-${id}`, onEditAxisRangeMenu);
    };
  }, [id, onEditAxisRangeMenu]);

  const axisCanvas = (
    <>
      <FloatingUserInput
        value={ampRange[1].toFixed(1)}
        onEndEditing={onEndEditingFloatingInput}
        hidden={floatingInputHidden}
        className={styles.ampFloatingInput}
      />
      <AxisCanvas
        id={id}
        width={AMP_CANVAS_WIDTH}
        height={height}
        axisPadding={VERTICAL_AXIS_PADDING}
        markerPos={AMP_MARKER_POS}
        markersAndLength={markersAndLength}
        direction="V"
        className="ampAxis"
        endInclusive
        onWheel={onWheel}
        onClick={onClick}
      />
    </>
  );
  return (
    <div className={styles.ampAxisWrapper}>
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
}

export default React.memo(AmpAxis);
