import React, {useRef, useCallback, useEffect, useLayoutEffect, useState} from "react";
import {throttle} from "throttle-debounce";
import AxisCanvas from "renderer/modules/AxisCanvas";
import useDropzone from "renderer/hooks/useDropzone";
import ImgCanvas from "renderer/modules/ImgCanvas";
import SplitView from "renderer/modules/SplitView";
import styles from "./MainViewer.scss";
import TrackInfo from "./TrackInfo";
import NativeAPI from "../../api";
import {
  TIME_CANVAS_HEIGHT,
  TIME_MARKER_POS,
  TIME_TICK_SIZE,
  TIME_BOUNDARIES,
  AMP_CANVAS_WIDTH,
  AMP_MARKER_POS,
  AMP_TICK_NUM,
  AMP_BOUNDARIES,
  FREQ_CANVAS_WIDTH,
  FREQ_MARKER_POS,
  FREQ_TICK_NUM,
  FREQ_BOUNDARIES,
  DB_CANVAS_WIDTH,
  DB_MARKER_POS,
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
  showOpenDialog: () => void;
  selectTrack: (e: React.MouseEvent, id: number) => void;
  showTrackContextMenu: (e: React.MouseEvent, id: number) => void;
};

type ReactRefsObject<T> = {
  [key: string]: T;
};

function useRefs<T>(): [
  React.MutableRefObject<ReactRefsObject<T>>,
  (refName: string) => React.RefCallback<T>,
] {
  const refs = useRef<ReactRefsObject<T>>({});

  const register = useCallback(
    (refName: string) => (ref: T) => {
      refs.current[refName] = ref;
    },
    [],
  );

  return [refs, register];
}

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
    showOpenDialog,
    selectTrack,
    showTrackContextMenu,
  } = props;

  const mainViewerElem = useRef<HTMLDivElement>(null);
  const prevTrackCountRef = useRef<number>(0);

  const startSecRef = useRef<number>(0);
  const maxTrackSecRef = useRef<number>(0);
  const canvasIsFitRef = useRef<boolean>(false);

  const requestRef = useRef<number>(0);

  const [imgCanvasesRef, registerImgCanvas] = useRefs<ImgCanvasHandleElement>();
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const drawOptionRef = useRef({px_per_sec: 100});
  const drawOptionForWavRef = useRef({min_amp: -1, max_amp: 1});
  const colorBarElem = useRef<HTMLDivElement>(null);
  const [colorBarHeight, setColorBarHeight] = useState<number>(0);

  const timeCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const timeMarkersRef = useRef<Markers>([]);
  const [timeUnitLabel, setTimeUnitLabel] = useState<string>("");
  const [ampCanvasesRef, registerAmpCanvas] = useRefs<AxisCanvasHandleElement>();
  const ampMarkersRef = useRef<Markers>([]);
  const [freqCanvasesRef, registerFreqCanvas] = useRefs<AxisCanvasHandleElement>();
  const freqMarkersRef = useRef<Markers>([]);
  const dbCanvasElem = useRef<AxisCanvasHandleElement>(null);
  const dbMarkersRef = useRef<Markers>([]);

  const [resizeObserver, setResizeObserver] = useState(
    new ResizeObserver((entries) => {
      const {target} = entries[0];
      setColorBarHeight(target.clientHeight - (16 + 2 + 24));
    }),
  );

  const {isDropzoneActive} = useDropzone({targetRef: mainViewerElem, handleDrop: addDroppedFile});

  const reloadAndRefreshTracks = useCallback(
    (ids: number[]) => {
      reloadTracks(ids);
      refreshTracks();
    },
    [reloadTracks, refreshTracks],
  );
  const removeAndRefreshTracks = useCallback(
    (ids: number[]) => {
      removeTracks(ids);
      refreshTracks();
    },
    [removeTracks, refreshTracks],
  );

  const getTickScale = (table: TickScaleTable, boundaries: number[], value: number) => {
    const target = boundaries.find((boundary) => value > boundary);
    if (target === undefined) return table[value];
    return table[target];
  };

  const throttledSetTimeMarkers = throttle(1000 / 240, (width: number) => {
    if (!trackIds.length) {
      timeMarkersRef.current = [];
      return;
    }
    const [minorUnit, minorTickNum] = getTickScale(
      TIME_TICK_SIZE,
      TIME_BOUNDARIES,
      drawOptionRef.current.px_per_sec,
    );
    const timeMarkers = NativeAPI.getTimeAxisMarkers(
      width,
      startSecRef.current,
      drawOptionRef.current.px_per_sec,
      minorUnit,
      minorTickNum,
    );
    const timeUnit = timeMarkers.pop()?.[1] || "ss";
    setTimeUnitLabel(timeUnit);
    timeMarkersRef.current = timeMarkers;
  });

  const throttledSetAmpFreqMarkers = throttle(1000 / 240, (height: number) => {
    if (!trackIds.length) return;
    const [maxAmpNumTicks, maxAmpNumLabels] = getTickScale(AMP_TICK_NUM, AMP_BOUNDARIES, height);
    ampMarkersRef.current = NativeAPI.getAmpAxisMarkers(
      height,
      maxAmpNumTicks,
      maxAmpNumLabels,
      drawOptionForWavRef.current,
    );
    const [maxFreqNumTicks, maxFreqNumLabels] = getTickScale(
      FREQ_TICK_NUM,
      FREQ_BOUNDARIES,
      height,
    );
    freqMarkersRef.current = NativeAPI.getFreqAxisMarkers(
      height,
      maxFreqNumTicks,
      maxFreqNumLabels,
    );
  });

  const throttledSetDbMarkers = throttle(1000 / 240, (height: number) => {
    if (!trackIds.length) return;
    const [maxDeciBelNumTicks, maxDeciBelNumLabels] = getTickScale(
      DB_TICK_NUM,
      DB_BOUNDARIES,
      height,
    );
    dbMarkersRef.current = NativeAPI.getDbAxisMarkers(
      height,
      maxDeciBelNumTicks,
      maxDeciBelNumLabels,
    );
  });

  const throttledSetImgState = useCallback(
    throttle(1000 / 240, (idChArr: IdChannel[], width: number, height: number) => {
      if (!idChArr.length) return;
      NativeAPI.setImageState(
        idChArr,
        startSecRef.current,
        width,
        {...drawOptionRef.current, height},
        drawOptionForWavRef.current,
        0.3,
      );
    }),
    [],
  );

  const getIdChArr = () => Object.keys(imgCanvasesRef.current);

  const handleWheel = (e: WheelEvent) => {
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
            drawOptionRef.current.px_per_sec * (1 + delta / 1000),
            width / (maxTrackSecRef.current - startSecRef.current),
          ),
          384000,
        );
        if (drawOptionRef.current.px_per_sec !== pxPerSec) {
          drawOptionRef.current.px_per_sec = pxPerSec;
          canvasIsFitRef.current = false;
          throttledSetImgState(getIdChArr(), width, height);
          throttledSetTimeMarkers(width);
        }
      } else {
        setHeight(Math.round(Math.min(Math.max(height * (1 + e.deltaY / 1000), 10), 5000)));
      }
    } else if ((e.shiftKey && yIsLarger) || !yIsLarger) {
      e.preventDefault();
      e.stopPropagation();
      const tempSec = Math.min(
        Math.max(startSecRef.current + delta / drawOptionRef.current.px_per_sec, 0),
        maxTrackSecRef.current - width / drawOptionRef.current.px_per_sec,
      );
      if (startSecRef.current !== tempSec) {
        startSecRef.current = tempSec;
        throttledSetImgState(getIdChArr(), width, height);
        throttledSetTimeMarkers(width);
      }
    }
  };

  const drawCanvas = async () => {
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
  };

  const timeUnit = (
    <div key="time_unit_label" className={styles.timeUnit}>
      <p>{timeUnitLabel}</p>
    </div>
  );

  const tracksInfos = trackIds.map((trackId: number) => {
    const isSelected = selectedTrackIds.includes(trackId);
    return (
      <TrackInfo
        key={`${trackId}`}
        trackId={trackId}
        height={height}
        isSelected={isSelected}
        selectTrack={selectTrack}
        showTrackContextMenu={showTrackContextMenu}
      />
    );
  });

  const tracksEmpty = (
    <div key="empty" className={styles.trackEmpty}>
      <button type="button" onClick={showOpenDialog}>
        <span className={styles.btnPlus} />
      </button>
    </div>
  );

  const timeRuler = (
    <AxisCanvas
      key="time"
      ref={timeCanvasElem}
      width={width}
      height={TIME_CANVAS_HEIGHT}
      markerPos={TIME_MARKER_POS}
      direction="H"
      className="timeRuler"
    />
  );

  const tracksRight = trackIds.map((id) => {
    const errorBox = (
      <div className={styles.errorBox}>
        <p>The file is corrupted and cannot be opened</p>
        <div>
          <button type="button" onClick={() => reloadAndRefreshTracks([id])}>
            Reload
          </button>
          <button type="button" onClick={() => ignoreError(id)}>
            Ignore
          </button>
          <button type="button" onClick={() => removeAndRefreshTracks([id])}>
            Close
          </button>
        </div>
      </div>
    );

    const channelsCanvases = [...Array(NativeAPI.getChannelCounts(id)).keys()].map((ch) => {
      return (
        <div key={`${id}_${ch}`} className={styles.chCanvases}>
          <AxisCanvas
            key={`freq_${id}_${ch}`}
            ref={registerFreqCanvas(`${id}_${ch}`)}
            width={FREQ_CANVAS_WIDTH}
            height={height}
            markerPos={FREQ_MARKER_POS}
            direction="V"
            className="freqAxis"
          />
          <AxisCanvas
            key={`amp_${id}_${ch}`}
            ref={registerAmpCanvas(`${id}_${ch}`)}
            width={AMP_CANVAS_WIDTH}
            height={height}
            markerPos={AMP_MARKER_POS}
            direction="V"
            className="ampAxis"
          />
          <ImgCanvas
            key={`img_${id}_${ch}`}
            ref={registerImgCanvas(`${id}_${ch}`)}
            width={width}
            height={height}
          />
        </div>
      );
    });
    return (
      <div key={`${id}`} className={`${styles.trackRight} js-track-right`}>
        {erroredTrackIds.includes(id) ? errorBox : null}
        {channelsCanvases}
      </div>
    );
  });

  useEffect(() => {
    const mainViewer = mainViewerElem.current;
    mainViewer?.addEventListener("wheel", handleWheel, {passive: false});

    return () => {
      mainViewer?.removeEventListener("wheel", handleWheel);
    };
  });

  useEffect(() => {
    if (!trackIds.length) return;

    const secOutOfCanvas = maxTrackSecRef.current - width / drawOptionRef.current.px_per_sec;
    if (canvasIsFitRef.current) {
      drawOptionRef.current.px_per_sec = width / maxTrackSecRef.current;
    } else {
      if (secOutOfCanvas <= 0) {
        canvasIsFitRef.current = true;
        return;
      }
      if (startSecRef.current > secOutOfCanvas) {
        startSecRef.current = secOutOfCanvas;
      }
    }
    throttledSetImgState(getIdChArr(), width, height);
    throttledSetTimeMarkers(width);
  }, [width]);

  useEffect(() => {
    if (!trackIds.length) return;
    throttledSetImgState(getIdChArr(), width, height);
    throttledSetAmpFreqMarkers(height);
  }, [height]);

  useEffect(() => {
    if (!trackIds.length) return;
    throttledSetDbMarkers(colorBarHeight);
  }, [colorBarHeight]);

  useEffect(() => {
    throttledSetImgState(needRefreshTrackIds, width, height);
    throttledSetTimeMarkers(width);
    throttledSetAmpFreqMarkers(height);
    throttledSetDbMarkers(colorBarHeight);
  }, [needRefreshTrackIds]);

  useEffect(() => {
    if (trackIds.length) {
      maxTrackSecRef.current = NativeAPI.getLongestTrackLength();
      if (!prevTrackCountRef.current) {
        drawOptionRef.current.px_per_sec = width / maxTrackSecRef.current;
        startSecRef.current = 0;
        canvasIsFitRef.current = true;
      }
    } else {
      maxTrackSecRef.current = 0;
      canvasIsFitRef.current = false;
    }
    prevTrackCountRef.current = trackIds.length;
  }, [trackIds]);

  useEffect(() => {
    requestRef.current = requestAnimationFrame(drawCanvas);
    return () => cancelAnimationFrame(requestRef.current);
  }, []);

  useLayoutEffect(() => {
    if (colorBarElem.current) {
      resizeObserver.observe(colorBarElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [colorBarElem, resizeObserver]);

  return (
    <div className={`${styles.MainViewer} row-flex`} ref={mainViewerElem}>
      {isDropzoneActive && <div className={styles.dropzone} />}
      <SplitView
        left={[timeUnit, ...tracksInfos, tracksEmpty]}
        right={[timeRuler, tracksRight]}
        setCanvasWidth={setWidth}
      />
      <div className={styles.colorBar} ref={colorBarElem}>
        <AxisCanvas
          ref={dbCanvasElem}
          width={DB_CANVAS_WIDTH}
          height={colorBarHeight}
          markerPos={DB_MARKER_POS}
          direction="V"
          className="dbAxis"
        />
      </div>
    </div>
  );
}

export default MainViewer;
