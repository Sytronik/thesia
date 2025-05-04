import React, {forwardRef, useEffect, useMemo, useRef, useState} from "react";
import AxisCanvas, {getAxisHeight, getAxisPos} from "renderer/modules/AxisCanvas";
import styles from "renderer/modules/AxisCanvas.module.scss";
import Draggable, {CursorStateInfo} from "renderer/modules/Draggable";
import useEvent from "react-use-event-hook";
import FloatingUserInput from "renderer/modules/FloatingUserInput";
import {ipcRenderer} from "electron";
import type {throttle} from "throttle-debounce";
import {
  FREQ_CANVAS_WIDTH,
  FREQ_MARKER_POS,
  MIN_HZ_RANGE,
  VERTICAL_AXIS_PADDING,
} from "../constants/tracks";
import BackendAPI from "../../api";

type FreqAxisProps = {
  id: number;
  height: number;
  markersAndLength: [Markers, number];
  maxTrackHz: number;
  hzRange: [number, number];
  setHzRange:
    | ((minHz: number, maxHz: number) => Promise<void>)
    | throttle<(minHz: number, maxHz: number) => Promise<void>>;
  resetHzRange: () => void;
  enableInteraction: boolean;
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
  cursorAxisPos: 0.0,
  hzRange: [0, Infinity],
};

const FreqAxis = forwardRef((props: FreqAxisProps, ref) => {
  const {
    id,
    height,
    markersAndLength,
    maxTrackHz,
    hzRange,
    setHzRange,
    resetHzRange,
    enableInteraction,
  } = props;
  const [minHzInputHidden, setMinHzInputHidden] = useState(true);
  const [maxHzInputHidden, setMaxHzInputHidden] = useState(true);
  const cursorStateRef = useRef<FreqAxisCursorState>("shift-hz-range");

  const clampMaxHz = (maxHz: number, minHz: number) => {
    if (maxHz > maxTrackHz) return Infinity;
    return Math.max(maxHz, minHz + MIN_HZ_RANGE);
  };
  const clampMinHz = (minHz: number, maxHz: number) => {
    return Math.min(Math.max(minHz, 0), maxHz - MIN_HZ_RANGE);
  };

  const calcDragAnchor = useEvent(
    (cursorState: FreqAxisCursorState, cursorPos: number, rect: DOMRect) => {
      const cursorAxisPos = getAxisPos(cursorPos);
      if (cursorState === "shift-hz-range") {
        const axisHeight = getAxisHeight(rect);
        const zeroHzPos = BackendAPI.freqHzToPos(0, axisHeight, hzRange);
        const maxTrackHzPos = BackendAPI.freqHzToPos(maxTrackHz, axisHeight, hzRange);
        return {cursorAxisPos, hzRange, zeroHzPos, maxTrackHzPos} as FreqAxisDragAnchor;
      }
      return {cursorAxisPos: getAxisPos(cursorPos), hzRange} as FreqAxisDragAnchor;
    },
  );

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
      let newHzRange = [anchorHzRange[0], anchorHzRange[1]];
      switch (cursorState) {
        case "control-max-hz": {
          const ratio = (anchorAxisPos - cursorAxisPos) / (axisHeight - anchorAxisPos);
          const maxHz = BackendAPI.freqPosToHz(ratio * axisHeight, axisHeight, anchorHzRange);
          newHzRange[1] = clampMaxHz(maxHz, anchorHzRange[0]);
          break;
        }
        case "control-min-hz": {
          const minHz = BackendAPI.freqPosToHz(
            anchorAxisPos,
            Math.max(cursorAxisPos, 1),
            anchorHzRange,
          );
          newHzRange[0] = clampMinHz(minHz, anchorHzRange[1]);
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
            newHzRange = [0, Infinity];
            break;
          }
          newHzRange = [
            BackendAPI.freqPosToHz(minHzPos, axisHeight, anchorHzRange),
            BackendAPI.freqPosToHz(maxHzPos, axisHeight, anchorHzRange),
          ];
          break;
        }
        default:
          break;
      }
      await setHzRange(newHzRange[0], newHzRange[1]);
    },
  );

  const onWheel = useEvent(async (e: WheelEvent) => {
    if (!enableInteraction) return;
    if (e.altKey) {
      e.preventDefault();
      if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;

      // TODO: control minHz
      const maxHz = BackendAPI.freqPosToHz(e.deltaY, 500, hzRange);
      await setHzRange(hzRange[0], clampMaxHz(maxHz, hzRange[0]));
    }
  });

  const onClick = useEvent((e: MouseEvent) => {
    if (!enableInteraction) return;
    if (e.button === 0 && e.altKey && e.detail === 1) {
      e.preventDefault();
      resetHzRange();
    }
    if (e.button === 0 && e.detail === 2) {
      e.preventDefault();
      if (cursorStateRef.current === "control-max-hz") setMaxHzInputHidden(false);
      if (cursorStateRef.current === "control-min-hz") setMinHzInputHidden(false);
    }
    if ((e.button === 0 && e.detail === 1) || e.button !== 0) {
      setMinHzInputHidden(true);
      setMaxHzInputHidden(true);
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

  const hzRangeLabel = hzRange.map((hz) => BackendAPI.hzToLabel(Math.min(hz, maxTrackHz)));

  const onCursorStateChange = useEvent((cursorState) => {
    cursorStateRef.current = cursorState;
  });

  const onEndEditingFloatingInput = async (v: string | null, idx: number) => {
    if (v !== null) {
      const hz = BackendAPI.freqLabelToHz(v);
      if (!Number.isNaN(hz)) {
        const newHzRange = [hzRange[0], hzRange[1]];
        newHzRange[idx] = idx === 0 ? clampMinHz(hz, newHzRange[1]) : clampMaxHz(hz, newHzRange[0]);
        await setHzRange(newHzRange[0], newHzRange[1]);
      }
    }
    if (idx === 0) setMinHzInputHidden(true);
    else setMaxHzInputHidden(true);
  };
  const onEndEditingMinHzInput = useEvent((v) => onEndEditingFloatingInput(v, 0));
  const onEndEditingMaxHzInput = useEvent((v) => onEndEditingFloatingInput(v, 1));

  const onEditAxisRangeMenu = useEvent((_, minOrMax: "min" | "max") =>
    minOrMax === "min" ? setMinHzInputHidden(false) : setMaxHzInputHidden(false),
  );

  useEffect(() => {
    ipcRenderer.on(`edit-freqAxis-range-${id}`, onEditAxisRangeMenu);
    return () => {
      ipcRenderer.removeListener(`edit-freqAxis-range-${id}`, onEditAxisRangeMenu);
    };
  }, [id, onEditAxisRangeMenu]);

  const axisCanvas = (
    <>
      <FloatingUserInput
        value={hzRangeLabel[0]}
        onEndEditing={onEndEditingMinHzInput}
        hidden={minHzInputHidden}
        className={styles.minHzFloatingInput}
      />
      <FloatingUserInput
        value={hzRangeLabel[1]}
        onEndEditing={onEndEditingMaxHzInput}
        hidden={maxHzInputHidden}
        className={styles.maxHzFloatingInput}
      />
      <AxisCanvas
        id={id}
        ref={ref}
        width={FREQ_CANVAS_WIDTH}
        height={height}
        axisPadding={VERTICAL_AXIS_PADDING}
        markerPos={FREQ_MARKER_POS}
        markersAndLength={markersAndLength}
        direction="V"
        className="freqAxis"
        endInclusive
        onWheel={onWheel}
        onClick={onClick}
      />
    </>
  );

  return (
    <div className={styles.freqAxisWrapper}>
      {enableInteraction ? (
        <Draggable
          cursorStateInfos={cursorStateInfos}
          calcCursorPos="y"
          determineCursorStates={determineCursorStates}
          calcDragAnchor={calcDragAnchor}
          dragAnchorDefault={DEFAULT_DRAG_ANCHOR}
          onCursorStateChange={onCursorStateChange}
        >
          {axisCanvas}
        </Draggable>
      ) : (
        axisCanvas
      )}
    </div>
  );
});

FreqAxis.displayName = "FreqAxis";

export default React.memo(FreqAxis);
