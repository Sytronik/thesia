/// <reference types="vite/client" />

interface Window {
  __THESIA_RENDER_STATS__?: {
    fps: number;
    frameTimeP95: number;
    visibleRows: number;
    pendingRequests: number;
    gpuCacheBytes: number;
    gpuCacheSourceBytes: number;
    tileHits: number;
    tileMisses: number;
    spectrogramMetadataRows: number;
    spectrogramTilesExpected: number;
    spectrogramRequests: number;
    spectrogramResponses: number;
    spectrogramSprites: number;
    spectrogramErrors: number;
    spectrogramSkipReason: string | null;
    lastSpectrogramError: string | null;
    waveformVertices: number;
    maxTrackHz: number;
    blend: number;
    verticalZoomTargetScrollTop?: number;
    verticalZoomActualScrollTop?: number;
    verticalZoomAnchorErrorPx?: number;
    verticalZoomCursorY?: number;
    verticalZoomBaseHeight?: number;
    verticalZoomNewHeight?: number;
    verticalZoomLogicalScrollTop?: number;
    verticalZoomContentY?: number;
    verticalZoomNewContentY?: number;
    verticalZoomRowIndex?: number;
    verticalZoomOffsetRatio?: number;
  };
}
