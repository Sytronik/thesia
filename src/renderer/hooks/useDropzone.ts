import React, {useRef, useCallback, useState} from "react";

type DropboxProps = {
  ref: React.RefObject<HTMLElement>;
  onDrop: (e: React.DragEvent) => void;
};

function useDropzone(props: DropboxProps) {
  const {ref, onDrop} = props;

  const dragCounterRef = useRef<number>(0);
  const [dropboxIsVisible, setDropboxIsVisible] = useState<boolean>(false);

  const dragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const dragEnter = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current += 1;
    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setDropboxIsVisible(true);
    }
    return false;
  };

  const dragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setDropboxIsVisible(false);
    }
    return false;
  };

  const handleDroppedAndResetDropbox = (e: React.DragEvent) => {
    onDrop(e);
    dragCounterRef.current = 0;
    setDropboxIsVisible(false);
  };

  return {
    dropboxIsVisible,
    dragOver,
    dragEnter,
    dragLeave,
    handleDroppedAndResetDropbox,
  };
}

export default useDropzone;
