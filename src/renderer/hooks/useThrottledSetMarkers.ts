import {useMemo} from "react";
import {MarkerDrawOption} from "../api";

type ThrottledSetMarkersParams = {
  scaleTable: TickScaleTable;
  boundaries: number[];
  getMarkers: (
    maxTickCount: number,
    maxLabelCount: number,
    drawOption?: MarkerDrawOption,
  ) => Markers;
  canvasLength: number;
  scaleDeterminant: number;
  drawOptions?: MarkerDrawOption;
};

const getTickScale = (
  table: TickScaleTable,
  boundaries: number[],
  value: number,
): [number, number] | null => {
  const target = boundaries.find((boundary) => value >= boundary);
  if (target === undefined) {
    console.error("invalid tick scale determinant");
    return null;
  }

  return table[target];
};

export default function useAxisMarkers(params: ThrottledSetMarkersParams) {
  const {scaleTable, boundaries, getMarkers, canvasLength, scaleDeterminant, drawOptions} = params;
  const tickScale: [number, number] | null = useMemo(
    () => getTickScale(scaleTable, boundaries, scaleDeterminant),
    [scaleTable, boundaries, scaleDeterminant],
  );

  const markersAndLength: [Markers, number] = useMemo(() => {
    if (!tickScale || canvasLength === 0) return [[], 1];

    // time axis returns [size of minor unit, number of minor tick]
    // instead of tick and lable count
    const [maxTickCount, maxLabelCount] = tickScale;
    const markers = getMarkers(maxTickCount, maxLabelCount, drawOptions);
    return [markers, canvasLength];
  }, [tickScale, getMarkers, drawOptions, canvasLength]);

  return markersAndLength;
}
