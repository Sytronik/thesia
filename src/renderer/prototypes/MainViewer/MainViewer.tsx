import React, {
  useRef,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useContext,
  useLayoutEffect,
} from "react";
import {throttle} from "throttle-debounce";
import useDropzone from "renderer/hooks/useDropzone";
import useRefs from "renderer/hooks/useRefs";
import ImgCanvas from "renderer/modules/ImgCanvas";
import SplitView from "renderer/modules/SplitView";
import useThrottledSetMarkers from "renderer/hooks/useThrottledSetMarkers";
import useEvent from "react-use-event-hook";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./MainViewer.module.scss";
import AmpAxis from "./AmpAxis";
import ColorMap from "./ColorMap";
import ErrorBox from "./ErrorBox";
import FreqAxis from "./FreqAxis";
import Overview from "../Overview/Overview";
import TrackInfo from "./TrackInfo";
import TimeUnitSection from "./TimeUnitSection";
import TimeAxis from "./TimeAxis";
import TrackAddButtonSection from "./TrackAddButtonSection";
import BackendAPI from "../../api";
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
  DEFAULT_AMP_RANGE,
} from "../constants";
import isCommand from "../../utils/commandKey";

type MainViewerProps = {
  trackIds: number[];
  erroredTrackIds: number[];
  selectedTrackIds: number[];
  trackIdChMap: IdChMap;
  needRefreshTrackIdChArr: IdChArr;
  maxTrackSec: number;
  blend: number;
  addDroppedFile: (e: DragEvent) => Promise<void>;
  reloadTracks: (ids: number[]) => Promise<void>;
  refreshTracks: () => Promise<void>;
  ignoreError: (id: number) => void;
  removeTracks: (ids: number[]) => void;
  selectTrack: (e: Event | React.MouseEvent, id: number) => void;
  finishRefreshTracks: () => void;
};

function MainViewer(props: MainViewerProps) {
  const {
    trackIds,
    erroredTrackIds,
    selectedTrackIds,
    trackIdChMap,
    needRefreshTrackIdChArr,
    maxTrackSec,
    blend,
    addDroppedFile,
    ignoreError,
    refreshTracks,
    reloadTracks,
    removeTracks,
    selectTrack,
    finishRefreshTracks,
  } = props;

  const mainViewerElem = useRef<HTMLDivElement | null>(null);
  const prevTrackCountRef = useRef<number>(0);

  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(100);
  const [canvasIsFit, setCanvasIsFit] = useState<boolean>(true);
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
  const dBCanvasElem = useRef<AxisCanvasHandleElement>(null);

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [ampCanvasesRef, registerAmpCanvas] = useRefs<AxisCanvasHandleElement>();
  const [freqCanvasesRef, registerFreqCanvas] = useRefs<AxisCanvasHandleElement>();
  const [trackInfosRef, registerTrackInfos] = useRefs<TrackInfoElement>();

  const prevCursorClientY = useRef<number>(0);
  const vScrollAnchorInfoRef = useRef<VScrollAnchorInfo>({
    imgIndex: 0,
    cursorRatioOnImg: 0.0,
    cursorOffset: 0,
  });

  const {isDropzoneActive} = useDropzone({targetRef: mainViewerElem, handleDrop: addDroppedFile});

  const getIdChArr = useCallback(() => Array.from(trackIdChMap.values()).flat(), [trackIdChMap]); // TODO: return only viewport

  const reloadAndRefreshTracks = useEvent(async (ids: number[]) => {
    await reloadTracks(ids);
    await refreshTracks();
  });
  const removeAndRefreshTracks = useEvent(async (ids: number[]) => {
    removeTracks(ids);
    await refreshTracks();
  });

  const {
    markersAndLengthRef: timeMarkersAndLengthRef,
    throttledSetMarkers: throttledSetTimeMarkers,
    resetMarkers: resetTimeMarkers,
  } = useThrottledSetMarkers({
    scaleTable: TIME_TICK_SIZE,
    boundaries: TIME_BOUNDARIES,
    getMarkers: BackendAPI.getTimeAxisMarkers,
  });

  const throttledSetTimeMarkersAndUnit = useCallback(() => {
    throttledSetTimeMarkers(width, pxPerSecRef.current, {
      startSec: startSecRef.current,
      endSec: startSecRef.current + width / pxPerSecRef.current,
    });
    const [markers] = timeMarkersAndLengthRef.current;
    if (markers.length === 0) return;
    const timeUnit = markers[markers.length - 1][1];
    setTimeUnitLabel(timeUnit);
  }, [throttledSetTimeMarkers, width, timeMarkersAndLengthRef]);

  const unsetTimeMarkersAndUnit = useEvent(() => {
    resetTimeMarkers();
    setTimeUnitLabel("");
  });

  const {markersAndLengthRef: ampMarkersAndLengthRef, throttledSetMarkers: throttledSetAmpMarkers} =
    useThrottledSetMarkers({
      scaleTable: AMP_TICK_NUM,
      boundaries: AMP_BOUNDARIES,
      getMarkers: BackendAPI.getAmpAxisMarkers,
    });

  const {
    markersAndLengthRef: freqMarkersAndLengthRef,
    throttledSetMarkers: throttledSetFreqMarkers,
  } = useThrottledSetMarkers({
    scaleTable: FREQ_TICK_NUM,
    boundaries: FREQ_BOUNDARIES,
    getMarkers: BackendAPI.getFreqAxisMarkers,
  });

  const {
    markersAndLengthRef: dBMarkersAndLengthRef,
    throttledSetMarkers: throttledSetdBMarkers,
    resetMarkers: resetdBMarkers,
  } = useThrottledSetMarkers({
    scaleTable: DB_TICK_NUM,
    boundaries: DB_BOUNDARIES,
    getMarkers: BackendAPI.getdBAxisMarkers,
  });

  const throttledSetImgState = useMemo(
    () =>
      throttle(1000 / 70, async (idChArr: IdChArr, canvasWidth: number, canvasHeight: number) => {
        if (!idChArr.length) return;

        await BackendAPI.setImageState(
          idChArr,
          startSecRef.current,
          canvasWidth * devicePixelRatio,
          canvasHeight * devicePixelRatio,
          pxPerSecRef.current * devicePixelRatio,
          {amp_range: ampRangeRef.current, dpr: devicePixelRatio},
          blend,
        );
      }),
    [blend, devicePixelRatio],
  );

  const setAmpRange = useEvent((newRange: [number, number]) => {
    ampRangeRef.current = newRange;
    throttledSetImgState(getIdChArr(), width, imgHeight);
    throttledSetAmpMarkers(imgHeight, imgHeight, {ampRange: ampRangeRef.current});
  });

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
    setCanvasIsFit(
      startSec <= FIT_TOLERANCE_SEC && width >= (maxTrackSec - FIT_TOLERANCE_SEC) * pxPerSec,
    );

    Object.values(imgCanvasesRef.current).forEach((value) =>
      value?.updateLensParams({startSec, pxPerSec}),
    );
    throttledSetImgState(getIdChArr(), width, imgHeight);
    throttledSetTimeMarkersAndUnit();
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

  const zoomHeight = useEvent((delta: number) => {
    const newHeight = Math.round(
      Math.min(Math.max(height * (1 + delta / 1000), MIN_HEIGHT), MAX_HEIGHT),
    );
    setHeight(newHeight);
    return newHeight;
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

    if (!e.altKey && !horizontal) {
      // vertical scroll (native)
      updateVScrollAnchorInfo(e.clientY);
      return;
    }

    e.preventDefault();
    e.stopPropagation();
    const anImgBoundngRect = imgCanvasesRef.current[getIdChArr()[0]].getBoundingClientRect();
    if (e.clientX > (anImgBoundngRect?.right ?? 0) || e.clientX < (anImgBoundngRect?.x ?? 0))
      return;

    if (e.altKey) {
      // zoom
      if (horizontal) {
        // horizontal zoom
        const pxPerSec = normalizePxPerSec(pxPerSecRef.current * (1 + delta / 1000), 0);
        const cursorX = e.clientX - (anImgBoundngRect?.x ?? 0);
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

        const newHeight = zoomHeight(delta);

        const cursorY = e.clientY - (splitView.getBoundingClientRect()?.y ?? 0);
        const {imgIndex, cursorRatioOnImg, cursorOffset} = vScrollAnchorInfoRef.current;
        // TODO: remove hard-coded 2
        setScrollTop(
          imgIndex * (newHeight + 2) +
            VERTICAL_AXIS_PADDING +
            cursorRatioOnImg * (newHeight - VERTICAL_AXIS_PADDING * 2) +
            cursorOffset -
            cursorY,
        );
      }
    } else if (horizontal) {
      // horizontal scroll
      updateLensParams({startSec: startSecRef.current + delta / pxPerSecRef.current});
    }
  });

  const deleteSelectedTracks = useEvent(async (e: KeyboardEvent) => {
    e.preventDefault();
    if (selectedTrackIds.length) {
      await removeAndRefreshTracks(selectedTrackIds);
    }
  });

  const handleKeyDown = useEvent(async (e: KeyboardEvent) => {
    if ((e.target as HTMLElement | null)?.tagName !== "BODY") return;
    if (isCommand(e)) {
      const calcPxPerSecDelta = () => 10 ** (Math.floor(Math.log10(pxPerSecRef.current)) - 1);
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          zoomHeight(100);
          break;
        case "ArrowUp":
          e.preventDefault();
          zoomHeight(-100);
          break;
        case "ArrowRight":
          e.preventDefault();
          updateLensParams({pxPerSec: pxPerSecRef.current + calcPxPerSecDelta()});
          break;
        case "ArrowLeft":
          e.preventDefault();
          updateLensParams({pxPerSec: pxPerSecRef.current - calcPxPerSecDelta()});
          break;
        default:
          break;
      }
    } else {
      switch (e.key) {
        case "Delete":
        case "Backspace":
          e.preventDefault();
          await deleteSelectedTracks(e);
          break;
        case "ArrowDown":
          e.preventDefault();
          selectTrack(
            e,
            trackIds[
              Math.min(
                trackIds.indexOf(selectedTrackIds[selectedTrackIds.length - 1]) + 1,
                trackIds.length - 1,
              )
            ],
          );
          break;
        case "ArrowUp":
          e.preventDefault();
          selectTrack(e, trackIds[Math.max(trackIds.indexOf(selectedTrackIds[0]) - 1, 0)]);
          break;
        case "ArrowRight":
          e.preventDefault();
          updateLensParams({startSec: startSecRef.current + 10 / pxPerSecRef.current});
          break;
        case "ArrowLeft":
          e.preventDefault();
          updateLensParams({startSec: startSecRef.current - 10 / pxPerSecRef.current});
          break;
        default:
          break;
      }
    }
  });

  useEffect(() => {
    splitViewElem.current?.scrollTo({top: scrollTop, behavior: "instant"});
  }, [scrollTop]);

  const drawCanvas = useEvent(async () => {
    getIdChArr().forEach((idChStr) => {
      ampCanvasesRef.current[idChStr]?.draw(ampMarkersAndLengthRef.current);
      freqCanvasesRef.current[idChStr]?.draw(freqMarkersAndLengthRef.current);
    });
    timeCanvasElem.current?.draw(timeMarkersAndLengthRef.current);
    dBCanvasElem.current?.draw(dBMarkersAndLengthRef.current);

    const images = BackendAPI.getImages();
    Object.entries(images).forEach(([idChStr, buf]) => {
      imgCanvasesRef.current[idChStr]?.draw(buf);
    });
    await overviewElem.current?.draw(startSecRef.current, width / pxPerSecRef.current);
    requestRef.current = requestAnimationFrame(drawCanvas);
  });

  const trackSummaryArr = useMemo(
    () =>
      trackIds.map((trackId) => {
        return {
          fileName: BackendAPI.getFileName(trackId),
          time: new Date(BackendAPI.getLengthSec(trackId) * 1000).toISOString().substring(11, 23),
          sampleFormat: BackendAPI.getSampleFormat(trackId),
          sampleRate: `${BackendAPI.getSampleRate(trackId)} Hz`,
          globalLUFS: `${BackendAPI.getGlobalLUFS(trackId).toFixed(2)} LUFS`,
        };
      }),
    [trackIds, needRefreshTrackIdChArr], // eslint-disable-line react-hooks/exhaustive-deps
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
            ref={registerTrackInfos(`${trackId}`)}
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
        <TimeAxis
          key="time_axis"
          ref={timeCanvasElem}
          width={width}
          shiftWhenResize={!canvasIsFit}
          startSecRef={startSecRef}
          pxPerSecRef={pxPerSecRef}
          moveLens={moveLens}
        />
        <span className={styles.axisLabelSection}>Amp</span>
        <span className={styles.axisLabelSection}>Hz</span>
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
      {trackIds.map((id) => (
        <div key={`${id}`} className={`${styles.trackRight}`}>
          {erroredTrackIds.includes(id) ? (
            <ErrorBox
              trackId={id}
              handleReload={(trackId) => reloadAndRefreshTracks([trackId])}
              handleIgnore={ignoreError}
              handleClose={(trackId) => removeAndRefreshTracks([trackId])}
            />
          ) : null}
          {trackIdChMap.get(id)?.map((idChStr) => {
            return (
              <div
                key={idChStr}
                className={styles.chCanvases}
                role="presentation"
                onClick={(e) => {
                  selectTrack(e, id);
                }}
              >
                <ImgCanvas
                  ref={registerImgCanvas(idChStr)}
                  width={width}
                  height={imgHeight}
                  maxTrackSec={maxTrackSec}
                  canvasIsFit={canvasIsFit}
                />
                <AmpAxis
                  ref={registerAmpCanvas(idChStr)}
                  height={height}
                  ampRangeRef={ampRangeRef}
                  setAmpRange={setAmpRange}
                />
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
      resetdBMarkers();
      return;
    }

    throttledSetdBMarkers(colorBarHeight, colorBarHeight, {});
  }, [resetdBMarkers, throttledSetdBMarkers, colorBarHeight, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    if (!trackIds.length) {
      unsetTimeMarkersAndUnit();
      return;
    }

    throttledSetTimeMarkersAndUnit();
  }, [unsetTimeMarkersAndUnit, throttledSetTimeMarkersAndUnit, trackIds, needRefreshTrackIdChArr]);

  useEffect(() => {
    requestRef.current = requestAnimationFrame(drawCanvas);
    return () => cancelAnimationFrame(requestRef.current);
  }, [drawCanvas]);

  useEffect(() => {
    if (selectedTrackIds.length === 0) return;
    const selectedIdUnderscore = `${selectedTrackIds[selectedTrackIds.length - 1]}_`;
    if (needRefreshTrackIdChArr.some((idCh: string) => idCh.startsWith(selectedIdUnderscore)))
      overviewElem.current?.draw(startSecRef.current, width / pxPerSecRef.current, true);
  }, [needRefreshTrackIdChArr]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (selectedTrackIds.length === 0) return;
    const selectedIdStr = `${selectedTrackIds[selectedTrackIds.length - 1]}`;
    const trackInfo = trackInfosRef.current[selectedIdStr];
    if (trackInfo === null) return;
    const infoRect = trackInfo.getBoundingClientRect();
    const viewElem = splitViewElem.current;
    const viewRect = viewElem?.getBoundingClientRect() ?? null;
    if (infoRect === null || viewElem === null || viewRect === null) return;
    const infoMiddle = infoRect.top + infoRect.height / 2;
    if (infoMiddle < viewRect.top) {
      viewElem.scrollTo({top: infoRect.top - viewRect.top, behavior: "smooth"});
    } else if (infoMiddle > viewRect.bottom) {
      viewElem.scrollTo({top: infoRect.bottom - viewRect.bottom, behavior: "smooth"});
    }
  }, [selectedTrackIds, trackInfosRef]);

  // set LensParams when track list or width change
  useLayoutEffect(() => {
    if (trackIds.length > 0) {
      const startSec =
        prevTrackCountRef.current === 0 || canvasIsFit
          ? 0
          : normalizeStartSec(startSecRef.current, pxPerSecRef.current, maxTrackSec);
      const pxPerSec = canvasIsFit
        ? width / maxTrackSec
        : normalizePxPerSec(pxPerSecRef.current, startSec);
      updateLensParams({startSec, pxPerSec});
    }

    prevTrackCountRef.current = trackIds.length;
  }, [
    trackIds,
    width,
    maxTrackSec,
    canvasIsFit,
    updateLensParams,
    normalizeStartSec,
    normalizePxPerSec,
  ]);

  const refreshImgs = useCallback(() => {
    if (needRefreshTrackIdChArr.length > 0) {
      throttledSetImgState(needRefreshTrackIdChArr, width, imgHeight);
      finishRefreshTracks();
    } else {
      throttledSetImgState(getIdChArr(), width, imgHeight);
    }
  }, [
    throttledSetImgState,
    getIdChArr,
    width,
    imgHeight,
    needRefreshTrackIdChArr,
    finishRefreshTracks,
  ]);

  useEffect(refreshImgs, [refreshImgs]);

  const mainViewerElemCallback = useCallback(
    (node: HTMLDivElement | null) => {
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

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  return (
    <div className={`flex-container-column flex-item-auto ${styles.mainViewerWrapper}`}>
      <div className="flex-container-row flex-item-fixed">
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
      </div>
      <div
        className={`flex-container-row flex-item-auto ${styles.MainViewer}`}
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
          dBAxisCanvasElem={dBCanvasElem}
        />
      </div>
    </div>
  );
}

export default MainViewer;
