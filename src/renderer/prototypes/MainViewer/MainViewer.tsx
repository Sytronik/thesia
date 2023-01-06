import React, {useRef, useCallback, useEffect, useMemo, useState} from "react";
import {throttle} from "throttle-debounce";
import useDropzone from "renderer/hooks/useDropzone";
import useRefs from "renderer/hooks/useRefs";
import ImgCanvas from "renderer/modules/ImgCanvas";
import SplitView from "renderer/modules/SplitView";
import useThrottledSetMarkers from "renderer/hooks/useThrottledSetMarkers";
import styles from "./MainViewer.scss";
import AmpAxis from "./AmpAxis";
import ColorMap from "./ColorMap";
import ErrorBox from "./ErrorBox";
import FreqAxis from "./FreqAxis";
import TrackInfo from "./TrackInfo";
import TimeUnitSection from "./TimeUnitSection";
import TimeAxis from "./TimeAxis";
import TrackAddButtonSection from "./TrackAddButtonSection";
import NativeAPI from "../../api";
import {
  TIME_TICK_SIZE,
  TIME_BOUNDARIES,
  AMP_TICK_NUM,
  AMP_BOUNDARIES,
  FREQ_TICK_NUM,
  FREQ_BOUNDARIES,
  DB_TICK_NUM,
  DB_BOUNDARIES,
} from "../constants";

type MainViewerProps = {
  trackIds: number[];
  erroredTrackIds: number[];
  needRefreshTrackIds: IdChArr;
  selectedTrackIds: number[];
  addDroppedFile: (e: DragEvent) => void;
  reloadTracks: (ids: number[]) => void;
  refreshTracks: () => void;
  ignoreError: (id: number) => void;
  removeTracks: (ids: number[]) => void;
  selectTrack: (e: React.MouseEvent, id: number) => void;
};

function MainViewer(props: MainViewerProps) {
  const {
    erroredTrackIds,
    needRefreshTrackIds,
    trackIds,
    selectedTrackIds,
    addDroppedFile,
    ignoreError,
    refreshTracks,
    reloadTracks,
    removeTracks,
    selectTrack,
  } = props;

  const mainViewerElem = useRef<HTMLDivElement>(null);
  const prevTrackCountRef = useRef<number>(0);

  const startSecRef = useRef<number>(0);
  const maxTrackSecRef = useRef<number>(0);
  const canvasIsFitRef = useRef<boolean>(false);
  const [timeUnitLabel, setTimeUnitLabel] = useState<string>("");

  const requestRef = useRef<number>(0);

  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const [colorMapHeight, setColorMapHeight] = useState<number>(0);
  const pxPerSecRef = useRef<number>(100);
  const drawOptionForWavRef = useRef({min_amp: -1, max_amp: 1});

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [ampCanvasesRef, registerAmpCanvas] = useRefs<AxisCanvasHandleElement>();
  const [freqCanvasesRef, registerFreqCanvas] = useRefs<AxisCanvasHandleElement>();
  const timeCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const dbCanvasElem = useRef<AxisCanvasHandleElement>(null);

  const {isDropzoneActive} = useDropzone({targetRef: mainViewerElem, handleDrop: addDroppedFile});

  const {markersRef: timeMarkersRef, throttledSetMarkers: throttledSetTimeMarkers} =
    useThrottledSetMarkers({
      scaleTable: TIME_TICK_SIZE,
      boundaries: TIME_BOUNDARIES,
      getMarkers: NativeAPI.getTimeAxisMarkers,
    });

  const throttledSetTimeMarkersAndUnit = useCallback(
    (width: number, pxPerSec: number, drawOptions: MarkerDrawOption) => {
      throttledSetTimeMarkers(width, pxPerSec, drawOptions);
      const timeUnit = timeMarkersRef.current.pop()?.[1] || "ss";
      setTimeUnitLabel(timeUnit);
    },
    [timeMarkersRef, throttledSetTimeMarkers],
  );

  const {markersRef: ampMarkersRef, throttledSetMarkers: throttledSetAmpMarkers} =
    useThrottledSetMarkers({
      scaleTable: AMP_TICK_NUM,
      boundaries: AMP_BOUNDARIES,
      getMarkers: NativeAPI.getAmpAxisMarkers,
    });

  const {markersRef: freqMarkersRef, throttledSetMarkers: throttledSetFreqMarkers} =
    useThrottledSetMarkers({
      scaleTable: FREQ_TICK_NUM,
      boundaries: FREQ_BOUNDARIES,
      getMarkers: NativeAPI.getFreqAxisMarkers,
    });

  const {markersRef: dbMarkersRef, throttledSetMarkers: throttledSetDbMarkers} =
    useThrottledSetMarkers({
      scaleTable: DB_TICK_NUM,
      boundaries: DB_BOUNDARIES,
      getMarkers: NativeAPI.getDbAxisMarkers,
    });

  const throttledSetImgState = useMemo(
    () =>
      throttle(1000 / 240, (idChArr: IdChannel[], width: number, height: number) => {
        if (!idChArr.length) return;

        NativeAPI.setImageState(
          idChArr,
          startSecRef.current,
          width,
          height,
          pxPerSecRef.current,
          drawOptionForWavRef.current,
          0.3,
        );
      }),
    [],
  );

  const getIdChArr = useCallback(() => Object.keys(imgCanvasesRef.current), [imgCanvasesRef]);
  const handleWheel = useCallback(
    (e: WheelEvent) => {
      if (!trackIds.length) return;

      let yIsLarger;
      let delta;
      if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
        delta = e.deltaY;
        yIsLarger = true;
      } else {
        delta = e.deltaX;
        yIsLarger = false;
      }
      if (e.altKey) {
        e.preventDefault();
        e.stopPropagation();
        if ((e.shiftKey && yIsLarger) || !yIsLarger) {
          const pxPerSec = Math.min(
            Math.max(
              pxPerSecRef.current * (1 + delta / 1000),
              width / (maxTrackSecRef.current - startSecRef.current),
            ),
            384000,
          );
          if (pxPerSecRef.current !== pxPerSec) {
            pxPerSecRef.current = pxPerSec;
            canvasIsFitRef.current = false;
            throttledSetImgState(getIdChArr(), width, height);
            throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
              startSec: startSecRef.current,
              pxPerSec: pxPerSecRef.current,
            });
          }
        } else {
          setHeight(Math.round(Math.min(Math.max(height * (1 + e.deltaY / 1000), 10), 5000)));
        }
      } else if ((e.shiftKey && yIsLarger) || !yIsLarger) {
        e.preventDefault();
        e.stopPropagation();
        const tempSec = Math.min(
          Math.max(startSecRef.current + delta / pxPerSecRef.current, 0),
          maxTrackSecRef.current - width / pxPerSecRef.current,
        );
        if (startSecRef.current !== tempSec) {
          startSecRef.current = tempSec;
          throttledSetImgState(getIdChArr(), width, height);
          throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
            startSec: startSecRef.current,
            pxPerSec: pxPerSecRef.current,
          });
        }
      }
    },
    [trackIds, getIdChArr, height, width, throttledSetImgState, throttledSetTimeMarkersAndUnit],
  );

  const drawCanvas = useCallback(async () => {
    const images = NativeAPI.getImages();
    const promises: void[] = [];

    Object.entries(images).forEach((image) => {
      const [idChStr, buf] = image;
      const ampCanvasRef = ampCanvasesRef.current[idChStr];
      const freqCanvasRef = freqCanvasesRef.current[idChStr];
      const imgCanvasRef = imgCanvasesRef.current[idChStr];
      if (imgCanvasRef) {
        promises.push(imgCanvasRef.draw(buf));
      }
      if (ampCanvasRef) {
        ampCanvasRef.draw(ampMarkersRef.current);
      }
      if (freqCanvasRef) {
        freqCanvasRef.draw(freqMarkersRef.current);
      }
    });
    if (timeCanvasElem.current) {
      timeCanvasElem.current.draw(timeMarkersRef.current);
    }
    if (dbCanvasElem.current) {
      dbCanvasElem.current.draw(dbMarkersRef.current);
    }
    await Promise.all(promises);
    requestRef.current = requestAnimationFrame(drawCanvas);
  }, [
    timeCanvasElem,
    timeMarkersRef,
    ampCanvasesRef,
    ampMarkersRef,
    freqCanvasesRef,
    freqMarkersRef,
    dbCanvasElem,
    dbMarkersRef,
    imgCanvasesRef,
  ]);

  const reloadAndRefreshTrack = useCallback(
    (id: number) => {
      reloadTracks([id]);
      refreshTracks();
    },
    [reloadTracks, refreshTracks],
  );
  const removeAndRefreshTrack = useCallback(
    (id: number) => {
      removeTracks([id]);
      refreshTracks();
    },
    [removeTracks, refreshTracks],
  );

  const leftPane = (
    <>
      <TimeUnitSection key="time_unit_label" timeUnitLabel={timeUnitLabel} />
      {trackIds.map((trackId: number) => {
        const isSelected = selectedTrackIds.includes(trackId);
        return (
          <TrackInfo
            key={`${trackId}`}
            trackId={trackId}
            height={height}
            isSelected={isSelected}
            selectTrack={selectTrack}
          />
        );
      })}
      <TrackAddButtonSection key="track_add_button" />
    </>
  );

  const rightPane = (
    <>
      <TimeAxis key="time_axis" ref={timeCanvasElem} width={width} />
      {trackIds.map((id) => (
        <div key={`${id}`} className={`${styles.trackRight}`}>
          {erroredTrackIds.includes(id) ? (
            <ErrorBox
              trackId={id}
              handleReload={reloadAndRefreshTrack}
              handleIgnore={ignoreError}
              handleClose={removeAndRefreshTrack}
            />
          ) : null}
          {[...Array(NativeAPI.getChannelCounts(id)).keys()].map((ch) => (
            <div key={`${id}_${ch}`} className={styles.chCanvases}>
              <FreqAxis
                key={`freq_${id}_${ch}`}
                ref={registerFreqCanvas(`${id}_${ch}`)}
                height={height}
              />
              <AmpAxis
                key={`amp_${id}_${ch}`}
                ref={registerAmpCanvas(`${id}_${ch}`)}
                height={height}
              />
              <ImgCanvas
                key={`img_${id}_${ch}`}
                ref={registerImgCanvas(`${id}_${ch}`)}
                width={width}
                height={height}
              />
            </div>
          ))}
        </div>
      ))}
    </>
  );

  // canvas img and markers setting logic
  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetAmpMarkers(height, height, {drawOptionForWav: drawOptionForWavRef.current});
  }, [throttledSetAmpMarkers, height, trackIds, needRefreshTrackIds]);

  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetFreqMarkers(height, height, {});
  }, [throttledSetFreqMarkers, height, trackIds, needRefreshTrackIds]);

  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetDbMarkers(colorMapHeight, colorMapHeight, {});
  }, [throttledSetDbMarkers, colorMapHeight, trackIds, needRefreshTrackIds]);

  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
      startSec: startSecRef.current,
      pxPerSec: pxPerSecRef.current,
    });
  }, [throttledSetTimeMarkersAndUnit, width, trackIds, needRefreshTrackIds]);

  useEffect(() => {
    if (!trackIds.length) return;

    const idChannels = needRefreshTrackIds.length ? needRefreshTrackIds : getIdChArr();
    throttledSetImgState(idChannels, width, height);
  }, [throttledSetImgState, getIdChArr, width, height, trackIds, needRefreshTrackIds]);

  useEffect(() => {
    requestRef.current = requestAnimationFrame(drawCanvas);
    return () => cancelAnimationFrame(requestRef.current);
  }, [drawCanvas]);

  // startSec setting logic
  useEffect(() => {
    if (!trackIds.length) return;

    const secOutOfCanvas = maxTrackSecRef.current - width / pxPerSecRef.current;

    if (canvasIsFitRef.current) {
      pxPerSecRef.current = width / maxTrackSecRef.current;
      return;
    }
    if (secOutOfCanvas <= 0) {
      canvasIsFitRef.current = true;
      return;
    }
    if (startSecRef.current > secOutOfCanvas) {
      startSecRef.current = secOutOfCanvas;
    }
  }, [trackIds, width]);

  // pxPerSec and canvasIsFit setting logic
  useEffect(() => {
    prevTrackCountRef.current = trackIds.length;

    if (!trackIds.length) {
      maxTrackSecRef.current = 0;
      canvasIsFitRef.current = false;
      return;
    }

    maxTrackSecRef.current = NativeAPI.getLongestTrackLength();
    if (!prevTrackCountRef.current) {
      pxPerSecRef.current = width / maxTrackSecRef.current;
      startSecRef.current = 0;
      canvasIsFitRef.current = true;
    }
  }, [trackIds, width]);

  useEffect(() => {
    const mainViewer = mainViewerElem.current;
    mainViewer?.addEventListener("wheel", handleWheel, {passive: false});

    return () => {
      mainViewer?.removeEventListener("wheel", handleWheel);
    };
  });

  return (
    <div className={`${styles.MainViewer} row-flex`} ref={mainViewerElem}>
      {isDropzoneActive && <div className={styles.dropzone} />}
      <SplitView left={leftPane} right={rightPane} setCanvasWidth={setWidth} />
      <ColorMap
        height={colorMapHeight}
        setHeight={setColorMapHeight}
        dbAxisCanvasElem={dbCanvasElem}
      />
    </div>
  );
}

export default MainViewer;
