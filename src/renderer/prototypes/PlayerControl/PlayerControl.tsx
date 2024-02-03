import React, {useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import backend from "renderer/api";
import {Player} from "renderer/hooks/usePlayer";
import styles from "./PlayerControl.module.scss";
import playIcon from "../../../../assets/buttons/play.svg";
import pauseIcon from "../../../../assets/buttons/pause.svg";
import rewindBackIcon from "../../../../assets/buttons/rewind-back.svg";
import rewindForwardIcon from "../../../../assets/buttons/rewind-forward.svg";
import skipToBeginningIcon from "../../../../assets/buttons/skip-to-beginning.svg";

function PlayerControl({player}: {player: Player}) {
  const prevPosSecRef = useRef<number>(0);
  const posLabelElem = useRef<HTMLDivElement | null>(null);
  const requestRef = useRef<number | null>(null);

  const updatePosLabel = useEvent(() => {
    const positionSec = player.positionSecRef.current ?? 0;
    if (
      posLabelElem.current !== null &&
      prevPosSecRef.current.toFixed(3) !== positionSec.toFixed(3)
    ) {
      const positionLabel = backend.secondsToLabel(positionSec);
      const {childNodes: positionLabelNodes} = posLabelElem.current;
      if (positionLabelNodes.length > 0) positionLabelNodes.item(0).nodeValue = positionLabel;
      prevPosSecRef.current = positionSec;
    }
    requestRef.current = requestAnimationFrame(updatePosLabel);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(updatePosLabel);
    return () => {
      if (requestRef.current !== null) cancelAnimationFrame(requestRef.current);
    };
  }, [updatePosLabel]);

  return (
    <div className={`flex-item-fixed ${styles.PlayerControl}`}>
      <div ref={posLabelElem}>{backend.secondsToLabel(player.positionSecRef.current ?? 0)}</div>
      <button
        type="button"
        className={styles.playerButton}
        onClick={async () => {
          await player.seek(0);
        }}
      >
        <img src={skipToBeginningIcon} alt="skip to beginning icon" />
      </button>
      <button
        type="button"
        className={styles.playerButton}
        onClick={async () => {
          await player.seek(Math.max((player.positionSecRef.current ?? 0) - 5, 0));
        }}
      >
        <img src={rewindBackIcon} alt="rewind back icon" />
      </button>
      <button
        type="button"
        className={styles.playerButton}
        onClick={async () => {
          player.togglePlay();
        }}
      >
        <img
          src={playIcon}
          alt="play button icon"
          style={{display: player.isPlaying ? "none" : "inline-block"}}
        />
        <img
          src={pauseIcon}
          alt="pause button icon"
          style={{display: player.isPlaying ? "inline-block" : "none"}}
        />
      </button>
      <button
        type="button"
        className={styles.playerButton}
        onClick={async () => {
          await player.seek((player.positionSecRef.current ?? 0) + 5);
        }}
      >
        <img src={rewindForwardIcon} alt="rewind forward icon" />
      </button>
    </div>
  );
}

export default React.memo(PlayerControl);
