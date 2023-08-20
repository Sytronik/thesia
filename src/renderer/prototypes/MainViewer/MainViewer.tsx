import React, {useRef, useCallback, useEffect, useMemo, useState} from "react";
import {throttle} from "throttle-debounce";
import {useDevicePixelRatio} from "use-device-pixel-ratio";
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
  MIN_HEIGHT,
  MAX_HEIGHT,
  VERTICAL_AXIS_PADDING,
} from "../constants";

type MainViewerProps = {
  trackIds: number[];
  erroredTrackIds: number[];
  selectedTrackIds: number[];
  trackIdChMap: IdChMap;
  needRefreshTrackIdChArr: IdChArr;
  addDroppedFile: (e: DragEvent) => void;
  reloadTracks: (ids: number[]) => void;
  refreshTracks: () => void;
  ignoreError: (id: number) => void;
  removeTracks: (ids: number[]) => void;
  selectTrack: (e: React.MouseEvent, id: number) => void;
};

function MainViewer(props: MainViewerProps) {
  const {
    trackIds,
    erroredTrackIds,
    selectedTrackIds,
    trackIdChMap,
    needRefreshTrackIdChArr,
    addDroppedFile,
    ignoreError,
    refreshTracks,
    reloadTracks,
    removeTracks,
    selectTrack,
  } = props;

  const mainViewerElem = useRef<HTMLDivElement | null>(null);
  const prevTrackCountRef = useRef<number>(0);

  const startSecRef = useRef<number>(0);
  const maxTrackSecRef = useRef<number>(0);
  const canvasIsFitRef = useRef<boolean>(false);
  const [timeUnitLabel, setTimeUnitLabel] = useState<string>("");

  const requestRef = useRef<number>(0);

  const pixelRatio = useDevicePixelRatio();
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const imgHeight = useMemo(() => height - 2 * VERTICAL_AXIS_PADDING, [height]);
  const [colorMapHeight, setColorMapHeight] = useState<number>(250);
  const colorBarHeight = useMemo(
    () => colorMapHeight - 2 * VERTICAL_AXIS_PADDING,
    [colorMapHeight],
  );

  const pxPerSecRef = useRef<number>(100);
  const drawOptionForWavRef = useRef<DrawOptionForWav>({amp_range: [-1, 1]});

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
    (canvasWidth: number, pxPerSec: number, drawOptions: MarkerDrawOption) => {
      if (canvasWidth === 0) {
        throttledSetTimeMarkers(0, 0, {});
        setTimeUnitLabel("");
        return;
      }
      throttledSetTimeMarkers(canvasWidth, pxPerSec, drawOptions);
      if (!timeMarkersRef.current.length) return;
      const timeUnit = timeMarkersRef.current[timeMarkersRef.current.length - 1][1];
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
      throttle(1000 / 240, async (idChArr: IdChArr, canvasWidth: number, canvasHeight: number) => {
        if (!idChArr.length) return;

        await NativeAPI.setImageState(
          idChArr,
          startSecRef.current,
          canvasWidth * pixelRatio,
          canvasHeight * pixelRatio,
          pxPerSecRef.current * pixelRatio,
          drawOptionForWavRef.current,
          0.3,
        );
      }),
    [pixelRatio],
  );

  const getIdChArr = useCallback(
    () => Array.from(trackIdChMap.values()).flatMap((v) => v),
    [trackIdChMap],
  ); // TODO: return only viewport
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
            throttledSetImgState(getIdChArr(), width, imgHeight);
            throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
              startSec: startSecRef.current,
              pxPerSec: pxPerSecRef.current,
            });
          }
        } else {
          setHeight(
            Math.round(Math.min(Math.max(height * (1 + e.deltaY / 1000), MIN_HEIGHT), MAX_HEIGHT)),
          );
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
          throttledSetImgState(getIdChArr(), width, imgHeight);
          throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
            startSec: startSecRef.current,
            pxPerSec: pxPerSecRef.current,
          });
        }
      }
    },
    [
      trackIds,
      getIdChArr,
      height,
      imgHeight,
      width,
      throttledSetImgState,
      throttledSetTimeMarkersAndUnit,
    ],
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

  const trackSummaryArr = useMemo(
    () =>
      trackIds.map((trackId) => {
        return {
          fileName: NativeAPI.getFileName(trackId),
          time: new Date(NativeAPI.getLength(trackId) * 1000).toISOString().substring(11, 23),
          sampleFormat: NativeAPI.getSampleFormat(trackId),
          sampleRate: `${NativeAPI.getSampleRate(trackId)} Hz`,
        };
      }),
    [trackIds],
  );

  const leftPane = (
    <>
      <TimeUnitSection key="time_unit_label" timeUnitLabel={timeUnitLabel} />
      {trackIds.map((trackId, i) => {
        const isSelected = selectedTrackIds.includes(trackId);
        return (
          <TrackInfo
            key={trackId}
            trackId={trackId}
            trackIdChArr={trackIdChMap.get(trackId) || []}
            trackSummary={trackSummaryArr[i]}
            channelHeight={height}
            imgHeight={imgHeight}
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
      <div className={styles.trackRightHeader}>
        <TimeAxis key="time_axis" ref={timeCanvasElem} width={width} pixelRatio={pixelRatio} />
        <span className={styles.axisLabelSection}>Amp</span>
        <span className={styles.axisLabelSection}>Hz</span>
      </div>
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
          {trackIdChMap.get(id)?.map((idChStr) => {
            return (
              <div key={idChStr} className={styles.chCanvases}>
                <ImgCanvas
                  ref={registerImgCanvas(idChStr)}
                  width={width}
                  height={imgHeight}
                  pixelRatio={pixelRatio}
                />
                <AmpAxis ref={registerAmpCanvas(idChStr)} height={height} pixelRatio={pixelRatio} />
                <FreqAxis
                  ref={registerFreqCanvas(idChStr)}
                  height={height}
                  pixelRatio={pixelRatio}
                />
              </div>
            );
          })}
        </div>
      ))}
    </>
  );

  // canvas img and markers setting logic
  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetAmpMarkers(imgHeight, imgHeight, {drawOptionForWav: drawOptionForWavRef.current});
  }, [throttledSetAmpMarkers, imgHeight, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    if (!trackIds.length) return;

    throttledSetFreqMarkers(imgHeight, imgHeight, {});
  }, [throttledSetFreqMarkers, imgHeight, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    if (!trackIds.length) {
      throttledSetDbMarkers(0, 0, {});
      return;
    }

    throttledSetDbMarkers(colorBarHeight, colorBarHeight, {});
  }, [throttledSetDbMarkers, colorBarHeight, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    if (!trackIds.length) {
      throttledSetTimeMarkersAndUnit(0, 0, {});
      return;
    }

    throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
      startSec: startSecRef.current,
      pxPerSec: pxPerSecRef.current,
    });
  }, [throttledSetTimeMarkersAndUnit, width, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    if (!trackIds.length) return;

    const currentIdChArr = needRefreshTrackIdChArr.length ? needRefreshTrackIdChArr : getIdChArr();
    throttledSetImgState(currentIdChArr, width, imgHeight);
  }, [throttledSetImgState, getIdChArr, width, imgHeight, trackIds, needRefreshTrackIdChArr]);

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

  const mainViewerElemCallback = useCallback(
    (node) => {
      if (node === null) {
        mainViewerElem.current?.removeEventListener("wheel", handleWheel);
        mainViewerElem.current = null;
        return;
      }
      node.addEventListener("wheel", handleWheel, {passive: false});
      mainViewerElem.current = node;
    },
    [handleWheel],
  );

  return (
    <div className={`${styles.MainViewer} row-flex`} ref={mainViewerElemCallback}>
      {isDropzoneActive && <div className={styles.dropzone} />}
      <SplitView left={leftPane} right={rightPane} setCanvasWidth={setWidth} />
      <ColorMap
        height={colorMapHeight}
        colorBarHeight={colorBarHeight}
        setHeight={setColorMapHeight}
        pixelRatio={pixelRatio}
        dbAxisCanvasElem={dbCanvasElem}
      />
    </div>
  );
}

export default MainViewer;
