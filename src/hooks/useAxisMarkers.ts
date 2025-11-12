import {useEffect, useMemo, useState} from "react";
import {MarkerDrawOption} from "../api";

type ThrottledSetMarkersParams = {
  scaleTable: TickScaleTable;
  boundaries: number[];
  getMarkers: (
    maxTickCount: number,
    maxLabelCount: number,
    drawOption?: MarkerDrawOption,
  ) => Promise<Markers>;
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
    console.warn("invalid tick scale determinant");
    return null;
  }

  return table[target];
};

export default function useAxisMarkers(params: ThrottledSetMarkersParams): [Markers, number] {
  const {scaleTable, boundaries, getMarkers, canvasLength, scaleDeterminant, drawOptions} = params;
  const tickScale: [number, number] | null = useMemo(
    () => getTickScale(scaleTable, boundaries, scaleDeterminant),
    [scaleTable, boundaries, scaleDeterminant],
  );

  const [markersAndLength, setMarkersAndLength] = useState<[Markers, number]>([[], 1]);
  useEffect(() => {
    if (!tickScale || canvasLength === 0) return;
    const [maxTickCount, maxLabelCount] = tickScale;
    getMarkers(maxTickCount, maxLabelCount, drawOptions).then((markers) => {
      setMarkersAndLength([markers, canvasLength]);
    });
  }, [tickScale, getMarkers, drawOptions, canvasLength]);

  return markersAndLength;
}
