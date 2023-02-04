import {isNil} from "../utils/arrayUtils";

const backend = require("backend");

backend.init();

// most api returns empty array for edge case

/* handle tracks */
export async function addTracks(newTrackIds: number[], newPaths: string[]): Promise<number[]> {
  // return successfully opened track ids
  return backend.addTracks(newTrackIds, newPaths);
}

export async function reloadTracks(trackIds: number[]): Promise<number[]> {
  // return successfully reloaded track ids
  return backend.reloadTracks(trackIds);
}

export async function removeTracks(trackIds: number[]): Promise<void> {
  return backend.removeTracks(trackIds);
}

export async function applyTrackListChanges(): Promise<Promise<IdChannel[]> | null> {
  return backend.applyTrackListChanges();
}

export async function findIdByPath(path: string): Promise<number> {
  // return -1 if path is new
  return backend.findIDbyPath(path);
}

/* get each track file's information */
export function getChannelCounts(trackId: number): 1 | 2 {
  return backend.getNumCh(trackId);
}

export function getLength(trackId: number): number {
  return backend.getSec(trackId);
}

export function getSampleRate(trackId: number): number {
  return backend.getSr(trackId);
}

export function getSampleFormat(trackId: number): string {
  return backend.getSampleFormat(trackId);
}

export async function getPath(trackId: number): Promise<string> {
  return backend.getPath(trackId);
}

export function getFileName(trackId: number): string {
  return backend.getFileName(trackId);
}

/* draw tracks */
/* time axis */
export function getLongestTrackLength(): number {
  // return track length of longest track in sec
  return backend.getMaxSec();
}

export async function getTimeAxisMarkers(
  width: number,
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {startSec, pxPerSec} = markerDrawOptions || {};

  if (isNil(startSec) || isNil(pxPerSec)) {
    console.error("no start sec of px per sec value exist");
    return [];
  }
  return backend.getTimeAxis(width, startSec, pxPerSec, subTickSec, subTickUnitCount);
}

/* track axis */
export async function getHzAtPointer(yPosition: number, height: number): Promise<number> {
  return backend.getHzAt(yPosition, height);
}

export async function getFreqAxisMarkers(
  height: number,
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getFreqAxis(height, maxNumTicks, maxNumLabels);
}

export async function getAmpAxisMarkers(
  height: number,
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {drawOptionForWav} = markerDrawOptions || {};

  if (!drawOptionForWav) {
    console.error("no draw option for wav exist");
    return [];
  }

  return backend.getAmpAxis(height, maxNumTicks, maxNumLabels, drawOptionForWav);
}

/* db axis */
export async function getMaxdB(): Promise<number> {
  return backend.getMaxdB();
}

export async function getMindB(): Promise<number> {
  return backend.getMindB();
}

export function getColorMap(): ArrayBuffer {
  return backend.getColormap();
}

export async function getDbAxisMarkers(
  height: number,
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getdBAxis(height, maxNumTicks, maxNumLabels);
}

/* images */
export function getImages(): SpecWavImages {
  return backend.getImages();
}

export async function getOverview(trackId: number, width: number, height: number) {
  return backend.getOverview(trackId, width, height);
}

export async function setImageState(
  idChArr: string[],
  startSec: number,
  width: number,
  height: number,
  pxPerSec: number,
  drawOptionForWav: DrawOptionForWav,
  blend: number,
) {
  const drawOption = {pxPerSec, height};
  return backend.setImgState(idChArr, startSec, width, drawOption, drawOptionForWav, blend);
}
