import init, {
  calcTimeAxisMarkers as _calcTimeAxisMarkers,
  calcFreqAxisMarkers as _calcFreqAxisMarkers,
  calcAmpAxisMarkers as _calcAmpAxisMarkers,
  calcDbAxisMarkers as _calcDbAxisMarkers,
  secondsToLabel,
  timeLabelToSeconds,
  hzToLabel,
  freqLabelToHz,
  freqPosToHz,
  freqHzToPos,
  formatLinearAxisTooltip as _formatLinearAxisTooltip,
  formatFrequencyAxisTooltip as _formatFrequencyAxisTooltip,
  formatTimeAxisTooltip,
  formatNumberLabel,
} from "thesia-wasm-module";
import { FreqScale } from "./backend-wrapper";

let wasmInitialized = false;

/**
 * Initializes the WASM module.
 * This function must be called once before using other WASM functions.
 */
export async function initWasm(): Promise<void> {
  if (!wasmInitialized) {
    await init();
    wasmInitialized = true;
  }
}

/**
 * Checks if the WASM module has been initialized.
 */
export function isWasmInitialized(): boolean {
  return wasmInitialized;
}

export type TickPxPosition = number;
export type TickLabel = string;
export type Markers = [TickPxPosition, TickLabel][];
export type MarkerDrawOption = {
  startSec?: number;
  endSec?: number;
  maxSec?: number;
  freqScale?: FreqScale;
  hzRange?: [number, number];
  maxTrackHz?: number;
  ampRange?: [number, number];
  mindB?: number;
  maxdB?: number;
};

export function calcTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { startSec, endSec, maxSec } = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined || maxSec === undefined) {
    console.error("no markerDrawOptions for time axis exist");
    return [];
  }
  return _calcTimeAxisMarkers(startSec, endSec, subTickSec, subTickUnitCount, maxSec);
}

/* track axis */
export function calcFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { maxTrackHz, freqScale, hzRange } = markerDrawOptions || {};

  if (maxTrackHz === undefined || freqScale === undefined || hzRange === undefined) {
    console.error("no markerDrawOptions for freq axis exist");
    return [];
  }
  return _calcFreqAxisMarkers(
    hzRange[0],
    hzRange[1],
    freqScale,
    maxNumTicks,
    maxNumLabels,
    maxTrackHz,
  );
}

export function calcAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { ampRange } = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no markerDrawOption for amp axis exist");
    return [];
  }

  return _calcAmpAxisMarkers(maxNumTicks, maxNumLabels, ampRange[0], ampRange[1]);
}

export function calcDbAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions?: MarkerDrawOption,
): Markers {
  const { mindB, maxdB } = markerDrawOptions || {};

  if (mindB === undefined || maxdB === undefined) {
    console.error("no markerDrawOptions for dB axis exist");
    return [];
  }

  return _calcDbAxisMarkers(maxNumTicks, maxNumLabels, mindB, maxdB);
}

const getValueAndResolution = (
  getValue: (axisPosition: number) => number,
  axisPosition: number,
  axisLength: number,
) => {
  const value = getValue(axisPosition);
  const adjacentPosition =
    axisPosition <= axisLength / 2
      ? Math.min(axisPosition + 1, axisLength)
      : Math.max(axisPosition - 1, 0);
  return [value, Math.abs(getValue(adjacentPosition) - value)] as const;
};

export const formatLinearAxisTooltip = (
  getValue: (axisPosition: number) => number,
  axisPosition: number,
  axisLength: number,
  markers: Markers,
  maxFractionDigits = 9,
) => {
  const [value, resolution] = getValueAndResolution(getValue, axisPosition, axisLength);
  const tickValues = markers
    .map(([ratio]) => getValue(Math.min(Math.max(ratio * axisLength, 0), axisLength)))
    .filter(Number.isFinite);
  const tickUnit = tickValues
    .slice(1)
    .map((tickValue, index) => Math.abs(tickValue - tickValues[index]))
    .find((unit) => unit > 0);
  return _formatLinearAxisTooltip(value, resolution, tickUnit ?? Number.NaN, maxFractionDigits);
};

export const formatFrequencyAxisTooltip = (
  getValue: (axisPosition: number) => number,
  axisPosition: number,
  axisLength: number,
) => {
  const [hz, resolutionHz] = getValueAndResolution(getValue, axisPosition, axisLength);
  return _formatFrequencyAxisTooltip(hz, resolutionHz);
};

export default {
  initWasm,
  isWasmInitialized,
  calcTimeAxisMarkers,
  calcFreqAxisMarkers,
  calcAmpAxisMarkers,
  calcDbAxisMarkers,
  secondsToLabel,
  timeLabelToSeconds,
  hzToLabel,
  freqLabelToHz,
  freqPosToHz,
  freqHzToPos,
  formatLinearAxisTooltip,
  formatFrequencyAxisTooltip,
  formatTimeAxisTooltip,
  formatNumberLabel,
};
