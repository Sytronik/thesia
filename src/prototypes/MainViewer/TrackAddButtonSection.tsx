import React from "react";
import styles from "./TrackAddButtonSection.module.scss";

function TrackAddButtonSection({
  openAudioTracksHandler,
}: {
  openAudioTracksHandler: () => Promise<void>;
}) {
  return (
    <div className={styles.trackAddButtonSection}>
      <button type="button" onClick={openAudioTracksHandler} aria-label="Add track">
        <span className={styles.btnPlus} />
      </button>
    </div>
  );
}

export default React.memo(TrackAddButtonSection);
