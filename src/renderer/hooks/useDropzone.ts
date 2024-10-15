import React, {useRef, useState, useEffect} from "react";
import useEvent from "react-use-event-hook";

type DropzoneProps = {
  targetRef: React.RefObject<HTMLElement>;
  handleDrop: (e: DragEvent) => Promise<void>;
};

function useDropzone(props: DropzoneProps) {
  const {targetRef, handleDrop} = props;

  const dragCounterRef = useRef<number>(0);
  const [isDragActive, setIsDragActive] = useState<boolean>(false);

  const onDragOver = useEvent((e: DragEvent) => {
    if (isDragActive) e.preventDefault();
  });

  const onDragEnter = useEvent((e: DragEvent) => {
    if (
      e.dataTransfer?.items === undefined ||
      e.dataTransfer.items.length === 0 ||
      e.dataTransfer.items[0].kind !== "file"
    ) {
      return;
    }
    e.preventDefault();
    dragCounterRef.current += 1;
    setIsDragActive(true);
  });

  const onDragLeave = useEvent((e: DragEvent) => {
    if (!isDragActive) return;
    e.preventDefault();

    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragActive(false);
    }
  });

  const onDrop = useEvent(async (e: DragEvent) => {
    if (!isDragActive) return;
    await handleDrop(e);
    dragCounterRef.current = 0;
    setIsDragActive(false);
  });

  useEffect(() => {
    const target = targetRef.current;

    target?.addEventListener("dragenter", onDragEnter);
    target?.addEventListener("dragleave", onDragLeave);
    target?.addEventListener("dragover", onDragOver);
    target?.addEventListener("drop", onDrop);

    return () => {
      target?.removeEventListener("dragenter", onDragEnter);
      target?.removeEventListener("dragleave", onDragLeave);
      target?.removeEventListener("dragover", onDragOver);
      target?.removeEventListener("drop", onDrop);
    };
  }, [targetRef, onDragEnter, onDragLeave, onDragOver, onDrop]);

  return {
    isDropzoneActive: isDragActive,
  };
}

export default useDropzone;
