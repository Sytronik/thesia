import React, {RefObject, forwardRef, useMemo} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import Draggable, {CursorStateInfo} from "renderer/modules/Draggable";
import useEvent from "react-use-event-hook";
import {HORIZONTAL_AXIS_PADDING, TIME_CANVAS_HEIGHT, TIME_MARKER_POS} from "../constants/tracks";

type TimeAxisProps = {
  width: number;
  shiftWhenResize: boolean;
  startSecRef: RefObject<number>;
  pxPerSecRef: RefObject<number>;
  moveLens: (sec: number, dragAnchor: number) => void;
  resetTimeAxis: () => void;
  enableInteraction: boolean;
};
type TimeAxisCursorState = "drag";
type TimeAxisDragAnchor = {
  cursorRatio: number;
  sec: number;
};
const DEFAULT_DRAG_ANCHOR: TimeAxisDragAnchor = {cursorRatio: 0, sec: 0};
const determineCursorStates: () => "drag" = () => "drag";

const TimeAxis = forwardRef((props: TimeAxisProps, ref) => {
  const {
    width,
    shiftWhenResize,
    startSecRef,
    pxPerSecRef,
    moveLens,
    resetTimeAxis,
    enableInteraction,
  } = props;
  const calcDragAnchor = useEvent(
    (cursorState: TimeAxisCursorState, cursorPos: number, rect: DOMRect) => {
      const cursorRatio = cursorPos / rect.width;
      const sec =
        (startSecRef.current ?? 0) + (cursorRatio * rect.width) / (pxPerSecRef.current ?? 1);
      return {cursorRatio, sec} as TimeAxisDragAnchor;
    },
  );

  const handleDragging = useEvent(
    (
      cursorState: TimeAxisCursorState,
      cursorPos: number,
      dragAnchorValue: TimeAxisDragAnchor,
      rect: DOMRect,
    ) => {
      const cursorRatio = cursorPos / rect.width;
      const {cursorRatio: anchorRatio, sec: anchorSec} = dragAnchorValue;
      const sec =
        anchorSec - ((cursorRatio - anchorRatio) * rect.width) / (pxPerSecRef.current ?? 1);
      moveLens(sec, anchorRatio);
    },
  );

  const cursorStateInfos: Map<
    TimeAxisCursorState,
    CursorStateInfo<TimeAxisCursorState, TimeAxisDragAnchor>
  > = useMemo(
    () =>
      new Map([["drag", {cursor: "text", cursorClassNameForBody: "textCursor", handleDragging}]]),
    [handleDragging],
  );

  const onClick = useEvent((e: MouseEvent) => {
    if (!enableInteraction) return;
    if (e.altKey && e.button === 0 && e.detail === 1) resetTimeAxis();
  });

  const axis = (
    <AxisCanvas
      id={0}
      ref={ref}
      key="time-axis"
      width={width}
      height={TIME_CANVAS_HEIGHT}
      axisPadding={HORIZONTAL_AXIS_PADDING}
      markerPos={TIME_MARKER_POS}
      direction="H"
      className="timeRuler"
      shiftWhenResize={shiftWhenResize}
      onClick={onClick}
    />
  );

  return enableInteraction ? (
    <Draggable
      cursorStateInfos={cursorStateInfos}
      calcCursorPos="x"
      determineCursorStates={determineCursorStates}
      calcDragAnchor={calcDragAnchor}
      dragAnchorDefault={DEFAULT_DRAG_ANCHOR}
    >
      {axis}
    </Draggable>
  ) : (
    axis
  );
});

TimeAxis.displayName = "TimeAxis";

export default React.memo(TimeAxis);
