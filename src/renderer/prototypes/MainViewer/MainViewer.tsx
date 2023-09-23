import React, {useRef, useCallback, useEffect, useMemo, useState, useContext} from "react";
import {throttle} from "throttle-debounce";
import useDropzone from "renderer/hooks/useDropzone";
import useRefs from "renderer/hooks/useRefs";
import ImgCanvas from "renderer/modules/ImgCanvas";
import SplitView from "renderer/modules/SplitView";
import useThrottledSetMarkers from "renderer/hooks/useThrottledSetMarkers";
import useEvent from "react-use-event-hook";
import {DevicePixelRatioContext} from "renderer/contexts";
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
  FIT_TOLERANCE_SEC,
  TIME_CANVAS_HEIGHT,
  DEFAULT_AMP_RANGE,
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

  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const [scrollTop, setScrollTop] = useState(0);
  const imgHeight = useMemo(() => height - 2 * VERTICAL_AXIS_PADDING, [height]);
  const [colorMapHeight, setColorMapHeight] = useState<number>(250);
  const colorBarHeight = useMemo(
    () => colorMapHeight - 2 * VERTICAL_AXIS_PADDING,
    [colorMapHeight],
  );

  const ampRangeRef = useRef<[number, number]>([...DEFAULT_AMP_RANGE]);

  const overviewElem = useRef<OverviewHandleElement>(null);
  const splitViewElem = useRef<SplitViewHandleElement>(null);
  const timeCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const dbCanvasElem = useRef<AxisCanvasHandleElement>(null);

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [ampCanvasesRef, registerAmpCanvas] = useRefs<AxisCanvasHandleElement>();
  const [freqCanvasesRef, registerFreqCanvas] = useRefs<AxisCanvasHandleElement>();

  const prevCursorClientY = useRef<number>(0);
  const vScrollAnchorInfoRef = useRef<VScrollAnchorInfo>({
    imgIndex: 0,
    cursorRatioOnImg: 0.0,
    cursorOffset: 0,
  });

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

  const throttledSetTimeMarkersAndUnit = useEvent(
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
          canvasWidth * devicePixelRatio,
          canvasHeight * devicePixelRatio,
          pxPerSecRef.current * devicePixelRatio,
          {amp_range: ampRangeRef.current, dpr: devicePixelRatio},
          0.3,
        );
      }),
    [devicePixelRatio],
  );

  const normalizeStartSec = useEvent((startSec, pxPerSec, maxEndSec) => {
    return Math.min(Math.max(startSec, 0), maxEndSec - width / pxPerSec);
  });

  const normalizePxPerSec = useEvent((pxPerSec, startSec) =>
    Math.min(Math.max(pxPerSec, width / (maxTrackSec - startSec)), MAX_PX_PER_SEC),
  );

  const updateLensParams = useEvent((params: OptionalLensParams) => {
    let startSec = params.startSec ?? startSecRef.current;
    let pxPerSec = params.pxPerSec ?? pxPerSecRef.current;

    if (startSec !== startSecRef.current)
      startSec = normalizeStartSec(startSec, pxPerSec, maxTrackSec);
    if (pxPerSec !== pxPerSecRef.current) pxPerSec = normalizePxPerSec(pxPerSec, startSec);

    startSecRef.current = startSec;
    pxPerSecRef.current = pxPerSec;
    canvasIsFitRef.current =
      startSec <= FIT_TOLERANCE_SEC && width >= (maxTrackSec - FIT_TOLERANCE_SEC) * pxPerSec;

    Object.values(imgCanvasesRef.current).forEach((value) =>
      value?.updateLensParams({startSec, pxPerSec}),
    );
    throttledSetImgState(getIdChArr(), width, imgHeight);
    throttledSetTimeMarkersAndUnit(width, pxPerSecRef.current, {
      startSec: startSecRef.current,
      pxPerSec: pxPerSecRef.current,
    });
  });

  const moveLens = useEvent((sec: number, anchorRatio: number) => {
    const lensDurationSec = width / pxPerSecRef.current;
    updateLensParams({startSec: sec - lensDurationSec * anchorRatio});
  });

  const resizeLensLeft = useEvent((sec: number) => {
    const endSec = startSecRef.current + width / pxPerSecRef.current;
    const startSec = normalizeStartSec(sec, MAX_PX_PER_SEC, endSec);
    const pxPerSec = normalizePxPerSec(width / (endSec - startSec), startSec);

    updateLensParams({startSec, pxPerSec});
  });

  const resizeLensRight = useEvent((sec: number) => {
    const pxPerSec = normalizePxPerSec(
      width / Math.max(sec - startSecRef.current, 0),
      startSecRef.current,
    );
    updateLensParams({pxPerSec});
  });

  const updateVScrollAnchorInfo = useEvent((cursorClientY: number) => {
    let i = 0;
    let prevBottom = 0;
    trackIds.forEach((id) =>
      trackIdChMap.get(id)?.forEach((idChStr) => {
        const imgClientRect = imgCanvasesRef.current[idChStr]?.getBoundingClientRect();
        if (imgClientRect === undefined) return;
        const bottom = imgClientRect.y + imgClientRect.height;
        // TODO: when cursor is out of ImgCanvas
        if (prevBottom <= cursorClientY && cursorClientY < imgClientRect.y) {
          vScrollAnchorInfoRef.current.imgIndex = i;
          vScrollAnchorInfoRef.current.cursorRatioOnImg = 0;
          vScrollAnchorInfoRef.current.cursorOffset = cursorClientY - imgClientRect.y;
        } else if (imgClientRect.y <= cursorClientY && cursorClientY < bottom) {
          vScrollAnchorInfoRef.current.imgIndex = i;
          vScrollAnchorInfoRef.current.cursorRatioOnImg =
            (cursorClientY - imgClientRect.y) / imgClientRect.height;
          vScrollAnchorInfoRef.current.cursorOffset = 0;
        }
        i += 1;
        prevBottom = bottom;
      }),
    );
    if (prevBottom <= cursorClientY) {
      vScrollAnchorInfoRef.current.imgIndex = i - 1;
      vScrollAnchorInfoRef.current.cursorRatioOnImg = 1;
      vScrollAnchorInfoRef.current.cursorOffset = cursorClientY - prevBottom;
    }
  });

  const onMouseMove = (e: React.MouseEvent) => {
    if (e.clientY === prevCursorClientY.current) return;
    updateVScrollAnchorInfo(e.clientY);
    prevCursorClientY.current = e.clientY;
  };

  const handleWheel = useEvent((e: WheelEvent) => {
    if (!trackIds.length) return;

    let delta: number;
    let horizontal: boolean;
    if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) {
      delta = e.deltaX;
      horizontal = !e.shiftKey;
    } else {
      delta = e.deltaY;
      horizontal = e.shiftKey;
    }
    if (e.altKey) {
      // zoom
      e.preventDefault();
      e.stopPropagation();
      if (horizontal) {
        // horizontal zoom
        const pxPerSec = normalizePxPerSec(pxPerSecRef.current * (1 + delta / 1000), 0);
        const cursorX =
          e.clientX - (imgCanvasesRef.current[getIdChArr()[0]].getBoundingClientRect()?.x ?? 0);
        const startSec = normalizeStartSec(
          startSecRef.current + cursorX / pxPerSecRef.current - cursorX / pxPerSec,
          pxPerSec,
          maxTrackSec,
        );
        updateLensParams({startSec, pxPerSec});
      } else {
        // vertical zoom
        const splitView = splitViewElem.current;
        if (!splitView) return;

        const newHeight = Math.round(
          Math.min(Math.max(height * (1 + delta / 1000), MIN_HEIGHT), MAX_HEIGHT),
        );
        setHeight(newHeight);

        const cursorY = e.clientY - splitView.getBoundingClientY();
        const {imgIndex, cursorRatioOnImg, cursorOffset} = vScrollAnchorInfoRef.current;
        // TODO: remove hard-coded 2
        setScrollTop(
          TIME_CANVAS_HEIGHT +
            imgIndex * (newHeight + 2) +
            VERTICAL_AXIS_PADDING +
            cursorRatioOnImg * (newHeight - VERTICAL_AXIS_PADDING * 2) +
            cursorOffset -
            cursorY,
        );
      }
    } else if (horizontal) {
      // horizontal scroll
      e.preventDefault();
      e.stopPropagation();
      updateLensParams({startSec: startSecRef.current + delta / pxPerSecRef.current});
    } else {
      // vertical scroll (native)
      updateVScrollAnchorInfo(e.clientY);
    }
  });

  useEffect(() => {
    splitViewElem.current?.scrollTo({top: scrollTop, behavior: "instant"});
  }, [scrollTop]);

  const drawCanvas = useEvent(async () => {
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
  });

  const reloadAndRefreshTrack = useEvent((id: number) => {
    reloadTracks([id]);
    refreshTracks();
  });
  const removeAndRefreshTrack = useEvent((id: number) => {
    removeTracks([id]);
    refreshTracks();
  });

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

  const createLeftPane = (leftWidth: number) => (
    <>
      <div className={styles.stickyHeader} style={{width: `${leftWidth}px`}}>
        <TimeUnitSection key="time_unit_label" timeUnitLabel={timeUnitLabel} />
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
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
      <div className={`${styles.trackRightHeader}  ${styles.stickyHeader}`}>
        <TimeAxis key="time_axis" ref={timeCanvasElem} width={width} />
        <span className={styles.axisLabelSection}>Amp</span>
        <span className={styles.axisLabelSection}>Hz</span>
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
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
                  maxTrackSec={maxTrackSec}
                />
                <AmpAxis ref={registerAmpCanvas(idChStr)} height={height} />
                <FreqAxis ref={registerFreqCanvas(idChStr)} height={height} />
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

    throttledSetAmpMarkers(imgHeight, imgHeight, {ampRange: ampRangeRef.current});
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
    requestRef.current = requestAnimationFrame(drawCanvas);
    return () => cancelAnimationFrame(requestRef.current);
  }, [drawCanvas]);

  // set LensParams when track changes
  useEffect(() => {
    if (!trackIds.length) return;
    if (prevTrackCountRef.current === 0 || canvasIsFitRef.current) {
      updateLensParams({startSec: 0, pxPerSec: width / maxTrackSec});
    } else {
      const startSec = normalizeStartSec(startSecRef.current, pxPerSecRef.current, maxTrackSec);
      updateLensParams({startSec, pxPerSec: pxPerSecRef.current});
    }

    prevTrackCountRef.current = trackIds.length;
  }, [trackIds, width, maxTrackSec, updateLensParams, normalizeStartSec, normalizePxPerSec]);

  useEffect(() => {
    const currentIdChArr = needRefreshTrackIdChArr.length ? needRefreshTrackIdChArr : getIdChArr();
    if (currentIdChArr.length) throttledSetImgState(currentIdChArr, width, imgHeight);
  }, [throttledSetImgState, getIdChArr, width, imgHeight, needRefreshTrackIdChArr]);

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
          moveLens={moveLens}
          resizeLensLeft={resizeLensLeft}
          resizeLensRight={resizeLensRight}
        />
        <SlideBar />
      </div>
      <div
        className={`${styles.MainViewer} row-flex`}
        ref={mainViewerElemCallback}
        onMouseMove={onMouseMove}
      >
        {isDropzoneActive && <div className={styles.dropzone} />}
        <SplitView
          ref={splitViewElem}
          createLeft={createLeftPane}
          right={rightPane}
          setCanvasWidth={setWidth}
        />
        <ColorMap
          height={colorMapHeight}
          colorBarHeight={colorBarHeight}
          setHeight={setColorMapHeight}
          dbAxisCanvasElem={dbCanvasElem}
        />
      </div>
    </>
  );
}

export default MainViewer;
