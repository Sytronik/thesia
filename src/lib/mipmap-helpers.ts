import {FreqScale} from "../api";

// Mel scale 변환 함수
function melFromHz(hz: number): number {
  return 2595 * Math.log10(1 + hz / 700);
}

function hzToRelativeFreq(freqScale: FreqScale, hz: number, hzRange: [number, number]): number {
  if (freqScale === "Linear") {
    return (hz - hzRange[0]) / (hzRange[1] - hzRange[0]);
  } else {
    // Mel
    const melRange: [number, number] = [melFromHz(hzRange[0]), melFromHz(hzRange[1])];
    return (melFromHz(hz) - melRange[0]) / (melRange[1] - melRange[0]);
  }
}

function addPrePostMargin(
  start: number,
  length: number,
  maxLength: number,
  margin: number,
): [number, number, number, number] {
  const startWMargin = Math.trunc(start) - margin;

  const lenWMargin = Math.max(Math.trunc(Math.ceil(start + length)) + margin - startWMargin, 0);

  const startWMarginClipped = Math.max(startWMargin, 0);
  const lenWMarginClipped = Math.min(lenWMargin, maxLength - startWMarginClipped);

  const preMargin = start - startWMarginClipped;
  const postMargin = lenWMarginClipped - length;

  return [startWMarginClipped, lenWMarginClipped, preMargin, postMargin];
}

export interface SpectrogramSliceArgs {
  pxPerSec: number;
  left: number;
  width: number;
  top: number;
  height: number;
  leftMargin: number;
  rightMargin: number;
  topMargin: number;
  bottomMargin: number;
}

export function createSpectrogramSliceArgs(
  nFrames: number,
  nFreqs: number,
  trackSec: number,
  secRange: [number, number],
  specHzRange: [number, number],
  hzRange: [number, number],
  marginPx: number,
  freqScale: FreqScale,
): SpectrogramSliceArgs {
  const hzRangeClamped = [hzRange[0], Math.min(hzRange[1], specHzRange[1])];
  const pxPerSec = nFrames / trackSec;
  const leftF64 = secRange[0] * pxPerSec;
  const widthF64 = Math.max((secRange[1] - secRange[0]) * pxPerSec, 0);

  const [leftWMarginClipped, widthWMarginClipped, leftMargin, rightMargin] = addPrePostMargin(
    leftF64,
    widthF64,
    nFrames,
    marginPx,
  );

  const topF64 = hzToRelativeFreq(freqScale, hzRangeClamped[0], specHzRange) * nFreqs;
  const bottomF64 = hzToRelativeFreq(freqScale, hzRangeClamped[1], specHzRange) * nFreqs;
  const heightF64 = bottomF64 - topF64;

  const [topWMarginClipped, heightWMarginClipped, topMargin, bottomMargin] = addPrePostMargin(
    topF64,
    heightF64,
    nFreqs,
    marginPx,
  );

  return {
    pxPerSec,
    left: leftWMarginClipped,
    width: widthWMarginClipped,
    top: topWMarginClipped,
    height: heightWMarginClipped,
    leftMargin,
    rightMargin,
    topMargin,
    bottomMargin,
  };
}

export function createMipmapSizeArr(
  origWidth: number,
  origHeight: number,
  maxSize: number,
): [number, number][][] {
  const mipmaps: [number, number][][] = [[[origWidth, origHeight]]];
  let skip = true;
  let height = origHeight;

  while (true) {
    if (!skip) mipmaps.push([]);

    const heightRounded = Math.round(height);
    let width = origWidth;

    while (true) {
      const widthRounded = Math.round(width);
      if (skip) skip = false;
      else mipmaps[mipmaps.length - 1].push([widthRounded, heightRounded]);

      if (widthRounded <= maxSize) break;

      width /= 2;
      if (widthRounded < maxSize) width = maxSize;
    }

    if (heightRounded <= maxSize) break;

    height /= 2;
    if (heightRounded < maxSize) height = maxSize;
  }

  return mipmaps;
}

export function calcMipmapSize(
  mipmapSizeArr: [number, number][][],
  trackSec: number,
  secRange: [number, number],
  specHzRange: [number, number],
  hzRange: [number, number],
  marginPx: number,
  freqScale: FreqScale,
  maxSize: number,
): [number, number] | null {
  for (const mipmapsAlongWidth of mipmapSizeArr) {
    for (const [width, height] of mipmapsAlongWidth) {
      const args = createSpectrogramSliceArgs(
        width,
        height,
        trackSec,
        secRange,
        specHzRange,
        hzRange,
        marginPx,
        freqScale,
      );
      if (args.height > maxSize) {
        break;
      }
      if (args.width <= maxSize) {
        return [width, height];
      }
    }
  }
  return null;
}
