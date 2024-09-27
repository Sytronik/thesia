import {useRef, useMemo} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import {MarkerDrawOption} from "../api";

type ThrottledSetMarkersParams = {
  scaleTable: TickScaleTable;
  boundaries: number[];
  getMarkers: (
    maxTickCount: number,
    maxLabelCount: number,
    drawOption?: MarkerDrawOption,
  ) => Markers;
};

const THRESHOLD = 1000 / 70;

const getTickScale = (table: TickScaleTable, boundaries: number[], value: number) => {
  const target = boundaries.find((boundary) => value >= boundary);
  if (target === undefined) {
    console.error("invalid tick scale determinant");
    return null;
  }

  return table[target];
};

function useThrottledSetMarkers(params: ThrottledSetMarkersParams) {
  const markersAndLengthRef = useRef<[Markers, number]>([[], 1]);
  const {scaleTable, boundaries, getMarkers} = params;

  const throttledSetMarkers = useMemo(
    () =>
      throttle(
        THRESHOLD,
        (canvasLength: number, scaleDeterminant: number, drawOptions?: MarkerDrawOption) => {
          const tickScale = getTickScale(scaleTable, boundaries, scaleDeterminant);
          if (!tickScale) return;

          // time axis returns [size of minor unit, number of minor tick]
          // instead of tick and lable count
          const [maxTickCount, maxLabelCount] = tickScale;
          const markers = getMarkers(maxTickCount, maxLabelCount, drawOptions);
          markersAndLengthRef.current = [markers, canvasLength];
        },
      ),
    [boundaries, getMarkers, scaleTable],
  );

  const resetMarkers = useEvent(() => {
    markersAndLengthRef.current = [[], 1];
  });

  return {
    markersAndLengthRef,
    throttledSetMarkers,
    resetMarkers,
  };
}

export default useThrottledSetMarkers;
