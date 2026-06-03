import {
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Application, Container, Graphics, Mesh, MeshGeometry, Sprite, Texture } from "pixi.js";
import useEvent from "react-use-event-hook";

import BackendAPI, { AudioRenderMetadata, FreqScale, WasmAPI } from "../api";
import { DevicePixelRatioContext } from "../contexts";
import {
  decodeSpectrogramTile,
  decodeWaveformTile,
  GpuTextureCache,
  WaveformTileCache,
  WaveformTile,
} from "../lib/audio-render-tiles";
import {
  TIME_CANVAS_HEIGHT,
  TINY_MARGIN,
  VERTICAL_AXIS_PADDING,
} from "../prototypes/constants/tracks";
import styles from "./AudioTrackViewport.module.scss";

const GPU_TEXTURE_BUDGET_BYTES = 256 * 1024 * 1024;
const WAVEFORM_TILE_BUDGET_BYTES = 64 * 1024 * 1024;
const HEADER_HEIGHT = TIME_CANVAS_HEIGHT + TINY_MARGIN;
const WAV_BORDER_COLOR = 0x000000;
const WAV_COLOR = 0x1389eb;
const WAV_CLIPPING_COLOR = 0xc42232;
const WAV_IMG_SCALE = 2;
const WAV_LINE_WIDTH = 1.75;
const WAV_BORDER_WIDTH = 0.75;
const WAV_CAP_SEGMENTS = 12;
const WAV_JOIN_DOT_THRESHOLD = 0.9975;
const WAV_JOIN_MIN_X_DELTA = 0.25;
const METADATA_RETRY_LIMIT = 20;
const METADATA_RETRY_DELAY_MS = 100;

export type AudioTrackViewportRow = {
  idChStr: string;
  trackId: number;
  top: number;
  hidden: boolean;
};

export type AudioTrackViewportRect = {
  left: number;
  top: number;
  width: number;
  height: number;
};

type Props = {
  rows: AudioTrackViewportRow[];
  getViewportRect: () => AudioTrackViewportRect | null;
  width: number;
  rowHeight: number;
  imageHeight: number;
  getScrollTop: () => number;
  startSec: number;
  pxPerSec: number;
  maxTrackHz: number;
  freqScale: FreqScale;
  hzRange: [number, number];
  ampRange: [number, number];
  blend: number;
  selectedTrackIds: number[];
  isPlaying: boolean;
  getPlayheadSec: () => number | null;
  refreshToken: string;
  layoutRevision: number;
};

type TooltipInfo = { left: number; top: number; lines: string[] };

const waveformKey = (idChStr: string, revision: number, level: number, tileIndex: number) =>
  `w:${idChStr}:${revision}:${level}:${tileIndex}`;
const spectrogramKey = (
  idChStr: string,
  revision: number,
  levelX: number,
  levelY: number,
  tileX: number,
  tileY: number,
) => `s:${idChStr}:${revision}:${levelX}:${levelY}:${tileX}:${tileY}`;
const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);
const log2Level = (scale: number) => Math.max(0, Math.floor(Math.log2(Math.max(scale, 1))));
const waveformLevel = (sampleRate: number, pxPerSec: number, devicePixelRatio: number) => {
  const internalPxPerSec = pxPerSec * WAV_IMG_SCALE * devicePixelRatio;
  if (internalPxPerSec >= sampleRate / 2) return 0;

  const samplesPerDevicePixel = sampleRate / Math.max(pxPerSec * devicePixelRatio, 1e-8);
  return Math.max(0, Math.ceil(Math.log2(Math.max(samplesPerDevicePixel, 1))));
};

type WaveformEnvelopeMesh = {
  xs: number[];
  tops: number[];
  bottoms: number[];
};

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

function addSolidMesh(
  layer: Container,
  positions: number[],
  indices: number[],
  color: number,
): number {
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

function addLineMesh(
  layer: Container,
  paths: number[][],
  color: number,
  strokeWidth: number,
): number {
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

function destroyChildren(layer: Container) {
  const destroyTree = (node: Container) => {
    node.removeChildren().forEach(destroyTree);
    const geometry = node instanceof Mesh ? node.geometry : null;
    node.destroy();
    geometry?.destroy(true);
  };
  layer.removeChildren().forEach(destroyTree);
}

function AudioTrackViewport(props: Props) {
  const {
    rows,
    getViewportRect,
    width,
    rowHeight,
    imageHeight,
    getScrollTop,
    startSec,
    pxPerSec,
    maxTrackHz,
    freqScale,
    hzRange,
    ampRange,
    blend,
    refreshToken,
    layoutRevision,
  } = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const host = useRef<HTMLDivElement>(null);
  const app = useRef<Application | null>(null);
  const rowLayer = useRef<Container | null>(null);
  const playheadLayer = useRef<Graphics | null>(null);
  const textureCache = useRef(new GpuTextureCache(GPU_TEXTURE_BUDGET_BYTES));
  const waveformTiles = useRef(new WaveformTileCache(WAVEFORM_TILE_BUDGET_BYTES));
  const pending = useRef(new Set<string>());
  const prevBounds = useRef<{ width: number; height: number } | null>(null);
  const metadataRef = useRef(new Map<string, AudioRenderMetadata>());
  const metadataRequestRevision = useRef(0);
  const metadataRetryCount = useRef(0);
  const tileRequestRevision = useRef(0);
  const latestProps = useRef(props);
  const frameTimes = useRef<number[]>([]);
  const prevFrame = useRef<number | null>(null);
  const cacheHits = useRef(0);
  const cacheMisses = useRef(0);
  const spectrogramTilesExpected = useRef(0);
  const spectrogramRequests = useRef(0);
  const spectrogramResponses = useRef(0);
  const spectrogramSprites = useRef(0);
  const spectrogramErrors = useRef(0);
  const spectrogramSkipReason = useRef<string | null>(null);
  const lastSpectrogramError = useRef<string | null>(null);
  const waveformVertices = useRef(0);
  const visibleRows = useRef(0);
  const visibleRowsKey = useRef("");
  const [metadata, setMetadata] = useState(new Map<string, AudioRenderMetadata>());
  const [sceneRevision, setSceneRevision] = useState(0);
  const [tooltip, setTooltip] = useState<TooltipInfo | null>(null);

  useLayoutEffect(() => {
    latestProps.current = props;
  }, [props]);
  useEffect(() => {
    metadataRef.current = metadata;
  }, [metadata]);

  const syncBounds = useEvent(() => {
    const rect = getViewportRect();
    const node = host.current;
    const pixi = app.current;
    if (!rect || !node || !pixi) return;
    if (rect.width <= 0 || rect.height <= 0) {
      node.style.display = "none";
      return;
    }
    node.style.display = "block";
    node.style.left = `${rect.left}px`;
    node.style.top = `${rect.top}px`;
    node.style.width = `${rect.width}px`;
    node.style.height = `${rect.height}px`;
    if (prevBounds.current?.width !== rect.width || prevBounds.current?.height !== rect.height) {
      pixi.renderer.resize(rect.width, rect.height, devicePixelRatio);
      prevBounds.current = { width: rect.width, height: rect.height };
    }
  });

  useEffect(() => {
    let disposed = false;
    const pixi = new Application();
    const textures = textureCache.current;
    const wavTiles = waveformTiles.current;
    const requests = pending.current;
    void pixi
      .init({
        width: 1,
        height: 1,
        preference: "webgl",
        preferWebGLVersion: 2,
        antialias: true,
        autoDensity: true,
        resolution: devicePixelRatio,
        backgroundAlpha: 0,
        autoStart: false,
      })
      .then(() => {
        if (disposed || !host.current) {
          pixi.destroy(true, { children: true });
          return;
        }
        const rowsContainer = new Container();
        const playhead = new Graphics();
        pixi.stage.addChild(rowsContainer, playhead);
        host.current.appendChild(pixi.canvas);
        app.current = pixi;
        rowLayer.current = rowsContainer;
        playheadLayer.current = playhead;
        prevBounds.current = null;
        syncBounds();
        setSceneRevision((value) => value + 1);
      });
    return () => {
      disposed = true;
      tileRequestRevision.current += 1;
      textures.clear();
      wavTiles.clear();
      requests.clear();
      if (app.current === pixi) {
        if (rowLayer.current) destroyChildren(rowLayer.current);
        app.current = null;
        rowLayer.current = null;
        playheadLayer.current = null;
      }
      pixi.destroy(true, { children: true });
      textures.destroyRetired();
    };
  }, [devicePixelRatio, syncBounds]);

  useLayoutEffect(syncBounds, [layoutRevision, rowHeight, syncBounds, width]);

  const rowIdsKey = useMemo(() => rows.map(({ idChStr }) => idChStr).join(","), [rows]);
  const prevRowIdsKey = useRef<string | null>(null);
  const refreshMetadata = useEvent(() => {
    const rowIds = rowIdsKey === "" ? [] : rowIdsKey.split(",");
    const requestRevision = ++metadataRequestRevision.current;
    void Promise.all(
      rowIds.map(
        async (idChStr) => [idChStr, await BackendAPI.getAudioRenderMetadata(idChStr)] as const,
      ),
    )
      .then((entries) => {
        if (requestRevision !== metadataRequestRevision.current) return;
        const next = new Map<string, AudioRenderMetadata>();
        entries.forEach(([idChStr, value]) => {
          if (value) next.set(idChStr, value);
        });
        textureCache.current.clear();
        waveformTiles.current.clear();
        pending.current.clear();
        tileRequestRevision.current += 1;
        metadataRef.current = next;
        setMetadata(next);
      })
      .catch((error) => console.error("Failed to fetch audio render metadata", error));
  });
  useEffect(() => {
    const rowsChanged = prevRowIdsKey.current !== rowIdsKey;
    prevRowIdsKey.current = rowIdsKey;
    if (!rowsChanged && refreshToken.length === 0 && metadataRef.current.size > 0) return;
    metadataRetryCount.current = 0;
    refreshMetadata();
  }, [refreshMetadata, refreshToken, rowIdsKey]);
  useEffect(() => {
    if (rows.length === 0 || maxTrackHz <= 0) {
      metadataRetryCount.current = 0;
      return;
    }
    const hasMissingSpectrogram = rows.some((row) => {
      const rowMetadata = metadataRef.current.get(row.idChStr);
      return (
        !rowMetadata || rowMetadata.spectrogramWidth === 0 || rowMetadata.spectrogramHeight === 0
      );
    });
    if (!hasMissingSpectrogram) {
      metadataRetryCount.current = 0;
      return;
    }
    if (metadataRetryCount.current >= METADATA_RETRY_LIMIT) return;
    const timeout = window.setTimeout(() => {
      metadataRetryCount.current += 1;
      refreshMetadata();
    }, METADATA_RETRY_DELAY_MS);
    return () => window.clearTimeout(timeout);
  }, [maxTrackHz, metadata, refreshMetadata, rows]);
  useEffect(() => {
    metadataRetryCount.current = 0;
  }, [refreshToken, rowIdsKey]);
  useEffect(
    () => () => {
      metadataRequestRevision.current += 1;
    },
    [],
  );

  const requestWaveformTile = useEvent(
    (idChStr: string, rowMetadata: AudioRenderMetadata, level: number, tileIndex: number) => {
      const key = waveformKey(idChStr, rowMetadata.waveformRevision, level, tileIndex);
      if (waveformTiles.current.get(key)) {
        cacheHits.current += 1;
        return;
      }
      if (pending.current.has(key)) return;
      cacheMisses.current += 1;
      pending.current.add(key);
      const requestRevision = tileRequestRevision.current;
      void BackendAPI.getWaveformTile(idChStr, level, tileIndex)
        .then((value) => {
          if (requestRevision !== tileRequestRevision.current) return;
          const tile = decodeWaveformTile(value);
          if (metadataRef.current.get(idChStr)?.waveformRevision !== tile.revision) return;
          waveformTiles.current.set(key, tile);
          setSceneRevision((revision) => revision + 1);
        })
        .catch((error) => console.error("Failed to fetch waveform tile", error))
        .finally(() => pending.current.delete(key));
    },
  );

  const requestSpectrogramTile = useEvent(
    (
      idChStr: string,
      rowMetadata: AudioRenderMetadata,
      levelX: number,
      levelY: number,
      tileX: number,
      tileY: number,
    ) => {
      const key = spectrogramKey(
        idChStr,
        rowMetadata.spectrogramRevision,
        levelX,
        levelY,
        tileX,
        tileY,
      );
      if (textureCache.current.get(key)) {
        cacheHits.current += 1;
        return;
      }
      if (pending.current.has(key)) return;
      cacheMisses.current += 1;
      pending.current.add(key);
      spectrogramRequests.current += 1;
      const requestRevision = tileRequestRevision.current;
      void BackendAPI.getSpectrogramTile(idChStr, levelX, levelY, tileX, tileY)
        .then((value) => {
          spectrogramResponses.current += 1;
          if (requestRevision !== tileRequestRevision.current) return;
          const tile = decodeSpectrogramTile(value);
          if (metadataRef.current.get(idChStr)?.spectrogramRevision !== tile.revision) return;
          if (tile.width === 0 || tile.height === 0) return;
          textureCache.current.set(key, tile);
          setSceneRevision((revision) => revision + 1);
        })
        .catch((error) => {
          spectrogramErrors.current += 1;
          lastSpectrogramError.current = String(error);
          console.error("Failed to fetch spectrogram tile", error);
        })
        .finally(() => pending.current.delete(key));
    },
  );

  const drawWaveformTiles = useCallback(
    (
      layer: Container,
      tiles: WaveformTile[],
      rowMetadata: AudioRenderMetadata,
      rowY: number,
      color: number,
      clampValues: boolean,
      needLineBorder: boolean,
      needEnvelopeBorder: boolean,
    ) => {
      if (tiles.length === 0) return;
      const toY = (value: number) => {
        const normalizedValue = clampValues ? clamp(value, -1, 1) : value;
        return (
          rowY +
          ((ampRange[1] - normalizedValue) / Math.max(ampRange[1] - ampRange[0], 1e-8)) *
            imageHeight
        );
      };
      const toX = (sample: number) => (sample / rowMetadata.sampleRate - startSec) * pxPerSec;
      const visibleStartSample = Math.max(startSec * rowMetadata.sampleRate, 0);
      const visibleEndSample = Math.min(
        (startSec + width / pxPerSec) * rowMetadata.sampleRate,
        rowMetadata.sampleCount,
      );
      if (visibleEndSample <= visibleStartSample) return;
      const getVisibleBinRange = (tile: WaveformTile, overscanBins: number) => {
        const tileFirstSample = tile.tileIndex * rowMetadata.waveformTileBins * tile.samplesPerBin;
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
          const tileFirstSample =
            tile.tileIndex * rowMetadata.waveformTileBins * tile.samplesPerBin;
          const [firstBin, lastBin] = getVisibleBinRange(tile, overscanBins);
          for (let i = firstBin; i < lastBin; i += 1) {
            const firstSample = tileFirstSample + i * tile.samplesPerBin;
            const lastSample = Math.min(firstSample + tile.samplesPerBin, rowMetadata.sampleCount);
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
        waveformVertices.current += addLineMesh(layer, [points], strokeColor, strokeWidth);
      };
      const drawEnvelope = (
        envelope: WaveformEnvelopeMesh,
        fillColor: number,
        drawBorder: boolean,
      ) => {
        if (drawBorder) {
          waveformVertices.current += addEnvelopeBorderMesh(
            layer,
            envelope,
            WAV_BORDER_COLOR,
            WAV_BORDER_WIDTH * 2,
          );
        }
        waveformVertices.current += addEnvelopeFillMesh(layer, envelope, fillColor);
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
            drawLine(linePoints, WAV_BORDER_COLOR, WAV_LINE_WIDTH + WAV_BORDER_WIDTH * 2);
          }
          drawLine(linePoints, color, WAV_LINE_WIDTH);
        });
        return;
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
          const halfLineWidth = WAV_LINE_WIDTH * 0.5;
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
          const y = toY(bin.representative);
          const top = toY(Math.max(bin.max, nextBin.max));
          const bottom = toY(Math.min(bin.min, nextBin.min));
          const previousY = i > 0 ? toY(bins[i - 1].representative) : y;

          if (bottom - top > WAV_LINE_WIDTH * 0.5) {
            if (envelopeXs.length === 0) {
              envelopeXs.push(xStart);
              envelopeTops.push(previousY);
              envelopeBottoms.push(previousY);
              linePoints.push(xMid, y);
            }

            envelopeXs.push(xMid);
            envelopeTops.push(top);
            envelopeBottoms.push(bottom);
            linePoints.push(xMid, (top + bottom) * 0.5);
          } else {
            if (envelopeXs.length > 0) {
              envelopeXs.push(xStart);
              envelopeTops.push(y);
              envelopeBottoms.push(y);
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
          drawLine(linePoints, WAV_BORDER_COLOR, WAV_LINE_WIDTH + WAV_BORDER_WIDTH * 2);
        }
        envelopes.forEach((envelope) => drawEnvelope(envelope, color, needEnvelopeBorder));
        drawLine(linePoints, color, WAV_LINE_WIDTH);
      });
    },
    [ampRange, imageHeight, pxPerSec, startSec, width],
  );

  const drawSpectrogram = useEvent(
    (
      layer: Container,
      row: AudioTrackViewportRow,
      rowMetadata: AudioRenderMetadata,
      rowY: number,
    ) => {
      if (
        blend <= 0 ||
        maxTrackHz <= 0 ||
        rowMetadata.spectrogramWidth === 0 ||
        rowMetadata.spectrogramHeight === 0
      ) {
        spectrogramSkipReason.current = "spectrogram-disabled-or-missing-metadata";
        return;
      }
      const minHz = Math.max(hzRange[0], 0);
      const maxHz = Math.min(hzRange[1], maxTrackHz);
      if (!Number.isFinite(minHz) || !Number.isFinite(maxHz) || maxHz <= minHz) {
        spectrogramSkipReason.current = "invalid-frequency-range";
        return;
      }
      const basePxPerSec = rowMetadata.spectrogramWidth / Math.max(rowMetadata.trackSec, 1e-8);
      const levelX = log2Level(basePxPerSec / pxPerSec);
      const levelY = log2Level(rowMetadata.spectrogramHeight / Math.max(imageHeight, 1));
      const scaleX = 2 ** levelX;
      const scaleY = 2 ** levelY;
      const tileSize = rowMetadata.spectrogramTileSize;
      const lodWidth = Math.ceil(rowMetadata.spectrogramWidth / scaleX);
      const lodHeight = Math.ceil(rowMetadata.spectrogramHeight / scaleY);
      const maxTileX = Math.max(Math.ceil(lodWidth / tileSize) - 1, 0);
      const maxTileY = Math.max(Math.ceil(lodHeight / tileSize) - 1, 0);
      const sourceTop =
        rowMetadata.spectrogramHeight -
        WasmAPI.freqHzToPos(
          freqScale,
          minHz,
          rowMetadata.spectrogramHeight,
          0,
          maxTrackHz,
          maxTrackHz,
        );
      const sourceBottom =
        rowMetadata.spectrogramHeight -
        WasmAPI.freqHzToPos(
          freqScale,
          maxHz,
          rowMetadata.spectrogramHeight,
          0,
          maxTrackHz,
          maxTrackHz,
        );
      if (
        !Number.isFinite(sourceTop) ||
        !Number.isFinite(sourceBottom) ||
        sourceBottom <= sourceTop
      ) {
        spectrogramSkipReason.current = "invalid-spectrogram-source-range";
        return;
      }
      const sourceHeight = Math.max(sourceBottom - sourceTop, 1e-8);
      const firstTileX = Math.max(Math.floor((startSec * basePxPerSec) / scaleX / tileSize) - 1, 0);
      const lastTileX = Math.min(
        Math.floor(((startSec + width / pxPerSec) * basePxPerSec) / scaleX / tileSize) + 1,
        maxTileX,
      );
      const firstTileY = Math.max(Math.floor(sourceTop / scaleY / tileSize) - 1, 0);
      const lastTileY = Math.min(Math.floor(sourceBottom / scaleY / tileSize) + 1, maxTileY);
      for (let tileY = firstTileY; tileY <= lastTileY; tileY += 1) {
        for (let tileX = firstTileX; tileX <= lastTileX; tileX += 1) {
          spectrogramTilesExpected.current += 1;
          const key = spectrogramKey(
            row.idChStr,
            rowMetadata.spectrogramRevision,
            levelX,
            levelY,
            tileX,
            tileY,
          );
          const cachedTexture = textureCache.current.get(key);
          if (!cachedTexture) {
            requestSpectrogramTile(row.idChStr, rowMetadata, levelX, levelY, tileX, tileY);
            continue;
          }
          cacheHits.current += 1;
          const { texture, originX, originY } = cachedTexture;
          const sprite = new Sprite(texture);
          sprite.x = ((originX * scaleX) / basePxPerSec - startSec) * pxPerSec;
          sprite.y =
            rowY +
            ((sourceBottom - (originY + texture.height) * scaleY) / sourceHeight) * imageHeight;
          sprite.width = (texture.width * scaleX * pxPerSec) / basePxPerSec;
          sprite.height = (texture.height * scaleY * imageHeight) / sourceHeight;
          layer.addChild(sprite);
          spectrogramSprites.current += 1;
        }
      }
    },
  );

  const getVisibleRowsKey = useEvent(() => {
    const rect = getViewportRect();
    if (!rect) return "";
    const scrollTop = getScrollTop();
    return rows
      .filter((row) => {
        const rowY = HEADER_HEIGHT + row.top - scrollTop + VERTICAL_AXIS_PADDING;
        return !row.hidden && rowY + imageHeight >= -rowHeight && rowY <= rect.height + rowHeight;
      })
      .map(({ idChStr }) => idChStr)
      .join(",");
  });

  const redrawRows = useEvent(() => {
    const layer = rowLayer.current;
    const rect = getViewportRect();
    if (!layer || !rect) return;
    destroyChildren(layer);
    textureCache.current.destroyRetired();
    spectrogramTilesExpected.current = 0;
    spectrogramSprites.current = 0;
    spectrogramSkipReason.current = null;
    waveformVertices.current = 0;
    const scrollTop = getScrollTop();
    layer.y = HEADER_HEIGHT - scrollTop;
    let count = 0;
    rows.forEach((row) => {
      const rowY = row.top + VERTICAL_AXIS_PADDING;
      const viewportRowY = HEADER_HEIGHT + rowY - scrollTop;
      if (
        row.hidden ||
        viewportRowY + imageHeight < -rowHeight ||
        viewportRowY > rect.height + rowHeight
      )
        return;
      const rowMetadata = metadata.get(row.idChStr);
      if (!rowMetadata) return;
      count += 1;
      const trackStartX = clamp(-startSec * pxPerSec, 0, width);
      const trackEndX = clamp((rowMetadata.trackSec - startSec) * pxPerSec, 0, width);
      const trackVisibleWidth = Math.max(trackEndX - trackStartX, 0);
      if (trackVisibleWidth <= 0) return;
      const rowContainer = new Container();
      const rowMask = new Graphics()
        .rect(trackStartX, rowY, trackVisibleWidth, imageHeight)
        .fill({ color: 0xffffff });
      rowContainer.mask = rowMask;
      layer.addChild(rowContainer, rowMask);
      const background = new Graphics()
        .rect(trackStartX, rowY, trackVisibleWidth, imageHeight)
        .fill({ color: 0x000000 });
      rowContainer.addChild(background);
      drawSpectrogram(rowContainer, row, rowMetadata, rowY);
      if (blend < 0.5) {
        rowContainer.addChild(
          new Graphics()
            .rect(trackStartX, rowY, trackVisibleWidth, imageHeight)
            .fill({ color: 0x000000, alpha: Math.max(0, 1 - 2 * blend) }),
        );
      }
      const wavAlpha = blend < 0.5 ? 1 : Math.max(2 - 2 * blend, 0);
      if (wavAlpha <= 0) return;
      const wavLayer = new Container({ alpha: wavAlpha });
      const level = waveformLevel(rowMetadata.sampleRate, pxPerSec, devicePixelRatio);
      const samplesPerBin = 2 ** level;
      const tileSpanSec = (rowMetadata.waveformTileBins * samplesPerBin) / rowMetadata.sampleRate;
      const maxTile = Math.max(
        Math.ceil(rowMetadata.sampleCount / (rowMetadata.waveformTileBins * samplesPerBin)) - 1,
        0,
      );
      const firstTile = Math.max(Math.floor(startSec / tileSpanSec) - 1, 0);
      const lastTile = Math.min(
        Math.floor((startSec + width / pxPerSec) / tileSpanSec) + 1,
        maxTile,
      );
      const loadedTiles: WaveformTile[] = [];
      for (let tileIndex = firstTile; tileIndex <= lastTile; tileIndex += 1) {
        const key = waveformKey(row.idChStr, rowMetadata.waveformRevision, level, tileIndex);
        const tile = waveformTiles.current.get(key);
        if (!tile) {
          requestWaveformTile(row.idChStr, rowMetadata, level, tileIndex);
          continue;
        }
        cacheHits.current += 1;
        loadedTiles.push(tile);
      }
      if (rowMetadata.isClipped) {
        drawWaveformTiles(
          wavLayer,
          loadedTiles,
          rowMetadata,
          rowY,
          WAV_CLIPPING_COLOR,
          false,
          true,
          true,
        );
      }
      drawWaveformTiles(
        wavLayer,
        loadedTiles,
        rowMetadata,
        rowY,
        WAV_COLOR,
        rowMetadata.isClipped,
        true,
        !rowMetadata.isClipped,
      );
      rowContainer.addChild(wavLayer);
    });
    visibleRows.current = count;
    visibleRowsKey.current = getVisibleRowsKey();
  });

  useLayoutEffect(() => {
    syncBounds();
    redrawRows();
  }, [
    ampRange,
    blend,
    devicePixelRatio,
    drawSpectrogram,
    drawWaveformTiles,
    freqScale,
    hzRange,
    imageHeight,
    maxTrackHz,
    metadata,
    pxPerSec,
    redrawRows,
    rows,
    sceneRevision,
    startSec,
    syncBounds,
    width,
  ]);

  useEffect(() => {
    let requestId = 0;
    let disposed = false;
    const render = (timestamp: number) => {
      if (disposed) return;
      const pixi = app.current;
      const playhead = playheadLayer.current;
      const current = latestProps.current;
      if (pixi && playhead) {
        const currentScrollTop = current.getScrollTop();
        const rowsContainer = rowLayer.current;
        if (rowsContainer) rowsContainer.y = HEADER_HEIGHT - currentScrollTop;
        const nextVisibleRowsKey = getVisibleRowsKey();
        if (nextVisibleRowsKey !== visibleRowsKey.current) redrawRows();
        playhead.clear();
        const sec = current.isPlaying ? current.getPlayheadSec() : null;
        const selectedTrackId = current.selectedTrackIds[current.selectedTrackIds.length - 1];
        if (sec !== null && selectedTrackId !== undefined) {
          const selectedRows = current.rows.filter(({ trackId }) => trackId === selectedTrackId);
          if (selectedRows.length > 0) {
            const x = (sec - current.startSec) * current.pxPerSec + 0.5;
            const top =
              HEADER_HEIGHT + selectedRows[0].top - currentScrollTop + VERTICAL_AXIS_PADDING;
            const bottom =
              HEADER_HEIGHT +
              (selectedRows[selectedRows.length - 1]?.top ?? 0) -
              currentScrollTop +
              VERTICAL_AXIS_PADDING +
              current.imageHeight;
            playhead.moveTo(x, top).lineTo(x, bottom).stroke({ color: 0xdddddd, width: 1 });
          }
        }
        pixi.render();
      }
      if (prevFrame.current !== null) {
        frameTimes.current.push(timestamp - prevFrame.current);
        if (frameTimes.current.length > 120) frameTimes.current.shift();
      }
      prevFrame.current = timestamp;
      const sorted = [...frameTimes.current].sort((a, b) => a - b);
      const average = sorted.reduce((sum, value) => sum + value, 0) / Math.max(sorted.length, 1);
      if (import.meta.env.DEV) {
        window.__THESIA_RENDER_STATS__ = {
          ...window.__THESIA_RENDER_STATS__,
          fps: average > 0 ? 1000 / average : 0,
          frameTimeP95: sorted[Math.floor(sorted.length * 0.95)] ?? 0,
          visibleRows: visibleRows.current,
          pendingRequests: pending.current.size,
          gpuCacheBytes: textureCache.current.bytes,
          tileHits: cacheHits.current,
          tileMisses: cacheMisses.current,
          spectrogramMetadataRows: Array.from(metadataRef.current.values()).filter(
            ({ spectrogramWidth, spectrogramHeight }) =>
              spectrogramWidth > 0 && spectrogramHeight > 0,
          ).length,
          spectrogramTilesExpected: spectrogramTilesExpected.current,
          spectrogramRequests: spectrogramRequests.current,
          spectrogramResponses: spectrogramResponses.current,
          spectrogramSprites: spectrogramSprites.current,
          spectrogramErrors: spectrogramErrors.current,
          spectrogramSkipReason: spectrogramSkipReason.current,
          lastSpectrogramError: lastSpectrogramError.current,
          waveformVertices: waveformVertices.current,
          maxTrackHz: latestProps.current.maxTrackHz,
          blend: latestProps.current.blend,
        };
      }
      requestId = requestAnimationFrame(render);
    };
    requestId = requestAnimationFrame(render);
    return () => {
      disposed = true;
      cancelAnimationFrame(requestId);
    };
  }, [getVisibleRowsKey, redrawRows]);

  useEffect(() => {
    const onMouseMove = (event: MouseEvent) => {
      const rect = getViewportRect();
      if (!rect || event.clientX < rect.left || event.clientX > rect.left + width) {
        setTooltip(null);
        return;
      }
      const contentY = event.clientY - rect.top + getScrollTop() - HEADER_HEIGHT;
      const row = rows.find(
        (value) => contentY >= value.top && contentY < value.top + rowHeight && !value.hidden,
      );
      const y = contentY - (row?.top ?? 0) - VERTICAL_AXIS_PADDING;
      if (!row || y < 0 || y > imageHeight) {
        setTooltip(null);
        return;
      }
      const time = clamp(startSec + (event.clientX - rect.left) / pxPerSec, 0, Infinity);
      const hz = WasmAPI.freqPosToHz(freqScale, y, imageHeight, hzRange[0], hzRange[1], maxTrackHz);
      setTooltip({
        left: event.clientX,
        top: event.clientY + 15,
        lines: [`${time.toFixed(3)} sec`, `${hz.toFixed(0)} Hz`],
      });
    };
    document.addEventListener("mousemove", onMouseMove);
    return () => document.removeEventListener("mousemove", onMouseMove);
  }, [
    freqScale,
    getScrollTop,
    getViewportRect,
    hzRange,
    imageHeight,
    maxTrackHz,
    pxPerSec,
    rowHeight,
    rows,
    startSec,
    width,
  ]);

  return (
    <>
      <div ref={host} className={styles.viewport} />
      {tooltip ? (
        <span className={styles.tooltip} style={{ left: tooltip.left, top: tooltip.top }}>
          {tooltip.lines.map((line) => (
            <p key={line}>{line}</p>
          ))}
        </span>
      ) : null}
    </>
  );
}

export default AudioTrackViewport;
