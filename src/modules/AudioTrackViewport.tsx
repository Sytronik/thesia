import { useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Application, Container, Graphics, Rectangle, Sprite, type Texture } from "pixi.js";
import useEvent from "react-use-event-hook";

import BackendAPI, { AudioRenderMetadata, FreqScale, type WaveformTile, WasmAPI } from "../api";
import { DevicePixelRatioContext } from "../contexts";
import { GpuTextureCache, WaveformTileCache } from "../lib/audio-render-tiles";
import {
  clamp,
  destroyPixiChildren,
  renderWaveformTiles,
  waveformKey,
  waveformLevel,
  waveformTileRange,
  WAV_CLIPPING_COLOR,
  WAV_COLOR,
} from "../lib/waveform-renderer";
import {
  TIME_CANVAS_HEIGHT,
  TINY_MARGIN,
  VERTICAL_AXIS_PADDING,
} from "../prototypes/constants/tracks";
import styles from "./AudioTrackViewport.module.scss";

const GPU_TEXTURE_BUDGET_BYTES = 128 * 1024 * 1024;
const WAVEFORM_TILE_BUDGET_BYTES = 32 * 1024 * 1024;
const HEADER_HEIGHT = TIME_CANVAS_HEIGHT + TINY_MARGIN;
const METADATA_RETRY_LIMIT = 20;
const METADATA_RETRY_DELAY_MS = 100;
const LOADING_INDICATOR_DELAY_MS = 500;

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
  isLoading: boolean;
  isPlaying: boolean;
  getPlayheadSec: () => number | null;
  refreshToken: string;
  layoutRevision: number;
};

type TooltipInfo = { left: number; top: number; lines: string[] };

const spectrogramKey = (
  idChStr: string,
  revision: number,
  levelX: number,
  levelY: number,
  tileX: number,
  tileY: number,
) => `s:${idChStr}:${revision}:${levelX}:${levelY}:${tileX}:${tileY}`;
const log2Level = (scale: number) => Math.max(0, Math.floor(Math.log2(Math.max(scale, 1))));

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
    isLoading,
    refreshToken,
    layoutRevision,
  } = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const host = useRef<HTMLDivElement>(null);
  const app = useRef<Application | null>(null);
  const rowLayer = useRef<Container | null>(null);
  const loadingLayer = useRef<Graphics | null>(null);
  const playheadLayer = useRef<Graphics | null>(null);
  const textureCache = useRef(new GpuTextureCache(GPU_TEXTURE_BUDGET_BYTES));
  const waveformTiles = useRef(new WaveformTileCache(WAVEFORM_TILE_BUDGET_BYTES));
  const waveformCompositeTextures = useRef(new Set<Texture>());
  const pending = useRef(new Set<string>());
  const prevBounds = useRef<{ width: number; height: number } | null>(null);
  const metadataRef = useRef(new Map<string, AudioRenderMetadata>());
  const metadataRequestRevision = useRef(0);
  const metadataRetryCount = useRef(0);
  const tileRequestRevision = useRef(0);
  const latestProps = useRef(props);
  const visibleRowsKey = useRef("");
  const [metadata, setMetadata] = useState(new Map<string, AudioRenderMetadata>());
  const [sceneRevision, setSceneRevision] = useState(0);
  const [tooltip, setTooltip] = useState<TooltipInfo | null>(null);
  const [showLoadingIndicator, setShowLoadingIndicator] = useState(false);

  useLayoutEffect(() => {
    latestProps.current = props;
  }, [props]);
  useEffect(() => {
    metadataRef.current = metadata;
  }, [metadata]);
  useEffect(() => {
    const timeout = window.setTimeout(
      () => setShowLoadingIndicator(isLoading),
      isLoading ? LOADING_INDICATOR_DELAY_MS : 0,
    );
    return () => window.clearTimeout(timeout);
  }, [isLoading]);

  const syncBounds = useEvent(() => {
    const rect = getViewportRect();
    const node = host.current;
    const pixi = app.current;
    if (!rect || !node) {
      if (node) node.style.display = "none";
      return;
    }
    if (rect.width <= 0 || rect.height <= 0) {
      node.style.display = "none";
      return;
    }
    node.style.display = "block";
    node.style.left = `${rect.left}px`;
    node.style.top = `${rect.top}px`;
    node.style.width = `${rect.width}px`;
    node.style.height = `${rect.height}px`;
    if (!pixi) return;
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
    const compositeTextures = waveformCompositeTextures.current;
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
        const rowsContainer = new Container();
        const loading = new Graphics();
        const playhead = new Graphics();
        pixi.stage.addChild(rowsContainer, loading, playhead);
        host.current.appendChild(pixi.canvas);
        app.current = pixi;
        rowLayer.current = rowsContainer;
        loadingLayer.current = loading;
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
        compositeTextures.forEach((texture) => texture.destroy(true));
        compositeTextures.clear();
        if (rowLayer.current) destroyPixiChildren(rowLayer.current);
        app.current = null;
        rowLayer.current = null;
        loadingLayer.current = null;
        playheadLayer.current = null;
      }
      pixi.destroy({ removeView: true }, { children: true });
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
      if (waveformTiles.current.get(key)) return;
      if (pending.current.has(key)) return;
      pending.current.add(key);
      const requestRevision = tileRequestRevision.current;
      void BackendAPI.getWaveformTile(idChStr, level, tileIndex)
        .then((tile) => {
          if (requestRevision !== tileRequestRevision.current) return;
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
        return;
      }
      if (pending.current.has(key)) return;
      pending.current.add(key);
      const requestRevision = tileRequestRevision.current;
      void BackendAPI.getSpectrogramTile(idChStr, levelX, levelY, tileX, tileY)
        .then((tile) => {
          if (requestRevision !== tileRequestRevision.current) return;
          if (metadataRef.current.get(idChStr)?.spectrogramRevision !== tile.revision) return;
          if (tile.width === 0 || tile.height === 0) return;
          textureCache.current.set(key, tile);
          setSceneRevision((revision) => revision + 1);
        })
        .catch((error) => console.error("Failed to fetch spectrogram tile", error))
        .finally(() => pending.current.delete(key));
    },
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
        return;
      }
      const minHz = Math.max(hzRange[0], 0);
      const maxHz = Math.min(hzRange[1], maxTrackHz);
      if (!Number.isFinite(minHz) || !Number.isFinite(maxHz) || maxHz <= minHz) {
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
          const { texture, originX, originY } = cachedTexture;
          const sprite = new Sprite(texture);
          sprite.x = ((originX * scaleX) / basePxPerSec - startSec) * pxPerSec;
          sprite.y =
            rowY +
            ((sourceBottom - (originY + texture.height) * scaleY) / sourceHeight) * imageHeight;
          sprite.width = (texture.width * scaleX * pxPerSec) / basePxPerSec;
          sprite.height = (texture.height * scaleY * imageHeight) / sourceHeight;
          layer.addChild(sprite);
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

  const destroyWaveformCompositeTextures = useEvent(() => {
    waveformCompositeTextures.current.forEach((texture) => texture.destroy(true));
    waveformCompositeTextures.current.clear();
  });

  const drawLoadingIndicators = useEvent((timestamp: number) => {
    const layer = loadingLayer.current;
    if (!layer) return;
    layer.clear();
    if (!showLoadingIndicator) return;

    const current = latestProps.current;
    const rect = current.getViewportRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) return;

    const scrollTop = current.getScrollTop();
    const radius = Math.max(8, Math.min(25, current.imageHeight * 0.25));
    const lineWidth = Math.max(2, Math.min(5, radius * 0.2));
    const centerX = rect.width / 2;
    const startAngle = (timestamp / 2000) * Math.PI * 2;
    const endAngle = startAngle + Math.PI * 1.5;
    let hasIndicator = false;

    current.rows.forEach((row) => {
      const rowY = HEADER_HEIGHT + row.top - scrollTop + VERTICAL_AXIS_PADDING;
      if (row.hidden || rowY + current.imageHeight < 0 || rowY > rect.height) return;
      const centerY = rowY + current.imageHeight / 2;
      layer.moveTo(
        centerX + Math.cos(startAngle) * radius,
        centerY + Math.sin(startAngle) * radius,
      );
      layer.arc(centerX, centerY, radius, startAngle, endAngle);
      hasIndicator = true;
    });

    if (hasIndicator) {
      layer.stroke({ color: 0xffffff, width: lineWidth, alpha: 0.95 });
    }
  });

  const redrawRows = useEvent(() => {
    const pixi = app.current;
    const layer = rowLayer.current;
    const rect = getViewportRect();
    if (!pixi || !layer || !rect) return;
    destroyWaveformCompositeTextures();
    destroyPixiChildren(layer);
    textureCache.current.destroyRetired();
    const scrollTop = getScrollTop();
    layer.y = HEADER_HEIGHT - scrollTop;
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
      const wavLayer = new Container();
      const level = waveformLevel(rowMetadata.sampleRate, pxPerSec, devicePixelRatio);
      const { firstTile, lastTile } = waveformTileRange(
        rowMetadata,
        level,
        startSec,
        startSec + width / pxPerSec,
      );
      const loadedTiles: WaveformTile[] = [];
      for (let tileIndex = firstTile; tileIndex <= lastTile; tileIndex += 1) {
        const key = waveformKey(row.idChStr, rowMetadata.waveformRevision, level, tileIndex);
        const tile = waveformTiles.current.get(key);
        if (!tile) {
          requestWaveformTile(row.idChStr, rowMetadata, level, tileIndex);
          continue;
        }
        loadedTiles.push(tile);
      }
      if (rowMetadata.isClipped) {
        renderWaveformTiles({
          layer: wavLayer,
          tiles: loadedTiles,
          metadata: rowMetadata,
          y: rowY,
          height: imageHeight,
          startSec,
          pxPerSec,
          width,
          ampRange,
          color: WAV_CLIPPING_COLOR,
          clampValues: false,
          needLineBorder: true,
          needEnvelopeBorder: true,
        });
      }
      renderWaveformTiles({
        layer: wavLayer,
        tiles: loadedTiles,
        metadata: rowMetadata,
        y: rowY,
        height: imageHeight,
        startSec,
        pxPerSec,
        width,
        ampRange,
        color: WAV_COLOR,
        clampValues: rowMetadata.isClipped,
        needLineBorder: true,
        needEnvelopeBorder: !rowMetadata.isClipped,
      });
      if (wavLayer.children.length === 0) return;
      const wavTexture = pixi.renderer.generateTexture({
        target: wavLayer,
        frame: new Rectangle(0, rowY, width, imageHeight),
        resolution: devicePixelRatio,
      });
      waveformCompositeTextures.current.add(wavTexture);
      destroyPixiChildren(wavLayer);
      const wavSprite = new Sprite(wavTexture);
      wavSprite.y = rowY;
      wavSprite.alpha = wavAlpha;
      rowContainer.addChild(wavSprite);
    });
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
        drawLoadingIndicators(timestamp);
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
        textureCache.current.releaseUploadedResources();
      }
      requestId = requestAnimationFrame(render);
    };
    requestId = requestAnimationFrame(render);
    return () => {
      disposed = true;
      cancelAnimationFrame(requestId);
    };
  }, [drawLoadingIndicators, getVisibleRowsKey, redrawRows]);

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
