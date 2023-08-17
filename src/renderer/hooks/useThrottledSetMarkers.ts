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
  ) => Promise<Markers>;
};

const THRESHOLD = 1000 / 240;

const getTickScale = (table: TickScaleTable, boundaries: number[], value: number) => {
  const target = boundaries.find((boundary) => value >= boundary);
  if (target === undefined) {
    console.error("invalid tick scale determinant");
    return null;
  }

  return table[target];
};

function useThrottledSetMarkers(params: ThrottledSetMarkersParams) {
  const markersRef = useRef<Markers>([]);
  const {scaleTable, boundaries, getMarkers} = params;

  const throttledSetMarkers = throttle(
    THRESHOLD,
    async (canvasLength: number, scaleDeterminant: number, drawOptions: MarkerDrawOption) => {
      // TODO: block execution when no trackIds exist
      if (!canvasLength) {
        console.error("invalid canvas");
        return;
      }

      const tickScale = getTickScale(scaleTable, boundaries, scaleDeterminant);
      if (!tickScale) {
        return;
      }

      // time axis returns [size of minor unit, number of minor tick]
      // instead of tick and lable count
      const [maxTickCount, maxLabelCount] = tickScale;
      markersRef.current = await getMarkers(canvasLength, maxTickCount, maxLabelCount, drawOptions);
    },
  );

  return {
    markersRef,
    throttledSetMarkers,
  };
}

export default useThrottledSetMarkers;
