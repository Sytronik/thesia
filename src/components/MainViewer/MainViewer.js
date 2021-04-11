import React, {useRef, useCallback, useEffect, useState} from "react";
import {throttle, debounce} from "throttle-debounce";

import "./MainViewer.scss";
import {SplitView} from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

const {native} = window.preload;
const {getFileName, getNumCh, getSampleFormat, getSec, getSr, getSpecWavImages} = native;
const CHANNEL = {
  1: ["M"],
  2: ["L", "R"],
};

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
  refreshList,
  trackIds,
  addDroppedFile,
  showOpenDialog,
  selectTrack,
  showContextMenu,
}) {
  const dragCounterRef = useRef(0);
  const prevTrackCountRef = useRef(0);
  const [dropboxIsVisible, setDropboxIsVisible] = useState(false);

  const secRef = useRef(0);
  const maxTrackSecRef = useRef(0);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const drawOptionRef = useRef({px_per_sec: 100});
  const [children, registerChild] = useRefs();

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
        setHeight(Math.round(Math.min(Math.max(height * (1 + delta / 1000), 10), 5000)));
      } else {
        const pxPerSec = Math.min(
          Math.max(drawOptionRef.current.px_per_sec * (1 + e.deltaY / 1000), 0),
          384000,
        );
        if (drawOptionRef.current.px_per_sec !== pxPerSec) {
          drawOptionRef.current.px_per_sec = pxPerSec;
          throttledDraw([getIdChArr()]);
        }
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
        throttledDraw([getIdChArr()]);
      }
    }
  };

  const draw = useCallback(
    async (idChArr) => {
      if (idChArr.reduce((sum, x) => sum + x.length, 0) === 0) return;
      const [images, promise] = getSpecWavImages(
        idChArr[0],
        secRef.current,
        width,
        {...drawOptionRef.current, height: height},
        {min_amp: -1, max_amp: 1},
      );

      for (const [idChStr, bufs] of Object.entries(images)) {
        const ref = children.current[idChStr];
        // let promises = [];
        if (ref) {
          // promises.push(
          ref.draw(bufs);
          // );
        }
        // Promise.all(promises);
      }

      // cached image
      if (promise !== null) {
        const arr = await promise;
        debouncedDraw(arr);
      }
    },
    [height, width],
  );

  const throttledDraw = useCallback(throttle(1000 / 60, draw), [draw]);
  const debouncedDraw = useCallback(debounce(1000 / 30, draw), [draw]);
  // const throttledDraw = draw;
  // const debouncedDraw = draw;

  const dropbox = <div className="dropbox"></div>;

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
        <TrackInfo data={trackSummary} height={(height + 2) * getNumCh(id) - 2} />
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

  const rightElements = trackIds.map((id) => {
    const canvases = [...Array(getNumCh(id)).keys()].map((ch) => {
      return (
        <Canvas
          key={`${id}_${ch}`}
          ref={registerChild(`${id}_${ch}`)}
          width={width}
          height={height}
        />
      );
    });
    return (
      <div key={`${id}`} className="canvases">
        {canvases}
      </div>
    );
  });

  const getIdChArr = () => Object.keys(children.current);

  useEffect(() => {
    const viewer = document.querySelector(".js-MainViewer");
    viewer.addEventListener("wheel", handleWheel, {passive: false});

    return () => {
      viewer.removeEventListener("wheel", handleWheel, {passive: false});
    };
  });

  useEffect(() => {
    throttledDraw([getIdChArr()]);
  }, [width]);

  useEffect(() => {
    debouncedDraw([getIdChArr()]);
  }, [height]);

  useEffect(() => {
    if (refreshList) {
      draw(refreshList);
    }
    dropReset();
  }, [refreshList]);

  useEffect(() => {
    if (trackIds.length) {
      maxTrackSecRef.current = trackIds.reduce((max, id) => {
        const now = getSec(id);
        return now > max ? now : max;
      }, 0);
      if (!prevTrackCountRef.current) {
        drawOptionRef.current.px_per_sec = width / maxTrackSecRef.current;
        secRef.current = 0;
      }
    }
    prevTrackCountRef.current = trackIds.length;
  }, [trackIds]);

  return (
    <div
      className="MainViewer js-MainViewer"
      onDrop={addDroppedFile}
      onDragOver={dragOver}
      onDragEnter={dragEnter}
      onDragLeave={dragLeave}
    >
      {dropboxIsVisible && dropbox}
      {/* <TimeRuler /> */}
      <SplitView
        left={[...leftElements, emptyTrack]}
        right={rightElements}
        setCanvasWidth={setWidth}
      />
    </div>
  );
}

export default MainViewer;
