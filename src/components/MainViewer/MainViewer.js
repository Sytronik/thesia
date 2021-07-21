import React, {useRef, useCallback, useEffect, useLayoutEffect, useState} from "react";
import {throttle} from "throttle-debounce";

import "./MainViewer.scss";
import {SplitView} from "./SplitView";
import AxisCanvas from "./AxisCanvas";
import TrackSummary from "./TrackSummary";
import ImgCanvas from "./ImgCanvas";
import {PROPERTY} from "../Property";

const {native} = window.preload;
const {
  getFileName,
  getMaxSec,
  getNumCh,
  getSampleFormat,
  getSec,
  getSr,
  setImgState,
  getImages,
  getTimeAxis,
  getAmpAxis,
  getFreqAxis,
  getdBAxis,
} = native;

const {
  CHANNEL,
  TIME_CANVAS_HEIGHT,
  TIME_MARKER_POS,
  TIME_TICK_SIZE,
  AMP_CANVAS_WIDTH,
  AMP_MARKER_POS,
  AMP_TICK_NUM,
  FREQ_CANVAS_WIDTH,
  FREQ_MARKER_POS,
  FREQ_TICK_NUM,
  DB_CANVAS_WIDTH,
  DB_MARKER_POS,
  DB_TICK_NUM,
} = PROPERTY;
const TIME_BOUNDARIES = Object.keys(TIME_TICK_SIZE)
  .map((boundary) => Number(boundary))
  .sort((a, b) => b - a);
const AMP_BOUNDARIES = Object.keys(AMP_TICK_NUM)
  .map((boundary) => Number(boundary))
  .sort((a, b) => b - a);
const FREQ_BOUNDARIES = Object.keys(FREQ_TICK_NUM)
  .map((boundary) => Number(boundary))
  .sort((a, b) => b - a);
const DB_BOUNDARIES = Object.keys(DB_TICK_NUM)
  .map((boundary) => Number(boundary))
  .sort((a, b) => b - a);

function useRefs() {
  const refs = useRef({});

  const register = useCallback(
    (refName) => (ref) => {
      refs.current[refName] = ref;
    },
    [],
  );

  return [refs, register];
}

function MainViewer({
  erroredList,
  refreshList,
  trackIds,
  addDroppedFile,
  ignoreError,
  reloadTracks,
  removeTracks,
  showOpenDialog,
  selectTrack,
  showContextMenu,
}) {
  const dragCounterRef = useRef(0);
  const prevTrackCountRef = useRef(0);
  const [dropboxIsVisible, setDropboxIsVisible] = useState(false);

  const startSecRef = useRef(0);
  const maxTrackSecRef = useRef(0);
  const canvasIsFitRef = useRef(false);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const drawOptionRef = useRef({px_per_sec: 100});
  const drawOptionForWavRef = useRef({min_amp: -1, max_amp: 1});
  const timeMarkersRef = useRef();
  const timeCanvasElem = useRef();
  const ampMarkersRef = useRef();
  const [ampCanvasesRef, registerAmpCanvas] = useRefs();
  const freqMarkersRef = useRef();
  const [freqCanvasesRef, registerFreqCanvas] = useRefs();
  const dbMarkersRef = useRef();
  const dbCanvasElem = useRef();
  const [imgCanvasesRef, registerImgCanvas] = useRefs();
  const requestRef = useRef();
  const [colorBarHeight, setColorBarHeight] = useState();
  const colorBarElem = useRef();
  const [resizeObserver, _] = useState(
    new ResizeObserver((entries) => {
      const target = entries[0].target;
      console.log(`entries height: ${target.clientHeight}`);
      setColorBarHeight(target.clientHeight - (16 + 2 + 24));
    }),
  );

  const dragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
  };
  const dragEnter = (e) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current++;
    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setDropboxIsVisible(true);
    }
    return false;
  };
  const dragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current--;
    if (dragCounterRef.current === 0) {
      setDropboxIsVisible(false);
    }
    return false;
  };
  const dropReset = () => {
    dragCounterRef.current = 0;
    setDropboxIsVisible(false);
  };

  const handleWheel = (e) => {
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

  const getTickScale = (table, boundaries, target) => {
    for (const boundary of boundaries) {
      if (target > boundary) {
        target = boundary;
        break;
      }
    }
    return table[target];
  };
  const throttledSetTimeMarkers = throttle(1000 / 240, (width) => {
    if (!trackIds.length) {
      timeMarkersRef.current = null;
      return;
    }
    const [minorUnit, minorTickNum] = getTickScale(
      TIME_TICK_SIZE,
      TIME_BOUNDARIES,
      drawOptionRef.current.px_per_sec,
    );
    timeMarkersRef.current = getTimeAxis(
      width,
      startSecRef.current,
      drawOptionRef.current.px_per_sec,
      minorUnit,
      minorTickNum,
    );
  });
  const throttledSetAmpFreqMarkers = throttle(1000 / 240, (height) => {
    if (!trackIds.length) return;
    const [ampTickNum, ampLableNum] = getTickScale(AMP_TICK_NUM, AMP_BOUNDARIES, height);
    ampMarkersRef.current = getAmpAxis(
      height,
      ampTickNum,
      ampLableNum,
      drawOptionForWavRef.current,
    );
    const [freqTickNum, freqLabelNum] = getTickScale(FREQ_TICK_NUM, FREQ_BOUNDARIES, height);
    freqMarkersRef.current = getFreqAxis(height, freqTickNum, freqLabelNum);
  });
  const throttledSetDbMarkers = throttle(1000 / 240, (height) => {
    if (!trackIds.length) return;
    const [dbTickNum, dbLabelNum] = getTickScale(DB_TICK_NUM, DB_BOUNDARIES, height);
    dbMarkersRef.current = getdBAxis(height, dbTickNum, dbLabelNum);
  });

  const throttledSetImgState = useCallback(
    throttle(1000 / 240, (idChArr, width, height) => {
      if (!idChArr.length) return;
      setImgState(
        idChArr,
        startSecRef.current,
        width,
        {...drawOptionRef.current, height: height},
        drawOptionForWavRef.current,
        0.3,
      );
    }),
    [],
  );

  const drawCanvas = async () => {
    const images = getImages();
    let promises = [];
    for (const [idChStr, buf] of Object.entries(images)) {
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
    }
    if (timeCanvasElem.current) {
      timeCanvasElem.current.draw(timeMarkersRef.current);
    }
    if (dbCanvasElem.current) {
      dbCanvasElem.current.draw(dbMarkersRef.current);
    }
    await Promise.all(promises);
    requestRef.current = requestAnimationFrame(drawCanvas);
  };

  const dropbox = <div className="dropbox"></div>;

  const timeUnit = (
    <div key="time" className="time-unit">
      <p>unit</p>
    </div>
  ); // [TEMP]
  const tracksLeft = trackIds.map((id) => {
    const channels = [...Array(getNumCh(id)).keys()].map((ch) => {
      return (
        <div key={`${id}_${ch}`} className="ch">
          <span>{CHANNEL[getNumCh(id)][ch]}</span>
        </div>
      );
    });
    const trackSummary = {
      fileName: getFileName(id),
      time: new Date(getSec(id).toFixed(3) * 1000).toISOString().substr(11, 12),
      sampleFormat: getSampleFormat(id),
      sampleRate: `${getSr(id)} Hz`,
    };

    return (
      <div
        key={`${id}`}
        className="track-left js-track-left"
        id={id}
        onClick={selectTrack}
        onContextMenu={showContextMenu}
      >
        <div className="channels">{channels}</div>
        <TrackSummary data={trackSummary} height={(height + 2) * getNumCh(id) - 2} />
      </div>
    );
  });
  const tracksEmpty = (
    <div key="empty" className="track-empty">
      <button onClick={showOpenDialog}>
        <span className="btn-plus"></span>
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
      className="time-ruler"
    />
  );
  const tracksRight = trackIds.map((id) => {
    const errorBox = (
      <div className="error-box">
        <p>The file is corrupted and cannot be opened</p>
        <div>
          <button type="button" onClick={() => reloadTracks([id])}>
            Reload
          </button>
          <button type="button" onClick={() => ignoreError(id)}>
            Ignore
          </button>
          <button type="button" onClick={() => removeTracks([id])}>
            Close
          </button>
        </div>
      </div>
    );
    const channelsCanvases = [...Array(getNumCh(id)).keys()].map((ch) => {
      return (
        <div key={`${id}_${ch}`} className="ch-canvases">
          <AxisCanvas
            key={`freq_${id}_${ch}`}
            ref={registerFreqCanvas(`${id}_${ch}`)}
            width={FREQ_CANVAS_WIDTH}
            height={height}
            markerPos={FREQ_MARKER_POS}
            direction="V"
            className="freq-axis"
          />
          <AxisCanvas
            key={`amp_${id}_${ch}`}
            ref={registerAmpCanvas(`${id}_${ch}`)}
            width={AMP_CANVAS_WIDTH}
            height={height}
            markerPos={AMP_MARKER_POS}
            direction="V"
            className="amp-axis"
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
      <div key={`${id}`} className="track-right js-track-right">
        {erroredList.includes(id) ? errorBox : null}
        {channelsCanvases}
      </div>
    );
  });

  const getIdChArr = () => Object.keys(imgCanvasesRef.current);

  useEffect(() => {
    const viewer = document.querySelector(".js-MainViewer");
    viewer.addEventListener("wheel", handleWheel, {passive: false});

    return () => {
      viewer.removeEventListener("wheel", handleWheel, {passive: false});
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
    dropReset();
  }, [erroredList]);

  useEffect(() => {
    throttledSetImgState(refreshList, width, height);
    throttledSetTimeMarkers(width);
    throttledSetAmpFreqMarkers(height);
    throttledSetDbMarkers(colorBarHeight);
    dropReset();
  }, [refreshList]);

  useEffect(() => {
    if (trackIds.length) {
      maxTrackSecRef.current = getMaxSec();
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
    <div
      className="MainViewer js-MainViewer row-flex"
      onDrop={addDroppedFile}
      onDragOver={dragOver}
      onDragEnter={dragEnter}
      onDragLeave={dragLeave}
    >
      {dropboxIsVisible && dropbox}
      <SplitView
        left={[timeUnit, ...tracksLeft, tracksEmpty]}
        right={[timeRuler, tracksRight]}
        setCanvasWidth={setWidth}
      />
      <div className="color-bar" ref={colorBarElem}>
        <AxisCanvas
          ref={dbCanvasElem}
          width={DB_CANVAS_WIDTH}
          height={colorBarHeight}
          markerPos={DB_MARKER_POS}
          direction="V"
          className="db-axis"
        />
      </div>
    </div>
  );
}

export default MainViewer;
