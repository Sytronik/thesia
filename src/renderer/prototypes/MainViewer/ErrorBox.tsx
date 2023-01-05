import React from "react";
import styles from "./ErrorBox.scss";

type ErrorBoxProps = {
  trackId: number;
  handleReload: (id: number) => void;
  handleIgnore: (id: number) => void;
  handleClose: (id: number) => void;
};

function ErrorBox(props: ErrorBoxProps) {
  const {trackId, handleReload, handleIgnore, handleClose} = props;

  return (
    <div className={styles.errorBox}>
      <p>The file is corrupted and cannot be opened</p>
      {/* TODO: need optimization? */}
      <div>
        <button type="button" onClick={() => handleReload(trackId)}>
          Reload
        </button>
        <button type="button" onClick={() => handleIgnore(trackId)}>
          Ignore
        </button>
        <button type="button" onClick={() => handleClose(trackId)}>
          Close
        </button>
      </div>
    </div>
  );
}

export default ErrorBox;
