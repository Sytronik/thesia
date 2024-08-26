import React, {useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import BackendAPI from "renderer/api";
import {Player} from "renderer/hooks/usePlayer";
import FloatRangeInput from "renderer/modules/FloatRangeInput";
import styles from "./PlayerControl.module.scss";
import playIcon from "../../../../assets/buttons/play.svg";
import pauseIcon from "../../../../assets/buttons/pause.svg";
import rewindBackIcon from "../../../../assets/buttons/rewind-back.svg";
import rewindForwardIcon from "../../../../assets/buttons/rewind-forward.svg";
import skipToBeginningIcon from "../../../../assets/buttons/skip-to-beginning.svg";
import volumeIcon from "../../../../assets/buttons/volume.svg";
import {PLAY_JUMP_SEC} from "../constants/tracks";

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
      const positionLabel = BackendAPI.secondsToLabel(positionSec);
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
    <div className={`flex-container-row ${styles.PlayerControl}`}>
      <div ref={posLabelElem} className={styles.positionLabel}>
        {BackendAPI.secondsToLabel(player.positionSecRef.current ?? 0)}
      </div>
      <div className={styles.playerButton}>
        <button
          type="button"
          onClick={async () => {
            if (player.isPlaying) await player.seek(0);
            else player.setSelectSec(0);
          }}
        >
          <img src={skipToBeginningIcon} alt="skip to beginning icon" />
        </button>
        <button
          type="button"
          onClick={async () => {
            if (player.isPlaying)
              await player.seek((player.positionSecRef.current ?? 0) - PLAY_JUMP_SEC);
            else player.setSelectSec((player.selectSecRef.current ?? 0) - PLAY_JUMP_SEC);
          }}
        >
          <img src={rewindBackIcon} alt="rewind back icon" />
        </button>
        <button type="button" onClick={player.togglePlay}>
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
          onClick={async () => {
            if (player.isPlaying)
              await player.seek((player.positionSecRef.current ?? 0) + PLAY_JUMP_SEC);
            else player.setSelectSec((player.selectSecRef.current ?? 0) + PLAY_JUMP_SEC);
          }}
        >
          <img src={rewindForwardIcon} alt="rewind forward icon" />
        </button>
      </div>
      <img src={volumeIcon} alt="volume icon" className={styles.volumeIcon} />
      <FloatRangeInput
        id="volumeRangeInput"
        className={styles.volumeRangeInput}
        unit="dB"
        min={-24}
        max={0}
        step={0.1}
        precision={1}
        detents={[]}
        initialValue={0.0}
        doubleClickValue={0.0}
        onChangeValue={async (volume) => {
          await BackendAPI.setVolumedB(volume);
        }}
      />
    </div>
  );
}

export default React.memo(PlayerControl);
