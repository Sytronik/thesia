/**
 * Backend API type definitions.
 * This file contains type definitions that can be used when the backend module
 * or its type definitions are not available.
 */

// Enum types (values are exported from backend or mock)
export enum GuardClippingMode {
  Clip = "Clip",
  ReduceGlobalLevel = "ReduceGlobalLevel",
  Limiter = "Limiter",
}
export enum FreqScale {
  Linear = "Linear",
  Mel = "Mel",
}

export interface AudioFormatInfo {
  name: string;
  sampleRate: number;
  bitDepth: string;
  bitrate: string;
}

export interface SpecSetting {
  winMillisec: number;
  tOverlap: number;
  fOverlap: number;
  freqScale: FreqScale;
}

export interface Spectrogram {
  buf: Uint8Array;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  leftMargin: number;
  rightMargin: number;
  topMargin: number;
  bottomMargin: number;
  isLowQuality: boolean;
}

export interface UserSettings {
  specSetting: SpecSetting;
  blend: number;
  dBRange: number;
  commonGuardClipping: GuardClippingMode;
  commonNormalize: any;
}

export interface UserSettingsOptionals {
  specSetting?: SpecSetting;
  blend?: number;
  dBRange?: number;
  commonGuardClipping?: GuardClippingMode;
  commonNormalize?: any;
}

export interface WavMetadata {
  length: number;
  sr: number;
  isClipped: boolean;
}

export interface PlayerState {
  isPlaying: boolean;
  positionSec: number;
  err: string;
}
