import {useRef} from "react";
import {throttle} from "throttle-debounce";

type ThrottledSetMarkersParams = {
  scaleTable: TickScaleTable;
  boundaries: number[];
  getMarkers: (
    value: number,
    maxTickCount: number,
    maxLabelCount: number,
    drawOption: MarkerDrawOption,
  ) => Markers;
};

const THRESHOLD = 1000 / 240;

const getTickScale = (table: TickScaleTable, boundaries: number[], value: number) => {
  const target = boundaries.find((boundary) => value > boundary);
  if (target === undefined) {
    console.error("tick scale boundary error");
    return null;
  }

  return table[target];
};

function useThrottledSetMarkers(params: ThrottledSetMarkersParams) {
  const markersRef = useRef<Markers>([]);
  const {scaleTable, boundaries, getMarkers} = params;

  const throttledSetMarkers = throttle(
    THRESHOLD,
    (canvasLength: number, scaleDeterminant: number, drawOptions: MarkerDrawOption) => {
      // TODO: block execution when no trackIds exist
      const tickScale = getTickScale(scaleTable, boundaries, scaleDeterminant);

      if (!canvasLength || !tickScale) {
        return;
      }

      // time axis returns [size of minor unit, number of minor tick]
      // instead of tick and lable count
      const [maxTickCount, maxLabelCount] = tickScale;
      markersRef.current = getMarkers(canvasLength, maxTickCount, maxLabelCount, drawOptions);
    },
  );

  return {
    markersRef,
    throttledSetMarkers,
  };
}

export default useThrottledSetMarkers;
