import { BufferImageSource, Texture } from "pixi.js";

import type { SpectrogramTile, WaveformTile } from "../api";

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
