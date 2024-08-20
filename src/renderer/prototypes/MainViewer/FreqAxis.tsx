import React, {forwardRef, useCallback, useMemo, useRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import Draggable, {CursorStateInfo} from "renderer/modules/Draggable";
import useEvent from "react-use-event-hook";
import {FREQ_CANVAS_WIDTH, FREQ_MARKER_POS, VERTICAL_AXIS_PADDING} from "../constants/tracks";
import BackendAPI from "../../api";

type FreqAxisProps = {
  height: number;
  setHzRange: (minHz: number, maxHz: number) => void;
};

type FreqAxisCursorState = "control-max-hz" | "shift-hz-range" | "control-min-hz";
const determineCursorStates = (cursorPos: number, rect: DOMRect) => {
  if (cursorPos < rect.height / 3) return "control-max-hz";
  if (cursorPos < (rect.height * 2) / 3) return "shift-hz-range";
  return "control-min-hz";
};

type FreqAxisDragAnchor = {
  cursorAxisPos: number;
  hzRange: [number, number];
  zeroHzPos?: number;
  maxTrackHzPos?: number;
};
const DEFAULT_DRAG_ANCHOR: FreqAxisDragAnchor = {
  cursorAxisPos: 0,
  hzRange: [0, Infinity],
};
const MIN_HZ_RANGE = 100;

const getAxisHeight = (rect: DOMRect) => rect.height - 2 * VERTICAL_AXIS_PADDING;
const getAxisPos = (pos: number) => pos - VERTICAL_AXIS_PADDING;

const clampMaxHz = (maxHz: number, minHz: number) => {
  if (maxHz > BackendAPI.getMaxTrackHz()) return Infinity;
  return Math.max(maxHz, minHz + MIN_HZ_RANGE);
};
const clampMinHz = (minHz: number, maxHz: number) => {
  return Math.min(Math.max(minHz, 0), maxHz - MIN_HZ_RANGE);
};

const calcDragAnchor = (cursorState: FreqAxisCursorState, cursorPos: number, rect: DOMRect) => {
  const cursorAxisPos = getAxisPos(cursorPos);
  const hzRange = BackendAPI.getHzRange();
  if (cursorState === "shift-hz-range") {
    const axisHeight = getAxisHeight(rect);
    const zeroHzPos = BackendAPI.convertFreqHzToPos(0, axisHeight, [hzRange[0], hzRange[1]]);
    const maxTrackHzPos = BackendAPI.convertFreqHzToPos(BackendAPI.getMaxTrackHz(), axisHeight, [
      hzRange[0],
      hzRange[1],
    ]);
    return {cursorAxisPos, hzRange, zeroHzPos, maxTrackHzPos} as FreqAxisDragAnchor;
  }
  return {cursorAxisPos: getAxisPos(cursorPos), hzRange} as FreqAxisDragAnchor;
};

const FreqAxis = forwardRef((props: FreqAxisProps, ref) => {
  const {height, setHzRange} = props;
  const wrapperDivElem = useRef<HTMLDivElement | null>(null);

  const handleDragging = useEvent(
    async (
      cursorState: FreqAxisCursorState,
      cursorPos: number,
      dragAnchorValue: FreqAxisDragAnchor,
      rect: DOMRect,
    ) => {
      const {cursorAxisPos: anchorAxisPos, hzRange: anchorHzRange} = dragAnchorValue;
      const axisHeight = getAxisHeight(rect);
      const cursorAxisPos = getAxisPos(cursorPos);
      let hzRange = [anchorHzRange[0], anchorHzRange[1]];
      switch (cursorState) {
        case "control-max-hz": {
          const ratio = (anchorAxisPos - cursorAxisPos) / (axisHeight - anchorAxisPos);
          const maxHz = await BackendAPI.convertFreqPosToHz(
            ratio * axisHeight,
            axisHeight,
            anchorHzRange,
          );
          hzRange[1] = clampMaxHz(maxHz, anchorHzRange[0]);
          break;
        }
        case "control-min-hz": {
          const minHz = await BackendAPI.convertFreqPosToHz(
            anchorAxisPos,
            Math.max(cursorAxisPos, 1),
            anchorHzRange,
          );
          hzRange[0] = clampMinHz(minHz, anchorHzRange[1]);
          break;
        }
        case "shift-hz-range": {
          const shift = anchorAxisPos - cursorAxisPos;
          let minHzPos = axisHeight + shift;
          let maxHzPos = shift;
          const zeroHzPos = dragAnchorValue.zeroHzPos ?? axisHeight;
          const maxTrackHzPos = dragAnchorValue.maxTrackHzPos ?? 0;
          if (minHzPos > zeroHzPos) {
            maxHzPos -= minHzPos - zeroHzPos;
            minHzPos = zeroHzPos;
          }
          if (maxHzPos < maxTrackHzPos) {
            minHzPos += maxTrackHzPos - maxHzPos;
            maxHzPos = maxTrackHzPos;
          }
          if (minHzPos > zeroHzPos) {
            hzRange = [0, Infinity];
            break;
          }
          hzRange = [
            await BackendAPI.convertFreqPosToHz(minHzPos, axisHeight, anchorHzRange),
            await BackendAPI.convertFreqPosToHz(maxHzPos, axisHeight, anchorHzRange),
          ];
          break;
        }
        default:
          break;
      }
      setHzRange(hzRange[0], hzRange[1]);
    },
  );

  const onWheel = useEvent(async (e: WheelEvent) => {
    if (e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;

      // TODO: control minHz
      const hzRange = BackendAPI.getHzRange();
      const maxHz = await BackendAPI.convertFreqPosToHz(e.deltaY, 500, [hzRange[0], hzRange[1]]);
      setHzRange(hzRange[0], clampMaxHz(maxHz, hzRange[0]));
    }
  });

  const onClick = useEvent((e: MouseEvent) => {
    e.preventDefault();
    if (e.button === 0 && e.detail === 2) {
      e.stopPropagation();
      setTimeout(() => setHzRange(0, Infinity));
    }
  });

  const cursorStateInfos: Map<
    FreqAxisCursorState,
    CursorStateInfo<FreqAxisCursorState, FreqAxisDragAnchor>
  > = useMemo(
    () =>
      new Map([
        [
          "control-max-hz",
          {
            cursor: "n-resize",
            cursorClassNameForBody: "nResizeCursor",
            handleDragging,
          },
        ],
        [
          "shift-hz-range",
          {
            cursor: "row-resize",
            cursorClassNameForBody: "rowResizeCursor",
            handleDragging,
          },
        ],
        [
          "control-min-hz",
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

  return (
    <div ref={wrapperDivElemCallback}>
      <Draggable
        cursorStateInfos={cursorStateInfos}
        calcCursorPos="y"
        determineCursorStates={determineCursorStates}
        calcDragAnchor={calcDragAnchor}
        dragAnchorDefault={DEFAULT_DRAG_ANCHOR}
      >
        <AxisCanvas
          ref={ref}
          width={FREQ_CANVAS_WIDTH}
          height={height}
          axisPadding={VERTICAL_AXIS_PADDING}
          markerPos={FREQ_MARKER_POS}
          direction="V"
          className="freqAxis"
          endInclusive
        />
      </Draggable>
    </div>
  );
});

FreqAxis.displayName = "FreqAxis";

export default FreqAxis;
