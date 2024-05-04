import {useContext} from "react";
import {StoreContext} from "../contexts";
import Store from "renderer/Store";

const useStore = (): Store => useContext(StoreContext);

export default useStore;
