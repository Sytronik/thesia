import { message, open } from "@tauri-apps/plugin-dialog";
import { path } from "@tauri-apps/api";
import { SUPPORTED_TYPES } from "../prototypes/constants/tracks";
import BackendAPI, {
  enableEditMenu,
  disableEditMenu,
  enablePlayMenu,
  disablePlayMenu,
} from "../api";

export async function showOpenFilesDialog() {
  // get the default path
  const defaultPath = await BackendAPI.getOpenFilesDialogPath();

  // show the open dialog
  const files = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Audio Files", extensions: SUPPORTED_TYPES }],
    title: "Select the audio files to be open",
    defaultPath,
    canCreateDirectories: false,
  });

  if (files && files.length > 0) {
    // find the common directory of the filepaths
    let commonDir = await path.resolve(files[0]);
    for (const filePath of files) {
      const resolved = await path.resolve(filePath);
      let newCommon = commonDir;
      while (!resolved.startsWith(newCommon)) {
        newCommon = await path.dirname(newCommon);
      }
      commonDir = newCommon;
    }

    // save the common directory to settings
    BackendAPI.setOpenFilesDialogPath(commonDir);
  }

  return files;
}

export function getOpenFilesDialogHandler(
  callback: (files: string[]) => void | Promise<void>,
): () => Promise<void> {
  return async () => {
    await Promise.all([disablePlayMenu(), enableEditMenu()]);
    const files = await showOpenFilesDialog();
    await Promise.all([enablePlayMenu(), disableEditMenu()]);
    if (files && files.length > 0) callback(files);
  };
}

const numFilesLabel = (numFiles: number) => (numFiles >= 5 ? ` (${numFiles} files)` : ``);
const joinManyPaths = (paths: string[]) => {
  // join with newline if less than 5 elements, else show first 2 elems + ellipse + the last elem
  return paths.length < 5
    ? paths.join("\n")
    : `${paths.slice(2).join("\n")}\n...\n${paths[paths.length - 1]}`;
};

export async function showFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  const msgUnsupported = unsupportedPaths.length
    ? `-- Not Supported Type${numFilesLabel(unsupportedPaths.length)} --\n` +
      `${joinManyPaths(unsupportedPaths)}\n\n`
    : "";
  const msgInvalid = invalidPaths.length
    ? `-- Not Valid Format${numFilesLabel(invalidPaths.length)} --\n` +
      `${joinManyPaths(invalidPaths)}\n\n`
    : "";
  await message(
    "The following files could not be opened\n\n" +
      `${msgUnsupported}` +
      `${msgInvalid}` +
      "Please ensure that the file properties are correct and that it is a supported file type.\n\n" +
      `Only files with the following extensions are allowed:\n  ${SUPPORTED_TYPES.join(", ")}`,
    { title: "File Open Error", kind: "error" },
  );
}
