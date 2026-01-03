import styles from "./ErrorBox.module.scss";

type ErrorBoxProps = {
  trackId: number;
  width: number;
  handleReload: (id: number) => void | Promise<void>;
  handleIgnore: (id: number) => void | Promise<void>;
  handleClose: (id: number) => void | Promise<void>;
};

function ErrorBox(props: ErrorBoxProps) {
  const {trackId, width, handleReload, handleIgnore, handleClose} = props;

  return (
    <div
      className={styles.errorBox}
      role="presentation"
      onClick={(e) => e.stopPropagation()}
      style={{width}}
    >
      <p>The file is corrupted and cannot be opened</p>
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
