import backend from "backend";

backend.init();

// most api returns empty array for edge case
/* get each track file's information */
export function getChannelCounts(trackId: number): 1 | 2 {
  const ch = backend.getChannelCounts(trackId);
  if (!(ch === 1 || ch === 2)) console.error(`No. of channel ${ch} not supported!`);
  if (ch >= 1.5) return 2;
  return 1;
}

/* draw tracks */
/* time axis */
export async function getTimeAxisMarkers(
  subTickSec: number,
  subTickUnitCount: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {startSec, endSec} = markerDrawOptions || {};

  if (startSec === undefined || endSec === undefined) {
    console.error("no start sec of px per sec value exist");
    return [];
  }
  return backend.getTimeAxisMarkers(startSec, endSec, subTickSec, subTickUnitCount);
}

/* track axis */
export async function getFreqAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getFreqAxisMarkers(maxNumTicks, maxNumLabels);
}

export async function getAmpAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
  markerDrawOptions: MarkerDrawOption,
): Promise<Markers> {
  const {ampRange} = markerDrawOptions || {};

  if (!ampRange) {
    console.error("no draw option for wav exist");
    return [];
  }

  return backend.getAmpAxisMarkers(maxNumTicks, maxNumLabels, ampRange);
}

/* db axis */

export async function getdBAxisMarkers(
  maxNumTicks: number,
  maxNumLabels: number,
): Promise<Markers> {
  return backend.getdBAxisMarkers(maxNumTicks, maxNumLabels);
}

/* images */
export function getImages(): SpecWavImages {
  return backend.getImages();
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
  return backend.setImageState(
    idChArr,
    startSec,
    width,
    {pxPerSec, height},
    drawOptionForWav,
    blend,
  );
}

export function getSpecSetting(): SpecSetting {
  return backend.getSpecSetting();
}

export async function setSpecSetting(specSetting: SpecSetting) {
  await backend.setSpecSetting(specSetting);
}

export const {
  addTracks,
  reloadTracks,
  removeTracks,
  applyTrackListChanges,
  findIdByPath,
  getPath,
  getFileName,
  getLengthSec,
  getSampleRate,
  getSampleFormat,
  getGlobalLUFS,
  getLongestTrackLengthSec,
  getHzAt,
  getMaxdB,
  getMindB,
  getColorMap,
  getOverview,
} = backend;