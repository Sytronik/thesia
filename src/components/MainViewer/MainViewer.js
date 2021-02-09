import React, {useRef, useCallback, useEffect, useState} from "react";
import {throttle, debounce} from "throttle-debounce";

import "./MainViewer.scss";
import {SplitView} from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

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

function MainViewer({native, dropFile, openDialog, refresh_list, track_ids}) {
  const {getSpecWavImages} = native;

  const dragcounter = useRef(0);
  const [show_dropbox, setShowDropbox] = useState(false);

  const dragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
  };
  const dragEnter = (e) => {
    e.preventDefault();
    e.stopPropagation();

    dragcounter.current++;
    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setShowDropbox(true);
    }
    return false;
  };
  const dragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();

    dragcounter.current--;
    if (dragcounter.current === 0) {
      setShowDropbox(false);
    }
    return false;
  };
  const dropReset = () => {
    dragcounter.current = 0;
    setShowDropbox(false);
  };

  const sec = useRef(0);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const draw_option = useRef({px_per_sec: 100});
  const [children, registerChild] = useRefs();

  const dropbox = <div className="dropbox"></div>;
  const info_arr = track_ids.map((i) => {
    return <TrackInfo key={`${i}`} height={height} />;
  });
  const empty = (
    <div key="empty" className="emptyTrack">
      <button onClick={openDialog}>
        <span className="plusbtn"></span>
      </button>
    </div>
  );
  const canvas_arr = track_ids.map((i) => {
    return <Canvas key={`${i}`} ref={registerChild(`${i}_0`)} width={width} height={height} />;
  });
  const getIdChArr = () => Object.keys(children.current);

  function handleWheel(e) {
    let y_is_larger;
    let delta;
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      delta = e.deltaY;
      y_is_larger = true;
    } else {
      delta = e.deltaX;
      y_is_larger = false;
    }
    if (e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      if ((e.shiftKey && y_is_larger) || !y_is_larger) {
        setHeight(Math.round(Math.min(Math.max(height * (1 + delta / 1000), 10), 5000)));
      } else {
        const px_per_sec = Math.min(
          Math.max(draw_option.current.px_per_sec * (1 + e.deltaY / 1000), 0),
          384000,
        );
        if (draw_option.current.px_per_sec !== px_per_sec) {
          draw_option.current.px_per_sec = px_per_sec;
          throttledDraw([getIdChArr()]);
        }
      }
    } else if ((e.shiftKey && y_is_larger) || !y_is_larger) {
      e.preventDefault();
      e.stopPropagation();
      sec.current += delta / draw_option.current.px_per_sec;
      throttledDraw([getIdChArr()]);
    }
  }

  const draw = useCallback(
    async (id_ch_arr) => {
      const [images, promise] = getSpecWavImages(
        id_ch_arr[0],
        sec.current,
        width,
        {...draw_option.current, height: height},
        {min_amp: -1, max_amp: 1},
      );

      for (const [id_ch_str, bufs] of Object.entries(images)) {
        const ref = children.current[id_ch_str];
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
        if (arr) {
          // console.log(arr);
          debouncedDraw(arr);
        }
      }
    },
    [height, width],
  );

  const throttledDraw = useCallback(throttle(1000 / 60, draw), [draw]);
  const debouncedDraw = useCallback(debounce(1000 / 60, draw), [draw]);
  // const throttledDraw = draw;
  // const debouncedDraw = draw;

  useEffect(() => {
    throttledDraw([getIdChArr()]);
  }, [draw, height, width]);

  useEffect(() => {
    if (refresh_list) {
      draw(refresh_list);
    }
    dropReset();
  }, [refresh_list]);

  return (
    <div
      onDrop={dropFile}
      onDragOver={dragOver}
      onDragEnter={dragEnter}
      onDragLeave={dragLeave}
      onWheel={handleWheel}
      className="MainViewer"
    >
      {show_dropbox && dropbox}
      {/* <TimeRuler /> */}
      <SplitView
        left={[...info_arr, empty]}
        right={canvas_arr}
        canvasWidth={width}
        setCanvasWidth={setWidth}
      />
    </div>
  );
}

export default MainViewer;
