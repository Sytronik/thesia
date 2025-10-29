import React, {useRef, useEffect, useMemo, useContext, useCallback} from "react";
import useEvent from "react-use-event-hook";

import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./Overview.module.scss";
import BackendAPI, {WasmAPI} from "../../api";
import {OVERVIEW_LENS_STYLE, OVERVIEW_MAX_CH} from "../constants/tracks";
import Draggable, {CursorStateInfo} from "../../modules/Draggable";

const {OUT_LENS_FILL_STYLE, LENS_STROKE_STYLE, OUT_TRACK_FILL_STYLE, LINE_WIDTH, RESIZE_CURSOR} =
  OVERVIEW_LENS_STYLE;

const THICKNESS = 3;

type OverviewProps = {
  trackId: number | null;
  idChArr: IdChArr;
  maxTrackSec: number;
  startSec: number;
  lensDurationSec: number;
  moveLens: (sec: number, anchorRatio: number) => void;
  resizeLensLeft: (sec: number) => void;
  resizeLensRight: (sec: number) => void;
  resetLens: () => void;
  needRefresh: boolean;
};

type OverviewCursorState = "left" | "right" | "inlens" | "outlens";

function Overview(props: OverviewProps) {
  const {
    trackId,
    idChArr: _idChArr,
    maxTrackSec,
    startSec,
    lensDurationSec,
    moveLens,
    resizeLensLeft,
    resizeLensRight,
    resetLens,
    needRefresh,
  } = props;
  const idChArr = _idChArr.slice(0, OVERVIEW_MAX_CH);
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const durationSec = useMemo(
    () => (trackId !== null ? BackendAPI.getLengthSec(trackId) : 0),
    [trackId],
  );

  const backgroundElem = useRef<HTMLCanvasElement>(null);
  const lensElem = useRef<HTMLCanvasElement>(null);

  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const lensCtxRef = useRef<CanvasRenderingContext2D | null>(null);

  /* const getWavDrawingOptionsBase = useCallback(
    (scaledWidth: number, scaledHeight: number, offsetY: number, pointsPerSec: number) => {
      const pxPerPoints = scaledWidth / maxTrackSec / pointsPerSec;
      // line case
      return {
        startPx: 0,
        pxPerPoints,
        height: scaledHeight,
        offsetY,
        scale: devicePixelRatio,
        devicePixelRatio,
        needBorder: false,
      };
    },
    [devicePixelRatio, maxTrackSec],
  ); */

  /* const drawChannel = useCallback(
    (
      ctx: CanvasRenderingContext2D,
      wavDrawingInfo: WavDrawingInfo,
      scaledWidth: number,
      scaledHeight: number,
      offsetY: number,
    ) => {
      const baseOptions = getWavDrawingOptionsBase(
        scaledWidth,
        scaledHeight,
        offsetY,
        wavDrawingInfo.pointsPerSec,
      );
      if (wavDrawingInfo.line) {
        if (wavDrawingInfo.clipValues) {
          drawWavLine(ctx, wavDrawingInfo.line, {...baseOptions, color: WAV_CLIPPING_COLOR}, 1);
        }

        drawWavLine(
          ctx,
          wavDrawingInfo.line,
          {
            ...baseOptions,
            color: WAV_COLOR,
            clipValues: wavDrawingInfo.clipValues,
          },
          1,
        );
      } else if (wavDrawingInfo.topEnvelope && wavDrawingInfo.bottomEnvelope) {
        // envelope case

        if (wavDrawingInfo.clipValues) {
          drawWavEnvelope(ctx, wavDrawingInfo.topEnvelope, wavDrawingInfo.bottomEnvelope, {
            ...baseOptions,
            color: WAV_CLIPPING_COLOR,
          });
        }

        drawWavEnvelope(ctx, wavDrawingInfo.topEnvelope, wavDrawingInfo.bottomEnvelope, {
          ...baseOptions,
          color: WAV_COLOR,
          clipValues: wavDrawingInfo.clipValues,
        });
      }
    },
    [getWavDrawingOptionsBase],
  ); */

  /* const drawLimiterGain = useCallback(
    (
      ctx: CanvasRenderingContext2D,
      gainTopDrawingInfo: WavDrawingInfo,
      gainBottomDrawingInfo: WavDrawingInfo,
      scaledWidth: number,
      gainHeight: number,
      chWoGainHeight: number,
      offsetY: number,
    ) => {
      if (!gainTopDrawingInfo.topEnvelope || !gainTopDrawingInfo.bottomEnvelope) {
        console.error(
          "gainTopDrawingInfo.topEnvelope or gainTopDrawingInfo.bottomEnvelope is null",
        );
        return;
      }

      const topBaseOptions = getWavDrawingOptionsBase(
        scaledWidth,
        gainHeight,
        offsetY,
        gainTopDrawingInfo.pointsPerSec,
      );
      drawWavEnvelope(ctx, gainTopDrawingInfo.topEnvelope, gainTopDrawingInfo.bottomEnvelope, {
        ...topBaseOptions,
        color: LIMITER_GAIN_COLOR,
        clipValues: gainTopDrawingInfo.clipValues,
      });

      if (!gainBottomDrawingInfo.topEnvelope || !gainBottomDrawingInfo.bottomEnvelope) {
        console.error(
          "gainBottomDrawingInfo.topEnvelope or gainBottomDrawingInfo.bottomEnvelope is null",
        );
        return;
      }

      const bottomBaseOptions = getWavDrawingOptionsBase(
        scaledWidth,
        gainHeight,
        offsetY + gainHeight + chWoGainHeight,
        gainBottomDrawingInfo.pointsPerSec,
      );
      drawWavEnvelope(
        ctx,
        gainBottomDrawingInfo.topEnvelope,
        gainBottomDrawingInfo.bottomEnvelope,
        {
          ...bottomBaseOptions,
          color: LIMITER_GAIN_COLOR,
          clipValues: gainBottomDrawingInfo.clipValues,
        },
      );
    },
    [getWavDrawingOptionsBase],
  ); */

  const draw = useCallback(async () => {
    if (!backgroundElem.current || !lensElem.current) return;

    const width = backgroundElem.current.clientWidth;
    const height = backgroundElem.current.clientHeight;
    const ctx = backgroundElem.current.getContext("2d", {desynchronized: true});
    if (trackId === null) {
      backgroundElem.current.width = width * devicePixelRatio;
      backgroundElem.current.height = height * devicePixelRatio;
      ctx?.clearRect(0, 0, width * devicePixelRatio, height * devicePixelRatio);
      lensCtxRef.current?.clearRect(
        0,
        0,
        lensElem.current.clientWidth,
        lensElem.current.clientHeight,
      );
      return;
    }

    if (!ctx) return;

    idChArr.forEach((idCh) => {
      const wavInfo = BackendAPI.getWav(idCh);
      if (wavInfo === null) return;
      const {wav, sr, isClipped} = wavInfo;
      WasmAPI.setWav(idCh, wav, sr, isClipped);
    }); // TODO: this is duplicated with ImgCanvas.tsx
    const limiterGainSeq = BackendAPI.getLimiterGainSeq(trackId);
    WasmAPI.drawOverview(
      backgroundElem.current,
      ctx,
      idChArr,
      width,
      height,
      maxTrackSec,
      limiterGainSeq,
    );

    /*
    const drawingInfo = await BackendAPI.getOverviewDrawingInfo(
      trackId,
      width,
      height,
      devicePixelRatio,
    );
    if (!drawingInfo) return;

    backgroundElem.current.width = width * devicePixelRatio;
    backgroundElem.current.height = height * devicePixelRatio;
    ctx.clearRect(0, 0, width * devicePixelRatio, height * devicePixelRatio);
    const {
      chDrawingInfos,
      limiterGainTopInfo: gainTopDrawingInfo,
      limiterGainBottomInfo: gainBottomDrawingInfo,
      scaledChHeight: chHeight,
      scaledGapHeight: gapHeight,
      scaledLimiterGainHeight: gainHeight,
      scaledChWoGainHeight: chWoGainHeight,
    } = drawingInfo;
    chDrawingInfos.forEach((chDrawingInfo, chIdx) => {
      if (gainTopDrawingInfo === null || gainBottomDrawingInfo === null) {
        drawChannel(
          ctx,
          chDrawingInfo,
          width * devicePixelRatio,
          chHeight,
          chIdx * (chHeight + gapHeight),
        );
      } else {
        drawChannel(
          ctx,
          chDrawingInfo,
          width * devicePixelRatio,
          chWoGainHeight,
          chIdx * (chHeight + gapHeight) + gainHeight,
        );
        drawLimiterGain(
          ctx,
          gainTopDrawingInfo,
          gainBottomDrawingInfo,
          width * devicePixelRatio,
          gainHeight,
          chWoGainHeight,
          chIdx * (chHeight + gapHeight),
        );
      }
    }); */

    // fill out of track area
    if (durationSec < maxTrackSec) {
      ctx.fillStyle = OUT_TRACK_FILL_STYLE;
      const x = width * devicePixelRatio * (durationSec / maxTrackSec);
      ctx.fillRect(x, 0, width * devicePixelRatio - x, height * devicePixelRatio);
    }
  }, [devicePixelRatio, durationSec, idChArr, maxTrackSec, trackId]);

  const prevDrawRef = useRef(draw);
  if (prevDrawRef.current === draw && needRefresh) draw();
  prevDrawRef.current = draw;

  useEffect(() => {
    draw();
  }, [draw]);

  const calcPxPerSec = useCallback(() => {
    const width = lensElem.current?.clientWidth ?? 0;
    return width / (maxTrackSec + 1e-8);
  }, [maxTrackSec]);

  const drawLens = useCallback(() => {
    const ctx = lensCtxRef.current;
    if (!lensElem.current || !ctx) return;
    const {clientWidth: width, clientHeight: height} = lensElem.current;
    const pxPerSec = calcPxPerSec();
    const lensEndSec = (startSec + lensDurationSec) * pxPerSec;
    ctx.clearRect(0, 0, width, height);
    if (startSec > 0) ctx.fillRect(0, 0, startSec * pxPerSec, height);
    ctx.beginPath();
    ctx.roundRect(
      startSec * pxPerSec + LINE_WIDTH / 2,
      LINE_WIDTH / 2,
      lensDurationSec * pxPerSec - LINE_WIDTH,
      height - LINE_WIDTH,
      2,
    );
    ctx.stroke();
    if (width > lensEndSec) ctx.fillRect(lensEndSec, 0, width - lensEndSec, height);
  }, [calcPxPerSec, lensDurationSec, startSec]);

  const drawLensRef = useRef(drawLens);
  useEffect(() => {
    drawLensRef.current = drawLens;
    // Request a redraw only when the draw function or its dependencies change
    const requestId = requestAnimationFrame(() => {
      // Ensure drawRef.current exists and call it
      if (drawLensRef.current) drawLensRef.current();
    });

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [drawLens]);

  const resizeObserverCallback = useEvent(async () => {
    if (!lensElem.current) return;
    lensElem.current.width = lensElem.current.clientWidth * devicePixelRatio;
    lensElem.current.height = lensElem.current.clientHeight * devicePixelRatio;
    const lensCtx = lensElem.current.getContext("2d", {desynchronized: true});
    lensCtxRef.current = lensCtx;
    if (!lensCtx) return;
    lensCtx.scale(devicePixelRatio, devicePixelRatio);
    lensCtx.lineWidth = LINE_WIDTH;
    lensCtx.fillStyle = OUT_LENS_FILL_STYLE;
    lensCtx.strokeStyle = LENS_STROKE_STYLE;
    drawLens();
    await draw();
  });

  useEffect(() => {
    resizeObserverRef.current?.disconnect();
    resizeObserverRef.current = new ResizeObserver(resizeObserverCallback);
    if (lensElem.current) resizeObserverRef.current.observe(lensElem.current);
  }, [resizeObserverCallback]);

  const calcSecFromX = useEvent((cursorX: number, rect: DOMRect) => {
    const ratioX = cursorX / rect.width;
    return ratioX * maxTrackSec;
  });

  const getInfoForResize = useCallback(
    (resizeLensFunc: (sec: number) => void) => ({
      cursor: RESIZE_CURSOR,
      cursorClassNameForBody: "colResizeCursor",
      handleDragging: (
        _: OverviewCursorState,
        cursorX: number,
        anchorValue: number,
        rect: DOMRect,
      ) => {
        resizeLensFunc(calcSecFromX(cursorX, rect));
      },
    }),
    [calcSecFromX],
  );

  const infoForInOutLens: CursorStateInfo<OverviewCursorState, number> = useMemo(
    () => ({
      cursor: "text",
      cursorClassNameForBody: "textCursor",
      handleDragging: (
        _: OverviewCursorState,
        cursorX: number,
        anchorValue: number,
        rect: DOMRect,
      ) => {
        moveLens(calcSecFromX(cursorX, rect), anchorValue);
      },
    }),
    [moveLens, calcSecFromX],
  );

  const cursorStateInfos: Map<
    OverviewCursorState,
    CursorStateInfo<OverviewCursorState, number>
  > = useMemo(
    () =>
      new Map([
        ["left", getInfoForResize(resizeLensLeft)],
        ["right", getInfoForResize(resizeLensRight)],
        ["inlens", infoForInOutLens],
        ["outlens", infoForInOutLens],
      ]),
    [resizeLensLeft, resizeLensRight, getInfoForResize, infoForInOutLens],
  );

  const determineCursorStates = useEvent((cursorX: number) => {
    const pxPerSec = calcPxPerSec();
    const lensStartX = Math.round(startSec * pxPerSec);
    const lensEndX = Math.round((startSec + lensDurationSec) * pxPerSec);
    if (lensStartX - THICKNESS <= cursorX && cursorX <= lensStartX + THICKNESS) {
      return "left";
    }
    if (lensStartX + THICKNESS < cursorX && cursorX < lensEndX - THICKNESS) {
      return "inlens";
    }
    if (lensEndX - THICKNESS <= cursorX && cursorX <= lensEndX + THICKNESS) {
      return "right";
    }
    return "outlens";
  });

  const calcDragAnchor = useEvent(
    (cursorState: OverviewCursorState, cursorPos: number, rect: DOMRect) => {
      if (cursorState === "inlens") {
        const sec = calcSecFromX(cursorPos, rect);
        return (sec - startSec) / lensDurationSec;
      }
      return 0.5;
    },
  );

  return (
    <div className={styles.Overview} role="navigation">
      <canvas className={styles.OverviewBackground} ref={backgroundElem} />
      <Draggable
        cursorStateInfos={cursorStateInfos}
        calcCursorPos="x"
        determineCursorStates={determineCursorStates}
        calcDragAnchor={calcDragAnchor}
        dragAnchorDefault={0.5}
      >
        <canvas
          className={styles.OverviewLens}
          ref={lensElem}
          style={{display: trackId !== null ? "block" : "none"}}
          onClick={(e) => {
            if (e.altKey && e.button === 0) {
              resetLens();
            }
          }}
        />
      </Draggable>
    </div>
  );
}

export default React.memo(Overview);
