import React, {useRef, useCallback, useEffect, useState} from "react";
import {throttle, debounce} from "throttle-debounce";

import "./MainViewer.scss";
import {SplitView} from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

const {native} = window.preload;
const {getFileName, getSampleFormat, getSec, getSr, getSpecWavImages} = native;

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

function MainViewer({refresh_list, track_ids, dropFile, openDialog, selectTrack, showContextMenu}) {
  const dragcounter = useRef(0);
  const prev_track_num = useRef(0);
  const [show_dropbox, setShowDropbox] = useState(false);

  const sec = useRef(0);
  const max_sec = useRef(0);
  const [width, setWidth] = useState(600);
  const [height, setHeight] = useState(250);
  const draw_option = useRef({px_per_sec: 100});
  const [children, registerChild] = useRefs();

  const track_infos = track_ids.map((id) => {
    return {
      filename: getFileName(id),
      time: new Date(getSec(id).toFixed(3) * 1000).toISOString().substr(11, 12),
      sampleformat: getSampleFormat(id),
      sr: `${getSr(id)} Hz`,
    };
  });

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

  const handleWheel = (e) => {
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
      if (sec.current < 0) {
        sec.current = 0;
      } else if (sec.current + width / draw_option.current.px_per_sec > max_sec.current) {
        sec.current = max_sec.current - width / draw_option.current.px_per_sec;
      }
      throttledDraw([getIdChArr()]);
    }
  };

  const draw = useCallback(
    async (id_ch_arr) => {
      if (id_ch_arr.reduce((sum, x) => sum + x.length, 0) === 0) return;
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
  const info_arr = track_ids.map((id, i) => {
    return (
      <TrackInfo
        key={`${id}`}
        trackid={id}
        trackinfo={track_infos[i]}
        height={height}
        selectTrack={selectTrack}
        showContextMenu={showContextMenu}
      />
    );
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

  useEffect(() => {
    const mainviewer = document.querySelector(".MainViewer");
    document.addEventListener("wheel", handleWheel, {passive: false});

    return () => {
      document.removeEventListener("wheel", handleWheel, {passive: false});
    };
  });

  useEffect(() => {
    throttledDraw([getIdChArr()]);
  }, [width]);

  useEffect(() => {
    debouncedDraw([getIdChArr()]);
  }, [height]);

  useEffect(() => {
    if (refresh_list) {
      draw(refresh_list);
    }
    dropReset();
  }, [refresh_list]);

  useEffect(() => {
    if (track_ids.length) {
      max_sec.current = track_ids.reduce((max, id) => {
        const now = getSec(id);
        return now > max ? now : max;
      });
      if (!prev_track_num.current) {
        draw_option.current.px_per_sec = width / max_sec.current;
        sec.current = 0;
      }
    }
    prev_track_num.current = track_ids.length;
  }, [track_ids]);

  return (
    <div
      className="MainViewer"
      onDrop={dropFile}
      onDragOver={dragOver}
      onDragEnter={dragEnter}
      onDragLeave={dragLeave}
    >
      {show_dropbox && dropbox}
      {/* <TimeRuler /> */}
      <SplitView left={[...info_arr, empty]} right={canvas_arr} setCanvasWidth={setWidth} />
    </div>
  );
}

export default MainViewer;
