import { invoke } from "@tauri-apps/api/core";

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

function decodeWaveformTile(value: ArrayBuffer | Uint8Array): WaveformTile {
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

function decodeSpectrogramTile(value: ArrayBuffer | Uint8Array): SpectrogramTile {
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

export async function getChannelCounts(trackId: number): Promise<1 | 2> {
  const ch = await invoke<number>("get_channel_counts", { trackId });
  if (!(ch === 1 || ch === 2)) console.error(`No. of channel ${ch} not supported!`);
  if (ch >= 1.5) return 2;
  return 1;
}

export type AxisKind = "timeRuler" | "ampAxis" | "freqAxis" | "dBAxis";

export interface AudioFormatInfo {
  name: string;
  sampleRate: number;
  bitDepth: string;
  bitrate: string;
}

export type FreqScale = "Linear" | "Mel";

export type GuardClippingMode = "Clip" | "ReduceGlobalLevel" | "Limiter";

export const NormalizeOnTypeValues = ["LUFS", "RMSdB", "PeakdB"] as const;
export type NormalizeOnType = (typeof NormalizeOnTypeValues)[number];
export type NormalizeTarget =
  | { type: "Off" }
  | {
      type: NormalizeOnType;
      target: number;
    };

export interface PlayerState {
  isPlaying: boolean;
  positionSec: number;
  eventTimeMs: number;
  trackId: number | null;
  err: string;
}

export interface SpecSetting {
  winMillisec: number;
  tOverlap: number;
  fOverlap: number;
  freqScale: FreqScale;
}

export interface UserSettings {
  specSetting: SpecSetting;
  blend: number;
  dBRange: number;
  commonGuardClipping: GuardClippingMode;
  commonNormalize: NormalizeTarget;
}

export interface UserSettingsOptionals {
  specSetting?: SpecSetting;
  blend?: number;
  dBRange?: number;
  commonGuardClipping?: GuardClippingMode;
  commonNormalize?: NormalizeTarget;
}

export interface BackendConstants {
  PLAY_JUMP_SEC: number;
  PLAY_BIG_JUMP_SEC: number;
}

export interface ConstsAndUserSettings {
  constants: BackendConstants;
  userSettings: UserSettings;
}

// IdChannel is form of id#_ch#
export type IdChannel = string;
export type IdChArr = IdChannel[];

export type AudioRenderMetadata = {
  waveformRevision: number;
  spectrogramRevision: number;
  sampleRate: number;
  sampleCount: number;
  trackSec: number;
  isClipped: boolean;
  spectrogramWidth: number;
  spectrogramHeight: number;
  waveformTileBins: number;
  spectrogramTileSize: number;
};

export async function getLimiterGainSeq(trackId: number): Promise<Float32Array | null> {
  const gainSeq = await invoke<number[] | null>("get_limiter_gain", { trackId });
  if (gainSeq === null) return null;
  return new Float32Array(gainSeq);
}

export async function getCommonNormalize(): Promise<NormalizeTarget> {
  const commonNormalize = await invoke<NormalizeTarget>("get_common_normalize");
  return commonNormalize;
}

export async function setCommonNormalize(target: NormalizeTarget): Promise<void> {
  return invoke<void>("set_common_normalize", { target });
}

export async function getGuardClipStats(trackId: number): Promise<[number, string][]> {
  // [channel, stats]
  // if [[-1, stats]], then all channels have the same stats
  return invoke<[number, string][]>("get_guard_clip_stats", { trackId });
}

export async function init(colormapRgba: Uint8Array): Promise<ConstsAndUserSettings> {
  return invoke<ConstsAndUserSettings>("init", { colormapRgba: Array.from(colormapRgba) });
}

export async function setUserSettings(settings: UserSettingsOptionals): Promise<void> {
  return invoke<void>("set_user_settings", { settings });
}

export async function getOpenFilesDialogPath(): Promise<string> {
  return invoke<string>("get_open_files_dialog_path");
}

export async function setOpenFilesDialogPath(path: string): Promise<void> {
  return invoke<void>("set_open_files_dialog_path", { path });
}

export async function addTracks(trackIds: number[], paths: string[]): Promise<number[]> {
  return invoke<number[]>("add_tracks", { trackIds, paths });
}

export async function reloadTracks(trackIds: number[]): Promise<number[]> {
  return invoke<number[]>("reload_tracks", { trackIds });
}

export async function removeTracks(trackIds: number[]): Promise<void> {
  return invoke<void>("remove_tracks", { trackIds });
}

export async function applyTrackListChanges(): Promise<string[]> {
  return invoke<string[]>("apply_track_list_changes");
}

export async function getdBRange(): Promise<number> {
  return invoke<number>("get_dB_range");
}

export async function setdBRange(dBRange: number): Promise<void> {
  return invoke<void>("set_dB_range", { dBRange });
}

export async function getSpecSetting(): Promise<SpecSetting> {
  return invoke<SpecSetting>("get_spec_setting");
}

export async function setSpecSetting(specSetting: SpecSetting): Promise<void> {
  return invoke<void>("set_spec_setting", { specSetting });
}

export async function getCommonGuardClipping(): Promise<GuardClippingMode> {
  return invoke<GuardClippingMode>("get_common_guard_clipping");
}

export async function setCommonGuardClipping(mode: GuardClippingMode): Promise<void> {
  return invoke<void>("set_common_guard_clipping", { mode });
}

export async function getAudioRenderMetadata(idChStr: string): Promise<AudioRenderMetadata | null> {
  return invoke<AudioRenderMetadata | null>("get_audio_render_metadata", { idChStr });
}

export async function getWaveformTile(
  idChStr: string,
  level: number,
  tileIndex: number,
): Promise<WaveformTile> {
  const tile = await invoke<ArrayBuffer>("get_waveform_tile", { idChStr, level, tileIndex });
  return decodeWaveformTile(tile);
}

export async function getSpectrogramTile(
  idChStr: string,
  levelX: number,
  levelY: number,
  tileX: number,
  tileY: number,
): Promise<SpectrogramTile> {
  const tile = await invoke<ArrayBuffer>("get_spectrogram_tile", {
    idChStr,
    levelX,
    levelY,
    tileX,
    tileY,
  });
  return decodeSpectrogramTile(tile);
}

export async function findIdByPath(path: string): Promise<number> {
  return invoke<number>("find_id_by_path", { path });
}

export async function getMaxdB(): Promise<number> {
  return invoke<number>("get_max_dB");
}

export async function getMindB(): Promise<number> {
  return invoke<number>("get_min_dB");
}

export async function getMaxTrackHz(): Promise<number> {
  return invoke<number>("get_max_track_hz");
}

export async function getLongestTrackLengthSec(): Promise<number> {
  return invoke<number>("get_longest_track_length_sec");
}

export async function getLengthSec(trackId: number): Promise<number> {
  return invoke<number>("get_length_sec", { trackId });
}

export async function getSampleRate(trackId: number): Promise<number> {
  return invoke<number>("get_sample_rate", { trackId });
}

export async function getFormatInfo(trackId: number): Promise<AudioFormatInfo> {
  const out = await invoke<{ name: string; sr: number; bitDepth: string; bitrate: string }>(
    "get_format_info",
    { trackId },
  );
  return {
    name: out.name,
    sampleRate: out.sr,
    bitDepth: out.bitDepth,
    bitrate: out.bitrate,
  };
}

export async function getGlobalLUFS(trackId: number): Promise<number> {
  return invoke<number>("get_global_lufs", { trackId });
}

export async function getRMSdB(trackId: number): Promise<number> {
  return invoke<number>("get_rms_dB", { trackId });
}

export async function getMaxPeakdB(trackId: number): Promise<number> {
  return invoke<number>("get_max_peak_dB", { trackId });
}

export async function getPath(trackId: number): Promise<string> {
  return invoke<string>("get_path", { trackId });
}

export async function getFileName(trackId: number): Promise<string> {
  return invoke<string>("get_file_name", { trackId });
}

export async function setVolumedB(volumedB: number): Promise<void> {
  return invoke<void>("set_volume_dB", { volumeDB: volumedB });
}

export async function setTrackPlayer(trackId: number, sec?: number): Promise<void> {
  return invoke<void>("set_track_player", { trackId, sec });
}

export async function seekPlayer(sec: number): Promise<void> {
  return invoke<void>("seek_player", { sec });
}

export async function pausePlayer(): Promise<void> {
  return invoke<void>("pause_player");
}

export async function resumePlayer(): Promise<void> {
  return invoke<void>("resume_player");
}

export async function showEditContextMenu(): Promise<void> {
  return invoke<void>("show_edit_context_menu");
}

export async function showAxisContextMenu(axisKind: AxisKind, id: number): Promise<void> {
  if (axisKind === "dBAxis") return;
  return invoke<void>("show_axis_context_menu", { axisKind, id });
}

export async function showTrackContextMenu(): Promise<void> {
  return invoke<void>("show_track_context_menu");
}

export async function enableEditMenu(): Promise<void> {
  return invoke<void>("enable_edit_menu");
}

export async function disableEditMenu(): Promise<void> {
  return invoke<void>("disable_edit_menu");
}

export async function enableAxisZoomMenu(): Promise<void> {
  return invoke<void>("enable_axis_zoom_menu");
}

export async function disableAxisZoomMenu(): Promise<void> {
  return invoke<void>("disable_axis_zoom_menu");
}

export async function enableRemoveTrackMenu(): Promise<void> {
  return invoke<void>("enable_remove_track_menu");
}

export async function disableRemoveTrackMenu(): Promise<void> {
  return invoke<void>("disable_remove_track_menu");
}

export async function enablePlayMenu(): Promise<void> {
  return invoke<void>("enable_play_menu");
}

export async function disablePlayMenu(): Promise<void> {
  return invoke<void>("disable_play_menu");
}

export async function enableTogglePlayMenu(): Promise<void> {
  return invoke<void>("enable_toggle_play_menu");
}

export async function disableTogglePlayMenu(): Promise<void> {
  return invoke<void>("disable_toggle_play_menu");
}

export async function showPlayMenu(): Promise<void> {
  return invoke<void>("show_play_menu");
}

export async function showPauseMenu(): Promise<void> {
  return invoke<void>("show_pause_menu");
}

export async function showPlayOrPauseMenu(isPlaying: boolean): Promise<void> {
  if (isPlaying) return showPauseMenu();
  return showPlayMenu();
}

export async function changeMenuDepsOnTrackExistence(
  trackExists: boolean,
): Promise<[void, void, void]> {
  if (trackExists) {
    return Promise.all([enableAxisZoomMenu(), enableRemoveTrackMenu(), enablePlayMenu()]);
  }
  return Promise.all([disableAxisZoomMenu(), disableRemoveTrackMenu(), disablePlayMenu()]);
}
