import React, {useRef, useCallback, useEffect, useMemo, useState, useLayoutEffect} from "react";
import {throttle} from "throttle-debounce";
import useRefs from "renderer/hooks/useRefs";
import ImgCanvas from "renderer/modules/ImgCanvas";
import SplitView from "renderer/modules/SplitView";
import useAxisMarkers from "renderer/hooks/useAxisMarkers";
import useEvent from "react-use-event-hook";
import {useHotkeys} from "react-hotkeys-hook";
import {Player} from "renderer/hooks/usePlayer";
import Locator from "renderer/modules/Locator";
import {ipcRenderer} from "electron";
import {DropTargetMonitor, XYCoord} from "react-dnd";
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
  BIG_SHIFT_PX,
  SHIFT_PX,
  TINY_MARGIN,
  TIME_CANVAS_HEIGHT,
} from "../constants/tracks";
import {isApple} from "../../utils/osSpecifics";
import TrackInfoDragLayer from "./TrackInfoDragLayer";

type MainViewerProps = {
  trackIds: number[];
  erroredTrackIds: number[];
  selectedTrackIds: number[];
  selectionIsAdded: boolean;
  trackIdChMap: IdChMap;
  isLoading: boolean;
  needRefreshTrackIdChArr: IdChArr;
  maxTrackSec: number;
  maxTrackHz: number;
  blend: number;
  player: Player;
  addDroppedFile: (item: {files: File[]}, index: number) => Promise<void>;
  reloadTracks: (ids: number[]) => Promise<void>;
  refreshTracks: () => Promise<void>;
  ignoreError: (id: number) => void;
  removeTracks: (ids: number[]) => void;
  hideTracks: (dragId: number, ids: number[]) => number;
  changeTrackOrder: (dragIndex: number, hoverIndex: number) => void;
  showHiddenTracks: (hoverIndex: number) => void;
  selectTrack: (e: MouseOrKeyboardEvent, id: number, trackIds: number[]) => void;
  selectAllTracks: (trackIds: number[]) => void;
  finishRefreshTracks: () => void;
};

function MainViewer(props: MainViewerProps) {
  const {
    trackIds,
    erroredTrackIds,
    selectedTrackIds,
    selectionIsAdded,
    trackIdChMap,
    isLoading,
    needRefreshTrackIdChArr,
    maxTrackSec,
    maxTrackHz,
    blend,
    player,
    addDroppedFile,
    ignoreError,
    refreshTracks,
    reloadTracks,
    removeTracks,
    hideTracks,
    changeTrackOrder,
    showHiddenTracks,
    selectTrack,
    selectAllTracks,
    finishRefreshTracks,
  } = props;

  const mainViewerElem = useRef<HTMLDivElement | null>(null);
  const prevTrackCountRef = useRef<number>(0);

  const [startSec, setStartSec] = useState<number>(0);
  const [pxPerSec, setPxPerSec] = useState<number>(100);
  const prevSelectSecRef = useRef<number>(0);
  const [canvasIsFit, setCanvasIsFit] = useState<boolean>(true);
  const [hzRange, setHzRange] = useState<[number, number]>([0, Infinity]);
  const setHzRangeIfNotSame = useEvent((newHzRange: [number, number]) => {
    if (hzRange[0] !== newHzRange[0] || hzRange[1] !== newHzRange[1]) setHzRange(newHzRange);
  });
  const [ampRange, setAmpRange] = useState<[number, number]>([...DEFAULT_AMP_RANGE]);
  const setAmpRangeIfNotSame = useEvent((newAmpRange: [number, number]) => {
    if (ampRange[0] !== newAmpRange[0] || ampRange[1] !== newAmpRange[1]) setAmpRange(newAmpRange);
  });

  const [width, setWidth] = useState(600);
  const endSec = startSec + width / (pxPerSec + 1e-8);

  const [height, setHeight] = useState(250);
  const [scrollTop, setScrollTop] = useState(0);
  const imgHeight = height - 2 * VERTICAL_AXIS_PADDING;
  const [colorMapHeight, setColorMapHeight] = useState<number>(250);
  const colorBarHeight = colorMapHeight - 2 * VERTICAL_AXIS_PADDING;

  const splitViewElem = useRef<SplitViewHandleElement>(null);
  const timeAxisCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const selectLocatorElem = useRef<LocatorHandleElement>(null);

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [trackInfosRef, registerTrackInfos] = useRefs<TrackInfoElement>();

  const needFollowCursor = useRef<boolean>(true);
  const prevCursorClientY = useRef<number>(0);
  const vScrollAnchorInfoRef = useRef<VScrollAnchorInfo>({
    imgIndex: 0,
    cursorRatioOnImg: 0.0,
    cursorOffset: 0,
  });

  const [fileDropIndex, setFileDropIndex] = useState<number>(-1);

  const trackIdsWithFileDropIndicator = useMemo(() => {
    // >=0 means a normal track, -1 means a file drop indicator
    if (fileDropIndex === -1) return trackIds;
    const result = [...trackIds];
    result.splice(fileDropIndex, 0, -1);
    return result;
  }, [trackIds, fileDropIndex]);

  const onFileHover = useEvent((item: any, monitor: DropTargetMonitor) => {
    const clientOffset = monitor.getClientOffset();
    if (clientOffset === null) return;

    const notlast = trackIds.some((id, index) => {
      const trackInfoElem = trackInfosRef.current[`${id}`];
      if (!trackInfoElem) return false;
      const rect = trackInfoElem.getBoundingClientRect();
      if (!rect) return false;
      if ((clientOffset as XYCoord).y >= rect.y + rect.height / 2) {
        return false;
      }
      setFileDropIndex(index);
      return true;
    });
    if (!notlast) setFileDropIndex(trackIds.length);
  });

  const onFileHoverLeave = useEvent(() => setFileDropIndex(-1));

  const onFileDrop = useEvent((item) => {
    addDroppedFile(item, fileDropIndex);
    setFileDropIndex(-1);
  });

  const getIdChArr = useEvent(() => Array.from(trackIdChMap.values()).flat());

  const timeMarkersDrawOptions = useMemo(
    () => ({startSec, endSec, maxSec: maxTrackSec}),
    [endSec, maxTrackSec, startSec],
  );
  const timeMarkersAndLength = useAxisMarkers({
    scaleTable: TIME_TICK_SIZE,
    boundaries: TIME_BOUNDARIES,
    getMarkers: BackendAPI.getTimeAxisMarkers,
    canvasLength: trackIds.length > 0 ? width : 0,
    scaleDeterminant: pxPerSec,
    drawOptions: timeMarkersDrawOptions,
  });
  const timeUnitLabel = useMemo(() => {
    if (!trackIds.length) return "";

    const [markers] = timeMarkersAndLength;
    if (markers.length === 0) return "";
    return markers[markers.length - 1][1];
  }, [timeMarkersAndLength, trackIds]);

  const ampMarkersDrawOptions = useMemo(() => ({ampRange}), [ampRange]);
  const ampMarkersAndLength = useAxisMarkers({
    scaleTable: AMP_TICK_NUM,
    boundaries: AMP_BOUNDARIES,
    getMarkers: BackendAPI.getAmpAxisMarkers,
    canvasLength: imgHeight,
    scaleDeterminant: imgHeight,
    drawOptions: ampMarkersDrawOptions,
  });

  const {freqScale} = BackendAPI.getSpecSetting();
  const freqMarkersDrawOptions = useMemo(
    () => ({maxTrackHz, hzRange, freqScale}),
    [maxTrackHz, hzRange, freqScale],
  );
  const freqMarkersAndLength = useAxisMarkers({
    scaleTable: FREQ_TICK_NUM,
    boundaries: FREQ_BOUNDARIES,
    getMarkers: BackendAPI.getFreqAxisMarkers,
    canvasLength: imgHeight,
    scaleDeterminant: imgHeight,
    drawOptions: freqMarkersDrawOptions,
  });

  const [minMaxdB, setMinMaxdB] = useState<{mindB: number; maxdB: number}>({
    mindB: -100,
    maxdB: 0,
  });

  const dBMarkersAndLength = useAxisMarkers({
    scaleTable: DB_TICK_NUM,
    boundaries: DB_BOUNDARIES,
    getMarkers: BackendAPI.getdBAxisMarkers,
    canvasLength: trackIds.length > 0 ? colorBarHeight : 0,
    scaleDeterminant: colorBarHeight,
    drawOptions: minMaxdB,
  });

  useEffect(() => {
    if (trackIds.length === 0) return;
    Promise.all([BackendAPI.getMindB(), BackendAPI.getMaxdB()])
      .then(([mindB, maxdB]) => setMinMaxdB({mindB, maxdB}))
      .catch(() => {});
  }, [trackIds, needRefreshTrackIdChArr]);

  const setSelectSec = useEvent((sec) => {
    player.setSelectSec(sec);
    selectLocatorElem.current?.draw();
  });
  const throttledSetSelectSec = useMemo(() => throttle(1000 / 70, setSelectSec), [setSelectSec]);

  const normalizeStartSec = useEvent((_startSec, _pxPerSec, maxEndSec) => {
    return Math.min(Math.max(_startSec, 0), Math.max(maxEndSec - width / _pxPerSec, 0));
  });

  const normalizePxPerSec = useEvent((_pxPerSec, _startSec) =>
    Math.min(
      Math.max(_pxPerSec, width / (maxTrackSec - _startSec)),
      Math.max(MAX_PX_PER_SEC, width / (maxTrackSec - _startSec)),
    ),
  );

  const updateLensParams = useEvent(
    (params: OptionalLensParams, turnOffFollowCursor: boolean = true) => {
      if (player.isPlaying && turnOffFollowCursor) {
        needFollowCursor.current = false;
      }
      let newStartSec = params.startSec ?? startSec;
      let newPxPerSec = params.pxPerSec ?? pxPerSec;

      if (newStartSec !== startSec)
        newStartSec = normalizeStartSec(newStartSec, newPxPerSec, maxTrackSec);
      if (newPxPerSec !== pxPerSec) newPxPerSec = normalizePxPerSec(newPxPerSec, newStartSec);

      setStartSec(newStartSec);
      setPxPerSec(newPxPerSec);
      setCanvasIsFit(
        newStartSec <= FIT_TOLERANCE_SEC &&
          width >= (maxTrackSec - FIT_TOLERANCE_SEC) * newPxPerSec,
      );
    },
  );

  const moveLens = useEvent((sec: number, anchorRatio: number) => {
    const lensDurationSec = width / pxPerSec;
    updateLensParams({startSec: sec - lensDurationSec * anchorRatio});
  });

  const resizeLensLeft = useEvent((sec: number) => {
    const newStartSec = normalizeStartSec(sec, MAX_PX_PER_SEC, endSec);
    const newPxPerSec = normalizePxPerSec(width / (endSec - newStartSec), newStartSec);

    updateLensParams({startSec: newStartSec, pxPerSec: newPxPerSec});
  });

  const resizeLensRight = useEvent((sec: number) => {
    const newPxPerSec = normalizePxPerSec(width / Math.max(sec - startSec, 0), startSec);
    updateLensParams({pxPerSec: newPxPerSec});
  });

  const zoomHeight = useEvent((delta: number) => {
    const newHeight = Math.round(Math.min(Math.max(height + delta, MIN_HEIGHT), MAX_HEIGHT));
    setHeight(newHeight);
    return newHeight;
  });

  const zoomHeightAtCursor = useEvent((delta, cursorY) => {
    const newHeight = zoomHeight((delta * height) / 1000);
    const {imgIndex, cursorRatioOnImg, cursorOffset} = vScrollAnchorInfoRef.current;
    // TODO: remove hard-coded 2
    setScrollTop(
      imgIndex * (newHeight + 2) +
        VERTICAL_AXIS_PADDING +
        cursorRatioOnImg * (newHeight - VERTICAL_AXIS_PADDING * 2) +
        cursorOffset -
        cursorY,
    );
  });

  const updateVScrollAnchorInfo = useEvent((cursorClientY: number) => {
    let i = 0;
    let prevBottom = 0;
    trackIds.forEach((id) =>
      trackIdChMap.get(id)?.forEach((idChStr) => {
        const imgClientRect = imgCanvasesRef.current[idChStr]?.getBoundingClientRect();
        if (imgClientRect === undefined) return;
        const bottom = imgClientRect.y + imgClientRect.height;
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
    if (Math.abs(e.clientY - prevCursorClientY.current) < 1e-3) return;
    updateVScrollAnchorInfo(e.clientY);
    prevCursorClientY.current = e.clientY;
  };

  const handleWheel = useEvent((e: WheelEvent) => {
    if (!trackIds.length) return;

    let horizontal: boolean;
    let delta: number;
    const isApplePinch = isApple() && e.ctrlKey;
    if (isApplePinch) {
      horizontal = !e.shiftKey;
      if (horizontal) delta = -12 * e.deltaY;
      else delta = -6 * e.deltaY;
    } else if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) {
      horizontal = !e.shiftKey;
      delta = e.deltaX;
    } else {
      horizontal = e.shiftKey;
      delta = e.deltaY;
    }

    if (!e.altKey && !isApplePinch && !horizontal) {
      // vertical scroll (native)
      selectLocatorElem.current?.disableInteraction();
      setTimeout(() => selectLocatorElem.current?.enableInteraction(), 1000 / 60);
      updateVScrollAnchorInfo(e.clientY);
      return;
    }

    e.preventDefault();
    const anImgBoundngRect = imgCanvasesRef.current[getIdChArr()[0]].getBoundingClientRect();
    if (e.clientX > (anImgBoundngRect?.right ?? 0) || e.clientX < (anImgBoundngRect?.x ?? 0))
      return;

    if (isApplePinch || e.altKey) {
      // zoom
      if (horizontal) {
        // horizontal zoom
        const newPxPerSec = normalizePxPerSec(pxPerSec * (1 + delta / 1000), 0);
        const cursorX = e.clientX - (anImgBoundngRect?.x ?? 0);
        const newStartSec = normalizeStartSec(
          startSec + cursorX / pxPerSec - cursorX / newPxPerSec,
          newPxPerSec,
          maxTrackSec,
        );
        updateLensParams({startSec: newStartSec, pxPerSec: newPxPerSec});
      } else {
        // vertical zoom
        const splitView = splitViewElem.current;
        if (!splitView) return;

        const cursorY = e.clientY - (splitView.getBoundingClientRect()?.y ?? 0);
        zoomHeightAtCursor(delta, cursorY);
      }
    } else if (horizontal) {
      // horizontal scroll
      updateLensParams({startSec: startSec + (0.5 * delta) / pxPerSec});
    }
  });

  const [hiddenIdChArr, setHiddenIdChArr] = useState<Set<string>>(new Set());
  const onVerticalViewportChange = useEvent(() => {
    const newHiddenIdChArr = new Set<string>();
    getIdChArr().forEach((idChStr) => {
      if (imgCanvasesRef.current[idChStr] === undefined) {
        newHiddenIdChArr.add(idChStr);
        return;
      }
      const imgCanvas = imgCanvasesRef.current[idChStr];
      const rect = imgCanvas.getBoundingClientRect();
      const splitViewRect = splitViewElem.current?.getBoundingClientRect() ?? new DOMRect();
      // check if the canvas is entirely outside of viewport
      if (rect.y > splitViewRect.y + splitViewRect.height || rect.y + rect.height < splitViewRect.y)
        newHiddenIdChArr.add(idChStr);
    });
    if (
      hiddenIdChArr.size === newHiddenIdChArr.size &&
      [...hiddenIdChArr].every((x) => newHiddenIdChArr.has(x))
    )
      return;
    setHiddenIdChArr(newHiddenIdChArr);
  });
  useEffect(onVerticalViewportChange, [onVerticalViewportChange, height, trackIdChMap]);

  const draggingTrackIdRef = useRef<number>(-1);
  const hideDraggingImage = useEvent((id) => {
    draggingTrackIdRef.current = id;
  });
  const unHideDraggingImage = useEvent(() => {
    draggingTrackIdRef.current = -1;
    onVerticalViewportChange();
  });

  // without useEvent, sometimes (when busy?) onClick event is not handled by this function.
  const changeLocatorByMouse = useEvent(
    (
      e: React.MouseEvent | MouseEvent,
      isPlayhead: boolean = false,
      allowOutside: boolean = true,
    ) => {
      const rect = timeAxisCanvasElem.current?.getBoundingClientRect() ?? null;
      if (rect === null) return;
      if (e.clientY < rect.bottom && e.altKey) return; // alt+click on TimeAxis fires the other event
      e.preventDefault();
      if (trackIds.length === 0) return;
      if (e.clientY < rect.top) return; // when cursor is between Overview and TimeAxis
      const cursorX = e.clientX - rect.left;
      if (!allowOutside) {
        if (cursorX < 0 || cursorX >= width) return;
        if (isPlayhead) {
          const lastTrackIdChArr = trackIdChMap.get(trackIds[trackIds.length - 1]);
          if (lastTrackIdChArr) {
            const lastIdCh = lastTrackIdChArr[lastTrackIdChArr.length - 1];
            const lastChImgRect = imgCanvasesRef.current[lastIdCh].getBoundingClientRect();
            if (e.clientY > lastChImgRect.bottom) return;
          }
        }
      }
      const sec = startSec + cursorX / pxPerSec;
      if (isPlayhead) player.seek(sec);
      else throttledSetSelectSec(sec);
    },
  );

  const endSelectLocatorDrag = useEvent(() => {
    document.removeEventListener("mousemove", changeLocatorByMouse);
  });

  // Browsing Hotkeys
  useHotkeys(
    "right, left, shift+right, shift+left",
    (_, hotkey) => {
      if (trackIds.length === 0) return;
      const shiftPx = hotkey.shift ? BIG_SHIFT_PX : SHIFT_PX;
      let shiftSec = shiftPx / pxPerSec;
      if (hotkey.keys?.join("") === "left") shiftSec = -shiftSec;
      updateLensParams({startSec: startSec + shiftSec});
    },
    [trackIds, updateLensParams],
  );

  const setScrollTopBySelectedTracks = useEvent((newHeight: number) => {
    if (splitViewElem.current === null) return;
    const splitViewHeight =
      (splitViewElem.current.getBoundingClientRect()?.height ?? 0) - TIME_CANVAS_HEIGHT - 2;
    const scrollMiddle = splitViewElem.current.scrollTop() + splitViewHeight / 2;
    const residualHeight = (scrollMiddle - TIME_CANVAS_HEIGHT - 2) % height;
    const idxViewportTrack = (scrollMiddle - TIME_CANVAS_HEIGHT - 2 - residualHeight) / height;
    setScrollTop(
      // TIME_CANVAS_HEIGHT will be added in SplitViewElem.current.scrollTo
      2 +
        newHeight * idxViewportTrack +
        (residualHeight * newHeight) / height -
        splitViewHeight / 2,
    );
  });
  const zoomHeightAndScrollToSelectedTrack = (isZoomOut: boolean) => {
    if (trackIds.length === 0) return;

    let delta = 2 ** (Math.floor(Math.log2(height)) - 1.2);
    if (isZoomOut) delta = -delta;
    setScrollTopBySelectedTracks(zoomHeight(delta));
  };
  const freqZoomIn = useEvent(() => zoomHeightAndScrollToSelectedTrack(false));
  const freqZoomOut = useEvent(() => zoomHeightAndScrollToSelectedTrack(true));
  useHotkeys("mod+down", freqZoomIn, {preventDefault: true}, [freqZoomIn]);
  useHotkeys("mod+up", freqZoomOut, {preventDefault: true}, [freqZoomOut]);
  useEffect(() => {
    ipcRenderer.on("freq-zoom-in", freqZoomIn);
    ipcRenderer.on("freq-zoom-out", freqZoomOut);
    return () => {
      ipcRenderer.removeAllListeners("freq-zoom-in");
      ipcRenderer.removeAllListeners("freq-zoom-out");
    };
  }, [freqZoomIn, freqZoomOut]);

  const zoomLens = useEvent((isZoomOut: boolean) => {
    let pxPerSecDelta = 2 ** (Math.floor(Math.log2(pxPerSec)) - 1.2);
    if (isZoomOut) pxPerSecDelta = -pxPerSecDelta;

    const newPxPerSec = normalizePxPerSec(pxPerSec + pxPerSecDelta, 0);
    const selectSec = player.selectSecRef.current ?? 0;
    const newStartSec = normalizeStartSec(
      selectSec - ((selectSec - startSec) * pxPerSec) / newPxPerSec,
      newPxPerSec,
      maxTrackSec,
    );
    updateLensParams({startSec: newStartSec, pxPerSec: newPxPerSec});
  });
  const timeZoomIn = useEvent(() => {
    if (trackIds.length > 0) zoomLens(false);
  });
  const timeZoomOut = useEvent(() => {
    if (trackIds.length > 0) zoomLens(true);
  });
  useEffect(() => {
    ipcRenderer.on("time-zoom-in", timeZoomIn);
    ipcRenderer.on("time-zoom-out", timeZoomOut);
    return () => {
      ipcRenderer.removeAllListeners("time-zoom-in");
      ipcRenderer.removeAllListeners("time-zoom-out");
    };
  }, [timeZoomIn, timeZoomOut]);

  // Track Selection Hotkeys
  const selectAllTracksEvent = useEvent(() => selectAllTracks(trackIds));
  useHotkeys("mod+a", selectAllTracksEvent, {preventDefault: true}, [selectAllTracksEvent]);
  useEffect(() => {
    ipcRenderer.on("select-all-tracks", selectAllTracksEvent);
    return () => {
      ipcRenderer.removeAllListeners("select-all-tracks");
    };
  }, [selectAllTracksEvent]);

  useHotkeys(
    "down, up, shift+down, shift+up",
    (e, hotkey) => {
      if (trackIds.length === 0) return;
      const recentSelectedIdx = trackIds.indexOf(selectedTrackIds[selectedTrackIds.length - 1]);
      const newSelectId =
        hotkey.keys?.join("") === "down"
          ? trackIds[Math.min(recentSelectedIdx + 1, trackIds.length - 1)]
          : trackIds[Math.max(recentSelectedIdx - 1, 0)];
      selectTrack(e, newSelectId, trackIds);
    },
    {preventDefault: true},
    [trackIds, selectedTrackIds, selectTrack],
  );

  const resetHzRange = useEvent(() => setTimeout(() => setHzRange([0, Infinity])));
  const resetAmpRange = useEvent(() => setTimeout(() => setAmpRange([...DEFAULT_AMP_RANGE])));
  const resetTimeAxis = useEvent(() => setCanvasIsFit(true));
  const resetAxisRange = useEvent((_, axisKind: AxisKind) => {
    switch (axisKind) {
      case "freqAxis":
        resetHzRange();
        break;
      case "ampAxis":
        resetAmpRange();
        break;
      case "timeRuler":
        resetTimeAxis();
        break;
      default:
        break;
    }
  });

  useEffect(() => {
    ipcRenderer.on("reset-axis-range", resetAxisRange);
    return () => {
      ipcRenderer.removeAllListeners("reset-axis-range");
    };
  }, [resetAxisRange]);

  useLayoutEffect(() => {
    splitViewElem.current?.scrollTo({top: scrollTop, behavior: "instant"});
  }, [scrollTop]);

  const requestRef = useRef<number>(0);

  const updateByPlayerStatus = useEvent(async () => {
    const selectSec = player.selectSecRef.current ?? 0;
    if (player.isPlaying) {
      if (
        needFollowCursor.current &&
        player.positionSecRef.current !== null &&
        (endSec < player.positionSecRef.current || startSec > player.positionSecRef.current)
      ) {
        updateLensParams({startSec: player.positionSecRef.current}, false);
      }
    } else {
      needFollowCursor.current = true;
      const diff = selectSec - prevSelectSecRef.current;
      if (Math.abs(diff) > 1e-6 && (endSec < selectSec || startSec > selectSec)) {
        let newStartSec = startSec + diff;
        const newEndSec = endSec + diff;

        if (newEndSec < selectSec || newStartSec > selectSec)
          newStartSec = selectSec - width / pxPerSec / 2;
        updateLensParams({startSec: newStartSec}, false);
      }
    }
    prevSelectSecRef.current = selectSec;

    requestRef.current = requestAnimationFrame(updateByPlayerStatus);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(updateByPlayerStatus);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updateByPlayerStatus]);

  // locator
  const getLocatorBoundingLeftWidth: () => [number, number] | null = useEvent(() => {
    if (timeAxisCanvasElem.current === null) return null;
    const rect = timeAxisCanvasElem.current.getBoundingClientRect();
    if (rect === null) return null;
    return [rect.left, rect.width];
  });

  // select locator
  const getSelectLocatorTopBottom: () => [number, number] = useEvent(() => [
    TINY_MARGIN * 2,
    (splitViewElem.current?.getBoundingClientRect()?.height ?? 500) + TINY_MARGIN * 2,
  ]);
  const calcSelectLocatorPos = useEvent(
    () => ((player.selectSecRef.current ?? 0) - startSec) * pxPerSec,
  );
  const onSelectLocatorMouseDown = useEvent(() => {
    document.addEventListener("mousemove", changeLocatorByMouse);
    document.addEventListener("mouseup", endSelectLocatorDrag, {once: true});
  });

  // playhead
  const getTimeAxisPlayheadTopBottom = useEvent(
    () => [TINY_MARGIN * 2, TIME_CANVAS_HEIGHT + TINY_MARGIN * 2] as [number, number],
  );
  const getTrackPlayheadTopBottom: () => [number, number] = useEvent(() => {
    const idChArr = trackIdChMap.get(selectedTrackIds[selectedTrackIds.length - 1]);
    if (idChArr === undefined) return [0, 0];
    const firstChImgRect = imgCanvasesRef.current[idChArr[0]].getBoundingClientRect();
    const lastChImgRect =
      imgCanvasesRef.current[idChArr[idChArr.length - 1]].getBoundingClientRect();
    const splitViewTop = splitViewElem.current?.getBoundingClientRect()?.top ?? 0;
    const mainViewBottom = mainViewerElem.current?.getBoundingClientRect().bottom ?? 0;
    const top = firstChImgRect.top - splitViewTop;
    const bottom = Math.min(lastChImgRect.bottom, mainViewBottom - TINY_MARGIN * 2) - splitViewTop;
    return [top, bottom];
  });
  const calcPlayheadPos = useEvent(() =>
    player.isPlaying ? ((player.positionSecRef.current ?? 0) - startSec) * pxPerSec : -Infinity,
  );

  const trackSummaryArr: TrackSummaryData[] = useMemo(
    () =>
      trackIds.map((trackId) => {
        const formatInfo = BackendAPI.getFormatInfo(trackId);
        return {
          fileName: BackendAPI.getFileName(trackId),
          time: new Date(BackendAPI.getLengthSec(trackId) * 1000).toISOString().substring(11, 23),
          formatName: formatInfo.name,
          bitDepth: formatInfo.bitDepth,
          bitrate: formatInfo.bitrate,
          sampleRate: `${formatInfo.sampleRate / 1000} kHz`,
          globalLUFS: `${BackendAPI.getGlobalLUFS(trackId).toFixed(2)} LUFS`,
          guardClipStats: BackendAPI.getGuardClipStats(trackId),
        };
      }),
    [trackIds, needRefreshTrackIdChArr], // eslint-disable-line react-hooks/exhaustive-deps
  );

  // auto-scroll to the recently selected track
  const reducerForTrackInfoElemRange = useEvent(
    (
      prev: {
        topPlusHalf: number;
        bottomMinusHalf: number;
        topElem: TrackInfoElement | null;
        bottomElem: TrackInfoElement | null;
        topId: number;
        bottomId: number;
      },
      id: number,
    ) => {
      const chCount = trackIdChMap.get(selectedTrackIds[selectedTrackIds.length - 1])?.length ?? 0;
      if (chCount <= 0) return prev;
      const trackInfoElem = trackInfosRef.current[`${id}`];
      if (trackInfoElem === null) return prev;
      const infoRect = trackInfoElem.getBoundingClientRect();
      if (infoRect === null) return prev;
      let currTopPlusHalf = infoRect.top + infoRect.height / chCount / 2;
      let currTopElem = prev.topElem;
      let currTopId = prev.topId;
      if (currTopPlusHalf < prev.topPlusHalf) {
        currTopElem = trackInfoElem;
        currTopId = id;
      } else {
        currTopPlusHalf = prev.topPlusHalf;
      }
      let currBottomMinusHalf = infoRect.bottom - infoRect.height / chCount / 2;
      let currBottomElem = prev.bottomElem;
      let currBottomId = prev.bottomId;
      if (currBottomMinusHalf > prev.bottomMinusHalf) {
        currBottomElem = trackInfoElem;
        currBottomId = id;
      } else {
        currBottomMinusHalf = prev.bottomMinusHalf;
      }
      return {
        topPlusHalf: currTopPlusHalf,
        bottomMinusHalf: currBottomMinusHalf,
        topElem: currTopElem,
        bottomElem: currBottomElem,
        topId: currTopId,
        bottomId: currBottomId,
      };
    },
  );
  useEffect(() => {
    if (selectedTrackIds.length === 0 || !selectionIsAdded) return;
    const viewRect = splitViewElem.current?.getBoundingClientRect() ?? null;
    if (viewRect === null) return;
    const {topPlusHalf, bottomMinusHalf, topElem, bottomElem, topId, bottomId} =
      selectedTrackIds.reduce(reducerForTrackInfoElemRange, {
        topPlusHalf: Infinity,
        bottomMinusHalf: -Infinity,
        topElem: null,
        bottomElem: null,
        topId: -1,
        bottomId: -1,
      });
    if (
      topId === selectedTrackIds[selectedTrackIds.length - 1] &&
      topPlusHalf < viewRect.top + TIME_CANVAS_HEIGHT
    ) {
      topElem?.scrollIntoView(true);
    } else if (
      bottomId === selectedTrackIds[selectedTrackIds.length - 1] &&
      bottomMinusHalf > viewRect.bottom
    ) {
      bottomElem?.scrollIntoView(false);
    }
  }, [selectedTrackIds, selectionIsAdded, reducerForTrackInfoElemRange]);

  // set LensParams when track list, width, or canvasIsFit change
  const setLensParamsForFitCanvas = useEvent((newWidth: number, _CanvasIsFit: boolean) => {
    const newStartSec =
      prevTrackCountRef.current === 0 || _CanvasIsFit
        ? 0
        : normalizeStartSec(startSec, pxPerSec, maxTrackSec);
    const newPxPerSec =
      prevTrackCountRef.current === 0 || _CanvasIsFit
        ? newWidth / maxTrackSec
        : normalizePxPerSec(pxPerSec, startSec);
    updateLensParams({startSec: newStartSec, pxPerSec: newPxPerSec}, false);
  });
  useEffect(() => {
    if (trackIds.length > 0) setLensParamsForFitCanvas(width, canvasIsFit);

    prevTrackCountRef.current = trackIds.length;
  }, [trackIds, width, setLensParamsForFitCanvas, canvasIsFit]);

  useEffect(() => {
    if (needRefreshTrackIdChArr.length > 0) finishRefreshTracks();
  }, [needRefreshTrackIdChArr, finishRefreshTracks]);

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

  const selectTrackByTrackInfo = useEvent((e, id) => selectTrack(e, id, trackIds));

  const createLeftPane = (leftWidth: number) => (
    <>
      <div className={styles.stickyHeader} style={{width: `${leftWidth}px`}}>
        <TimeUnitSection key="time_unit_label" timeUnitLabel={timeUnitLabel} />
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
      <TrackInfoDragLayer />
      {trackIdsWithFileDropIndicator.map((trackId, iWithIndicator) => {
        if (trackId === -1) {
          return <div key="file_drop_indicator_left" className={styles.fileDropIndicator} />;
        }
        const i =
          fileDropIndex > -1 && iWithIndicator > fileDropIndex
            ? iWithIndicator - 1
            : iWithIndicator;
        const isSelected = selectedTrackIds.includes(trackId);
        return (
          <TrackInfo
            ref={registerTrackInfos(`${trackId}`)}
            key={trackId}
            id={trackId}
            index={i}
            trackIdChArr={trackIdChMap.get(trackId) || []}
            selectedTrackIds={selectedTrackIds}
            trackSummary={trackSummaryArr[i]}
            channelHeight={height}
            imgHeight={imgHeight}
            isSelected={isSelected}
            selectTrack={selectTrackByTrackInfo}
            hideTracks={hideTracks}
            hideImage={hideDraggingImage}
            onDnd={changeTrackOrder}
            showHiddenTracks={showHiddenTracks}
            showHiddenImage={unHideDraggingImage}
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
          ref={timeAxisCanvasElem}
          width={width}
          markersAndLength={timeMarkersAndLength}
          shiftWhenResize={!canvasIsFit}
          startSec={startSec}
          pxPerSec={pxPerSec}
          moveLens={moveLens}
          resetTimeAxis={resetTimeAxis}
          enableInteraction={trackIds.length > 0}
        />
        <span className={styles.axisLabelSection}>Amp</span>
        <span className={styles.axisLabelSection}>Hz</span>
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
      {trackIdsWithFileDropIndicator.map((id) => {
        if (id === -1) {
          return <div key="file_drop_indicator_right" className={styles.fileDropIndicator} />;
        }
        return (
          <div key={`${id}`} className={styles.trackRight}>
            {trackIdChMap.get(id)?.map((idChStr) => {
              return (
                <div
                  key={idChStr}
                  className={styles.chCanvases}
                  role="presentation"
                  onClick={(e) => selectTrack(e, id, trackIds)}
                >
                  <ImgCanvas
                    ref={registerImgCanvas(idChStr)}
                    idChStr={idChStr}
                    width={width}
                    height={imgHeight}
                    startSec={startSec}
                    pxPerSec={pxPerSec}
                    trackSec={BackendAPI.getLengthSec(id)}
                    maxTrackSec={maxTrackSec}
                    hzRange={hzRange}
                    ampRange={ampRange}
                    blend={blend}
                    isLoading={isLoading}
                    needRefresh={needRefreshTrackIdChArr.includes(idChStr)}
                    hidden={hiddenIdChArr.has(idChStr) || id === draggingTrackIdRef.current}
                  />
                  {erroredTrackIds.includes(id) ? (
                    <ErrorBox
                      trackId={id}
                      width={width}
                      handleReload={async (trackId) => {
                        await reloadTracks([trackId]);
                        await refreshTracks();
                      }}
                      handleIgnore={ignoreError}
                      handleClose={async (trackId) => {
                        removeTracks([trackId]);
                        await refreshTracks();
                      }}
                    />
                  ) : null}
                  <AmpAxis
                    id={id}
                    height={height}
                    markersAndLength={ampMarkersAndLength}
                    ampRange={ampRange}
                    setAmpRange={setAmpRangeIfNotSame}
                    resetAmpRange={resetAmpRange}
                    enableInteraction={blend < 1}
                  />
                  <FreqAxis
                    id={id}
                    height={height}
                    markersAndLength={freqMarkersAndLength}
                    maxTrackHz={maxTrackHz}
                    hzRange={hzRange}
                    setHzRange={setHzRangeIfNotSame}
                    resetHzRange={resetHzRange}
                    enableInteraction={blend > 0}
                  />
                </div>
              );
            })}
          </div>
        );
      })}
      <Locator // on track img
        locatorStyle="playhead"
        getTopBottom={getTrackPlayheadTopBottom}
        getBoundingLeftWidth={getLocatorBoundingLeftWidth}
        calcLocatorPos={calcPlayheadPos}
        zIndex={1}
      />
    </>
  );

  const overviewTrackId =
    trackIds.length > 0 && selectedTrackIds.length > 0
      ? selectedTrackIds[selectedTrackIds.length - 1]
      : null;
  return (
    <div className={`flex-container-column flex-item-auto ${styles.mainViewerWrapper}`}>
      {trackIds.length ? (
        <Overview
          trackId={overviewTrackId}
          maxTrackSec={maxTrackSec}
          startSec={startSec}
          lensDurationSec={width / pxPerSec}
          moveLens={moveLens}
          resizeLensLeft={resizeLensLeft}
          resizeLensRight={resizeLensRight}
          resetLens={resetTimeAxis}
          needRefresh={
            overviewTrackId !== null &&
            needRefreshTrackIdChArr.some((idCh) => idCh.startsWith(`${overviewTrackId}_`))
          }
        />
      ) : null}
      <div
        className={`flex-container-row flex-item-auto ${styles.MainViewer}`}
        ref={mainViewerElemCallback}
        onMouseMove={onMouseMove}
        onClick={(e) => changeLocatorByMouse(e, player.isPlaying, false)}
        role="presentation"
      >
        <SplitView
          ref={splitViewElem}
          createLeft={createLeftPane}
          right={rightPane}
          setCanvasWidth={setWidth}
          onFileHover={onFileHover}
          onFileHoverLeave={onFileHoverLeave}
          onFileDrop={onFileDrop}
          onVerticalViewportChange={onVerticalViewportChange}
        />
        <Locator // on time axis
          locatorStyle="playhead"
          getTopBottom={getTimeAxisPlayheadTopBottom}
          getBoundingLeftWidth={getLocatorBoundingLeftWidth}
          calcLocatorPos={calcPlayheadPos}
        />
        <Locator
          ref={selectLocatorElem}
          locatorStyle="selection"
          getTopBottom={getSelectLocatorTopBottom}
          getBoundingLeftWidth={getLocatorBoundingLeftWidth}
          calcLocatorPos={calcSelectLocatorPos}
          onMouseDown={onSelectLocatorMouseDown}
        />
        <ColorMap
          height={colorMapHeight}
          colorBarHeight={colorBarHeight}
          setHeight={setColorMapHeight}
          markersAndLength={dBMarkersAndLength}
        />
      </div>
    </div>
  );
}

export default MainViewer;
