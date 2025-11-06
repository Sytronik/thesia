/**
 * Mock implementation of the backend API for testing purposes.
 * Use this when the native backend module is not available.
 *
 * To use the mock, set the environment variable USE_BACKEND_MOCK=true
 * or import this module directly in your test files.
 */

import type {
  AudioFormatInfo,
  SpecSetting,
  Spectrogram,
  UserSettings,
  UserSettingsOptionals,
  WavMetadata,
  PlayerState,
} from "./backend-types";
import { GuardClippingMode, FreqScale } from "./backend-types";

// Import enum types with different names to avoid conflicts with enum values

// Mock state storage
const mockState = {
  tracks: new Map<
    number,
    {path: string; fileName: string; lengthSec: number; sampleRate: number; channels: number}
  >(),
  nextTrackId: 0,
  playerState: {isPlaying: false, positionSec: 0, err: ""} as PlayerState,
  settings: {
    specSetting: {
      winMillisec: 23.2,
      tOverlap: 0.5,
      fOverlap: 0.5,
      freqScale: FreqScale.Linear,
    },
    blend: 0.5,
    dBRange: 120,
    commonGuardClipping: GuardClippingMode.Clip,
    commonNormalize: {type: "Off"},
  } as UserSettings,
  colormapLength: 256,
  maxTrackHz: 24000,
  maxdB: 0,
  mindB: -120,
};

// Helper function to create mock audio format info
function createMockFormatInfo(sampleRate: number = 44100): AudioFormatInfo {
  return {
    name: "WAV",
    sampleRate,
    bitDepth: "16",
    bitrate: `${sampleRate * 16 * 2}kbps`,
  };
}

// Helper function to create mock spectrogram
function createMockSpectrogram(
  idChStr: string,
  secRange: [number, number],
  hzRange: [number, number],
  marginPx: number,
): Spectrogram {
  const [startSec, endSec] = secRange;
  const width = Math.max(1, Math.floor((endSec - startSec) * 100));
  const height = Math.max(1, Math.floor((hzRange[1] - hzRange[0]) / 10));

  return {
    buf: new Uint8Array(width * height * 4), // RGBA
    width,
    height,
    startSec,
    pxPerSec: 100,
    leftMargin: marginPx,
    rightMargin: marginPx,
    topMargin: marginPx,
    bottomMargin: marginPx,
    isLowQuality: false,
  };
}

export async function addTracks(
  idList: Array<number>,
  pathList: Array<string>,
): Promise<Array<number>> {
  const addedIds: number[] = [];
  for (let i = 0; i < pathList.length; i++) {
    const path = pathList[i];
    const id = idList[i] ?? mockState.nextTrackId++;
    const fileName = path.split(/[/\\]/).pop() || "unknown";
    mockState.tracks.set(id, {
      path,
      fileName,
      lengthSec: 60, // Default 60 seconds
      sampleRate: 44100,
      channels: 2,
    });
    addedIds.push(id);
  }
  return addedIds;
}

export async function applyTrackListChanges(): Promise<Array<string>> {
  return Array.from(mockState.tracks.values()).map((t) => t.path);
}

export function assignLimiterGainTo(arr: Float32Array, trackId: number): void {
  // Fill with zeros (no limiting)
  arr.fill(0);
}

export function assignWavTo(arr: Float32Array, idChStr: string): void {
  // Fill with zeros (silence)
  arr.fill(0);
}

export function findIdByPath(path: string): number {
  for (const [id, track] of mockState.tracks.entries()) {
    if (track.path === path) return id;
  }
  return -1;
}

export function freqHzToPos(hz: number, height: number, hzRange: [number, number]): number {
  const [minHz, maxHz] = hzRange;
  if (maxHz === minHz) return height / 2;
  return height * (1 - (hz - minHz) / (maxHz - minHz));
}

export function freqLabelToHz(label: string): number {
  const match = label.match(/(\d+(?:\.\d+)?)\s*(?:Hz|kHz|KHz)/i);
  if (!match) return 0;
  const value = parseFloat(match[1]);
  return label.toLowerCase().includes("khz") ? value * 1000 : value;
}

export function freqPosToHz(y: number, height: number, hzRange: [number, number]): number {
  const [minHz, maxHz] = hzRange;
  const ratio = 1 - y / height;
  return minHz + ratio * (maxHz - minHz);
}

export function getAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  ampRange: [number, number],
): any {
  const [minAmp, maxAmp] = ampRange;
  const ticks: [number, string][] = [];
  const step = (maxAmp - minAmp) / maxNumTicks;
  for (let i = 0; i <= maxNumTicks; i++) {
    const value = minAmp + step * i;
    ticks.push([i * 10, value.toFixed(2)]);
  }
  return ticks;
}

export function getChannelCounts(trackId: number): number {
  const track = mockState.tracks.get(trackId);
  return track?.channels ?? 2;
}

export function getCommonGuardClipping(): GuardClippingMode {
  return mockState.settings.commonGuardClipping;
}

export function getCommonNormalize(): any {
  return mockState.settings.commonNormalize;
}

export function getdBAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  minDB: number,
  maxDB: number,
): any {
  const ticks: [number, string][] = [];
  const step = (maxDB - minDB) / maxNumTicks;
  for (let i = 0; i <= maxNumTicks; i++) {
    const value = minDB + step * i;
    ticks.push([i * 10, `${value.toFixed(0)} dB`]);
  }
  return ticks;
}

export function getdBRange(): number {
  return mockState.settings.dBRange;
}

export function getFileName(trackId: number): string {
  const track = mockState.tracks.get(trackId);
  return track?.fileName ?? "";
}

export function getFormatInfo(trackId: number): AudioFormatInfo {
  const track = mockState.tracks.get(trackId);
  return createMockFormatInfo(track?.sampleRate);
}

export function getFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  hzRange: [number, number],
  maxTrackHz: number,
): any {
  const [minHz, maxHz] = hzRange;
  const ticks: [number, string][] = [];
  const step = (maxHz - minHz) / maxNumTicks;
  for (let i = 0; i <= maxNumTicks; i++) {
    const value = minHz + step * i;
    const label = value >= 1000 ? `${(value / 1000).toFixed(1)} kHz` : `${value.toFixed(0)} Hz`;
    ticks.push([i * 10, label]);
  }
  return ticks;
}

export function getGlobalLUFS(trackId: number): number {
  return -23; // Default LUFS value
}

export function getGuardClipStats(trackId: number): any {
  return [[-1, "No clipping detected"]];
}

export function getLengthSec(trackId: number): number {
  const track = mockState.tracks.get(trackId);
  return track?.lengthSec ?? 0;
}

export function getLimiterGainLength(trackId: number): number {
  const track = mockState.tracks.get(trackId);
  if (!track) return 0;
  return Math.floor(track.lengthSec * track.sampleRate);
}

export function getLongestTrackLengthSec(): number {
  let maxLength = 0;
  for (const track of mockState.tracks.values()) {
    maxLength = Math.max(maxLength, track.lengthSec);
  }
  return maxLength;
}

export function getMaxdB(): number {
  return mockState.maxdB;
}

export function getMaxPeakdB(trackId: number): number {
  return -0.1; // Just below 0 dB
}

export function getMaxTrackHz(): number {
  return mockState.maxTrackHz;
}

export function getMindB(): number {
  return mockState.mindB;
}

export function getPath(trackId: number): string {
  const track = mockState.tracks.get(trackId);
  return track?.path ?? "";
}

export function getPlayerState(): PlayerState {
  return {...mockState.playerState};
}

export function getRMSdB(trackId: number): number {
  return -20; // Default RMS value
}

export function getSampleRate(trackId: number): number {
  const track = mockState.tracks.get(trackId);
  return track?.sampleRate ?? 44100;
}

export function getSpecSetting(): SpecSetting {
  return {...mockState.settings.specSetting};
}

export async function getSpectrogram(
  idChStr: string,
  secRange: [number, number],
  hzRange: [number, number],
  marginPx: number,
): Promise<Spectrogram | null> {
  return createMockSpectrogram(idChStr, secRange, hzRange, marginPx);
}

export function getTimeAxisMarkers(
  startSec: number,
  endSec: number,
  tickUnit: number,
  labelInterval: number,
  maxSec: number,
): any {
  const ticks: [number, string][] = [];
  const duration = endSec - startSec;
  const numTicks = Math.floor(duration / tickUnit);
  for (let i = 0; i <= numTicks; i++) {
    const sec = startSec + i * tickUnit;
    const label =
      sec >= 60
        ? `${Math.floor(sec / 60)}:${(sec % 60).toFixed(1).padStart(4, "0")}`
        : `${sec.toFixed(1)}s`;
    ticks.push([i * 10, label]);
  }
  return ticks;
}

export function getWavMetadata(idChStr: string): WavMetadata {
  const match = idChStr.match(/^(\d+)_\d+$/);
  const trackId = match ? parseInt(match[1], 10) : -1;
  const track = mockState.tracks.get(trackId);
  if (!track) {
    return {length: 0, sr: 44100, isClipped: false};
  }
  return {
    length: Math.floor(track.lengthSec * track.sampleRate),
    sr: track.sampleRate,
    isClipped: false,
  };
}

export function hzToLabel(hz: number): string {
  if (hz >= 1000) {
    return `${(hz / 1000).toFixed(1)} kHz`;
  }
  return `${hz.toFixed(0)} Hz`;
}

export function init(
  userSettings: UserSettingsOptionals,
  maxSpectrogramSize: number,
  tmpDirPath: string,
): UserSettings {
  mockState.settings = {
    specSetting: userSettings.specSetting ?? mockState.settings.specSetting,
    blend: userSettings.blend ?? mockState.settings.blend,
    dBRange: userSettings.dBRange ?? mockState.settings.dBRange,
    commonGuardClipping: userSettings.commonGuardClipping ?? mockState.settings.commonGuardClipping,
    commonNormalize: userSettings.commonNormalize ?? mockState.settings.commonNormalize,
  };
  return {...mockState.settings};
}

export async function pausePlayer(): Promise<void> {
  mockState.playerState.isPlaying = false;
}

export async function reloadTracks(trackIds: Array<number>): Promise<Array<number>> {
  // In mock, just return the same IDs
  return trackIds;
}

export function removeTracks(trackIds: Array<number>): void {
  for (const id of trackIds) {
    mockState.tracks.delete(id);
  }
}

export async function resumePlayer(): Promise<void> {
  mockState.playerState.isPlaying = true;
}

export function secondsToLabel(sec: number): string {
  if (sec >= 3600) {
    const hours = Math.floor(sec / 3600);
    const minutes = Math.floor((sec % 3600) / 60);
    const seconds = sec % 60;
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toFixed(1).padStart(4, "0")}`;
  } else if (sec >= 60) {
    const minutes = Math.floor(sec / 60);
    const seconds = sec % 60;
    return `${minutes}:${seconds.toFixed(1).padStart(4, "0")}`;
  }
  return `${sec.toFixed(1)}s`;
}

export async function seekPlayer(sec: number): Promise<void> {
  mockState.playerState.positionSec = Math.max(0, sec);
}

export async function setColormapLength(colormapLength: number): Promise<void> {
  mockState.colormapLength = colormapLength;
}

export async function setCommonGuardClipping(mode: GuardClippingMode): Promise<void> {
  mockState.settings.commonGuardClipping = mode;
}

export async function setCommonNormalize(target: any): Promise<void> {
  mockState.settings.commonNormalize = target;
}

export async function setdBRange(dBRange: number): Promise<void> {
  mockState.settings.dBRange = dBRange;
}

export async function setSpecSetting(specSetting: SpecSetting): Promise<void> {
  mockState.settings.specSetting = specSetting;
}

export async function setTrackPlayer(
  trackId: number,
  sec?: number | undefined | null,
): Promise<void> {
  mockState.playerState.positionSec = sec ?? 0;
  mockState.playerState.isPlaying = false;
}

export async function setVolumedB(volumeDB: number): Promise<void> {
  // Mock doesn't need to do anything
}

export function timeLabelToSeconds(label: string): number {
  // Parse formats like "1:23.4", "123.4s", "1:23:45.6"
  const parts = label.split(":");
  if (parts.length === 3) {
    // HH:MM:SS
    return parseInt(parts[0], 10) * 3600 + parseInt(parts[1], 10) * 60 + parseFloat(parts[2]);
  } else if (parts.length === 2) {
    // MM:SS
    return parseInt(parts[0], 10) * 60 + parseFloat(parts[1]);
  } else {
    // SS or SSs
    return parseFloat(label.replace(/s$/i, ""));
  }
}

// Export types (enum types are already exported from backend-types)
export type {
  AudioFormatInfo,
  SpecSetting,
  Spectrogram,
  UserSettings,
  UserSettingsOptionals,
  WavMetadata,
  PlayerState,
};
