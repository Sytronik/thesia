import { Container, Mesh, MeshGeometry, Texture } from "pixi.js";

import { AudioRenderMetadata } from "../api";
import { WaveformTile } from "./audio-render-tiles";

export const WAV_BORDER_COLOR = 0x000000;
export const WAV_COLOR = 0x1389eb;
export const WAV_CLIPPING_COLOR = 0xc42232;
export const WAV_IMG_SCALE = 2;
export const WAV_LINE_WIDTH = 1.75;
export const WAV_BORDER_WIDTH = 0.75;

const WAV_CAP_SEGMENTS = 12;
const WAV_JOIN_DOT_THRESHOLD = 0.9975;
const WAV_JOIN_MIN_X_DELTA = 0.25;

type WaveformEnvelopeMesh = {
  xs: number[];
  tops: number[];
  bottoms: number[];
};

export type WaveformRenderOptions = {
  layer: Container;
  tiles: WaveformTile[];
  metadata: AudioRenderMetadata;
  y: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  width: number;
  ampRange: [number, number];
  color: number;
  clampValues: boolean;
  needLineBorder: boolean;
  needEnvelopeBorder: boolean;
  lineWidth?: number;
  borderWidth?: number;
};

export const waveformKey = (
  idChStr: string,
  revision: number,
  level: number,
  tileIndex: number,
) => `w:${idChStr}:${revision}:${level}:${tileIndex}`;

export const waveformLevel = (sampleRate: number, pxPerSec: number, devicePixelRatio: number) => {
  const internalPxPerSec = pxPerSec * WAV_IMG_SCALE * devicePixelRatio;
  if (internalPxPerSec >= sampleRate / 2) return 0;

  const samplesPerDevicePixel = sampleRate / Math.max(pxPerSec * devicePixelRatio, 1e-8);
  return Math.max(0, Math.ceil(Math.log2(Math.max(samplesPerDevicePixel, 1))));
};

export const waveformTileRange = (
  metadata: AudioRenderMetadata,
  level: number,
  startSec: number,
  endSec: number,
) => {
  const samplesPerBin = 2 ** level;
  const tileSpanSec = (metadata.waveformTileBins * samplesPerBin) / metadata.sampleRate;
  const maxTile = Math.max(
    Math.ceil(metadata.sampleCount / (metadata.waveformTileBins * samplesPerBin)) - 1,
    0,
  );
  return {
    firstTile: Math.max(Math.floor(startSec / tileSpanSec) - 1, 0),
    lastTile: Math.min(Math.floor(endSec / tileSpanSec) + 1, maxTile),
  };
};

export const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);

function appendCircleMesh(
  positions: number[],
  indices: number[],
  x: number,
  y: number,
  radius: number,
) {
  if (radius <= 0) return;
  const centerIndex = positions.length / 2;
  positions.push(x, y);
  for (let i = 0; i < WAV_CAP_SEGMENTS; i += 1) {
    const angle = (i / WAV_CAP_SEGMENTS) * Math.PI * 2;
    positions.push(x + Math.cos(angle) * radius, y + Math.sin(angle) * radius);
  }
  for (let i = 0; i < WAV_CAP_SEGMENTS; i += 1) {
    indices.push(centerIndex, centerIndex + 1 + i, centerIndex + 1 + ((i + 1) % WAV_CAP_SEGMENTS));
  }
}

function appendLinePathMesh(
  positions: number[],
  indices: number[],
  points: number[],
  strokeWidth: number,
) {
  if (points.length < 2) return;
  const halfWidth = strokeWidth * 0.5;
  let firstCap: [number, number] | null = null;
  let lastCap: [number, number] | null = null;
  for (let i = 0; i + 3 < points.length; i += 2) {
    const x0 = points[i];
    const y0 = points[i + 1];
    const x1 = points[i + 2];
    const y1 = points[i + 3];
    const dx = x1 - x0;
    const dy = y1 - y0;
    const length = Math.hypot(dx, dy);
    if (length < 1e-6) continue;
    const nx = (-dy / length) * halfWidth;
    const ny = (dx / length) * halfWidth;
    const base = positions.length / 2;
    positions.push(x0 + nx, y0 + ny, x0 - nx, y0 - ny, x1 + nx, y1 + ny, x1 - nx, y1 - ny);
    indices.push(base, base + 1, base + 2, base + 1, base + 3, base + 2);
    firstCap ??= [x0, y0];
    lastCap = [x1, y1];
  }

  const pointCount = points.length / 2;
  for (let i = 1; i < pointCount - 1; i += 1) {
    const prevX = points[(i - 1) * 2];
    const prevY = points[(i - 1) * 2 + 1];
    const x = points[i * 2];
    const y = points[i * 2 + 1];
    const nextX = points[(i + 1) * 2];
    const nextY = points[(i + 1) * 2 + 1];
    const prevDx = x - prevX;
    const prevDy = y - prevY;
    const nextDx = nextX - x;
    const nextDy = nextY - y;
    const prevLength = Math.hypot(prevDx, prevDy);
    const nextLength = Math.hypot(nextDx, nextDy);
    if (prevLength < 1e-6 || nextLength < 1e-6) continue;
    if (
      Math.max(Math.abs(prevDx), Math.abs(nextDx)) < WAV_JOIN_MIN_X_DELTA &&
      Math.max(prevLength, nextLength) < halfWidth * 2
    )
      continue;
    const dot = (prevDx * nextDx + prevDy * nextDy) / (prevLength * nextLength);
    if (dot > WAV_JOIN_DOT_THRESHOLD) continue;
    appendCircleMesh(positions, indices, x, y, halfWidth);
  }

  if (firstCap && lastCap) {
    appendCircleMesh(positions, indices, firstCap[0], firstCap[1], halfWidth);
    appendCircleMesh(positions, indices, lastCap[0], lastCap[1], halfWidth);
  } else {
    appendCircleMesh(positions, indices, points[0], points[1], halfWidth);
  }
}

function addSolidMesh(layer: Container, positions: number[], indices: number[], color: number) {
  if (positions.length < 6 || indices.length < 3) return 0;
  const geometry = new MeshGeometry({
    positions: new Float32Array(positions),
    indices: new Uint32Array(indices),
    shrinkBuffersToFit: true,
  });
  const mesh = new Mesh({ geometry, texture: Texture.WHITE, tint: color });
  layer.addChild(mesh);
  return positions.length / 2;
}

function addLineMesh(layer: Container, paths: number[][], color: number, strokeWidth: number) {
  const positions: number[] = [];
  const indices: number[] = [];
  paths.forEach((points) => appendLinePathMesh(positions, indices, points, strokeWidth));
  return addSolidMesh(layer, positions, indices, color);
}

function addEnvelopeFillMesh(layer: Container, envelope: WaveformEnvelopeMesh, color: number) {
  if (envelope.xs.length < 2) return 0;
  const positions: number[] = [];
  const indices: number[] = [];
  envelope.xs.forEach((x, i) => {
    positions.push(x, envelope.tops[i], x, envelope.bottoms[i]);
    if (i < envelope.xs.length - 1) {
      const base = i * 2;
      indices.push(base, base + 1, base + 2, base + 1, base + 3, base + 2);
    }
  });
  return addSolidMesh(layer, positions, indices, color);
}

function addEnvelopeBorderMesh(
  layer: Container,
  envelope: WaveformEnvelopeMesh,
  color: number,
  strokeWidth: number,
) {
  const last = envelope.xs.length - 1;
  if (last < 1) return 0;
  const topPath: number[] = [];
  const bottomPath: number[] = [];
  envelope.xs.forEach((x, i) => {
    topPath.push(x, envelope.tops[i]);
    bottomPath.push(x, envelope.bottoms[i]);
  });
  return addLineMesh(
    layer,
    [
      topPath,
      bottomPath,
      [envelope.xs[0], envelope.tops[0], envelope.xs[0], envelope.bottoms[0]],
      [envelope.xs[last], envelope.tops[last], envelope.xs[last], envelope.bottoms[last]],
    ],
    color,
    strokeWidth,
  );
}

export function destroyPixiChildren(layer: Container) {
  const destroyTree = (node: Container) => {
    node.removeChildren().forEach(destroyTree);
    const geometry = node instanceof Mesh ? node.geometry : null;
    node.destroy();
    geometry?.destroy(true);
  };
  layer.removeChildren().forEach(destroyTree);
}

export function renderWaveformTiles({
  layer,
  tiles,
  metadata,
  y,
  height,
  startSec,
  pxPerSec,
  width,
  ampRange,
  color,
  clampValues,
  needLineBorder,
  needEnvelopeBorder,
  lineWidth = WAV_LINE_WIDTH,
  borderWidth = WAV_BORDER_WIDTH,
}: WaveformRenderOptions) {
  if (tiles.length === 0 || pxPerSec <= 0 || width <= 0 || height <= 0) return 0;
  let vertices = 0;
  const toY = (value: number) => {
    const normalizedValue = clampValues ? clamp(value, -1, 1) : value;
    return y + ((ampRange[1] - normalizedValue) / Math.max(ampRange[1] - ampRange[0], 1e-8)) * height;
  };
  const toX = (sample: number) => (sample / metadata.sampleRate - startSec) * pxPerSec;
  const visibleStartSample = Math.max(startSec * metadata.sampleRate, 0);
  const visibleEndSample = Math.min((startSec + width / pxPerSec) * metadata.sampleRate, metadata.sampleCount);
  if (visibleEndSample <= visibleStartSample) return 0;
  const getVisibleBinRange = (tile: WaveformTile, overscanBins: number) => {
    const tileFirstSample = tile.tileIndex * metadata.waveformTileBins * tile.samplesPerBin;
    const firstBin = Math.max(
      Math.floor((visibleStartSample - tileFirstSample) / tile.samplesPerBin) - overscanBins,
      0,
    );
    const lastBin = Math.min(
      Math.ceil((visibleEndSample - tileFirstSample) / tile.samplesPerBin) + overscanBins,
      tile.binCount,
    );
    return [firstBin, Math.max(firstBin, lastBin)] as const;
  };

  type WaveformBin = {
    firstSample: number;
    lastSample: number;
    centerSample: number;
    min: number;
    max: number;
    representative: number;
  };
  const getBins = (segment: WaveformTile[], overscanBins: number) => {
    const bins: WaveformBin[] = [];
    segment.forEach((tile) => {
      const tileFirstSample = tile.tileIndex * metadata.waveformTileBins * tile.samplesPerBin;
      const [firstBin, lastBin] = getVisibleBinRange(tile, overscanBins);
      for (let i = firstBin; i < lastBin; i += 1) {
        const firstSample = tileFirstSample + i * tile.samplesPerBin;
        const lastSample = Math.min(firstSample + tile.samplesPerBin, metadata.sampleCount);
        bins.push({
          firstSample,
          lastSample,
          centerSample: firstSample + (lastSample - firstSample) * 0.5,
          min: tile.min[i],
          max: tile.max[i],
          representative: tile.representative[i],
        });
      }
    });
    return bins;
  };
  const drawLine = (points: number[], strokeColor: number, strokeWidth: number) => {
    vertices += addLineMesh(layer, [points], strokeColor, strokeWidth);
  };
  const drawEnvelope = (
    envelope: WaveformEnvelopeMesh,
    fillColor: number,
    drawBorder: boolean,
  ) => {
    if (drawBorder) {
      vertices += addEnvelopeBorderMesh(layer, envelope, WAV_BORDER_COLOR, borderWidth * 2);
    }
    vertices += addEnvelopeFillMesh(layer, envelope, fillColor);
  };
  const contiguousSegments: WaveformTile[][] = [];
  tiles.forEach((tile) => {
    const segment = contiguousSegments[contiguousSegments.length - 1];
    if (!segment || segment[segment.length - 1].tileIndex + 1 !== tile.tileIndex) {
      contiguousSegments.push([tile]);
    } else {
      segment.push(tile);
    }
  });

  const samplesPerBin = tiles[0].samplesPerBin;
  if (samplesPerBin === 1) {
    contiguousSegments.forEach((segment) => {
      const linePoints = getBins(segment, 1).flatMap(({ firstSample, representative }) => [
        toX(firstSample),
        toY(representative),
      ]);
      if (needLineBorder) {
        drawLine(linePoints, WAV_BORDER_COLOR, lineWidth + borderWidth * 2);
      }
      drawLine(linePoints, color, lineWidth);
    });
    return vertices;
  }

  contiguousSegments.forEach((segment) => {
    const bins = getBins(segment, 2);
    if (bins.length === 0) return;

    const linePoints: number[] = [];
    const envelopes: WaveformEnvelopeMesh[] = [];
    let envelopeXs: number[] = [];
    let envelopeTops: number[] = [];
    let envelopeBottoms: number[] = [];
    const finishEnvelope = () => {
      if (envelopeXs.length === 0) return;
      const halfLineWidth = lineWidth * 0.5;
      envelopes.push({
        xs: envelopeXs,
        tops: envelopeTops.map((value) => value - halfLineWidth),
        bottoms: envelopeBottoms.map((value) => value + halfLineWidth),
      });
      envelopeXs = [];
      envelopeTops = [];
      envelopeBottoms = [];
    };

    bins.forEach((bin, i) => {
      const nextBin = bins[Math.min(i + 1, bins.length - 1)];
      const xStart = toX(bin.firstSample);
      const xMid = toX(bin.centerSample);
      const sampleY = toY(bin.representative);
      const top = toY(Math.max(bin.max, nextBin.max));
      const bottom = toY(Math.min(bin.min, nextBin.min));
      const previousY = i > 0 ? toY(bins[i - 1].representative) : sampleY;

      if (bottom - top > lineWidth * 0.5) {
        if (envelopeXs.length === 0) {
          envelopeXs.push(xStart);
          envelopeTops.push(previousY);
          envelopeBottoms.push(previousY);
          linePoints.push(xMid, sampleY);
        }

        envelopeXs.push(xMid);
        envelopeTops.push(top);
        envelopeBottoms.push(bottom);
        linePoints.push(xMid, (top + bottom) * 0.5);
      } else {
        if (envelopeXs.length > 0) {
          envelopeXs.push(xStart);
          envelopeTops.push(sampleY);
          envelopeBottoms.push(sampleY);
          finishEnvelope();
          linePoints.push(xStart, previousY);
        }

        linePoints.push(xMid, (top + bottom) * 0.5);
      }
    });

    if (envelopeXs.length > 0) {
      const lastBin = bins[bins.length - 1];
      const lastY = toY(lastBin.representative);
      envelopeXs.push(toX(lastBin.lastSample));
      envelopeTops.push(lastY);
      envelopeBottoms.push(lastY);
      finishEnvelope();
      linePoints.push(toX(lastBin.centerSample), lastY);
    }

    if (needLineBorder) {
      drawLine(linePoints, WAV_BORDER_COLOR, lineWidth + borderWidth * 2);
    }
    drawLine(linePoints, color, lineWidth);
    envelopes.forEach((envelope) => drawEnvelope(envelope, color, needEnvelopeBorder));
  });
  return vertices;
}
