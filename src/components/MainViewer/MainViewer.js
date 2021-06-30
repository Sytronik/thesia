import React, {useRef, useCallback, useEffect, useState} from "react";
import {throttle} from "throttle-debounce";

import "./MainViewer.scss";
import {SplitView} from "./SplitView";
import AxisCanvas from "./AxisCanvas";
import TrackSummary from "./TrackSummary";
import ImgCanvas from "./ImgCanvas";
import {PROPERTY} from "../Property";

const {native} = window.preload;
const {getFileName, getMaxSec, getNumCh, getSampleFormat, getSec, getSr, setImgState, getImages} =
  native;

const CHANNEL = PROPERTY.CHANNEL;
const TIME_CANVAS_HEIGHT = PROPERTY.TIME_CANVAS_HEIGHT;
const TIME_MARKER_POS = PROPERTY.TIME_MARKER_POS;
const TIME_DIVISION_SCALE = PROPERTY.TIME_DIVISION_SCALE;
const TIME_BOUNDARIES = Object.keys(TIME_DIVISION_SCALE)
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

  const secRef = useRef(0);
  const maxTrackSecRef = useRef(0);
  const canvasIsFitRef = useRef(false);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const drawOptionRef = useRef({px_per_sec: 100});
  const timeCanvasElem = useRef();
  const [imgCanvasesRef, registerImgCanvas] = useRefs();
  const requestRef = useRef();

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
            width / (maxTrackSecRef.current - secRef.current),
          ),
          384000,
        );
        if (drawOptionRef.current.px_per_sec !== pxPerSec) {
          drawOptionRef.current.px_per_sec = pxPerSec;
          canvasIsFitRef.current = false;
          throttledSetImgState(getIdChArr(), width, height);
        }
      } else {
        setHeight(Math.round(Math.min(Math.max(height * (1 + e.deltaY / 1000), 10), 5000)));
      }
    } else if ((e.shiftKey && yIsLarger) || !yIsLarger) {
      e.preventDefault();
      e.stopPropagation();
      const tempSec = Math.min(
        Math.max(secRef.current + delta / drawOptionRef.current.px_per_sec, 0),
        maxTrackSecRef.current - width / drawOptionRef.current.px_per_sec,
      );
      if (secRef.current !== tempSec) {
        secRef.current = tempSec;
        throttledSetImgState(getIdChArr(), width, height);
      }
    }
  };

  const getLinearAxis = (BOUNDARIES) => {
    let minorUnit = 0;
    let minorTickNum = 0;
    for (const PX_BOUNDARY of BOUNDARIES) {
      if (drawOptionRef.current.px_per_sec > PX_BOUNDARY) {
        [minorUnit, minorTickNum] = TIME_DIVISION_SCALE[PX_BOUNDARY];
        break;
      }
    }

    const endCount = (secRef.current + width / drawOptionRef.current.px_per_sec) / minorUnit;
    const markers = [];
    for (let count = Math.ceil(secRef.current / minorUnit); count < endCount; count++) {
      const sec = count * minorUnit;
      markers.push([
        (sec - secRef.current) * drawOptionRef.current.px_per_sec,
        count % minorTickNum ? 0 : `${sec}`,
      ]); // sec hh:mm:ss.000 형식 맞추기, 부동 소숫점 주의
    }
    return markers;
  };

  const throttledSetImgState = useCallback(
    throttle(1000 / 240, (idChArr, width, height) => {
      if (idChArr.length === 0) return;
      setImgState(
        idChArr,
        secRef.current,
        width,
        {...drawOptionRef.current, height: height},
        {min_amp: -1, max_amp: 1},
        0.3,
      );
    }),
    [],
  );

  const drawCanvas = async (_) => {
    const images = getImages();
    let promises = [];
    for (const [idChStr, buf] of Object.entries(images)) {
      const imgCanvasRef = imgCanvasesRef.current[idChStr];
      if (imgCanvasRef) {
        promises.push(imgCanvasRef.draw(buf));
      }
    }
    if (timeCanvasElem.current) {
      const timeMarkers = getLinearAxis(TIME_BOUNDARIES);
      timeCanvasElem.current.draw(timeMarkers);
    }
    await Promise.all(promises);
    requestRef.current = requestAnimationFrame(drawCanvas);
  };

  const dropbox = <div className="dropbox"></div>;

  const timeUnit = (
    <div key="time" className="LeftPane-time">
      <p>unit</p>
    </div>
  ); // [TEMP]
  const leftElements = trackIds.map((id) => {
    const channels = [...Array(getNumCh(id)).keys()].map((ch) => {
      return (
        <div key={ch} className="ch">
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
        className="LeftPane-track js-LeftPane-track"
        id={id}
        onClick={selectTrack}
        onContextMenu={showContextMenu}
      >
        <div className="channels">{channels}</div>
        <TrackSummary data={trackSummary} height={(height + 2) * getNumCh(id) - 2} />
      </div>
    );
  });
  const emptyTrack = (
    <div key="empty" className="LeftPane-empty">
      <button onClick={showOpenDialog}>
        <span className="btn-plus"></span>
      </button>
    </div>
  );

  const timeRuler = (
    <AxisCanvas
      key="timeruler"
      ref={timeCanvasElem}
      width={width}
      height={TIME_CANVAS_HEIGHT}
      markerPos={TIME_MARKER_POS}
      direction="H"
    />
  );

  const rightElements = trackIds.map((id) => {
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
    const canvases = [...Array(getNumCh(id)).keys()].map((ch) => {
      return (
        <ImgCanvas
          key={`${id}_${ch}`}
          ref={registerImgCanvas(`${id}_${ch}`)}
          width={width}
          height={height}
        />
      );
    });
    return (
      <div key={`${id}`} className="canvases js-canvases">
        {erroredList.includes(id) ? errorBox : null}
        {canvases}
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
      if (secRef.current > secOutOfCanvas) {
        secRef.current = secOutOfCanvas;
      }
    }
    throttledSetImgState(getIdChArr(), width, height);
  }, [width]);

  useEffect(() => {
    if (!trackIds.length) return;
    throttledSetImgState(getIdChArr(), width, height);
  }, [height]);

  useEffect(() => {
    dropReset();
  }, [erroredList]);

  useEffect(() => {
    throttledSetImgState(refreshList, width, height);
    dropReset();
  }, [refreshList]);

  useEffect(() => {
    if (trackIds.length) {
      maxTrackSecRef.current = getMaxSec();
      if (!prevTrackCountRef.current) {
        drawOptionRef.current.px_per_sec = width / maxTrackSecRef.current;
        secRef.current = 0;
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

  return (
    <div
      className="MainViewer js-MainViewer"
      onDrop={addDroppedFile}
      onDragOver={dragOver}
      onDragEnter={dragEnter}
      onDragLeave={dragLeave}
    >
      {dropboxIsVisible && dropbox}
      <SplitView
        left={[timeUnit, ...leftElements, emptyTrack]}
        right={[timeRuler, rightElements]}
        setCanvasWidth={setWidth}
      />
    </div>
  );
}

export default MainViewer;
