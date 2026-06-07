import React, { useRef, useCallback, useEffect, useMemo, useState, useLayoutEffect } from "react";
import { throttle } from "throttle-debounce";
import useRefs from "src/hooks/useRefs";
import AudioTrackViewport, {
  AudioTrackViewportRect,
  AudioTrackViewportRow,
} from "src/modules/AudioTrackViewport";
import SplitView from "src/modules/SplitView";
import useAxisMarkers from "src/hooks/useAxisMarkers";
import useEvent from "react-use-event-hook";
import { useHotkeys } from "react-hotkeys-hook";
import { DragDropEvent, getCurrentWindow } from "@tauri-apps/api/window";
import { Event } from "@tauri-apps/api/event";

import { Player } from "../../hooks/usePlayer";
import Locator from "../../modules/Locator";
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
import BackendAPI, { FreqScale, WasmAPI } from "../../api";
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
import { isApple } from "../../utils/osSpecifics";
import TrackInfoDragLayer from "./TrackInfoDragLayer";
import {
  listenFreqZoomIn,
  listenFreqZoomOut,
  listenMenuResetAxisRange,
  listenMenuSelectAllTracks,
  listenTimeZoomIn,
  listenTimeZoomOut,
} from "../../api";

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
  openAudioTracksHandler: () => Promise<void>;
  addDroppedFile: (paths: string[], index: number) => Promise<void>;
  reloadTracks: (ids: number[]) => Promise<void>;
  refreshTracks: () => Promise<void>;
  ignoreError: (id: number) => void;
  removeTracks: (ids: number[]) => void;
  hideTracks: (dragId: number, ids: number[]) => number;
  changeTrackOrder: (dragIndex: number, hoverIndex: number) => void;
  showHiddenTracks: (hoverIndex: number) => void;
  selectTrack: (e: MouseOrKeyboardEvent | null, id: number, trackIds: number[]) => number[];
  selectAllTracks: (trackIds: number[]) => void;
  finishRefreshTracks: () => void;
};

const FILE_DROP_INDICATOR_HEIGHT = 10;
const TRACK_HEADER_HEIGHT = TIME_CANVAS_HEIGHT + TINY_MARGIN;

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
    openAudioTracksHandler,
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
  const endSec = startSec + width / Math.max(pxPerSec, 1e-8);

  const [height, setHeight] = useState(250);
  const pendingScrollTopRef = useRef<number | null>(null);
  const pendingHeightRef = useRef<number | null>(null);
  const logicalScrollTopRef = useRef<number | null>(null);
  const programmaticScrollTargetRef = useRef<number | null>(null);
  const scrollCorrectionRequestRef = useRef<number | null>(null);
  const scrollCorrectionFrameRef = useRef(0);
  const [viewportLayoutRevision, setViewportLayoutRevision] = useState(0);
  const imgHeight = height - 2 * VERTICAL_AXIS_PADDING;
  const [colorMapHeight, setColorMapHeight] = useState<number>(250);
  const colorBarHeight = colorMapHeight - 2 * VERTICAL_AXIS_PADDING;

  const splitViewElem = useRef<SplitViewHandleElement>(null);
  const timeAxisCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const selectLocatorElem = useRef<LocatorHandleElement>(null);

  const [trackInfosRef, registerTrackInfos] = useRefs<TrackInfoElement>();

  const needFollowCursor = useRef<boolean>(true);
  const [fileDropIndex, setFileDropIndex] = useState<number>(-1);
  const [draggingTrackId, setDraggingTrackId] = useState(-1);

  const trackIdsWithFileDropIndicator = useMemo(() => {
    // >=0 means a normal track, -1 means a file drop indicator
    if (fileDropIndex === -1) return trackIds;
    const result = [...trackIds];
    result.splice(fileDropIndex, 0, -1);
    return result;
  }, [trackIds, fileDropIndex]);

  const calculateDropIndex = useEvent((clientY: number) => {
    let dropIndex = trackIds.length;
    trackIds.some((id, index) => {
      const trackInfoElem = trackInfosRef.current[`${id}`];
      if (!trackInfoElem) return false;
      const rect = trackInfoElem.getBoundingClientRect();
      if (!rect) return false;
      if (clientY >= rect.y + rect.height / 2) {
        return false;
      }
      dropIndex = index;
      return true;
    });
    return dropIndex;
  });

  const onFileDragDropEvent = useEvent((event: Event<DragDropEvent>) => {
    if (event.payload.type === "over") {
      const { y } = event.payload.position;
      const index = calculateDropIndex(y);
      setFileDropIndex(index);
    } else if (event.payload.type === "drop") {
      addDroppedFile(event.payload.paths, fileDropIndex);
      setFileDropIndex(-1);
    } else if (event.payload.type === "leave") {
      setFileDropIndex(-1);
    }
  });

  // Tauri drag & drop event handler
  useEffect(() => {
    const currentWindow = getCurrentWindow();
    const unlisten = currentWindow.onDragDropEvent(onFileDragDropEvent);

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [onFileDragDropEvent]);

  const getIdChArr = useEvent(() => Array.from(trackIdChMap.values()).flat());
  const audioViewportRows = useMemo(() => {
    let top = 0;
    const rows: AudioTrackViewportRow[] = [];
    trackIdsWithFileDropIndicator.forEach((trackId) => {
      if (trackId === -1) {
        top += FILE_DROP_INDICATOR_HEIGHT;
        return;
      }
      trackIdChMap.get(trackId)?.forEach((idChStr) => {
        rows.push({
          idChStr,
          trackId,
          top,
          hidden: trackId === draggingTrackId,
        });
        top += height;
      });
    });
    return rows;
  }, [draggingTrackId, height, trackIdChMap, trackIdsWithFileDropIndicator]);
  const getAudioViewportRect = useEvent((): AudioTrackViewportRect | null => {
    const timeAxisRect = timeAxisCanvasElem.current?.getBoundingClientRect() ?? null;
    const splitViewRect = splitViewElem.current?.getBoundingClientRect() ?? null;
    if (!timeAxisRect || !splitViewRect) return null;
    return {
      left: timeAxisRect.left,
      top: splitViewRect.top,
      width,
      height: splitViewRect.height,
    };
  });
  const getViewportScrollTop = useEvent(() => splitViewElem.current?.scrollTop() ?? 0);
  const getChannelRect = useEvent((idChStr: string) => {
    const viewportRect = getAudioViewportRect();
    const row = audioViewportRows.find((value) => value.idChStr === idChStr);
    if (!viewportRect || !row) return new DOMRect();
    return new DOMRect(
      viewportRect.left,
      viewportRect.top +
        TIME_CANVAS_HEIGHT +
        TINY_MARGIN +
        row.top -
        getViewportScrollTop() +
        VERTICAL_AXIS_PADDING,
      width,
      imgHeight,
    );
  });

  const timeMarkersDrawOptions = useMemo(
    () => ({ startSec, endSec, maxSec: maxTrackSec }),
    [endSec, maxTrackSec, startSec],
  );
  const timeMarkersAndLength = useAxisMarkers({
    scaleTable: TIME_TICK_SIZE,
    boundaries: TIME_BOUNDARIES,
    getMarkers: WasmAPI.calcTimeAxisMarkers,
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

  const ampMarkersDrawOptions = useMemo(() => ({ ampRange }), [ampRange]);
  const ampMarkersAndLength = useAxisMarkers({
    scaleTable: AMP_TICK_NUM,
    boundaries: AMP_BOUNDARIES,
    getMarkers: WasmAPI.calcAmpAxisMarkers,
    canvasLength: imgHeight,
    scaleDeterminant: imgHeight,
    drawOptions: ampMarkersDrawOptions,
  });

  const [freqScale, setFreqScale] = useState<FreqScale>("Mel");
  useEffect(() => {
    BackendAPI.getSpecSetting().then(({ freqScale: _freqScale }) => {
      setFreqScale(_freqScale);
    });
  });
  const freqMarkersDrawOptions = useMemo(
    () => ({ maxTrackHz, hzRange, freqScale }),
    [maxTrackHz, hzRange, freqScale],
  );
  const freqMarkersAndLength = useAxisMarkers({
    scaleTable: FREQ_TICK_NUM,
    boundaries: FREQ_BOUNDARIES,
    getMarkers: WasmAPI.calcFreqAxisMarkers,
    canvasLength: imgHeight,
    scaleDeterminant: imgHeight,
    drawOptions: freqMarkersDrawOptions,
  });

  const [minMaxdB, setMinMaxdB] = useState<{ mindB: number; maxdB: number }>({
    mindB: -100,
    maxdB: 0,
  });

  const dBMarkersAndLength = useAxisMarkers({
    scaleTable: DB_TICK_NUM,
    boundaries: DB_BOUNDARIES,
    getMarkers: WasmAPI.calcDbAxisMarkers,
    canvasLength: trackIds.length > 0 ? colorBarHeight : 0,
    scaleDeterminant: colorBarHeight,
    drawOptions: minMaxdB,
  });

  useEffect(() => {
    if (trackIds.length === 0 || needRefreshTrackIdChArr.length === 0) return;
    Promise.all([BackendAPI.getMindB(), BackendAPI.getMaxdB()]).then(([mindB, maxdB]) => {
      setMinMaxdB({ mindB, maxdB });
    });
  }, [trackIds, needRefreshTrackIdChArr]);

  const setSelectSec = useEvent(player.setSelectSec);
  const throttledSetSelectSec = useMemo(() => throttle(1000 / 70, setSelectSec), [setSelectSec]);

  const normalizeStartSec = useEvent((_startSec, _pxPerSec, maxEndSec) =>
    Math.min(Math.max(_startSec, 0), Math.max(maxEndSec - width / Math.max(_pxPerSec, 1e-8), 0)),
  );

  const normalizePxPerSec = useEvent((_pxPerSec, _startSec) => {
    if (maxTrackSec - _startSec < 1e-6) {
      return Math.min(_pxPerSec, MAX_PX_PER_SEC, 1e-8);
    }
    return Math.min(
      Math.max(_pxPerSec, width / (maxTrackSec - _startSec), 1e-8),
      Math.max(MAX_PX_PER_SEC, width / (maxTrackSec - _startSec)),
    );
  });

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
    updateLensParams({ startSec: sec - lensDurationSec * anchorRatio });
  });

  const resizeLensLeft = useEvent((sec: number) => {
    const newStartSec = normalizeStartSec(sec, MAX_PX_PER_SEC, endSec);
    const newPxPerSec = normalizePxPerSec(
      width / Math.max(endSec - newStartSec, 1e-8),
      newStartSec,
    );

    updateLensParams({ startSec: newStartSec, pxPerSec: newPxPerSec });
  });

  const resizeLensRight = useEvent((sec: number) => {
    const newPxPerSec = normalizePxPerSec(width / Math.max(sec - startSec, 0), startSec);
    updateLensParams({ pxPerSec: newPxPerSec });
  });

  const calcZoomedHeight = useEvent((baseHeight: number, delta: number) => {
    return Math.round(Math.min(Math.max(baseHeight + delta, MIN_HEIGHT), MAX_HEIGHT));
  });
  const updateHeightAndScrollTop = useEvent(
    (baseHeight: number, newHeight: number, newScrollTop: number) => {
      if (newHeight === baseHeight) return;
      if (scrollCorrectionRequestRef.current !== null) {
        cancelAnimationFrame(scrollCorrectionRequestRef.current);
        scrollCorrectionRequestRef.current = null;
      }
      logicalScrollTopRef.current = newScrollTop;
      programmaticScrollTargetRef.current = newScrollTop;
      pendingHeightRef.current = newHeight;
      pendingScrollTopRef.current = newScrollTop;
      setHeight(newHeight);
    },
  );
  const getPendingNativeScrollTop = useEvent(() => {
    if (pendingScrollTopRef.current !== null) return pendingScrollTopRef.current;
    if (logicalScrollTopRef.current !== null) return logicalScrollTopRef.current;
    return splitViewElem.current?.scrollTop() ?? 0;
  });
  const getRowTopAtHeight = useEvent(
    (row: AudioTrackViewportRow, rowIndex: number, rowHeight: number) => {
      return row.top + rowIndex * (rowHeight - height);
    },
  );

  const calcScrollTopAtCursor = useEvent(
    (baseHeight: number, newHeight: number, cursorClientY: number) => {
      const splitView = splitViewElem.current;
      const splitViewRect = splitView?.getBoundingClientRect();
      if (!splitView || !splitViewRect || audioViewportRows.length === 0) return null;

      const cursorY = cursorClientY - splitViewRect.y;
      const scrollTopForZoom = getPendingNativeScrollTop();
      const contentY = scrollTopForZoom + cursorY - TRACK_HEADER_HEIGHT;
      const lastRowIndex = audioViewportRows.length - 1;
      let newContentY = 0;
      let foundAnchor = false;

      for (let rowIndex = 0; rowIndex < audioViewportRows.length; rowIndex += 1) {
        const row = audioViewportRows[rowIndex];
        const rowTop = getRowTopAtHeight(row, rowIndex, baseHeight);
        const newRowTop = getRowTopAtHeight(row, rowIndex, newHeight);
        if (contentY < rowTop) {
          newContentY = newRowTop + (contentY - rowTop);
          foundAnchor = true;
          break;
        }
        if (contentY <= rowTop + baseHeight) {
          const offsetRatio = (contentY - rowTop) / Math.max(baseHeight, 1e-8);
          newContentY = newRowTop + offsetRatio * newHeight;
          foundAnchor = true;
          break;
        }
      }

      if (!foundAnchor) {
        const row = audioViewportRows[lastRowIndex];
        const rowBottom = getRowTopAtHeight(row, lastRowIndex, baseHeight) + baseHeight;
        const newRowTop = getRowTopAtHeight(row, lastRowIndex, newHeight);
        newContentY = newRowTop + newHeight + (contentY - rowBottom);
      }

      return TRACK_HEADER_HEIGHT + newContentY - cursorY;
    },
  );

  const zoomHeightAtCursor = useEvent((delta: number, cursorClientY: number) => {
    const baseHeight = pendingHeightRef.current ?? height;
    const newHeight = calcZoomedHeight(baseHeight, (delta * baseHeight) / 1000);
    const newScrollTop = calcScrollTopAtCursor(baseHeight, newHeight, cursorClientY);
    if (newScrollTop === null) return;
    updateHeightAndScrollTop(baseHeight, newHeight, newScrollTop);
  });

  const onMouseDown = (e: React.MouseEvent) => {
    const rect = timeAxisCanvasElem.current?.getBoundingClientRect() ?? null;
    if (rect === null) return;
    if (e.clientY < rect.bottom) return; // click on TimeAxis fires the other event

    if (selectLocatorElem.current?.isOnLocator(e.clientX) ?? false) {
      changeLocatorByMouse(e, false, false, false);
      onSelectLocatorMouseDown();
    } else {
      for (const id of trackIds) {
        const trackInfoElem = trackInfosRef.current[`${id}`];
        if (!trackInfoElem) continue;
        const trackInfoRect = trackInfoElem.getBoundingClientRect();
        if (!trackInfoRect) continue;
        if (trackInfoRect.y <= e.clientY && e.clientY <= trackInfoRect.y + trackInfoRect.height) {
          selectTrack(e, id, trackIds);
          break;
        }
      }
      changeLocatorByMouse(e, player.isPlaying, false, false);
    }
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (mainViewerElem.current === null) return;
    if (selectLocatorElem.current?.isOnLocator(e.clientX) ?? false) {
      mainViewerElem.current.style.cursor = "col-resize";
    } else {
      mainViewerElem.current.style.cursor = "default";

      // if on one of audio rows, cursor should be crosshair
      for (const idChStr of getIdChArr()) {
        const rect = getChannelRect(idChStr);
        if (
          rect.left <= e.clientX &&
          e.clientX <= rect.right &&
          rect.top <= e.clientY &&
          e.clientY <= rect.bottom
        ) {
          mainViewerElem.current.style.cursor = "crosshair";
          break;
        }
      }
    }
  };

  const handleWheel = useEvent((e: WheelEvent) => {
    if (!trackIds.length) return;

    let horizontal: boolean;
    let delta: number;
    const isApplePinch = isApple() && e.ctrlKey;
    const isAppleZoom = isApple() && e.altKey;
    const isNonAppleZoom = !isApple() && e.ctrlKey;
    const isZoom = isApplePinch || isAppleZoom || isNonAppleZoom;
    if (isApplePinch) {
      horizontal = !e.shiftKey;
      if (horizontal) delta = -12 * e.deltaY;
      else delta = -6 * e.deltaY;
    } else {
      if (Math.abs(e.deltaY) < Math.abs(e.deltaX)) {
        horizontal = !e.shiftKey;
        delta = e.deltaX;
      } else {
        horizontal = e.shiftKey;
        delta = e.deltaY;
      }
      if (isNonAppleZoom) delta = -delta;
    }

    if (!isZoom && !horizontal) {
      // vertical scroll (native)
      return;
    }

    e.preventDefault();
    const anImgBoundngRect = getAudioViewportRect();
    if (!anImgBoundngRect) return;
    if (
      e.clientX > anImgBoundngRect.left + anImgBoundngRect.width ||
      e.clientX < anImgBoundngRect.left
    )
      return;

    if (isZoom) {
      if (horizontal) {
        // horizontal zoom
        const newPxPerSec = normalizePxPerSec(pxPerSec * (1 + delta / 1000), 0);
        const cursorX = e.clientX - anImgBoundngRect.left;
        const newStartSec = normalizeStartSec(
          startSec + cursorX / pxPerSec - cursorX / newPxPerSec,
          newPxPerSec,
          maxTrackSec,
        );
        updateLensParams({ startSec: newStartSec, pxPerSec: newPxPerSec });
      } else {
        // vertical zoom
        zoomHeightAtCursor(delta, e.clientY);
      }
    } else if (horizontal) {
      // horizontal scroll
      updateLensParams({ startSec: startSec + (0.5 * delta) / pxPerSec });
    }
  });

  const viewportResizeRequestRef = useRef<number | null>(null);
  const onVerticalViewportChange = useEvent(() => {
    const actualScrollTop = splitViewElem.current?.scrollTop() ?? 0;
    const programmaticTarget = programmaticScrollTargetRef.current;
    if (programmaticTarget !== null && Math.abs(programmaticTarget - actualScrollTop) <= 1) {
      logicalScrollTopRef.current = programmaticTarget;
      return;
    }
    programmaticScrollTargetRef.current = null;
    logicalScrollTopRef.current = actualScrollTop;
  });
  const onVerticalViewportResize = useEvent(() => {
    if (viewportResizeRequestRef.current !== null) return;
    viewportResizeRequestRef.current = requestAnimationFrame(() => {
      viewportResizeRequestRef.current = null;
      setViewportLayoutRevision((value) => value + 1);
    });
  });
  useEffect(
    () => () => {
      if (viewportResizeRequestRef.current !== null) {
        cancelAnimationFrame(viewportResizeRequestRef.current);
      }
    },
    [],
  );

  const hideDraggingImage = useEvent((id) => {
    setDraggingTrackId(id);
  });
  const unHideDraggingImage = useEvent(() => {
    setDraggingTrackId(-1);
    onVerticalViewportResize();
  });

  // without useEvent, sometimes (when busy?) onClick event is not handled by this function.
  const changeLocatorByMouse = useEvent(
    (
      e: React.MouseEvent | MouseEvent,
      isPlayhead: boolean = false,
      allowOutside: boolean = true,
      preventDefault: boolean = true,
    ) => {
      const rect = timeAxisCanvasElem.current?.getBoundingClientRect() ?? null;
      if (rect === null) return;
      if (e.clientY < rect.bottom && e.altKey) return; // alt+click on TimeAxis fires the other event
      if (preventDefault) e.preventDefault();
      if (trackIds.length === 0) return;
      if (e.clientY < rect.top) return; // when cursor is between Overview and TimeAxis
      const cursorX = e.clientX - rect.left;
      if (!allowOutside) {
        if (cursorX < 0 || cursorX >= width) return;
        if (isPlayhead) {
          const lastTrackIdChArr = trackIdChMap.get(trackIds[trackIds.length - 1]);
          if (lastTrackIdChArr) {
            const lastIdCh = lastTrackIdChArr[lastTrackIdChArr.length - 1];
            const lastChImgRect = getChannelRect(lastIdCh);
            if (e.clientY > lastChImgRect.bottom) return;
          }
        }
      }

      if (mainViewerElem.current === null) return;
      mainViewerElem.current.style.cursor = "col-resize";

      const sec = startSec + cursorX / pxPerSec;
      if (isPlayhead) player.seek(sec);
      else throttledSetSelectSec(sec);
    },
  );

  const changeLocatorByMouseNotAllowOutside = useEvent((e: React.MouseEvent) => {
    changeLocatorByMouse(e, player.isPlaying, false);
  });

  const endSelectLocatorDrag = useEvent(() => {
    document.removeEventListener("mousemove", changeLocatorByMouse);
  });

  // Browsing Hotkeys
  useHotkeys(
    "right, left, shift+right, shift+left",
    (_, hotkey) => {
      if (hotkey.mod) return;
      if (trackIds.length === 0) return;
      const shiftPx = hotkey.shift ? BIG_SHIFT_PX : SHIFT_PX;
      let shiftSec = shiftPx / pxPerSec;
      if (hotkey.keys?.join("").endsWith("left")) shiftSec = -shiftSec;
      updateLensParams({ startSec: startSec + shiftSec });
    },
    [pxPerSec, startSec, trackIds, updateLensParams],
  );

  const calcScrollTopBySelectedTracks = useEvent((baseHeight: number, newHeight: number) => {
    if (splitViewElem.current === null) return null;
    const splitViewHeight =
      (splitViewElem.current.getBoundingClientRect()?.height ?? 0) - TRACK_HEADER_HEIGHT;
    const contentMiddle = getPendingNativeScrollTop() + splitViewHeight / 2;
    const rowIndex = audioViewportRows.findIndex(
      (row, index) => contentMiddle < getRowTopAtHeight(row, index, baseHeight) + baseHeight,
    );
    const safeRowIndex =
      rowIndex === -1 ? Math.max(audioViewportRows.length - 1, 0) : Math.max(rowIndex, 0);
    const row = audioViewportRows[safeRowIndex];
    if (!row) return null;

    const rowTop = getRowTopAtHeight(row, safeRowIndex, baseHeight);
    const offsetInRow = contentMiddle - rowTop;
    const newOffsetInRow = (offsetInRow / Math.max(baseHeight, 1e-8)) * newHeight;

    const newContentMiddle = getRowTopAtHeight(row, safeRowIndex, newHeight) + newOffsetInRow;
    return newContentMiddle - splitViewHeight / 2;
  });
  const zoomHeightAndScrollToSelectedTrack = (isZoomOut: boolean) => {
    if (trackIds.length === 0) return;

    const baseHeight = pendingHeightRef.current ?? height;
    let delta = 2 ** (Math.floor(Math.log2(baseHeight)) - 1.2);
    if (isZoomOut) delta = -delta;
    const newHeight = calcZoomedHeight(baseHeight, delta);
    const newScrollTop = calcScrollTopBySelectedTracks(baseHeight, newHeight);
    if (newScrollTop === null) return;
    updateHeightAndScrollTop(baseHeight, newHeight, newScrollTop);
  };
  const freqZoomIn = useEvent(() => zoomHeightAndScrollToSelectedTrack(false));
  const freqZoomOut = useEvent(() => zoomHeightAndScrollToSelectedTrack(true));
  useHotkeys("mod+down", freqZoomIn, { preventDefault: true }, [freqZoomIn]);
  useHotkeys("mod+up", freqZoomOut, { preventDefault: true }, [freqZoomOut]);
  useEffect(() => {
    const promiseUnlistenFreqZoomIn = listenFreqZoomIn(freqZoomIn);
    const promiseUnlistenFreqZoomOut = listenFreqZoomOut(freqZoomOut);
    return () => {
      promiseUnlistenFreqZoomIn.then((unlistenFn) => unlistenFn());
      promiseUnlistenFreqZoomOut.then((unlistenFn) => unlistenFn());
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
    updateLensParams({ startSec: newStartSec, pxPerSec: newPxPerSec });
  });
  const timeZoomIn = useEvent(() => {
    if (trackIds.length > 0) zoomLens(false);
  });
  const timeZoomOut = useEvent(() => {
    if (trackIds.length > 0) zoomLens(true);
  });
  useHotkeys(
    "mod+left, mod+right",
    (_, hotkey) => {
      if (hotkey.keys?.join("").endsWith("left")) timeZoomOut();
      else timeZoomIn();
    },
    { preventDefault: true },
    [timeZoomIn, timeZoomOut],
  );
  useEffect(() => {
    const promiseUnlistenTimeZoomIn = listenTimeZoomIn(timeZoomIn);
    const promiseUnlistenTimeZoomOut = listenTimeZoomOut(timeZoomOut);
    return () => {
      promiseUnlistenTimeZoomIn.then((unlistenFn) => unlistenFn());
      promiseUnlistenTimeZoomOut.then((unlistenFn) => unlistenFn());
    };
  }, [timeZoomIn, timeZoomOut]);

  // Track Selection Hotkeys
  const selectAllTracksEvent = useEvent(() => selectAllTracks(trackIds));
  useHotkeys("mod+a", selectAllTracksEvent, { preventDefault: true }, [selectAllTracksEvent]);
  useEffect(() => {
    const promiseUnlisten = listenMenuSelectAllTracks(selectAllTracksEvent);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [selectAllTracksEvent]);

  useHotkeys(
    "down, up, shift+down, shift+up",
    (e, hotkey) => {
      if (trackIds.length === 0) return;
      const recentSelectedIdx = trackIds.indexOf(selectedTrackIds[selectedTrackIds.length - 1]);
      const newSelectId = hotkey.keys?.join("").endsWith("down")
        ? trackIds[Math.min(recentSelectedIdx + 1, trackIds.length - 1)]
        : trackIds[Math.max(recentSelectedIdx - 1, 0)];
      selectTrack(e, newSelectId, trackIds);
    },
    { preventDefault: true },
    [trackIds, selectedTrackIds, selectTrack],
  );

  const resetHzRange = useEvent(() => setTimeout(() => setHzRange([0, Infinity])));
  const resetAmpRange = useEvent(() => setTimeout(() => setAmpRange([...DEFAULT_AMP_RANGE])));
  const resetTimeAxis = useEvent(() => setCanvasIsFit(true));
  useEffect(() => {
    const promiseUnlisten = listenMenuResetAxisRange(
      new Map([
        ["freqAxis", resetHzRange],
        ["ampAxis", resetAmpRange],
        ["timeRuler", resetTimeAxis],
      ]),
    );
    return () => {
      promiseUnlisten.then((unlistenFns) => unlistenFns.forEach((unlistenFn) => unlistenFn()));
    };
  }, [resetHzRange, resetAmpRange, resetTimeAxis]);

  const applyVerticalZoomScrollTop = useEvent((targetScrollTop: number) => {
    logicalScrollTopRef.current = targetScrollTop;
    programmaticScrollTargetRef.current = targetScrollTop;
    splitViewElem.current?.scrollTo({ top: targetScrollTop });
    return splitViewElem.current?.scrollTop() ?? 0;
  });
  const scheduleVerticalZoomScrollCorrection = useEvent((targetScrollTop: number) => {
    if (scrollCorrectionRequestRef.current !== null) {
      cancelAnimationFrame(scrollCorrectionRequestRef.current);
    }
    scrollCorrectionFrameRef.current = 0;
    const correct = () => {
      scrollCorrectionRequestRef.current = null;
      const actualScrollTop = applyVerticalZoomScrollTop(targetScrollTop);
      if (Math.abs(targetScrollTop - actualScrollTop) <= 0.5) return;
      scrollCorrectionFrameRef.current += 1;
      if (scrollCorrectionFrameRef.current >= 3) return;
      scrollCorrectionRequestRef.current = requestAnimationFrame(correct);
    };
    scrollCorrectionRequestRef.current = requestAnimationFrame(correct);
  });
  useLayoutEffect(() => {
    const nextScrollTop = pendingScrollTopRef.current;
    if (nextScrollTop === null) return;
    pendingScrollTopRef.current = null;
    pendingHeightRef.current = null;
    applyVerticalZoomScrollTop(nextScrollTop);
    scheduleVerticalZoomScrollCorrection(nextScrollTop);
  }, [applyVerticalZoomScrollTop, height, scheduleVerticalZoomScrollCorrection]);
  useEffect(
    () => () => {
      if (scrollCorrectionRequestRef.current !== null) {
        cancelAnimationFrame(scrollCorrectionRequestRef.current);
      }
    },
    [],
  );

  const requestRef = useRef<number>(0);
  const updateByPlayerStatusRef = useRef<(() => Promise<void>) | null>(null);
  const updateByPlayerStatus = useEvent(async () => {
    const selectSec = player.selectSecRef.current ?? 0;
    if (player.isPlaying) {
      if (
        needFollowCursor.current &&
        player.positionSecRef.current !== null &&
        (endSec < player.positionSecRef.current || startSec > player.positionSecRef.current)
      ) {
        updateLensParams({ startSec: player.positionSecRef.current }, false);
      }
    } else {
      needFollowCursor.current = true;
      const diff = selectSec - prevSelectSecRef.current;
      if (Math.abs(diff) > 1e-6 && (endSec < selectSec || startSec > selectSec)) {
        let newStartSec = startSec + diff;
        const newEndSec = endSec + diff;

        if (newEndSec < selectSec || newStartSec > selectSec)
          newStartSec = selectSec - width / pxPerSec / 2;
        updateLensParams({ startSec: newStartSec }, false);
      }
    }
    prevSelectSecRef.current = selectSec;

    if (updateByPlayerStatusRef.current)
      requestRef.current = requestAnimationFrame(updateByPlayerStatusRef.current);
  });

  useEffect(() => {
    updateByPlayerStatusRef.current = updateByPlayerStatus;
    requestRef.current = requestAnimationFrame(updateByPlayerStatusRef.current);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updateByPlayerStatus]);

  // locator
  const getTimeAxisBoundingLeftWidthTop = useEvent(() => {
    const rect = timeAxisCanvasElem.current?.getBoundingClientRect() ?? null;
    if (rect === null) return null;
    return [rect.left, rect.width, rect.top] as [number, number, number];
  });

  // select locator
  const getSelectLocatorTopBottom: () => [number, number] = useEvent(() => [
    0,
    splitViewElem.current?.getBoundingClientRect()?.height ?? 500,
  ]);
  const calcSelectLocatorPos = useEvent(
    () => ((player.selectSecRef.current ?? 0) - startSec) * pxPerSec,
  );
  const onSelectLocatorMouseDown = useEvent(() => {
    document.addEventListener("mousemove", changeLocatorByMouse);
    document.addEventListener("mouseup", endSelectLocatorDrag, { once: true });
  });

  // playhead
  const getTimeAxisPlayheadTopBottom = useEvent(() => [0, TIME_CANVAS_HEIGHT] as [number, number]);
  const calcPlayheadPos = useEvent(() =>
    player.isPlaying ? ((player.positionSecRef.current ?? 0) - startSec) * pxPerSec : -Infinity,
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
    const { topPlusHalf, bottomMinusHalf, topElem, bottomElem, topId, bottomId } =
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
        ? newWidth / Math.max(maxTrackSec, 1e-8)
        : normalizePxPerSec(pxPerSec, startSec);
    updateLensParams({ startSec: newStartSec, pxPerSec: newPxPerSec }, false);
  });

  // should be useLayoutEffect to avoid jittering of overview lens by width change
  useLayoutEffect(() => {
    if (trackIds.length > 0) setLensParamsForFitCanvas(width, canvasIsFit);

    prevTrackCountRef.current = trackIds.length;
  }, [trackIds, width, setLensParamsForFitCanvas, canvasIsFit, maxTrackSec]);

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
      node.addEventListener("wheel", handleWheel, { passive: false });
      mainViewerElem.current = node;
    },
    [handleWheel],
  );

  const selectTrackByTrackInfo = useEvent((e, id) => selectTrack(e, id, trackIds));
  const getPlayheadSec = useEvent(() => player.positionSecRef.current);

  const leftPane = (
    <>
      <div className={styles.stickyHeader}>
        <TimeUnitSection key="time_unit_label" timeUnitLabel={timeUnitLabel} />
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
      <TrackInfoDragLayer />
      {trackIdsWithFileDropIndicator.map((trackId, iWithIndicator) => {
        if (trackId === -1) {
          return (
            <div
              key="file_drop_indicator_left"
              className={styles.fileDropIndicator}
              style={{ height: FILE_DROP_INDICATOR_HEIGHT }}
            />
          );
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
      <TrackAddButtonSection
        key="track_add_button"
        openAudioTracksHandler={openAudioTracksHandler}
      />
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
          onClickWithoutMods={changeLocatorByMouseNotAllowOutside}
        />
        <span className={styles.axisLabelSection}>Amp</span>
        <span className={styles.axisLabelSection}>Hz</span>
      </div>
      <div className={styles.dummyBoxForStickyHeader} />
      <AudioTrackViewport
        rows={audioViewportRows}
        getViewportRect={getAudioViewportRect}
        width={width}
        rowHeight={height}
        imageHeight={imgHeight}
        getScrollTop={getViewportScrollTop}
        startSec={startSec}
        pxPerSec={pxPerSec}
        maxTrackHz={maxTrackHz}
        freqScale={freqScale}
        hzRange={hzRange}
        ampRange={ampRange}
        blend={blend}
        selectedTrackIds={selectedTrackIds}
        isLoading={isLoading}
        isPlaying={player.isPlaying}
        getPlayheadSec={getPlayheadSec}
        refreshToken={needRefreshTrackIdChArr.join(",")}
        layoutRevision={viewportLayoutRevision}
      />
      {trackIdsWithFileDropIndicator.map((id) => {
        if (id === -1) {
          return (
            <div
              key="file_drop_indicator_right"
              className={styles.fileDropIndicator}
              style={{ height: FILE_DROP_INDICATOR_HEIGHT }}
            />
          );
        }
        return (
          <div key={`${id}`} className={styles.trackRight}>
            {trackIdChMap.get(id)?.map((idChStr) => {
              return (
                <div key={idChStr} className={styles.chCanvases} role="presentation">
                  <div
                    className={styles.audioTrackPlaceholder}
                    style={{ width, height: imgHeight }}
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
                        await removeTracks([trackId]);
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
                    freqScale={freqScale}
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
    </>
  );

  const overviewTrackId =
    trackIds.length > 0 && selectedTrackIds.length > 0
      ? selectedTrackIds[selectedTrackIds.length - 1]
      : null;
  const overviewIdChArr = useMemo(() => {
    if (overviewTrackId === null) return [];
    return trackIdChMap.get(overviewTrackId) || [];
  }, [overviewTrackId, trackIdChMap]);
  return (
    <div className={`flex-container-column flex-item-auto ${styles.mainViewerWrapper}`}>
      {trackIds.length ? (
        <Overview
          trackId={overviewTrackId}
          idChArr={overviewIdChArr}
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
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        role="presentation"
      >
        <SplitView
          ref={splitViewElem}
          left={leftPane}
          right={rightPane}
          setCanvasWidth={setWidth}
          onVerticalViewportChange={onVerticalViewportChange}
          onVerticalViewportResize={onVerticalViewportResize}
        />
        <Locator // on time axis
          locatorStyle="playhead"
          getLineTopBottom={getTimeAxisPlayheadTopBottom}
          getBoundingLeftWidthTop={getTimeAxisBoundingLeftWidthTop}
          calcLocatorPos={calcPlayheadPos}
        />
        <Locator
          ref={selectLocatorElem}
          locatorStyle="selection"
          getLineTopBottom={getSelectLocatorTopBottom}
          getBoundingLeftWidthTop={getTimeAxisBoundingLeftWidthTop}
          calcLocatorPos={calcSelectLocatorPos}
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
