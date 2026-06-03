import { useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Application, Container, Graphics } from "pixi.js";
import useEvent from "react-use-event-hook";

import BackendAPI, { AudioRenderMetadata } from "../../api";
import { DevicePixelRatioContext } from "../../contexts";
import { decodeWaveformTile, WaveformTile, WaveformTileCache } from "../../lib/audio-render-tiles";
import {
  destroyPixiChildren,
  renderWaveformTiles,
  waveformKey,
  waveformLevel,
  waveformTileRange,
  WAV_CLIPPING_COLOR,
  WAV_COLOR,
} from "../../lib/waveform-renderer";

const WAVEFORM_TILE_BUDGET_BYTES = 16 * 1024 * 1024;
const OVERVIEW_CH_GAP_HEIGHT = 1;
const OVERVIEW_GAIN_HEIGHT_RATIO = 0.2;
const OVERVIEW_LINE_WIDTH = 1;
const LIMITER_GAIN_COLOR = 0xda972e;
const OUT_TRACK_COLOR = 0x000000;
const OUT_TRACK_ALPHA = 0.2;

type Props = {
  trackId: number | null;
  idChArr: IdChArr;
  maxTrackSec: number;
  needRefresh: boolean;
  className: string;
};

type OverviewRow = {
  idChStr: string;
  metadata: AudioRenderMetadata;
  tiles: WaveformTile[];
  y: number;
  height: number;
  trackWidth: number;
};

function calcAmpRange(rows: OverviewRow[]): [number, number] {
  let min = -1;
  let max = 1;
  rows.forEach(({ tiles }) => {
    tiles.forEach((tile) => {
      for (let i = 0; i < tile.binCount; i += 1) {
        min = Math.min(min, tile.min[i]);
        max = Math.max(max, tile.max[i]);
      }
    });
  });
  return [min, max];
}

function calcLimiterGainEnvelopes(
  gainSeq: Float32Array,
  width: number,
  height: number,
  gainRange: [number, number],
) {
  if (gainSeq.length === 0 || width <= 0 || height <= 0) return [];
  const xScale = width / gainSeq.length;
  const idxToX = (i: number) => i * xScale;
  const yScale = -height / Math.max(gainRange[1] - gainRange[0], 1e-8);
  const yOffset = -gainRange[1] * yScale;
  const gainToY = (value: number) => value * yScale + yOffset;
  const yUnityGain = gainToY(gainRange[1]);
  const envelopes: [number, number][][] = [];
  let current: [number, number][] = [];
  let i = 0;

  while (i < gainSeq.length) {
    const x = idxToX(i);
    const xFloor = Math.floor(x);
    const xMid = xFloor + 0.5;
    let i2 = i;
    let iNext = gainSeq.length;
    while (i2 < gainSeq.length) {
      const x2Floor = Math.floor(idxToX(i2));
      if (x2Floor > xFloor && iNext === gainSeq.length) iNext = i2;
      if (x2Floor > xFloor + 1) break;
      i2 += 1;
    }
    if (i2 === i) i2 = Math.min(i + 1, gainSeq.length);

    let minGain = Infinity;
    for (let j = i; j < i2; j += 1) {
      minGain = Math.min(minGain, gainSeq[j]);
    }
    const bottom = gainToY(minGain);
    if (bottom > yUnityGain) {
      if (current.length === 0) current.push([xFloor, yUnityGain]);
      current.push([xMid, bottom]);
    } else if (current.length > 0) {
      current.push([xFloor, yUnityGain]);
      envelopes.push(current);
      current = [];
    }
    i = iNext;
  }

  if (current.length > 0) {
    const lastX = idxToX(gainSeq.length - 1);
    current.push([Math.floor(lastX) + 1, gainToY(gainSeq[gainSeq.length - 1])]);
    envelopes.push(current);
  }
  return envelopes;
}

function addFilledPath(layer: Container, points: [number, number][], color: number) {
  if (points.length < 2) return;
  const graphic = new Graphics();
  points.forEach(([x, y], i) => {
    if (i === 0) graphic.moveTo(x, y);
    else graphic.lineTo(x, y);
  });
  graphic.closePath().fill({ color });
  layer.addChild(graphic);
}

function drawLimiterGain(
  layer: Container,
  gainSeq: Float32Array,
  width: number,
  height: number,
  yAbove: number,
  yBelow: number,
) {
  calcLimiterGainEnvelopes(gainSeq, width, height, [0.5, 1]).forEach((envelope) => {
    addFilledPath(
      layer,
      envelope.map(([x, y]) => [x, y + yAbove]),
      LIMITER_GAIN_COLOR,
    );
    addFilledPath(
      layer,
      envelope.map(([x, y]) => [x, yBelow + height - y]),
      LIMITER_GAIN_COLOR,
    );
  });
}

function OverviewWaveformViewport({ trackId, idChArr, maxTrackSec, needRefresh, className }: Props) {
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const host = useRef<HTMLDivElement>(null);
  const app = useRef<Application | null>(null);
  const layer = useRef<Container | null>(null);
  const waveformTiles = useRef(new WaveformTileCache(WAVEFORM_TILE_BUDGET_BYTES));
  const pending = useRef(new Set<string>());
  const metadataRef = useRef(new Map<string, AudioRenderMetadata>());
  const metadataRequestRevision = useRef(0);
  const tileRequestRevision = useRef(0);
  const limiterRequestRevision = useRef(0);
  const limiterGainSeqRef = useRef<Float32Array | null>(null);
  const prevBounds = useRef<{ width: number; height: number } | null>(null);
  const loadedMetadataKey = useRef<string | null>(null);
  const pendingMetadataKey = useRef<string | null>(null);
  const loadedLimiterKey = useRef<string | null>(null);
  const pendingLimiterKey = useRef<string | null>(null);
  const lastRenderedTrackKey = useRef<string | null>(null);
  const lastRenderedBounds = useRef<{ width: number; height: number } | null>(null);
  const [metadata, setMetadata] = useState(new Map<string, AudioRenderMetadata>());
  const [sceneRevision, setSceneRevision] = useState(0);
  const [layoutRevision, setLayoutRevision] = useState(0);
  const [limiterRevision, setLimiterRevision] = useState(0);

  const idChKey = useMemo(() => idChArr.join(","), [idChArr]);

  const syncBounds = useEvent(() => {
    const node = host.current;
    const pixi = app.current;
    const waveformLayer = layer.current;
    if (!node || !pixi) return;
    const width = node.clientWidth;
    const height = node.clientHeight;
    if (width <= 0 || height <= 0) return;
    if (prevBounds.current?.width !== width || prevBounds.current?.height !== height) {
      pixi.renderer.resize(width, height, devicePixelRatio);
      prevBounds.current = { width, height };
      if (waveformLayer && waveformLayer.children.length > 0 && lastRenderedBounds.current) {
        waveformLayer.scale.set(
          width / Math.max(lastRenderedBounds.current.width, 1),
          height / Math.max(lastRenderedBounds.current.height, 1),
        );
        pixi.render();
      }
    }
  });

  useEffect(() => {
    let disposed = false;
    const pixi = new Application();
    const tiles = waveformTiles.current;
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
          pixi.destroy({ removeView: true }, { children: true });
          return;
        }
        const waveformLayer = new Container();
        pixi.stage.addChild(waveformLayer);
        host.current.appendChild(pixi.canvas);
        app.current = pixi;
        layer.current = waveformLayer;
        prevBounds.current = null;
        syncBounds();
        setSceneRevision((value) => value + 1);
      });
    return () => {
      disposed = true;
      tileRequestRevision.current += 1;
      tiles.clear();
      requests.clear();
      if (app.current === pixi) {
        if (layer.current) destroyPixiChildren(layer.current);
        app.current = null;
        layer.current = null;
      }
      pixi.destroy({ removeView: true }, { children: true });
    };
  }, [devicePixelRatio, syncBounds]);

  useEffect(() => {
    const node = host.current;
    if (!node) return;
    const resizeObserver = new ResizeObserver(() => {
      syncBounds();
      setLayoutRevision((value) => value + 1);
    });
    resizeObserver.observe(node);
    return () => resizeObserver.disconnect();
  }, [syncBounds]);

  useEffect(() => {
    const ids = idChKey === "" ? [] : idChKey.split(",");
    if (trackId === null || ids.length === 0) {
      const requestRevision = ++metadataRequestRevision.current;
      loadedMetadataKey.current = null;
      pendingMetadataKey.current = null;
      metadataRef.current = new Map();
      waveformTiles.current.clear();
      pending.current.clear();
      void Promise.resolve().then(() => {
        if (requestRevision === metadataRequestRevision.current) setMetadata(new Map());
      });
      return;
    }
    const metadataKey = `${trackId}:${idChKey}`;
    if (
      !needRefresh &&
      (loadedMetadataKey.current === metadataKey || pendingMetadataKey.current === metadataKey)
    )
      return;

    const requestRevision = ++metadataRequestRevision.current;
    pendingMetadataKey.current = metadataKey;
    void Promise.all(
      ids.map(
        async (idChStr) => [idChStr, await BackendAPI.getAudioRenderMetadata(idChStr)] as const,
      ),
    )
      .then((entries) => {
        if (requestRevision !== metadataRequestRevision.current) return;
        const next = new Map<string, AudioRenderMetadata>();
        entries.forEach(([idChStr, value]) => {
          if (value) next.set(idChStr, value);
        });
        metadataRef.current = next;
        setMetadata(next);
        waveformTiles.current.clear();
        pending.current.clear();
        tileRequestRevision.current += 1;
        loadedMetadataKey.current = metadataKey;
        pendingMetadataKey.current = null;
      })
      .catch((error) => {
        if (requestRevision === metadataRequestRevision.current) pendingMetadataKey.current = null;
        console.error("Failed to fetch overview waveform metadata", error);
      });
  }, [idChKey, needRefresh, trackId]);

  useEffect(() => {
    if (trackId === null) {
      const requestRevision = ++limiterRequestRevision.current;
      loadedLimiterKey.current = null;
      pendingLimiterKey.current = null;
      limiterGainSeqRef.current = null;
      const requestId = requestAnimationFrame(() => {
        if (requestRevision === limiterRequestRevision.current) {
          setLimiterRevision((value) => value + 1);
        }
      });
      return () => cancelAnimationFrame(requestId);
    }
    const limiterKey = `${trackId}`;
    if (
      !needRefresh &&
      (loadedLimiterKey.current === limiterKey || pendingLimiterKey.current === limiterKey)
    )
      return;

    const requestRevision = ++limiterRequestRevision.current;
    pendingLimiterKey.current = limiterKey;
    void BackendAPI.getLimiterGainSeq(trackId)
      .then((value) => {
        if (requestRevision !== limiterRequestRevision.current) return;
        limiterGainSeqRef.current = value;
        loadedLimiterKey.current = limiterKey;
        pendingLimiterKey.current = null;
        setLimiterRevision((revision) => revision + 1);
      })
      .catch((error) => {
        if (requestRevision === limiterRequestRevision.current) pendingLimiterKey.current = null;
        console.error("Failed to fetch overview limiter gain", error);
      });
  }, [needRefresh, trackId]);

  const requestWaveformTile = useEvent(
    (idChStr: string, rowMetadata: AudioRenderMetadata, level: number, tileIndex: number) => {
      const key = waveformKey(idChStr, rowMetadata.waveformRevision, level, tileIndex);
      if (waveformTiles.current.get(key) || pending.current.has(key)) return;
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
        .catch((error) => console.error("Failed to fetch overview waveform tile", error))
        .finally(() => pending.current.delete(key));
    },
  );

  const redraw = useEvent(() => {
    const pixi = app.current;
    const waveformLayer = layer.current;
    const node = host.current;
    if (!pixi || !waveformLayer || !node) return;
    syncBounds();
    const width = node.clientWidth;
    const height = node.clientHeight;
    if (trackId === null || idChArr.length === 0 || width <= 0 || height <= 0 || maxTrackSec <= 0) {
      waveformLayer.scale.set(1, 1);
      destroyPixiChildren(waveformLayer);
      lastRenderedTrackKey.current = null;
      lastRenderedBounds.current = null;
      pixi.render();
      return;
    }

    const renderTrackKey = `${trackId}:${idChArr.join(",")}`;
    const pxPerSec = width / Math.max(maxTrackSec, 1e-8);
    const gap = OVERVIEW_CH_GAP_HEIGHT;
    const chHeight = (height - gap * Math.max(idChArr.length - 1, 0)) / idChArr.length;
    const gainSeq = limiterGainSeqRef.current;
    const gainHeight = gainSeq ? chHeight * OVERVIEW_GAIN_HEIGHT_RATIO : 0;
    const waveformHeight = chHeight - 2 * gainHeight;
    const rows: OverviewRow[] = [];
    let hasMissingTiles = false;

    idChArr.forEach((idChStr, index) => {
      const rowMetadata = metadata.get(idChStr);
      if (!rowMetadata) return;
      const level = waveformLevel(rowMetadata.sampleRate, pxPerSec, devicePixelRatio);
      const { firstTile, lastTile } = waveformTileRange(rowMetadata, level, 0, maxTrackSec);
      const tiles: WaveformTile[] = [];
      for (let tileIndex = firstTile; tileIndex <= lastTile; tileIndex += 1) {
        const key = waveformKey(idChStr, rowMetadata.waveformRevision, level, tileIndex);
        const tile = waveformTiles.current.get(key);
        if (!tile) {
          requestWaveformTile(idChStr, rowMetadata, level, tileIndex);
          hasMissingTiles = true;
          continue;
        }
        tiles.push(tile);
      }
      rows.push({
        idChStr,
        metadata: rowMetadata,
        tiles,
        y: index * (chHeight + gap) + gainHeight,
        height: waveformHeight,
        trackWidth: Math.min(rowMetadata.trackSec * pxPerSec, width),
      });
    });

    if (
      hasMissingTiles &&
      waveformLayer.children.length > 0 &&
      lastRenderedTrackKey.current === renderTrackKey &&
      lastRenderedBounds.current
    ) {
      waveformLayer.scale.set(
        width / Math.max(lastRenderedBounds.current.width, 1),
        height / Math.max(lastRenderedBounds.current.height, 1),
      );
      pixi.render();
      return;
    }

    waveformLayer.scale.set(1, 1);
    destroyPixiChildren(waveformLayer);
    const ampRange = calcAmpRange(rows);
    rows.forEach((row) => {
      if (gainSeq && gainHeight > 0) {
        drawLimiterGain(
          waveformLayer,
          gainSeq,
          row.trackWidth,
          gainHeight,
          row.y - gainHeight,
          row.y + waveformHeight,
        );
      }
      if (row.metadata.isClipped) {
        renderWaveformTiles({
          layer: waveformLayer,
          tiles: row.tiles,
          metadata: row.metadata,
          y: row.y,
          height: row.height,
          startSec: 0,
          pxPerSec,
          width,
          ampRange,
          color: WAV_CLIPPING_COLOR,
          clampValues: false,
          needLineBorder: false,
          needEnvelopeBorder: false,
          lineWidth: OVERVIEW_LINE_WIDTH,
        });
      }
      renderWaveformTiles({
        layer: waveformLayer,
        tiles: row.tiles,
        metadata: row.metadata,
        y: row.y,
        height: row.height,
        startSec: 0,
        pxPerSec,
        width,
        ampRange,
        color: WAV_COLOR,
        clampValues: row.metadata.isClipped,
        needLineBorder: false,
        needEnvelopeBorder: false,
        lineWidth: OVERVIEW_LINE_WIDTH,
      });
    });

    const trackWidth = rows.reduce((max, row) => Math.max(max, row.trackWidth), 0);
    if (trackWidth < width) {
      waveformLayer.addChild(
        new Graphics()
          .rect(trackWidth, 0, width - trackWidth, height)
          .fill({ color: OUT_TRACK_COLOR, alpha: OUT_TRACK_ALPHA }),
      );
    }
    lastRenderedTrackKey.current = renderTrackKey;
    lastRenderedBounds.current = { width, height };
    pixi.render();
  });

  useLayoutEffect(redraw, [
    devicePixelRatio,
    idChArr,
    layoutRevision,
    limiterRevision,
    maxTrackSec,
    metadata,
    redraw,
    sceneRevision,
    syncBounds,
    trackId,
  ]);

  return <div ref={host} className={className} />;
}

export default OverviewWaveformViewport;
