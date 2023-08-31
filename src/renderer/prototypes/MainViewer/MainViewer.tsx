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
import Overview from "../Overview/Overview";
import SlideBar from "../SlideBar/SlideBar";
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
  MAX_PX_PER_SEC,
} from "../constants";

type MainViewerProps = {
  trackIds: number[];
  erroredTrackIds: number[];
  selectedTrackIds: number[];
  trackIdChMap: IdChMap;
  needRefreshTrackIdChArr: IdChArr;
  maxTrackSec: number;
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
    maxTrackSec,
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
  const pxPerSecRef = useRef<number>(100);
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

  const drawOptionForWavRef = useRef<DrawOptionForWav>({amp_range: [-1, 1]});

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [ampCanvasesRef, registerAmpCanvas] = useRefs<AxisCanvasHandleElement>();
  const [freqCanvasesRef, registerFreqCanvas] = useRefs<AxisCanvasHandleElement>();
  const overviewElem = useRef<OverviewCanvasHandleElement>(null);
  const timeCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const dbCanvasElem = useRef<AxisCanvasHandleElement>(null);

  const {isDropzoneActive} = useDropzone({targetRef: mainViewerElem, handleDrop: addDroppedFile});

  const getIdChArr = useCallback(
    () => Array.from(trackIdChMap.values()).flatMap((v) => v),
    [trackIdChMap],
  ); // TODO: return only viewport

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

  const updateLensParams = useCallback(
    (params: {startSec?: number; pxPerSec?: number}) => {
      let startSec = params.startSec ?? startSecRef.current;
      let pxPerSec = params.pxPerSec ?? pxPerSecRef.current;
      if (startSec !== startSecRef.current) {
        const lensDurationSec = width / pxPerSec;
        startSec = Math.min(Math.max(startSec, 0), maxTrackSec - lensDurationSec);
      }
      if (pxPerSec !== pxPerSecRef.current)
        pxPerSec = Math.min(Math.max(pxPerSec, width / (maxTrackSec - startSec)), MAX_PX_PER_SEC);
      startSecRef.current = startSec;
      pxPerSecRef.current = pxPerSec;

      throttledSetImgState(getIdChArr(), width, imgHeight);
      throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
        startSec: startSecRef.current,
        pxPerSec: pxPerSecRef.current,
      });
    },
    [
      getIdChArr,
      imgHeight,
      maxTrackSec,
      throttledSetImgState,
      throttledSetTimeMarkersAndUnit,
      width,
    ],
  );

  const moveLens = useCallback(
    (sec: number, anchorRatio: number) => {
      const lensDurationSec = width / pxPerSecRef.current;
      updateLensParams({startSec: sec - lensDurationSec * anchorRatio});
    },
    [width, updateLensParams],
  );

  const resizeLensLeft = useCallback(
    (sec: number) => {
      const endSec = startSecRef.current + width / pxPerSecRef.current;
      const startSec = Math.min(Math.max(sec, 0), endSec - width / MAX_PX_PER_SEC);
      const pxPerSec = width / (endSec - startSec);

      updateLensParams({startSec, pxPerSec});
      canvasIsFitRef.current = false;
    },
    [width, updateLensParams],
  );

  const resizeLensRight = useCallback(
    (sec: number) => {
      const pxPerSec = Math.min(width / Math.max(sec - startSecRef.current, 0), MAX_PX_PER_SEC);
      updateLensParams({pxPerSec});
      canvasIsFitRef.current = false;
    },
    [width, updateLensParams],
  );

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
          updateLensParams({pxPerSec: pxPerSecRef.current * (1 + delta / 1000)});
          canvasIsFitRef.current = false;
        } else {
          setHeight(
            Math.round(Math.min(Math.max(height * (1 + e.deltaY / 1000), MIN_HEIGHT), MAX_HEIGHT)),
          );
        }
      } else if ((e.shiftKey && yIsLarger) || !yIsLarger) {
        e.preventDefault();
        e.stopPropagation();
        updateLensParams({startSec: startSecRef.current + delta / pxPerSecRef.current});
      }
    },
    [trackIds, height, updateLensParams],
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
      ampCanvasRef?.draw(ampMarkersRef.current);
      freqCanvasRef?.draw(freqMarkersRef.current);
    });
    overviewElem.current?.draw(startSecRef.current, width / pxPerSecRef.current);
    timeCanvasElem.current?.draw(timeMarkersRef.current);
    dbCanvasElem.current?.draw(dbMarkersRef.current);
    await Promise.all(promises);
    requestRef.current = requestAnimationFrame(drawCanvas);
  }, [
    width,
    timeMarkersRef,
    ampCanvasesRef,
    ampMarkersRef,
    freqCanvasesRef,
    freqMarkersRef,
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

    const secOutOfCanvas = maxTrackSec - width / pxPerSecRef.current;

    if (canvasIsFitRef.current) {
      updateLensParams({pxPerSec: width / maxTrackSec});
      return;
    }
    if (secOutOfCanvas <= 0) {
      canvasIsFitRef.current = true;
      return;
    }
    if (startSecRef.current > secOutOfCanvas) {
      updateLensParams({startSec: secOutOfCanvas});
    }
  }, [trackIds, width, maxTrackSec, updateLensParams]);

  // pxPerSec and canvasIsFit setting logic
  useEffect(() => {
    prevTrackCountRef.current = trackIds.length;

    if (!trackIds.length) {
      canvasIsFitRef.current = false;
      return;
    }

    if (prevTrackCountRef.current === 0) {
      updateLensParams({startSec: 0, pxPerSec: width / maxTrackSec});
      canvasIsFitRef.current = true;
    }
  }, [trackIds, width, maxTrackSec, updateLensParams]);

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
    <>
      <div className="row-fixed overview">
        <Overview
          ref={overviewElem}
          selectedTrackId={
            trackIds.length > 0 && selectedTrackIds.length > 0
              ? selectedTrackIds[selectedTrackIds.length - 1]
              : null
          }
          maxTrackSec={maxTrackSec}
          pixelRatio={pixelRatio}
          moveLens={moveLens}
          resizeLensLeft={resizeLensLeft}
          resizeLensRight={resizeLensRight}
        />
        <SlideBar />
      </div>
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
    </>
  );
}

export default MainViewer;
