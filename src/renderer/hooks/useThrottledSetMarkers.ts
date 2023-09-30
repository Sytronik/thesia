import {useRef, useMemo} from "react";
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

const THRESHOLD = 1000 / 120;

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
        async (canvasLength: number, scaleDeterminant: number, drawOptions: MarkerDrawOption) => {
          // TODO: block execution when no trackIds exist
          if (!canvasLength) {
            markersAndLengthRef.current = [[], 1];
            return;
          }

          const tickScale = getTickScale(scaleTable, boundaries, scaleDeterminant);
          if (!tickScale) return;

          // time axis returns [size of minor unit, number of minor tick]
          // instead of tick and lable count
          const [maxTickCount, maxLabelCount] = tickScale;
          const markers = await getMarkers(canvasLength, maxTickCount, maxLabelCount, drawOptions);
          markersAndLengthRef.current = [markers, canvasLength];
        },
      ),
    [boundaries, getMarkers, scaleTable],
  );

  return {
    markersAndLengthRef,
    throttledSetMarkers,
  };
}

export default useThrottledSetMarkers;
