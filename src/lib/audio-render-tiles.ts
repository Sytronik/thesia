import { BufferImageSource, Texture } from "pixi.js";

const WAVEFORM_HEADER_BYTES = 24;
const SPECTROGRAM_HEADER_BYTES = 40;

export type WaveformTile = {
  revision: number;
  binCount: number;
  samplesPerBin: number;
  tileIndex: number;
  min: Float32Array;
  max: Float32Array;
  representative: Float32Array;
};

export type SpectrogramTile = {
  revision: number;
  width: number;
  height: number;
  levelX: number;
  levelY: number;
  tileX: number;
  tileY: number;
  originX: number;
  originY: number;
  rgba: Uint8Array;
};

const asArrayBuffer = (value: ArrayBuffer | Uint8Array) => {
  if (value instanceof ArrayBuffer) return value;
  return value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength) as ArrayBuffer;
};

export function decodeWaveformTile(value: ArrayBuffer | Uint8Array): WaveformTile {
  const buffer = asArrayBuffer(value);
  const view = new DataView(buffer);
  const revision = Number(view.getBigUint64(0, true));
  const binCount = view.getUint32(8, true);
  const samplesPerBin = view.getUint32(12, true);
  const tileIndex = view.getUint32(16, true);
  const min = new Float32Array(binCount);
  const max = new Float32Array(binCount);
  const representative = new Float32Array(binCount);
  for (let i = 0; i < binCount; i += 1) {
    const offset = WAVEFORM_HEADER_BYTES + i * 12;
    min[i] = view.getFloat32(offset, true);
    max[i] = view.getFloat32(offset + 4, true);
    representative[i] = view.getFloat32(offset + 8, true);
  }
  return { revision, binCount, samplesPerBin, tileIndex, min, max, representative };
}

export function decodeSpectrogramTile(value: ArrayBuffer | Uint8Array): SpectrogramTile {
  const buffer = asArrayBuffer(value);
  const view = new DataView(buffer);
  return {
    revision: Number(view.getBigUint64(0, true)),
    width: view.getUint32(8, true),
    height: view.getUint32(12, true),
    levelX: view.getUint32(16, true),
    levelY: view.getUint32(20, true),
    tileX: view.getUint32(24, true),
    tileY: view.getUint32(28, true),
    originX: view.getUint32(32, true),
    originY: view.getUint32(36, true),
    rgba: new Uint8Array(buffer, SPECTROGRAM_HEADER_BYTES),
  };
}

type TextureCacheEntry = {
  texture: Texture;
  originX: number;
  originY: number;
  bytes: number;
  sourceBytes: number;
  touched: number;
};

type ReleasableTextureSource = Texture["source"] & {
  resource: unknown | null;
  _gpuData?: Record<string, unknown>;
};

type WaveformCacheEntry = {
  tile: WaveformTile;
  bytes: number;
  touched: number;
};

export class WaveformTileCache {
  private entries = new Map<string, WaveformCacheEntry>();
  private touched = 0;
  private _bytes = 0;

  constructor(private readonly budgetBytes: number) {}

  get(key: string) {
    const entry = this.entries.get(key);
    if (!entry) return null;
    entry.touched = ++this.touched;
    return entry.tile;
  }

  set(key: string, tile: WaveformTile) {
    if (this.entries.has(key)) return;
    const bytes = tile.min.byteLength + tile.max.byteLength + tile.representative.byteLength;
    this.entries.set(key, { tile, bytes, touched: ++this.touched });
    this._bytes += bytes;
    this.evict();
  }

  clear() {
    this.entries.clear();
    this._bytes = 0;
  }

  private evict() {
    while (this._bytes > this.budgetBytes) {
      const oldest = [...this.entries.entries()].reduce((a, b) =>
        a[1].touched <= b[1].touched ? a : b,
      );
      this._bytes -= oldest[1].bytes;
      this.entries.delete(oldest[0]);
    }
  }
}

export class GpuTextureCache {
  private entries = new Map<string, TextureCacheEntry>();
  private retiredTextures = new Set<Texture>();
  private touched = 0;
  private _bytes = 0;
  private _sourceBytes = 0;

  constructor(private readonly budgetBytes: number) {}

  get bytes() {
    return this._bytes;
  }

  get(key: string) {
    const entry = this.entries.get(key);
    if (!entry) return null;
    entry.touched = ++this.touched;
    return entry;
  }

  set(key: string, tile: SpectrogramTile) {
    const previous = this.entries.get(key);
    if (previous) {
      previous.touched = ++this.touched;
      return previous;
    }
    const source = new BufferImageSource({
      resource: tile.rgba,
      width: tile.width,
      height: tile.height,
      format: "rgba8unorm",
      scaleMode: "linear",
      alphaMode: "no-premultiply-alpha",
      autoGarbageCollect: false,
    });
    const texture = new Texture({ source });
    const bytes = tile.rgba.byteLength;
    const entry = {
      texture,
      originX: tile.originX,
      originY: tile.originY,
      bytes,
      sourceBytes: bytes,
      touched: ++this.touched,
    };
    this.entries.set(key, entry);
    this._bytes += bytes;
    this._sourceBytes += bytes;
    this.evict();
    return entry;
  }

  clear() {
    this.entries.forEach(({ texture }) => this.retiredTextures.add(texture));
    this.entries.clear();
    this._bytes = 0;
    this._sourceBytes = 0;
  }

  destroyRetired() {
    this.retiredTextures.forEach((texture) => texture.destroy(true));
    this.retiredTextures.clear();
  }

  releaseUploadedResources() {
    this.entries.forEach((entry) => {
      if (entry.sourceBytes === 0) return;
      const source = entry.texture.source as ReleasableTextureSource;
      const hasGpuUpload = source._gpuData && Object.keys(source._gpuData).length > 0;
      if (!hasGpuUpload || !source.resource) return;
      source.resource = null;
      this._sourceBytes -= entry.sourceBytes;
      entry.sourceBytes = 0;
    });
    if (this._sourceBytes < 0) this._sourceBytes = 0;
  }

  private evict() {
    while (this._bytes > this.budgetBytes) {
      const oldest = [...this.entries.entries()].reduce((a, b) =>
        a[1].touched <= b[1].touched ? a : b,
      );
      this.retiredTextures.add(oldest[1].texture);
      this._bytes -= oldest[1].bytes;
      this._sourceBytes -= oldest[1].sourceBytes;
      this.entries.delete(oldest[0]);
    }
    if (this._sourceBytes < 0) this._sourceBytes = 0;
  }
}
